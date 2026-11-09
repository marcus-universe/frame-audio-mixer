//! Loading a VST3 module and running one of its audio effects.

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusDirections_, IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentHandler,
    IComponentTrait, IConnectionPoint, IConnectionPointTrait, IEditController, IEditControllerTrait, IParameterChanges,
    MediaTypes_, ParamID, ParamValue, ProcessData, ProcessModes_, ProcessSetup, SpeakerArr, SymbolicSampleSizes_,
};
use vst3::Steinberg::{kResultOk, FUnknown, IBStream, IPluginBaseTrait, IPluginFactory, IPluginFactoryTrait, PClassInfo, TUID};
use vst3::{ComPtr, ComWrapper, Interface};

use crate::com::{EditQueue, Handler, Host, MemoryStream, ParamChanges};

pub const RATE: f64 = 48_000.0;
pub const BLOCK: usize = 480;
const AUDIO_CLASS: &str = "Audio Module Class";

// `c_char` is i8 on Windows and u8 on aarch64 Linux; the cast is needed on one of them.
#[allow(clippy::unnecessary_cast)]
fn c_text(raw: &[c_char]) -> String {
    let bytes: Vec<u8> = raw.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Class ids are written as 32 hex digits, the form moduleinfo.json uses.
pub fn parse_cid(hex: &str) -> Option<TUID> {
    let hex = hex.trim();
    if hex.len() != 32 {
        return None;
    }
    let mut cid: TUID = [0; 16];
    for (index, byte) in cid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()? as c_char;
    }
    Some(cid)
}

/// The DLL or shared object inside a bundle, or `path` itself when it is a file.
pub fn binary_in(path: &str) -> std::path::PathBuf {
    let bundle = std::path::Path::new(path);
    if bundle.is_file() {
        return bundle.to_path_buf();
    }
    let stem = bundle.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
    let (arch, ext) = if cfg!(windows) { ("x86_64-win", "vst3") } else { ("aarch64-linux", "so") };
    bundle.join("Contents").join(arch).join(format!("{stem}.{ext}"))
}

pub struct Module {
    /// Released in `drop` before the library unloads; its code lives there.
    factory: Option<ComPtr<IPluginFactory>>,
    library: libloading::Library,
}

impl Module {
    pub fn load(path: &str) -> Result<Self, String> {
        let binary = binary_in(path);
        let library = unsafe { libloading::Library::new(&binary) }.map_err(|err| format!("load {}: {err}", binary.display()))?;
        unsafe {
            let entry_name: &[u8] = if cfg!(windows) { b"InitDll\0" } else { b"ModuleEntry\0" };
            if cfg!(windows) {
                if let Ok(entry) = library.get::<unsafe extern "system" fn() -> bool>(entry_name) {
                    let _ = entry();
                }
            } else if let Ok(entry) = library.get::<unsafe extern "system" fn(*mut c_void) -> bool>(entry_name) {
                let _ = entry(ptr::null_mut());
            }
            let get: libloading::Symbol<unsafe extern "system" fn() -> *mut IPluginFactory> =
                library.get(b"GetPluginFactory\0").map_err(|err| format!("GetPluginFactory: {err}"))?;
            let factory = ComPtr::from_raw(get()).ok_or("plugin returned no factory")?;
            Ok(Self { factory: Some(factory), library })
        }
    }

    fn factory(&self) -> &ComPtr<IPluginFactory> {
        self.factory.as_ref().expect("factory lives until drop")
    }

    pub fn audio_classes(&self) -> Vec<(TUID, String)> {
        let mut out = Vec::new();
        let factory = self.factory();
        unsafe {
            for index in 0..factory.countClasses() {
                let mut info: PClassInfo = std::mem::zeroed();
                if factory.getClassInfo(index, &mut info) == kResultOk && c_text(&info.category) == AUDIO_CLASS {
                    out.push((info.cid, c_text(&info.name)));
                }
            }
        }
        out
    }

    pub fn create<I: Interface>(&self, cid: &TUID) -> Option<ComPtr<I>> {
        let mut obj = ptr::null_mut();
        let result = unsafe { self.factory().createInstance(cid.as_ptr(), I::IID.as_ptr() as *const c_char, &mut obj) };
        if result != kResultOk {
            return None;
        }
        unsafe { ComPtr::from_raw(obj as *mut I) }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        self.factory = None;
        unsafe {
            let exit_name: &[u8] = if cfg!(windows) { b"ExitDll\0" } else { b"ModuleExit\0" };
            if let Ok(exit) = self.library.get::<unsafe extern "system" fn() -> bool>(exit_name) {
                let _ = exit();
            }
        }
    }
}

/// One running effect: its processor, its controller when it has one, and the
/// edits the editor made since the last audio block.
pub struct Plugin {
    pub component: ComPtr<IComponent>,
    pub audio: ComPtr<IAudioProcessor>,
    pub controller: Option<ComPtr<IEditController>>,
    separate_controller: bool,
    pub edits: EditQueue,
    _host: ComWrapper<Host>,
    _handler: ComWrapper<Handler>,
}

/// State on disk: component length (u32, little endian), component state, controller state.
fn split_state(saved: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = u32::from_le_bytes(saved.get(..4)?.try_into().ok()?) as usize;
    let component = saved.get(4..4 + len)?;
    Some((component, &saved[4 + len..]))
}

impl Plugin {
    pub fn start(module: &Module, cid: &TUID, saved: Option<&[u8]>) -> Result<Self, String> {
        let host = ComWrapper::new(Host);
        let context = host.to_com_ptr::<FUnknown>().ok_or("host context")?;
        let component: ComPtr<IComponent> = module.create(cid).ok_or("the plugin has no IComponent")?;
        let audio = component.cast::<IAudioProcessor>().ok_or("the plugin has no IAudioProcessor")?;
        let edits: EditQueue = Arc::new(Mutex::new(Vec::new()));
        let handler = ComWrapper::new(Handler { edits: edits.clone() });
        unsafe {
            if component.initialize(context.as_ptr()) != kResultOk {
                return Err("the plugin failed to initialize".into());
            }
            let (controller, separate) = match component.cast::<IEditController>() {
                Some(single) => (Some(single), false),
                None => {
                    let mut controller_cid: TUID = [0; 16];
                    let created = if component.getControllerClassId(&mut controller_cid) == kResultOk {
                        module.create::<IEditController>(&controller_cid)
                    } else {
                        None
                    };
                    let created = created.filter(|controller| controller.initialize(context.as_ptr()) == kResultOk);
                    (created, true)
                }
            };
            if let Some(controller) = &controller {
                if let Some(handler_ptr) = handler.to_com_ptr::<IComponentHandler>() {
                    controller.setComponentHandler(handler_ptr.as_ptr());
                }
                if separate {
                    if let (Some(a), Some(b)) = (component.cast::<IConnectionPoint>(), controller.cast::<IConnectionPoint>()) {
                        a.connect(b.as_ptr());
                        b.connect(a.as_ptr());
                    }
                }
            }
            let plugin = Self { component, audio, controller, separate_controller: separate, edits, _host: host, _handler: handler };
            if let Some((component_state, controller_state)) = saved.and_then(split_state) {
                plugin.restore(component_state, controller_state);
            }
            plugin.activate()?;
            Ok(plugin)
        }
    }

    unsafe fn restore(&self, component_state: &[u8], controller_state: &[u8]) {
        let stream = MemoryStream::new(component_state.to_vec());
        if let Some(ptr) = stream.to_com_ptr::<IBStream>() {
            self.component.setState(ptr.as_ptr());
        }
        let Some(controller) = &self.controller else { return };
        let stream = MemoryStream::new(component_state.to_vec());
        if let Some(ptr) = stream.to_com_ptr::<IBStream>() {
            controller.setComponentState(ptr.as_ptr());
        }
        if !controller_state.is_empty() && self.separate_controller {
            let stream = MemoryStream::new(controller_state.to_vec());
            if let Some(ptr) = stream.to_com_ptr::<IBStream>() {
                controller.setState(ptr.as_ptr());
            }
        }
    }

    unsafe fn activate(&self) -> Result<(), String> {
        let mut stereo_in = [SpeakerArr::kStereo; 1];
        let mut stereo_out = [SpeakerArr::kStereo; 1];
        let _ = self.audio.setBusArrangements(stereo_in.as_mut_ptr(), 1, stereo_out.as_mut_ptr(), 1);
        let audio_type = MediaTypes_::kAudio as i32;
        if self.component.getBusCount(audio_type, BusDirections_::kInput as i32) > 0 {
            self.component.activateBus(audio_type, BusDirections_::kInput as i32, 0, 1);
        }
        self.component.activateBus(audio_type, BusDirections_::kOutput as i32, 0, 1);
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: crate::shared::MAX_FRAMES as i32,
            sampleRate: RATE,
        };
        if self.audio.setupProcessing(&mut setup) != kResultOk {
            return Err("the plugin rejected 48 kHz stereo".into());
        }
        if self.component.setActive(1) != kResultOk {
            return Err("the plugin failed to activate".into());
        }
        let _ = self.audio.setProcessing(1);
        Ok(())
    }

    /// Component and controller state, in the format `start` reads back.
    pub fn save(&self) -> Vec<u8> {
        unsafe {
            let component_stream = MemoryStream::new(Vec::new());
            if let Some(ptr) = component_stream.to_com_ptr::<IBStream>() {
                self.component.getState(ptr.as_ptr());
            }
            let component_state = component_stream.take();
            let mut out = (component_state.len() as u32).to_le_bytes().to_vec();
            out.extend_from_slice(&component_state);
            if let (Some(controller), true) = (&self.controller, self.separate_controller) {
                let controller_stream = MemoryStream::new(Vec::new());
                if let Some(ptr) = controller_stream.to_com_ptr::<IBStream>() {
                    controller.getState(ptr.as_ptr());
                }
                out.extend_from_slice(&controller_stream.take());
            }
            out
        }
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        unsafe {
            let _ = self.audio.setProcessing(0);
            let _ = self.component.setActive(0);
            if let Some(controller) = &self.controller {
                if self.separate_controller {
                    if let (Some(a), Some(b)) = (self.component.cast::<IConnectionPoint>(), controller.cast::<IConnectionPoint>()) {
                        a.disconnect(b.as_ptr());
                        b.disconnect(a.as_ptr());
                    }
                    let _ = controller.terminate();
                }
            }
            let _ = self.component.terminate();
        }
    }
}

/// The audio side, safe to move to the audio thread. VST3 allows `process` on
/// another thread than the one that created the plugin.
pub struct AudioPort {
    audio: ComPtr<IAudioProcessor>,
    edits: EditQueue,
}

unsafe impl Send for AudioPort {}

impl AudioPort {
    pub fn of(plugin: &Plugin) -> Self {
        Self { audio: plugin.audio.clone(), edits: plugin.edits.clone() }
    }

    pub fn process(&self, input: [&mut [f32]; 2], output: [&mut [f32]; 2]) -> bool {
        let [in_l, in_r] = input;
        let [out_l, out_r] = output;
        let frames = in_l.len().min(in_r.len()).min(out_l.len()).min(out_r.len());
        let edits: Vec<(ParamID, ParamValue)> = self.edits.try_lock().map(|mut edits| std::mem::take(&mut *edits)).unwrap_or_default();
        let changes = (!edits.is_empty()).then(|| ParamChanges::from_edits(&edits));
        let changes_ptr = changes
            .as_ref()
            .and_then(|changes| changes.to_com_ptr::<IParameterChanges>())
            .map_or(ptr::null_mut(), |ptr| ptr.as_ptr());
        let mut inputs = [in_l.as_mut_ptr(), in_r.as_mut_ptr()];
        let mut outputs = [out_l.as_mut_ptr(), out_r.as_mut_ptr()];
        let mut in_bus = AudioBusBuffers { numChannels: 2, silenceFlags: 0, __field0: AudioBusBuffers__type0 { channelBuffers32: inputs.as_mut_ptr() } };
        let mut out_bus = AudioBusBuffers { numChannels: 2, silenceFlags: 0, __field0: AudioBusBuffers__type0 { channelBuffers32: outputs.as_mut_ptr() } };
        let mut data = ProcessData {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            numSamples: frames as i32,
            numInputs: 1,
            numOutputs: 1,
            inputs: &mut in_bus,
            outputs: &mut out_bus,
            inputParameterChanges: changes_ptr,
            outputParameterChanges: ptr::null_mut(),
            inputEvents: ptr::null_mut(),
            outputEvents: ptr::null_mut(),
            processContext: ptr::null_mut(),
        };
        unsafe { self.audio.process(&mut data) == kResultOk }
    }
}

/// Loads a plugin, runs two seconds of a 440 Hz sine through it and reports.
pub fn probe(path: &str) -> Result<(), String> {
    let started = Instant::now();
    let module = Module::load(path)?;
    println!("loaded in {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
    let classes = module.audio_classes();
    for (_, name) in &classes {
        println!("class: {name}");
    }
    let (cid, name) = classes.first().ok_or("no audio effect class")?;
    let plugin = Plugin::start(&module, cid, None)?;
    println!("started {name} (editor: {})", if plugin.controller.is_some() { "yes" } else { "no" });
    let port = AudioPort::of(&plugin);
    let blocks = (2.0 * RATE) as usize / BLOCK;
    let (mut energy_in, mut energy_out) = (0.0f64, 0.0f64);
    let (mut worst, mut total, mut failed) = (Duration::ZERO, Duration::ZERO, 0);
    for block in 0..blocks {
        let mut left = [0.0f32; BLOCK];
        for (i, sample) in left.iter_mut().enumerate() {
            let t = (block * BLOCK + i) as f64 / RATE;
            *sample = (std::f64::consts::TAU * 440.0 * t).sin() as f32 * 0.25;
        }
        let mut right = left;
        let (mut out_l, mut out_r) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
        energy_in += left.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
        let at = Instant::now();
        if !port.process([&mut left, &mut right], [&mut out_l, &mut out_r]) {
            failed += 1;
        }
        let spent = at.elapsed();
        worst = worst.max(spent);
        total += spent;
        energy_out += out_l.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
    }
    let state = plugin.save();
    println!(
        "processed {blocks} blocks: {failed} failed, level {:+.1} dB, average {:.0} us, worst {:.0} us of 10000, state {} bytes",
        10.0 * (energy_out / energy_in.max(1e-30)).log10(),
        total.as_secs_f64() * 1e6 / blocks as f64,
        worst.as_secs_f64() * 1e6,
        state.len()
    );
    drop(port);
    drop(plugin);
    drop(module);
    Ok(())
}

#[allow(clippy::unnecessary_cast)] // `c_char` differs per platform, see `c_text`.
pub fn self_test() -> Result<(), String> {
    let cid = parse_cid("ABCDEF0123456789ABCDEF0123456789").ok_or("cid parse")?;
    if cid[0] as u8 != 0xAB || cid[15] as u8 != 0x89 {
        return Err("cid bytes".into());
    }
    if parse_cid("xyz").is_some() {
        return Err("bad cid accepted".into());
    }
    let mut saved = 3u32.to_le_bytes().to_vec();
    saved.extend_from_slice(b"abcde");
    if split_state(&saved) != Some((&b"abc"[..], &b"de"[..])) {
        return Err("state split".into());
    }
    Ok(())
}

//! Windows: every stream is one thread on one WASAPI endpoint.
//!
//! A microphone stream (B1–B4) plays what arrives from the headset into the
//! playback side of a virtual cable; applications record its capture side,
//! which is the device the user sees ("Steam Streaming Mic B1"). An output stream (PC1–PC4)
//! records what applications play on a virtual output ("Steam Streaming Out PC1") with a
//! loopback capture.
//!
//! All streams are opened in shared mode as 48 kHz float stereo, the format of
//! the link. Windows converts if a device is set to something else.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Context;
use frame_link_core::{Link, PcmReader, PcmWriter, CHANNELS, FRAME, RATE};
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    eCapture, eRender, EDataFlow, IAudioCaptureClient, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

use crate::config::Config;
use crate::model::{AudioStatus, DeviceState, Kind, STREAMS};

/// Buffer asked of WASAPI, in 100 ns units: 20 ms, two device periods. One
/// period would glitch whenever this thread is scheduled a little late.
const BUFFER_HNS: i64 = 200_000;
/// How long a stream thread waits for the device before it looks at the stop flag.
const WAIT_MS: u32 = 200;
/// A device that is missing or failed is looked for again this often.
const RETRY: Duration = Duration::from_secs(2);

/// `WAVE_FORMAT_EXTENSIBLE`, front left + front right, and the float subtype.
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const STEREO_MASK: u32 = 0x3;
const SUBTYPE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

/// The device instance an endpoint belongs to, as `{1}.ROOT\NAME\0001`. The
/// two ends of a virtual cable carry the same value. Undocumented but stable
/// since Windows Vista; the Sound control panel shows it as "Controller".
const PKEY_ENDPOINT_INSTANCE: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xb3f8fa53_0004_438e_9003_51a46e139bfc), pid: 2 };

const STATE_MISSING: u8 = 0;
const STATE_READY: u8 = 1;
const STATE_FAILED: u8 = 2;

struct Shared {
    stop: AtomicBool,
    mics: [AtomicU8; STREAMS],
    outputs: [AtomicU8; STREAMS],
}

impl Shared {
    fn slot(&self, kind: Kind, index: usize) -> &AtomicU8 {
        match kind {
            Kind::Mic => &self.mics[index],
            Kind::Output => &self.outputs[index],
        }
    }
}

pub struct Backend {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

/// The link end a stream thread works with.
enum Io {
    /// Plays what the headset sent.
    Feed(PcmReader),
    /// Sends what the PC plays.
    Tap(PcmWriter),
}

impl Backend {
    pub fn start(link: &Link, config: &Config) -> anyhow::Result<Self> {
        let shared = Arc::new(Shared { stop: AtomicBool::new(false), mics: Default::default(), outputs: Default::default() });
        let mut backend = Self { shared, threads: Vec::with_capacity(STREAMS * 2) };
        for index in 0..STREAMS {
            backend.spawn(Kind::Mic, index, config.mic_devices[index].clone(), Io::Feed(link.playback(index)))?;
            backend.spawn(Kind::Output, index, config.output_devices[index].clone(), Io::Tap(link.capture(index)))?;
        }
        Ok(backend)
    }

    fn spawn(&mut self, kind: Kind, index: usize, device: String, io: Io) -> anyhow::Result<()> {
        let shared = Arc::clone(&self.shared);
        let name = format!("audio-{}{}", kind.prefix().to_lowercase(), index + 1);
        let thread = std::thread::Builder::new()
            .name(name)
            .spawn(move || stream_thread(&shared, kind, index, &device, io))
            .context("starting an audio thread")?;
        self.threads.push(thread);
        Ok(())
    }

    pub fn status(&self) -> AudioStatus {
        let read = |slots: &[AtomicU8; STREAMS]| {
            std::array::from_fn(|index| match slots[index].load(Ordering::Relaxed) {
                STATE_READY => DeviceState::Ready,
                STATE_FAILED => DeviceState::Failed,
                _ => DeviceState::Missing,
            })
        };
        AudioStatus { mics: read(&self.shared.mics), outputs: read(&self.shared.outputs) }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// Opens the stream's device and runs it. A device that is missing, fails or
/// goes away (unplugged, driver restarted, format changed) is opened again.
fn stream_thread(shared: &Shared, kind: Kind, index: usize, device: &str, mut io: Io) {
    let _com = Com::enter();
    let _priority = AudioPriority::enter();
    let slot = shared.slot(kind, index);
    let label = format!("{}{}", kind.prefix(), index + 1);
    let mut last_problem = String::new();
    while !shared.stop.load(Ordering::Relaxed) {
        let outcome = find_endpoint(kind, device).and_then(|endpoint| match endpoint {
            Some(endpoint) => run_stream(shared, &endpoint, &mut io, slot).map(|()| None),
            None => Ok(Some(STATE_MISSING)),
        });
        let (state, problem) = match outcome {
            Ok(Some(state)) => (state, format!("no sound device named `{device}`")),
            // The stream ran until the add-on was told to stop.
            Ok(None) => (STATE_MISSING, String::new()),
            Err(err) => (STATE_FAILED, format!("`{device}`: {}", err.message())),
        };
        slot.store(state, Ordering::Relaxed);
        // Reported once, not on every retry.
        if !problem.is_empty() && problem != last_problem {
            crate::log::line(&format!("{label}: {problem}"));
            last_problem = problem;
        }
        let waited_until = std::time::Instant::now() + RETRY;
        while !shared.stop.load(Ordering::Relaxed) && std::time::Instant::now() < waited_until {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    slot.store(STATE_MISSING, Ordering::Relaxed);
}

/// The endpoint a stream opens, or `None` while no device has its name.
fn find_endpoint(kind: Kind, name: &str) -> windows::core::Result<Option<IMMDevice>> {
    // SAFETY: COM is initialized on this thread for the lifetime of `Com`.
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    match kind {
        Kind::Output => Ok(endpoints(&enumerator, eRender)?.into_iter().find(|(_, props)| name_matches(&props.name, name)).map(|(device, _)| device)),
        Kind::Mic => {
            let playback = endpoints(&enumerator, eRender)?;
            // The user picks the microphone by name. This add-on plays into
            // the other end of the same cable: the playback endpoint of the
            // same device instance.
            let mic = endpoints(&enumerator, eCapture)?.into_iter().find(|(_, props)| name_matches(&props.name, name));
            let feed = match mic {
                Some((_, mic)) => playback.into_iter().find(|(_, props)| !mic.instance.is_empty() && props.instance == mic.instance),
                // A cable whose two ends are separate devices is set up by
                // naming the playback end directly.
                None => playback.into_iter().find(|(_, props)| name_matches(&props.name, name)),
            };
            Ok(feed.map(|(device, _)| device))
        }
    }
}

struct EndpointProps {
    name: String,
    instance: String,
}

fn endpoints(enumerator: &IMMDeviceEnumerator, flow: EDataFlow) -> windows::core::Result<Vec<(IMMDevice, EndpointProps)>> {
    // SAFETY: plain COM calls on live interfaces.
    unsafe {
        let collection = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        let mut out = Vec::new();
        for index in 0..collection.GetCount()? {
            let device = collection.Item(index)?;
            let Ok(store) = device.OpenPropertyStore(STGM_READ) else { continue };
            let props = EndpointProps {
                name: property_text(&store, &PKEY_Device_FriendlyName).unwrap_or_default(),
                instance: property_text(&store, &PKEY_ENDPOINT_INSTANCE).unwrap_or_default().to_ascii_uppercase(),
            };
            out.push((device, props));
        }
        Ok(out)
    }
}

fn property_text(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    // SAFETY: `store` is live; the returned string is owned by us and freed below.
    unsafe {
        let value = store.GetValue(key).ok()?;
        let text = PropVariantToStringAlloc(&value).ok()?;
        let out = text.to_string().ok();
        CoTaskMemFree(Some(text.0.cast()));
        out
    }
}

/// Whether an endpoint is the device the settings name.
///
/// Windows names an endpoint "Mikrofon (Steam Streaming Mic B1)": its own
/// name, then the device's in brackets, with "2- " in front when two devices
/// share a name. Either part may be given. `Steam Streaming Mic B1` matches
/// that endpoint, and `CABLE Output` matches `CABLE Output (VB-Audio Virtual
/// Cable)`, but `Steam Streaming Mic B1` never matches `... B10`.
fn name_matches(endpoint: &str, wanted: &str) -> bool {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return false;
    }
    let starts = endpoint.get(..wanted.len()).is_some_and(|head| head.eq_ignore_ascii_case(wanted))
        && endpoint[wanted.len()..].chars().next().is_none_or(|next| next == ' ' || next == '(');
    starts || device_part(endpoint).is_some_and(|device| device.eq_ignore_ascii_case(wanted))
}

/// The device's name inside the last brackets, without a "2- " in front.
fn device_part(endpoint: &str) -> Option<&str> {
    let inner = endpoint.trim_end().strip_suffix(')')?;
    let inner = &inner[inner.rfind('(')? + 1..];
    let unnumbered = inner.split_once("- ").filter(|(number, _)| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()));
    Some(unnumbered.map_or(inner, |(_, rest)| rest).trim())
}

fn link_format() -> WAVEFORMATEXTENSIBLE {
    let block = (CHANNELS * size_of::<f32>()) as u16;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: FORMAT_EXTENSIBLE,
            nChannels: CHANNELS as u16,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * u32::from(block),
            nBlockAlign: block,
            wBitsPerSample: 32,
            cbSize: (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
        dwChannelMask: STEREO_MASK,
        SubFormat: SUBTYPE_FLOAT,
    }
}

/// Runs one stream until the add-on stops (`Ok`) or the device fails (`Err`).
fn run_stream(shared: &Shared, endpoint: &IMMDevice, io: &mut Io, slot: &AtomicU8) -> windows::core::Result<()> {
    let mut flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    if matches!(io, Io::Tap(_)) {
        flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
    }
    let format = link_format();
    let wake = Event::new()?;
    // SAFETY: `format` outlives the call, and the event outlives the client,
    // which is stopped and released before `wake` is dropped.
    let client: IAudioClient = unsafe {
        let client: IAudioClient = endpoint.Activate(CLSCTX_ALL, None)?;
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, BUFFER_HNS, 0, &format.Format, None)?;
        client.SetEventHandle(wake.0)?;
        client
    };
    let result = match io {
        Io::Feed(reader) => feed(shared, &client, &wake, reader, slot),
        Io::Tap(writer) => tap(shared, &client, &wake, writer, slot),
    };
    // SAFETY: plain COM call. Stopping a stream that already failed may fail too.
    let _ = unsafe { client.Stop() };
    result
}

/// Plays what the headset sent into the cable.
fn feed(shared: &Shared, client: &IAudioClient, wake: &Event, reader: &mut PcmReader, slot: &AtomicU8) -> windows::core::Result<()> {
    // SAFETY: plain COM calls on a client initialized for rendering. The buffer
    // WASAPI hands out holds `frames` frames of the float stereo format asked
    // for in `Initialize`, and is not touched after `ReleaseBuffer`.
    unsafe {
        let capacity = client.GetBufferSize()?;
        let render: IAudioRenderClient = client.GetService()?;
        client.Start()?;
        slot.store(STATE_READY, Ordering::Relaxed);
        while !shared.stop.load(Ordering::Relaxed) {
            if WaitForSingleObject(wake.0, WAIT_MS) != WAIT_OBJECT_0 {
                continue;
            }
            let frames = capacity.saturating_sub(client.GetCurrentPadding()?);
            if frames == 0 {
                continue;
            }
            let buffer = render.GetBuffer(frames)?;
            let samples = std::slice::from_raw_parts_mut(buffer.cast::<f32>(), frames as usize * CHANNELS);
            reader.read_interleaved(samples);
            render.ReleaseBuffer(frames, 0)?;
        }
    }
    Ok(())
}

/// Sends what applications play on the output.
fn tap(shared: &Shared, client: &IAudioClient, wake: &Event, writer: &mut PcmWriter, slot: &AtomicU8) -> windows::core::Result<()> {
    const SILENCE: [f32; FRAME * CHANNELS] = [0.0; FRAME * CHANNELS];
    // SAFETY: plain COM calls on a client initialized for loopback capture. A
    // packet's buffer holds `frames` frames of the float stereo format asked
    // for in `Initialize`, and is not touched after `ReleaseBuffer`.
    unsafe {
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        slot.store(STATE_READY, Ordering::Relaxed);
        while !shared.stop.load(Ordering::Relaxed) {
            // An output nothing plays on delivers no packets and no events; the
            // timeout only brings this loop back to the stop flag.
            let _ = WaitForSingleObject(wake.0, WAIT_MS);
            while capture.GetNextPacketSize()? > 0 {
                let mut data = std::ptr::null_mut();
                let (mut frames, mut flags) = (0u32, 0u32);
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                if silent {
                    let mut left = frames as usize * CHANNELS;
                    while left > 0 {
                        let part = left.min(SILENCE.len());
                        writer.write_interleaved(&SILENCE[..part]);
                        left -= part;
                    }
                } else {
                    writer.write_interleaved(std::slice::from_raw_parts(data.cast::<f32>(), frames as usize * CHANNELS));
                }
                capture.ReleaseBuffer(frames)?;
            }
        }
    }
    Ok(())
}

/// COM on this thread, for as long as the value lives.
struct Com {
    entered: bool,
}

impl Com {
    fn enter() -> Self {
        // SAFETY: paired with `CoUninitialize` in `drop` when it succeeded.
        let entered = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        Self { entered }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.entered {
            // SAFETY: balances the successful `CoInitializeEx` of this thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// An auto-reset event WASAPI signals when a stream wants attention.
struct Event(HANDLE);

impl Event {
    fn new() -> windows::core::Result<Self> {
        // SAFETY: plain Win32 call; the handle is closed in `drop`.
        unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.map(Self)
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateEventW` and is closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Puts the thread in Windows' "Pro Audio" scheduling class, so a busy game
/// does not starve it. Without it the stream still works, with more glitches
/// under load.
struct AudioPriority(Option<HANDLE>);

impl AudioPriority {
    fn enter() -> Self {
        let mut task = 0u32;
        // SAFETY: plain Win32 call; reverted in `drop`.
        Self(unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task) }.ok())
    }
}

impl Drop for AudioPriority {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            // SAFETY: the handle came from `AvSetMmThreadCharacteristicsW`.
            let _ = unsafe { AvRevertMmThreadCharacteristics(handle) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_should_match_the_whole_endpoint_name() {
        assert!(name_matches("Frame B1", "Frame B1"));
    }

    #[test]
    fn name_should_match_before_the_adapter_name() {
        assert!(name_matches("Frame B1 (Frame Audio Link)", "frame b1"));
    }

    #[test]
    fn name_should_not_match_a_longer_number() {
        assert!(!name_matches("Frame B10 (Frame Audio Link)", "Frame B1"));
    }

    #[test]
    fn device_name_in_brackets_should_match() {
        assert!(name_matches("Mikrofon (Steam Streaming Mic B1)", "Steam Streaming Mic B1"));
    }

    #[test]
    fn numbered_device_name_should_match() {
        assert!(name_matches("Lautsprecher (2- Steam Streaming Out PC1)", "steam streaming out pc1"));
    }

    #[test]
    fn device_name_in_brackets_should_not_match_a_longer_number() {
        assert!(!name_matches("Mikrofon (Steam Streaming Mic B10)", "Steam Streaming Mic B1"));
    }

    #[test]
    fn steam_original_should_not_match_a_new_device() {
        assert!(!name_matches("Mikrofon (Steam Streaming Microphone)", "Steam Streaming Mic B1"));
    }

    #[test]
    fn empty_name_should_match_nothing() {
        assert!(!name_matches("Speakers", " "));
    }

    #[test]
    fn link_format_should_be_48_khz_float_stereo() {
        let format = link_format();
        let (rate, block, bits) = (format.Format.nSamplesPerSec, format.Format.nBlockAlign, format.Format.wBitsPerSample);

        assert_eq!((rate, block, bits), (48_000, 8, 32));
    }
}

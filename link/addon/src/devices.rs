//! The eight virtual sound devices on Windows.
//!
//! Windows only gets a sound device from a kernel driver, and a driver has to
//! be signed by Microsoft. Steam already ships two that are: the Steam
//! Streaming Microphone (a cable: what is played into it comes out as a
//! microphone) and the Steam Streaming Speakers (an output that goes nowhere,
//! which can be recorded). Both are root-enumerated, so Windows will start as
//! many instances as it is given device nodes. This module creates four more
//! of each and names them "Steam Streaming Mic B1"–"B4" and
//! "Steam Streaming Out PC1"–"PC4". Nothing of Steam's is copied or changed:
//! the drivers stay where Steam installed them, and Steam's own two devices
//! are not touched.
//!
//! Installing the driver gives a node the driver's own name back, so every
//! new node is named again afterwards, and each of its endpoints gets the
//! name as its device name too. Without that, Windows would show four more
//! "Steam Streaming Microphone"s, numbered "2-" to "5-".
//!
//! Creating device nodes needs administrator rights, so `install` and
//! `remove` restart themselves elevated and Windows asks for consent.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_Status, DiInstallDevice, SetupDiCallClassInstaller, SetupDiCreateDeviceInfoList, SetupDiCreateDeviceInfoW,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
    SetupDiGetDeviceRegistryPropertyW, SetupDiSetDeviceRegistryPropertyW, CM_DEVNODE_STATUS_FLAGS, CM_PROB, CR_SUCCESS, DICD_GENERATE_ID,
    DIF_REGISTERDEVICE, DIF_REMOVE, DIINSTALLDEVICE_FLAGS, GUID_DEVCLASS_MEDIA, HDEVINFO, SETUP_DI_GET_CLASS_DEVS_FLAGS, SETUP_DI_REGISTRY_PROPERTY,
    SPDRP_DEVICEDESC, SPDRP_FRIENDLYNAME, SPDRP_HARDWAREID, SP_DEVINFO_DATA,
};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eMultimedia, eRender, EDataFlow, ERole, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE,
    DEVICE_STATEMASK_ALL,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

use crate::config::device_name;
use crate::model::{Kind, STREAMS};

/// Device nodes created here are `ROOT\FRAMEAUDIOLINK\0000` and up. That
/// prefix is how they are found again, and what tells them from Steam's own.
const ENUMERATOR: &str = "FrameAudioLink";
const INSTANCE_PREFIX: &str = "ROOT\\FRAMEAUDIOLINK\\";
/// Hardware ids of Steam's two drivers. A node with one of these is started
/// with that driver.
const MIC_HARDWARE_ID: &str = "ROOT\\SteamStreamingMicrophone";
const SPEAKERS_HARDWARE_ID: &str = "ROOT\\SteamStreamingSpeakers";
const STEAM_MIC_INSTANCE: &str = "ROOT\\STEAMSTREAMINGMICROPHONE\\";
const STEAM_SPEAKERS_INSTANCE: &str = "ROOT\\STEAMSTREAMINGSPEAKERS\\";
/// How long Windows gets to build the endpoints of a new device.
const ENDPOINT_WAIT: Duration = Duration::from_secs(20);

const ENDPOINTS_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio";
/// The first part of an endpoint's name ("Mikrofon"), and the device name in
/// brackets after it. Windows builds the full name from the two. Only the
/// device name is set here; the first part stays Windows' own.
const VALUE_ENDPOINT_NAME: &str = "{a45c254e-df1c-4efd-8020-67d146a850e0},2";
const VALUE_ADAPTER_NAME: &str = "{b3f8fa53-0004-438e-9003-51a46e139bfc},6";
/// The device instance an endpoint belongs to, as `{1}.ROOT\NAME\0001`.
const VALUE_INSTANCE: &str = "{b3f8fa53-0004-438e-9003-51a46e139bfc},2";

/// The interface the Sound control panel sets the default device with. It is
/// undocumented but unchanged since Windows 7. Only `SetDefaultEndpoint` is
/// called; the methods before it are declared so it lands in the right slot.
mod policy {
    // The methods keep their COM names, so the table reads like its documentation.
    #![allow(non_snake_case)]

    use std::ffi::c_void;

    use windows::core::{interface, IUnknown, IUnknown_Vtbl, HRESULT, PCWSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::ERole;

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub(super) unsafe trait IPolicyConfig: IUnknown {
        fn GetMixFormat(&self, id: PCWSTR, format: *mut *mut c_void) -> HRESULT;
        fn GetDeviceFormat(&self, id: PCWSTR, default: i32, format: *mut *mut c_void) -> HRESULT;
        fn ResetDeviceFormat(&self, id: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(&self, id: PCWSTR, endpoint: *mut c_void, mix: *mut c_void) -> HRESULT;
        fn GetProcessingPeriod(&self, id: PCWSTR, default: i32, default_period: *mut i64, minimum_period: *mut i64) -> HRESULT;
        fn SetProcessingPeriod(&self, id: PCWSTR, period: *mut i64) -> HRESULT;
        fn GetShareMode(&self, id: PCWSTR, mode: *mut c_void) -> HRESULT;
        fn SetShareMode(&self, id: PCWSTR, mode: *mut c_void) -> HRESULT;
        fn GetPropertyValue(&self, id: PCWSTR, fx_store: i32, key: *const PROPERTYKEY, value: *mut c_void) -> HRESULT;
        fn SetPropertyValue(&self, id: PCWSTR, fx_store: i32, key: *const PROPERTYKEY, value: *mut c_void) -> HRESULT;
        fn SetDefaultEndpoint(&self, id: PCWSTR, role: ERole) -> HRESULT;
    }

    /// Makes the endpoint with this id the default for one role. The generated
    /// methods are private to this module, so this is the way in.
    ///
    /// # Safety
    /// `id` must point to a NUL-terminated UTF-16 string.
    pub(super) unsafe fn set_default(policy: &IPolicyConfig, id: PCWSTR, role: ERole) -> bool {
        // SAFETY: the caller guarantees `id`; the interface is live.
        unsafe { policy.SetDefaultEndpoint(id, role) }.is_ok()
    }
}

use policy::IPolicyConfig;

const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// One of the eight devices this module manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    kind: Kind,
    index: usize,
}

impl Slot {
    fn all() -> impl Iterator<Item = Slot> {
        [Kind::Mic, Kind::Output].into_iter().flat_map(|kind| (0..STREAMS).map(move |index| Slot { kind, index }))
    }

    /// "Steam Streaming Mic B1": the device node's name in Device Manager and the endpoint
    /// name applications show.
    fn name(self) -> String {
        device_name(self.kind, self.index)
    }

    fn hardware_id(self) -> &'static str {
        match self.kind {
            Kind::Mic => MIC_HARDWARE_ID,
            Kind::Output => SPEAKERS_HARDWARE_ID,
        }
    }

    /// Endpoints Windows builds for this device: a cable has two ends.
    fn endpoint_count(self) -> usize {
        match self.kind {
            Kind::Mic => 2,
            Kind::Output => 1,
        }
    }
}

/// A device node of the MEDIA class.
#[derive(Debug, Clone)]
struct Node {
    instance: String,
    /// The name set on the node, or the driver's description when none is.
    name: String,
    /// First hardware id. Tells a microphone node of ours from a speaker node.
    hardware_id: String,
    /// Windows' problem code. Zero while the driver runs.
    problem: u32,
}

/// One audio endpoint, read from the registry.
#[derive(Debug, Clone)]
struct Endpoint {
    flow: Flow,
    /// The GUID part of the endpoint id, with braces.
    guid: String,
    /// Upper-case device instance, without the `{1}.` in front.
    instance: String,
    name: String,
    adapter: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Render,
    Capture,
}

impl Flow {
    fn key(self) -> &'static str {
        match self {
            Flow::Render => "Render",
            Flow::Capture => "Capture",
        }
    }

    /// The endpoint id Windows' audio API uses for an endpoint of this flow.
    fn endpoint_id(self, guid: &str) -> String {
        match self {
            Flow::Render => format!("{{0.0.0.00000000}}.{guid}"),
            Flow::Capture => format!("{{0.0.1.00000000}}.{guid}"),
        }
    }
}

/// Prints what the installer does, and keeps it for the unelevated process
/// that started it: an elevated process has no console of its own.
struct Report {
    file: Option<PathBuf>,
    lines: Vec<String>,
}

impl Report {
    fn new() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let file = args.iter().position(|arg| arg == "--report").and_then(|at| args.get(at + 1)).map(PathBuf::from);
        Self { file, lines: Vec::new() }
    }

    fn say(&mut self, line: impl Into<String>) {
        let line = line.into();
        println!("{line}");
        self.lines.push(line);
        if let Some(file) = &self.file {
            let _ = std::fs::write(file, self.lines.join("\n"));
        }
    }
}

/// Creates the devices that are missing and names their endpoints.
///
/// # Errors
/// When Steam's drivers are not installed, the user declines the elevation
/// prompt, or Windows refuses to start a device.
pub fn install() -> anyhow::Result<()> {
    if !elevated() {
        return run_elevated("install");
    }
    let mut report = Report::new();
    let result = install_elevated(&mut report);
    if let Err(err) = &result {
        report.say(format!("Failed: {err:#}"));
    }
    result
}

/// Removes every device this module created.
///
/// # Errors
/// When the user declines the elevation prompt or Windows refuses a removal.
pub fn remove() -> anyhow::Result<()> {
    if !elevated() {
        return run_elevated("remove");
    }
    let mut report = Report::new();
    let result = remove_elevated(&mut report);
    if let Err(err) = &result {
        report.say(format!("Failed: {err:#}"));
    }
    result
}

/// Lists the devices and their endpoints.
///
/// # Errors
/// When the device list cannot be read.
pub fn print_status() -> anyhow::Result<()> {
    let nodes = media_nodes(false)?;
    let endpoints = endpoints();
    let ours: Vec<&Node> = nodes.iter().filter(|node| is_ours(&node.instance)).collect();
    println!("Steam drivers: microphone {}, speakers {}", presence(&nodes, STEAM_MIC_INSTANCE), presence(&nodes, STEAM_SPEAKERS_INSTANCE));
    if ours.is_empty() {
        println!("No Frame Audio Link devices are installed. Run `frame-audio-link devices install`.");
        return Ok(());
    }
    for node in ours {
        let state = if node.problem == 0 { "running".to_string() } else { format!("problem code {}", node.problem) };
        println!("{} [{}] {state}", node.name, node.instance);
        for endpoint in endpoints.iter().filter(|endpoint| endpoint.instance == node.instance) {
            println!("    {} endpoint: {} ({})", endpoint.flow.key().to_lowercase(), endpoint.name, endpoint.adapter);
        }
    }
    Ok(())
}

fn presence(nodes: &[Node], instance_prefix: &str) -> &'static str {
    match nodes.iter().find(|node| node.instance.starts_with(instance_prefix)) {
        Some(node) if node.problem == 0 => "installed",
        Some(_) => "installed but not running",
        None => "not installed",
    }
}

fn is_ours(instance: &str) -> bool {
    instance.starts_with(INSTANCE_PREFIX)
}

fn install_elevated(report: &mut Report) -> anyhow::Result<()> {
    let _com = Com::enter();
    let nodes = media_nodes(false)?;
    for (prefix, what) in [(STEAM_MIC_INSTANCE, "Steam Streaming Microphone"), (STEAM_SPEAKERS_INSTANCE, "Steam Streaming Speakers")] {
        if !nodes.iter().any(|node| node.instance.starts_with(prefix)) {
            bail!(
                "the {what} driver is not installed. Steam installs it the first time Remote Play or wireless VR streaming is used on this PC; \
                 start a streaming session once and run this again"
            );
        }
    }
    // Windows may make a device that just appeared the default one. Whatever
    // is default now is put back at the end.
    let defaults = default_endpoints();
    let mut created = 0;
    let mut ours: Vec<Node> = nodes.into_iter().filter(|node| is_ours(&node.instance)).collect();
    // A node an older version created carries the driver's name. It is taken
    // over by the first free slot of its kind instead of making another one.
    for slot in Slot::all() {
        if ours.iter().any(|node| node.name == slot.name()) {
            continue;
        }
        let taken: Vec<String> = Slot::all().map(Slot::name).collect();
        let unnamed = ours.iter_mut().find(|node| node.hardware_id.eq_ignore_ascii_case(slot.hardware_id()) && !taken.contains(&node.name));
        if let Some(node) = unnamed {
            rename_node(&node.instance, &slot.name()).with_context(|| format!("naming {}", node.instance))?;
            node.name = slot.name();
        }
    }
    for slot in Slot::all() {
        let name = slot.name();
        if let Some(node) = ours.iter().find(|node| node.name == name) {
            report.say(format!("{name}: already there ({})", node.instance));
            continue;
        }
        let instance = create_node(slot).with_context(|| format!("creating {name}"))?;
        report.say(format!("{name}: created ({instance})"));
        ours.push(Node { instance, name, hardware_id: slot.hardware_id().into(), problem: 0 });
        created += 1;
    }
    let named = name_endpoints(report)?;
    restore_defaults(&defaults, report);
    report.say(format!("Done: {created} devices created, {named} endpoints named."));
    Ok(())
}

fn remove_elevated(report: &mut Report) -> anyhow::Result<()> {
    let _com = Com::enter();
    let defaults = default_endpoints();
    let removed = remove_nodes(report)?;
    restore_defaults(&defaults, report);
    report.say(format!("Done: {removed} devices removed."));
    Ok(())
}

/// A device information set that is released when it goes out of scope.
struct DeviceSet(HDEVINFO);

impl Drop for DeviceSet {
    fn drop(&mut self) {
        // SAFETY: the set came from SetupAPI and is destroyed once.
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

fn empty_info() -> SP_DEVINFO_DATA {
    SP_DEVINFO_DATA { cbSize: size_of::<SP_DEVINFO_DATA>() as u32, ..SP_DEVINFO_DATA::default() }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A string as the bytes of a `REG_MULTI_SZ`: UTF-16 with two terminators.
fn multi_sz(text: &str) -> Vec<u8> {
    text.encode_utf16().chain([0, 0]).flat_map(u16::to_le_bytes).collect()
}

/// A string as the bytes of a `REG_SZ`: UTF-16 with one terminator.
fn reg_sz(text: &str) -> Vec<u8> {
    text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

/// Creates one device node and starts Steam's driver on it. Returns its
/// instance id. A node Windows cannot start is removed again.
fn create_node(slot: Slot) -> anyhow::Result<String> {
    let (enumerator, name) = (wide(ENUMERATOR), wide(&slot.name()));
    let mut info = empty_info();
    // SAFETY: plain SetupAPI calls. Every pointer refers to a local that
    // outlives the call, and `info` is filled in by `SetupDiCreateDeviceInfoW`
    // before it is used.
    unsafe {
        let set = DeviceSet(SetupDiCreateDeviceInfoList(Some(&GUID_DEVCLASS_MEDIA), None).context("opening the device list")?);
        SetupDiCreateDeviceInfoW(set.0, PCWSTR(enumerator.as_ptr()), &GUID_DEVCLASS_MEDIA, PCWSTR(name.as_ptr()), None, DICD_GENERATE_ID, Some(&mut info))
            .context("creating the device node")?;
        SetupDiSetDeviceRegistryPropertyW(set.0, &mut info, SPDRP_HARDWAREID, Some(&multi_sz(slot.hardware_id()))).context("setting the hardware id")?;
        SetupDiSetDeviceRegistryPropertyW(set.0, &mut info, SPDRP_FRIENDLYNAME, Some(&reg_sz(&slot.name()))).context("naming the device node")?;
        SetupDiCallClassInstaller(DIF_REGISTERDEVICE, set.0, Some(&info)).context("registering the device node")?;
        let instance = instance_id(set.0, &info).unwrap_or_default();
        // Installs the best driver in the driver store for this node only.
        // Steam's own devices, which share the hardware id, are left alone.
        let installed = DiInstallDevice(None, set.0, &info, None, DIINSTALLDEVICE_FLAGS(0), None);
        let problem = node_problem(&info);
        // The driver install named the node after the driver; it gets its own name back.
        let _ = SetupDiSetDeviceRegistryPropertyW(set.0, &mut info, SPDRP_FRIENDLYNAME, Some(&reg_sz(&slot.name())));
        if installed.is_err() || problem != 0 {
            let _ = SetupDiCallClassInstaller(DIF_REMOVE, set.0, Some(&info));
            return match installed {
                Err(err) => Err(anyhow!("Windows could not install Steam's driver on it: {}", err.message())),
                Ok(()) => Err(anyhow!("Steam's driver did not start on a second device (problem code {problem})")),
            };
        }
        Ok(instance)
    }
}

/// Removes every node this module created. Returns how many.
fn remove_nodes(report: &mut Report) -> anyhow::Result<usize> {
    let mut removed = 0;
    // SAFETY: plain SetupAPI calls on a set that lives until the end of the block.
    unsafe {
        let set = DeviceSet(SetupDiGetClassDevsW(Some(&GUID_DEVCLASS_MEDIA), PCWSTR::null(), None, SETUP_DI_GET_CLASS_DEVS_FLAGS(0)).context("listing sound devices")?);
        // Removing a member does not renumber the rest, so one pass sees all.
        for index in 0.. {
            let mut info = empty_info();
            if SetupDiEnumDeviceInfo(set.0, index, &mut info).is_err() {
                break;
            }
            let Some(instance) = instance_id(set.0, &info) else { continue };
            if !is_ours(&instance) {
                continue;
            }
            let name = node_name(set.0, &info);
            SetupDiCallClassInstaller(DIF_REMOVE, set.0, Some(&info)).with_context(|| format!("removing {name} ({instance})"))?;
            report.say(format!("{name}: removed ({instance})"));
            removed += 1;
        }
    }
    Ok(removed)
}

/// Sets the name of the node with this instance id.
fn rename_node(instance: &str, name: &str) -> anyhow::Result<()> {
    // SAFETY: plain SetupAPI calls on a set that lives until the end of the block.
    unsafe {
        let set = DeviceSet(SetupDiGetClassDevsW(Some(&GUID_DEVCLASS_MEDIA), PCWSTR::null(), None, SETUP_DI_GET_CLASS_DEVS_FLAGS(0)).context("listing sound devices")?);
        for index in 0.. {
            let mut info = empty_info();
            if SetupDiEnumDeviceInfo(set.0, index, &mut info).is_err() {
                break;
            }
            if instance_id(set.0, &info).as_deref() == Some(instance) {
                return SetupDiSetDeviceRegistryPropertyW(set.0, &mut info, SPDRP_FRIENDLYNAME, Some(&reg_sz(name))).context("setting the name");
            }
        }
    }
    bail!("the device is gone")
}

/// Every device node of the MEDIA class. `present_only` leaves out nodes whose
/// device is not there right now.
fn media_nodes(present_only: bool) -> anyhow::Result<Vec<Node>> {
    let flags = if present_only { windows::Win32::Devices::DeviceAndDriverInstallation::DIGCF_PRESENT } else { SETUP_DI_GET_CLASS_DEVS_FLAGS(0) };
    let mut nodes = Vec::new();
    // SAFETY: plain SetupAPI calls on a set that lives until the end of the block.
    unsafe {
        let set = DeviceSet(SetupDiGetClassDevsW(Some(&GUID_DEVCLASS_MEDIA), PCWSTR::null(), None, flags).context("listing sound devices")?);
        for index in 0.. {
            let mut info = empty_info();
            if SetupDiEnumDeviceInfo(set.0, index, &mut info).is_err() {
                break;
            }
            let Some(instance) = instance_id(set.0, &info) else { continue };
            let hardware_id = node_text(set.0, &info, SPDRP_HARDWAREID);
            nodes.push(Node { instance, name: node_name(set.0, &info), hardware_id, problem: node_problem(&info) });
        }
    }
    Ok(nodes)
}

/// The node's instance id in upper case, as Windows compares them.
unsafe fn instance_id(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<String> {
    let mut buffer = [0u16; 512];
    // SAFETY: the caller passes a live set and a member of it; the buffer is local.
    unsafe { SetupDiGetDeviceInstanceIdW(set, info, Some(&mut buffer), None) }.ok()?;
    let len = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]).to_ascii_uppercase())
}

/// The node's own name, or the driver's description of it when it has none.
unsafe fn node_name(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> String {
    // SAFETY: passed on from the caller.
    let name = unsafe { node_text(set, info, SPDRP_FRIENDLYNAME) };
    if name.is_empty() {
        // SAFETY: as above.
        unsafe { node_text(set, info, SPDRP_DEVICEDESC) }
    } else {
        name
    }
}

/// A text property of a node; the first entry of a list. Empty when it is not set.
unsafe fn node_text(set: HDEVINFO, info: &SP_DEVINFO_DATA, property: SETUP_DI_REGISTRY_PROPERTY) -> String {
    let mut buffer = [0u8; 512];
    // SAFETY: the caller passes a live set and a member of it; the buffer is local.
    if unsafe { SetupDiGetDeviceRegistryPropertyW(set, info, property, None, Some(&mut buffer), None) }.is_err() {
        return String::new();
    }
    utf16_bytes(&buffer)
}

fn utf16_bytes(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).take_while(|unit| *unit != 0).collect();
    String::from_utf16_lossy(&units)
}

/// Windows' problem code for a node: zero while its driver runs. A node whose
/// status cannot be read is reported as failed (code 1).
fn node_problem(info: &SP_DEVINFO_DATA) -> u32 {
    let (mut status, mut problem) = (CM_DEVNODE_STATUS_FLAGS(0), CM_PROB(0));
    // SAFETY: both out-pointers refer to locals; `DevInst` comes from SetupAPI.
    let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) };
    if result == CR_SUCCESS {
        problem.0
    } else {
        1
    }
}

/// Every audio endpoint in the registry, whatever its state.
fn endpoints() -> Vec<Endpoint> {
    let mut found = Vec::new();
    for flow in [Flow::Render, Flow::Capture] {
        let root = format!("{ENDPOINTS_KEY}\\{}", flow.key());
        for guid in subkeys(&root) {
            let key = format!("{root}\\{guid}\\Properties");
            let Some(instance) = read_value(&key, VALUE_INSTANCE) else { continue };
            let instance = instance.rsplit_once('.').map_or(instance.as_str(), |(_, tail)| tail).to_ascii_uppercase();
            found.push(Endpoint {
                flow,
                guid,
                instance,
                name: read_value(&key, VALUE_ENDPOINT_NAME).unwrap_or_default(),
                adapter: read_value(&key, VALUE_ADAPTER_NAME).unwrap_or_default(),
            });
        }
    }
    found
}

/// Waits for Windows to build the endpoints of every node of ours, then gives
/// each its name. Returns how many names were written.
fn name_endpoints(report: &mut Report) -> anyhow::Result<usize> {
    let deadline = Instant::now() + ENDPOINT_WAIT;
    loop {
        let nodes: Vec<Node> = media_nodes(true)?.into_iter().filter(|node| is_ours(&node.instance)).collect();
        let endpoints = endpoints();
        let complete = Slot::all().all(|slot| {
            let Some(node) = nodes.iter().find(|node| node.name == slot.name()) else { return false };
            endpoints.iter().filter(|endpoint| endpoint.instance == node.instance).count() >= slot.endpoint_count()
        });
        if complete || Instant::now() >= deadline {
            let mut named = 0;
            for slot in Slot::all() {
                let Some(node) = nodes.iter().find(|node| node.name == slot.name()) else { continue };
                for endpoint in endpoints.iter().filter(|endpoint| endpoint.instance == node.instance) {
                    named += usize::from(name_endpoint(slot, endpoint, report));
                }
            }
            if !complete {
                report.say("Windows has not built every endpoint yet. Run `frame-audio-link devices install` again in a moment to name the rest.");
            }
            return Ok(named);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Gives one endpoint the device's name, the part Windows shows in brackets.
/// True when something changed.
fn name_endpoint(slot: Slot, endpoint: &Endpoint, report: &mut Report) -> bool {
    let wanted = slot.name();
    if endpoint.adapter == wanted {
        return false;
    }
    let key = format!("{ENDPOINTS_KEY}\\{}\\{}\\Properties", endpoint.flow.key(), endpoint.guid);
    match write_value(&key, VALUE_ADAPTER_NAME, &wanted) {
        Ok(()) => {
            report.say(format!("{wanted}: {} endpoint `{} ({wanted})`", endpoint.flow.key().to_lowercase(), endpoint.name));
            true
        }
        Err(err) => {
            report.say(format!("{}: the {} endpoint could not be named: {err:#}", slot.name(), endpoint.flow.key().to_lowercase()));
            false
        }
    }
}

/// An open registry key under HKEY_LOCAL_MACHINE, closed on drop.
struct Key(HKEY);

impl Key {
    fn open(path: &str, access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> anyhow::Result<Self> {
        let path_wide = wide(path);
        let mut key = HKEY::default();
        // SAFETY: `path_wide` is a terminated string and `key` a local out-pointer.
        let status = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(path_wide.as_ptr()), None, access, &mut key) };
        status.ok().with_context(|| format!("opening HKLM\\{path}"))?;
        Ok(Self(key))
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened by `RegOpenKeyExW` and is closed once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn subkeys(path: &str) -> Vec<String> {
    use windows::Win32::System::Registry::{RegEnumKeyExW, KEY_ENUMERATE_SUB_KEYS};
    let Ok(key) = Key::open(path, KEY_ENUMERATE_SUB_KEYS) else { return Vec::new() };
    let mut names = Vec::new();
    for index in 0.. {
        let mut buffer = [0u16; 256];
        let mut len = buffer.len() as u32;
        // SAFETY: `buffer` and `len` are locals sized to match each other.
        let status = unsafe { RegEnumKeyExW(key.0, index, Some(windows::core::PWSTR(buffer.as_mut_ptr())), &mut len, None, None, None, None) };
        if status.is_err() {
            break;
        }
        names.push(String::from_utf16_lossy(&buffer[..len as usize]));
    }
    names
}

fn read_value(path: &str, name: &str) -> Option<String> {
    let key = Key::open(path, KEY_QUERY_VALUE).ok()?;
    let name_wide = wide(name);
    let mut buffer = [0u8; 1024];
    let mut len = buffer.len() as u32;
    let mut kind = REG_SZ;
    // SAFETY: `buffer` and `len` are locals sized to match each other.
    let status = unsafe { RegQueryValueExW(key.0, PCWSTR(name_wide.as_ptr()), None, Some(&mut kind), Some(buffer.as_mut_ptr()), Some(&mut len)) };
    (status.is_ok() && kind == REG_SZ).then(|| utf16_bytes(&buffer[..len as usize]))
}

fn write_value(path: &str, name: &str, text: &str) -> anyhow::Result<()> {
    let key = Key::open(path, KEY_SET_VALUE)?;
    let name_wide = wide(name);
    // SAFETY: the value bytes are a terminated UTF-16 string that outlives the call.
    let status = unsafe { RegSetValueExW(key.0, PCWSTR(name_wide.as_ptr()), None, REG_SZ, Some(&reg_sz(text))) };
    status.ok().with_context(|| format!("writing {name}"))
}

/// The default endpoint of each direction and role, by endpoint id.
type Defaults = Vec<(EDataFlow, ERole, String)>;

fn default_endpoints() -> Defaults {
    let mut found = Vec::new();
    // SAFETY: plain COM calls; the id string is owned here and freed after copying.
    unsafe {
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return found };
        for flow in [eRender, eCapture] {
            for role in [eConsole, eMultimedia, eCommunications] {
                let Ok(device) = enumerator.GetDefaultAudioEndpoint(flow, role) else { continue };
                let Ok(id) = device.GetId() else { continue };
                if let Ok(text) = id.to_string() {
                    found.push((flow, role, text));
                }
                CoTaskMemFree(Some(id.0.cast()));
            }
        }
    }
    found
}

/// Puts back every default that moved to one of the devices of this module.
/// A default the user had on another device is theirs and stays.
fn restore_defaults(before: &Defaults, report: &mut Report) {
    // Windows changes the default a moment after a device arrives or leaves.
    std::thread::sleep(Duration::from_secs(2));
    let ours: Vec<String> = endpoints().into_iter().filter(|endpoint| is_ours(&endpoint.instance)).map(|endpoint| endpoint.flow.endpoint_id(&endpoint.guid)).collect();
    let now = default_endpoints();
    // SAFETY: plain COM calls; `id` is a terminated string that outlives the call.
    unsafe {
        let Ok(policy) = CoCreateInstance::<_, IPolicyConfig>(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL) else {
            report.say("The default sound devices could not be checked; have a look at the Windows sound settings.");
            return;
        };
        for (flow, role, wanted) in before {
            let current = now.iter().find(|(now_flow, now_role, _)| now_flow == flow && now_role == role).map(|(_, _, id)| id.as_str());
            let moved_to_ours = current.is_some_and(|id| id != wanted && ours.iter().any(|our| our.eq_ignore_ascii_case(id)));
            if !moved_to_ours {
                continue;
            }
            let id = wide(wanted);
            if policy::set_default(&policy, PCWSTR(id.as_ptr()), *role) {
                report.say(format!("Windows had made a new device the default {}; the previous one is back.", if *flow == eRender { "output" } else { "input" }));
            } else {
                report.say("Windows made a new device the default and it could not be put back; choose your device in the sound settings.");
            }
        }
    }
}

fn elevated() -> bool {
    // SAFETY: plain shell call without arguments.
    unsafe { IsUserAnAdmin() }.as_bool()
}

/// Runs `devices <action>` again with administrator rights and relays what it reported.
fn run_elevated(action: &str) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("finding this program")?;
    let report = std::env::temp_dir().join(format!("frame-audio-link-devices-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&report);
    println!("Windows will ask for permission to change sound devices.");
    let code = elevate(&exe, &format!("devices {action} --report \"{}\"", report.display()))?;
    if let Ok(text) = std::fs::read_to_string(&report) {
        println!("{text}");
    }
    let _ = std::fs::remove_file(&report);
    if code == 0 {
        Ok(())
    } else {
        Err(anyhow!("the elevated step ended with code {code}"))
    }
}

/// Starts `exe` elevated, waits for it, and returns its exit code.
fn elevate(exe: &Path, parameters: &str) -> anyhow::Result<u32> {
    let (file, parameters) = (wide(&exe.to_string_lossy()), wide(parameters));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        ..SHELLEXECUTEINFOW::default()
    };
    // SAFETY: `info` and the strings it points to outlive the call. The
    // process handle is ours to wait on and close.
    unsafe {
        ShellExecuteExW(&mut info).context("the elevation prompt was declined")?;
        if info.hProcess.is_invalid() {
            bail!("the elevated step did not start");
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        let read = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        read.context("reading the result of the elevated step")?;
        Ok(code)
    }
}

/// COM on this thread, for as long as the value lives.
struct Com {
    entered: bool,
}

impl Com {
    fn enter() -> Self {
        // SAFETY: paired with `CoUninitialize` in `drop` when it succeeded.
        Self { entered: unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok() }
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

// `DEVICE_STATE` and the all-states mask are only needed by callers that list
// endpoints through the audio API; the registry walk above sees every state.
const _: DEVICE_STATE = DEVICE_STATE(DEVICE_STATEMASK_ALL);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_slots_should_cover_both_kinds() {
        let names: Vec<String> = Slot::all().map(Slot::name).collect();

        assert_eq!(
            names,
            [
                "Steam Streaming Mic B1",
                "Steam Streaming Mic B2",
                "Steam Streaming Mic B3",
                "Steam Streaming Mic B4",
                "Steam Streaming Out PC1",
                "Steam Streaming Out PC2",
                "Steam Streaming Out PC3",
                "Steam Streaming Out PC4"
            ]
        );
    }

    #[test]
    fn microphone_slot_should_use_steams_microphone_driver() {
        assert_eq!(Slot { kind: Kind::Mic, index: 0 }.hardware_id(), MIC_HARDWARE_ID);
    }

    #[test]
    fn multi_sz_should_end_with_two_terminators() {
        assert_eq!(multi_sz("A"), [0x41, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn utf16_bytes_should_stop_at_the_terminator() {
        assert_eq!(utf16_bytes(&[0x41, 0, 0x42, 0, 0, 0, 0x43, 0]), "AB");
    }

    #[test]
    fn steam_devices_should_not_count_as_ours() {
        assert!(!is_ours("ROOT\\STEAMSTREAMINGMICROPHONE\\0000"));
    }

    #[test]
    fn endpoint_id_should_carry_the_flow() {
        assert_eq!(Flow::Capture.endpoint_id("{abc}"), "{0.0.1.00000000}.{abc}");
    }
}

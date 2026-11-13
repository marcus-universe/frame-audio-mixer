//! The SteamVR dashboard overlay: the session with SteamVR and the loop that
//! keeps the panel on screen.
//!
//! OpenVR is loaded at run time from the SteamVR installation, so the add-on
//! needs no SDK to build and always matches the runtime it talks to. It uses
//! OpenVR's C interface: `VR_GetGenericInterface("FnTable:...")` returns a
//! table of function pointers in the order `openvr_capi.h` declares them. The
//! slot numbers below are that order for the interface versions named next to
//! them (OpenVR 2.5.1); SteamVR keeps serving older versions, and a version it
//! no longer knows is refused by name instead of being called wrongly.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};

use crate::app::App;
use crate::manifest::{self, APP_KEY};
use crate::ui::{self, Lang, Panel};

/// Name of the dashboard tab.
const OVERLAY_NAME: &str = "Audio Link";
/// Width of the panel in the dashboard, in meters. The dashboard scales its
/// tabs to fit, so this mostly sets how sharp the panel is drawn.
const WIDTH_METERS: f32 = 2.5;

const OVERLAY_VERSION: &CStr = c"FnTable:IVROverlay_027";
const SYSTEM_VERSION: &CStr = c"FnTable:IVRSystem_022";
const APPLICATIONS_VERSION: &CStr = c"FnTable:IVRApplications_007";

/// Positions in `VR_IVROverlay_FnTable` (IVROverlay_027).
mod overlay_slot {
    pub const FIND_OVERLAY: usize = 0;
    pub const SET_OVERLAY_WIDTH_IN_METERS: usize = 21;
    pub const IS_OVERLAY_VISIBLE: usize = 43;
    pub const POLL_NEXT_OVERLAY_EVENT: usize = 46;
    pub const SET_OVERLAY_INPUT_METHOD: usize = 48;
    pub const SET_OVERLAY_MOUSE_SCALE: usize = 50;
    pub const SET_OVERLAY_RAW: usize = 60;
    pub const CREATE_DASHBOARD_OVERLAY: usize = 65;
    pub const IS_DASHBOARD_VISIBLE: usize = 66;
}

/// Positions in `VR_IVRSystem_FnTable` (IVRSystem_022).
mod system_slot {
    pub const POLL_NEXT_EVENT: usize = 29;
    pub const ACKNOWLEDGE_QUIT_EXITING: usize = 43;
    pub const GET_RUNTIME_VERSION: usize = 45;
}

/// Positions in `VR_IVRApplications_FnTable` (IVRApplications_007).
mod applications_slot {
    pub const ADD_APPLICATION_MANIFEST: usize = 0;
    pub const REMOVE_APPLICATION_MANIFEST: usize = 1;
    pub const SET_APPLICATION_AUTO_LAUNCH: usize = 17;
}

/// `EVRApplicationType`. An overlay application would start SteamVR when it is
/// not running; a background application never does.
const APPLICATION_OVERLAY: i32 = 2;
const APPLICATION_BACKGROUND: i32 = 3;
/// `VROverlayInputMethod_Mouse`: the laser pointer arrives as mouse events.
const INPUT_METHOD_MOUSE: u32 = 1;

const EVENT_MOUSE_MOVE: u32 = 300;
const EVENT_MOUSE_DOWN: u32 = 301;
const EVENT_MOUSE_UP: u32 = 302;
const EVENT_FOCUS_LEAVE: u32 = 304;
const EVENT_QUIT: u32 = 700;

/// `VREvent_t` is packed to 4 bytes on Linux and to 8 on Windows, which moves
/// the event's data and changes its size.
const EVENT_DATA_OFFSET: usize = if cfg!(windows) { 16 } else { 12 };
const EVENT_SIZE: u32 = if cfg!(windows) { 64 } else { 60 };

/// How often SteamVR is looked for while it is not running.
const RETRY_EVERY: Duration = Duration::from_secs(3);
/// How often the link's masks follow the switches and devices.
const TICK_EVERY: Duration = Duration::from_millis(250);
/// Loop pace while the panel is on screen, while another dashboard tab is, and otherwise.
const PACE_VISIBLE: Duration = Duration::from_millis(30);
const PACE_DASHBOARD: Duration = Duration::from_millis(100);
const PACE_HIDDEN: Duration = Duration::from_millis(250);
/// How often the session checks that SteamVR still answers.
const ALIVE_EVERY: Duration = Duration::from_secs(2);
/// Failed checks in a row after which SteamVR is taken to be gone.
const ALIVE_FAILURES: u32 = 3;

type OverlayHandle = u64;

/// The OpenVR library and its three plain entry points.
struct Runtime {
    /// Never unloaded: the library keeps threads of its own.
    _library: libloading::Library,
    init: unsafe extern "C" fn(*mut i32, i32, *const c_char) -> u32,
    shutdown: unsafe extern "C" fn(),
    interface: unsafe extern "C" fn(*const c_char, *mut i32) -> isize,
}

impl Runtime {
    fn load() -> anyhow::Result<Self> {
        let mut problems = Vec::new();
        for candidate in library_candidates() {
            // SAFETY: loading SteamVR's own client library; its initializers only set up state.
            match unsafe { libloading::Library::new(&candidate) } {
                Ok(library) => return Self::bind(library).with_context(|| format!("using {}", candidate.display())),
                Err(err) => problems.push(format!("{}: {err}", candidate.display())),
            }
        }
        bail!("the OpenVR library was not found ({})", problems.join("; "))
    }

    fn bind(library: libloading::Library) -> anyhow::Result<Self> {
        // SAFETY: the three symbols are OpenVR's documented C entry points with these signatures.
        unsafe {
            let init = *library.get(b"VR_InitInternal2\0").context("VR_InitInternal2 is missing")?;
            let shutdown = *library.get(b"VR_ShutdownInternal\0").context("VR_ShutdownInternal is missing")?;
            let interface = *library.get(b"VR_GetGenericInterface\0").context("VR_GetGenericInterface is missing")?;
            Ok(Self { _library: library, init, shutdown, interface })
        }
    }
}

/// The library from SteamVR's own folder first, then wherever the system finds one.
fn library_candidates() -> Vec<PathBuf> {
    let (folder, file) = if cfg!(windows) {
        ("win64", "openvr_api.dll")
    } else if cfg!(target_arch = "aarch64") {
        ("linuxarm64", "libopenvr_api.so")
    } else {
        ("linux64", "libopenvr_api.so")
    };
    let mut candidates: Vec<PathBuf> =
        manifest::openvr_paths().map(|paths| paths.runtime).unwrap_or_default().into_iter().map(|runtime| runtime.join("bin").join(folder).join(file)).collect();
    candidates.push(PathBuf::from(file));
    candidates
}

/// The loaded library. Loading is tried again on every call until it works,
/// so SteamVR may be installed while the add-on runs.
fn runtime() -> anyhow::Result<&'static Runtime> {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let loaded = Runtime::load()?;
    Ok(RUNTIME.get_or_init(|| loaded))
}

/// An initialized OpenVR client. Dropping it shuts the client down.
struct Connection {
    runtime: &'static Runtime,
}

impl Connection {
    fn open(application: i32) -> anyhow::Result<Self> {
        let runtime = runtime()?;
        let mut error = 0;
        // SAFETY: `error` outlives the call and the startup info may be null.
        unsafe { (runtime.init)(&mut error, application, std::ptr::null()) };
        if error != 0 {
            bail!("SteamVR is not available (init error {error})");
        }
        Ok(Self { runtime })
    }

    /// The function table of one interface version.
    fn table(&self, version: &CStr) -> anyhow::Result<Table> {
        let mut error = 0;
        // SAFETY: `version` is NUL-terminated and `error` outlives the call.
        let address = unsafe { (self.runtime.interface)(version.as_ptr(), &mut error) };
        if address == 0 || error != 0 {
            bail!("SteamVR does not offer {} (error {error})", version.to_string_lossy());
        }
        Ok(Table(address as *const *const c_void))
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: pairs with the successful init in `open`.
        unsafe { (self.runtime.shutdown)() };
    }
}

/// A table of function pointers, as `VR_GetGenericInterface("FnTable:...")` returns it.
struct Table(*const *const c_void);

impl Table {
    /// The function in `slot`, as the function pointer type `F`.
    ///
    /// # Safety
    /// `F` must be the `extern "system"` signature `openvr_capi.h` declares for
    /// that slot of this interface version.
    unsafe fn function<F: Copy>(&self, slot: usize) -> anyhow::Result<F> {
        if std::mem::size_of::<F>() != std::mem::size_of::<*const c_void>() {
            bail!("slot {slot} was asked for as something that is not a function pointer");
        }
        // SAFETY: the table has at least `slot + 1` entries for the versions this
        // module names, and the caller vouches for the signature.
        let entry = unsafe { *self.0.add(slot) };
        if entry.is_null() {
            bail!("slot {slot} of the interface is empty");
        }
        // SAFETY: `F` is pointer-sized (checked above) and `entry` is a function of that type.
        Ok(unsafe { std::mem::transmute_copy(&entry) })
    }
}

/// One `VREvent_t`, with room for either packing.
#[repr(C, align(8))]
struct RawEvent([u8; 64]);

impl RawEvent {
    fn kind(&self) -> u32 {
        self.u32_at(0)
    }

    fn u32_at(&self, offset: usize) -> u32 {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.0[offset..offset + 4]);
        u32::from_ne_bytes(bytes)
    }

    /// The pointer position of a mouse event, in panel pixels from the top left.
    /// OpenVR counts y from the bottom edge.
    fn pointer(&self) -> (f32, f32) {
        let x = f32::from_bits(self.u32_at(EVENT_DATA_OFFSET));
        let y = f32::from_bits(self.u32_at(EVENT_DATA_OFFSET + 4));
        (x, ui::HEIGHT as f32 - y)
    }
}

/// The overlay functions this add-on calls.
struct OverlayFns {
    find_overlay: unsafe extern "system" fn(*const c_char, *mut OverlayHandle) -> u32,
    set_width_in_meters: unsafe extern "system" fn(OverlayHandle, f32) -> u32,
    is_overlay_visible: unsafe extern "system" fn(OverlayHandle) -> bool,
    poll_next_overlay_event: unsafe extern "system" fn(OverlayHandle, *mut RawEvent, u32) -> bool,
    set_input_method: unsafe extern "system" fn(OverlayHandle, u32) -> u32,
    set_mouse_scale: unsafe extern "system" fn(OverlayHandle, *const [f32; 2]) -> u32,
    set_raw: unsafe extern "system" fn(OverlayHandle, *mut c_void, u32, u32, u32) -> u32,
    create_dashboard_overlay: unsafe extern "system" fn(*const c_char, *const c_char, *mut OverlayHandle, *mut OverlayHandle) -> u32,
    is_dashboard_visible: unsafe extern "system" fn() -> bool,
}

impl OverlayFns {
    fn bind(table: &Table) -> anyhow::Result<Self> {
        use overlay_slot as slot;
        // SAFETY: each type is the declaration of that slot in IVROverlay_027.
        unsafe {
            Ok(Self {
                find_overlay: table.function(slot::FIND_OVERLAY)?,
                set_width_in_meters: table.function(slot::SET_OVERLAY_WIDTH_IN_METERS)?,
                is_overlay_visible: table.function(slot::IS_OVERLAY_VISIBLE)?,
                poll_next_overlay_event: table.function(slot::POLL_NEXT_OVERLAY_EVENT)?,
                set_input_method: table.function(slot::SET_OVERLAY_INPUT_METHOD)?,
                set_mouse_scale: table.function(slot::SET_OVERLAY_MOUSE_SCALE)?,
                set_raw: table.function(slot::SET_OVERLAY_RAW)?,
                create_dashboard_overlay: table.function(slot::CREATE_DASHBOARD_OVERLAY)?,
                is_dashboard_visible: table.function(slot::IS_DASHBOARD_VISIBLE)?,
            })
        }
    }
}

/// The system functions this add-on calls.
struct SystemFns {
    poll_next_event: unsafe extern "system" fn(*mut RawEvent, u32) -> bool,
    acknowledge_quit_exiting: unsafe extern "system" fn(),
    get_runtime_version: unsafe extern "system" fn() -> *const c_char,
}

impl SystemFns {
    fn bind(table: &Table) -> anyhow::Result<Self> {
        use system_slot as slot;
        // SAFETY: each type is the declaration of that slot in IVRSystem_022.
        unsafe {
            Ok(Self {
                poll_next_event: table.function(slot::POLL_NEXT_EVENT)?,
                acknowledge_quit_exiting: table.function(slot::ACKNOWLEDGE_QUIT_EXITING)?,
                get_runtime_version: table.function(slot::GET_RUNTIME_VERSION)?,
            })
        }
    }
}

/// What the laser pointer and SteamVR did since the last poll.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    Move(f32, f32),
    Down(f32, f32),
    Up(f32, f32),
    Leave,
    Quit,
}

/// What the loop should do after one pass.
enum Step {
    /// Come back after this long.
    Continue(Duration),
    /// SteamVR is shutting down and was told this add-on is exiting.
    Quit,
    /// SteamVR stopped answering.
    Lost,
}

/// A live dashboard overlay.
struct Session {
    overlay: OverlayFns,
    system: SystemFns,
    main: OverlayHandle,
    thumbnail: OverlayHandle,
    key: CString,
    checked_at: Instant,
    failed_checks: u32,
    /// The last `SetOverlayRaw` error, so a repeating one is logged once.
    submit_error: u32,
    // Last, so SteamVR is told goodbye after everything above stopped using it.
    _connection: Connection,
}

impl Session {
    fn connect() -> anyhow::Result<Self> {
        // An overlay application would start SteamVR when it is not running.
        // Starting this add-on by hand must not do that, so the first knock is
        // as a background application, which is simply turned away.
        drop(Connection::open(APPLICATION_BACKGROUND)?);
        let connection = Connection::open(APPLICATION_OVERLAY)?;
        let overlay = OverlayFns::bind(&connection.table(OVERLAY_VERSION)?)?;
        let system = SystemFns::bind(&connection.table(SYSTEM_VERSION)?)?;

        let key = CString::new(APP_KEY).context("the overlay key")?;
        let name = CString::new(OVERLAY_NAME).context("the overlay name")?;
        let (mut main, mut thumbnail) = (0, 0);
        // SAFETY: both strings are NUL-terminated and both handles outlive the call.
        let error = unsafe { (overlay.create_dashboard_overlay)(key.as_ptr(), name.as_ptr(), &mut main, &mut thumbnail) };
        if error != 0 {
            bail!("the dashboard overlay could not be created (overlay error {error})");
        }
        let scale = [ui::WIDTH as f32, ui::HEIGHT as f32];
        // SAFETY: `main` is the handle just created and `scale` outlives the call.
        let errors = unsafe {
            [
                (overlay.set_input_method)(main, INPUT_METHOD_MOUSE),
                (overlay.set_mouse_scale)(main, &scale),
                (overlay.set_width_in_meters)(main, WIDTH_METERS),
            ]
        };
        if let Some(error) = errors.into_iter().find(|error| *error != 0) {
            bail!("the dashboard overlay could not be set up (overlay error {error})");
        }
        Ok(Self { overlay, system, main, thumbnail, key, checked_at: Instant::now(), failed_checks: 0, submit_error: 0, _connection: connection })
    }

    fn runtime_version(&self) -> String {
        // SAFETY: returns a NUL-terminated string owned by the runtime, or null.
        let version = unsafe { (self.system.get_runtime_version)() };
        if version.is_null() {
            return "unknown".into();
        }
        // SAFETY: non-null and NUL-terminated, valid for the length of this call.
        unsafe { CStr::from_ptr(version) }.to_string_lossy().into_owned()
    }

    /// Hands `pixels` (RGBA, top row first) to SteamVR as the picture of `handle`.
    fn submit(&mut self, handle: OverlayHandle, pixels: &[u8], width: u32, height: u32) {
        if pixels.len() != (width * height * 4) as usize {
            return;
        }
        // SAFETY: the buffer holds `width * height` RGBA pixels (checked above).
        // SteamVR copies it before returning and does not write to it.
        let error = unsafe { (self.overlay.set_raw)(handle, pixels.as_ptr().cast_mut().cast(), width, height, 4) };
        if error != 0 && error != self.submit_error {
            crate::log::line(&format!("the overlay picture was refused (overlay error {error})"));
        }
        self.submit_error = error;
    }

    fn events(&self) -> Vec<Event> {
        let mut events = Vec::new();
        let mut raw = RawEvent([0; 64]);
        // SAFETY: `raw` has room for `EVENT_SIZE` bytes and outlives each call.
        while unsafe { (self.overlay.poll_next_overlay_event)(self.main, &mut raw, EVENT_SIZE) } {
            let (x, y) = raw.pointer();
            match raw.kind() {
                EVENT_MOUSE_MOVE => events.push(Event::Move(x, y)),
                EVENT_MOUSE_DOWN => events.push(Event::Down(x, y)),
                EVENT_MOUSE_UP => events.push(Event::Up(x, y)),
                EVENT_FOCUS_LEAVE => events.push(Event::Leave),
                _ => {}
            }
        }
        // SAFETY: as above.
        while unsafe { (self.system.poll_next_event)(&mut raw, EVENT_SIZE) } {
            if raw.kind() == EVENT_QUIT {
                events.push(Event::Quit);
            }
        }
        events
    }

    /// False once SteamVR has failed to find this overlay several times in a row.
    fn alive(&mut self, now: Instant) -> bool {
        if now.duration_since(self.checked_at) < ALIVE_EVERY {
            return true;
        }
        self.checked_at = now;
        let mut found = 0;
        // SAFETY: the key is NUL-terminated and `found` outlives the call.
        let error = unsafe { (self.overlay.find_overlay)(self.key.as_ptr(), &mut found) };
        self.failed_checks = if error == 0 { 0 } else { self.failed_checks + 1 };
        self.failed_checks < ALIVE_FAILURES
    }

    /// One pass of the loop: input, then the picture if it changed.
    fn step(&mut self, app: &mut App, panel: &mut Panel, now: Instant) -> Step {
        for event in self.events() {
            match event {
                Event::Move(x, y) => panel.pointer_move(x, y),
                Event::Down(x, y) => panel.pointer_down(x, y),
                Event::Up(x, y) => {
                    if let Some(action) = panel.pointer_up(x, y) {
                        app.apply(action);
                    }
                }
                Event::Leave => panel.pointer_leave(),
                Event::Quit => {
                    // SAFETY: plain call; tells SteamVR not to wait for this process.
                    unsafe { (self.system.acknowledge_quit_exiting)() };
                    return Step::Quit;
                }
            }
        }
        if let Some(pixels) = panel.render(&app.snapshot()) {
            self.submit(self.main, pixels, ui::WIDTH, ui::HEIGHT);
        }
        if !self.alive(now) {
            return Step::Lost;
        }
        // SAFETY: plain queries on a live session.
        let (on_screen, dashboard) = unsafe { ((self.overlay.is_overlay_visible)(self.main), (self.overlay.is_dashboard_visible)()) };
        Step::Continue(if on_screen {
            PACE_VISIBLE
        } else if dashboard {
            PACE_DASHBOARD
        } else {
            PACE_HIDDEN
        })
    }
}

/// Runs the add-on with its dashboard overlay until SteamVR quits or `stop` is set.
///
/// While SteamVR is not running the audio link keeps working and SteamVR is
/// looked for every few seconds. When SteamVR goes away the function returns:
/// SteamVR starts its startup overlays again with the next session.
///
/// # Errors
/// Never at present: every problem with SteamVR is logged and retried.
pub fn run(app: &mut App, stop: &AtomicBool) -> anyhow::Result<()> {
    let mut panel = Panel::in_language(Lang::from_setting(app.language()));
    let mut session: Option<Session> = None;
    let mut retry_at = Instant::now();
    let mut ticked_at = Instant::now();
    // The last reason SteamVR could not be reached, so it is logged once.
    let mut problem = String::new();
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now.duration_since(ticked_at) >= TICK_EVERY {
            app.tick();
            ticked_at = now;
        }
        let Some(live) = session.as_mut() else {
            if now >= retry_at {
                retry_at = now + RETRY_EVERY;
                match Session::connect() {
                    Ok(mut connected) => {
                        crate::log::line(&format!("dashboard overlay ready (SteamVR {})", connected.runtime_version()));
                        connected.submit(connected.thumbnail, &ui::thumbnail(), ui::THUMBNAIL, ui::THUMBNAIL);
                        panel.invalidate();
                        problem.clear();
                        session = Some(connected);
                        continue;
                    }
                    Err(err) => {
                        let text = format!("{err:#}");
                        if text != problem {
                            crate::log::line(&format!("no dashboard overlay yet: {text}"));
                            problem = text;
                        }
                    }
                }
            }
            std::thread::sleep(PACE_HIDDEN);
            continue;
        };
        match live.step(app, &mut panel, now) {
            Step::Continue(pace) => std::thread::sleep(pace),
            Step::Quit => {
                crate::log::line("SteamVR is shutting down");
                return Ok(());
            }
            Step::Lost => {
                crate::log::line("SteamVR stopped answering");
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Tells a running SteamVR that the manifest was added or is going away, so
/// its list matches the files `manifest` wrote. Does nothing when SteamVR is
/// not running: it reads the files on its next start.
///
/// # Errors
/// When SteamVR is running and refuses the change.
pub(crate) fn announce_manifest(manifest: &Path, installed: bool) -> anyhow::Result<()> {
    // A background application never starts SteamVR; failing here means it is not running.
    let Ok(connection) = Connection::open(APPLICATION_BACKGROUND) else { return Ok(()) };
    let table = connection.table(APPLICATIONS_VERSION)?;
    let path = CString::new(manifest.to_string_lossy().as_bytes()).map_err(|_| anyhow!("the manifest path contains a NUL byte"))?;
    let key = CString::new(APP_KEY).context("the application key")?;
    use applications_slot as slot;
    // SAFETY: each type is the declaration of that slot in IVRApplications_007,
    // and every string passed is NUL-terminated and outlives its call.
    unsafe {
        if installed {
            let add: unsafe extern "system" fn(*const c_char, bool) -> u32 = table.function(slot::ADD_APPLICATION_MANIFEST)?;
            let auto_launch: unsafe extern "system" fn(*const c_char, bool) -> u32 = table.function(slot::SET_APPLICATION_AUTO_LAUNCH)?;
            let error = add(path.as_ptr(), false);
            if error != 0 {
                bail!("SteamVR did not take the manifest (application error {error})");
            }
            let error = auto_launch(key.as_ptr(), true);
            if error != 0 {
                bail!("SteamVR did not switch autolaunch on (application error {error})");
            }
        } else {
            let remove: unsafe extern "system" fn(*const c_char) -> u32 = table.function(slot::REMOVE_APPLICATION_MANIFEST)?;
            let error = remove(path.as_ptr());
            if error != 0 {
                bail!("SteamVR did not drop the manifest (application error {error})");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mouse_event(kind: u32, x: f32, y: f32) -> RawEvent {
        let mut raw = RawEvent([0; 64]);
        raw.0[..4].copy_from_slice(&kind.to_ne_bytes());
        raw.0[EVENT_DATA_OFFSET..EVENT_DATA_OFFSET + 4].copy_from_slice(&x.to_ne_bytes());
        raw.0[EVENT_DATA_OFFSET + 4..EVENT_DATA_OFFSET + 8].copy_from_slice(&y.to_ne_bytes());
        raw
    }

    #[test]
    fn event_kind_should_be_the_first_field() {
        assert_eq!(mouse_event(EVENT_MOUSE_DOWN, 0.0, 0.0).kind(), EVENT_MOUSE_DOWN);
    }

    #[test]
    fn pointer_should_count_y_from_the_top_edge() {
        let raw = mouse_event(EVENT_MOUSE_MOVE, 120.0, 30.0);

        assert_eq!(raw.pointer(), (120.0, ui::HEIGHT as f32 - 30.0));
    }

    #[test]
    fn library_candidates_should_end_with_the_system_search() {
        let file = if cfg!(windows) { "openvr_api.dll" } else { "libopenvr_api.so" };

        assert_eq!(library_candidates().last(), Some(&PathBuf::from(file)));
    }
}

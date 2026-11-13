//! The few things Windows and Linux do differently outside of audio.

use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};

/// Holds the single-instance lock for as long as it lives.
pub struct Instance {
    _lock: File,
}

/// Takes the single-instance lock. `None` means another instance holds it.
///
/// # Errors
/// When the lock file cannot be created.
pub fn single_instance() -> std::io::Result<Option<Instance>> {
    let dir = crate::config::dir();
    std::fs::create_dir_all(&dir)?;
    let file = File::options().create(true).truncate(false).write(true).open(dir.join("instance.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Instance { _lock: file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(err)) => Err(err),
    }
}

static STOP_FLAG: std::sync::OnceLock<&'static AtomicBool> = std::sync::OnceLock::new();

fn request_stop() {
    if let Some(flag) = STOP_FLAG.get() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Sets `flag` when the process is asked to end: Ctrl+C, a closed terminal,
/// logoff, or SIGTERM. The link then says goodbye instead of timing out.
pub fn on_stop_request(flag: &'static AtomicBool) {
    if STOP_FLAG.set(flag).is_err() {
        return;
    }
    imp::install_stop_handler();
}

/// Lets output of a command typed in a terminal show up there. A no-op when
/// SteamVR started the add-on, and everywhere but Windows.
pub fn attach_console() {
    imp::attach_console();
}

/// Windows wakes sleeping threads on a 15.6 ms tick unless asked otherwise.
/// One millisecond keeps the link thread's fallback timer honest.
pub fn raise_timer_resolution() {
    imp::raise_timer_resolution();
}

#[cfg(windows)]
mod imp {
    use windows::core::BOOL;
    use windows::Win32::Media::timeBeginPeriod;
    use windows::Win32::System::Console::{AttachConsole, SetConsoleCtrlHandler, ATTACH_PARENT_PROCESS};

    unsafe extern "system" fn on_console_event(_kind: u32) -> BOOL {
        super::request_stop();
        // Handled: the main loop ends on its own within a moment.
        BOOL(1)
    }

    pub(super) fn install_stop_handler() {
        // SAFETY: the handler only stores to an atomic and lives for the process.
        let _ = unsafe { SetConsoleCtrlHandler(Some(on_console_event), true) };
    }

    pub(super) fn attach_console() {
        // SAFETY: plain Win32 call; failing (no parent console) is the normal
        // case when SteamVR starts the add-on.
        let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }

    pub(super) fn raise_timer_resolution() {
        // SAFETY: plain Win32 call. The setting ends with the process.
        unsafe { timeBeginPeriod(1) };
    }
}

#[cfg(not(windows))]
mod imp {
    extern "C" fn on_signal(_signal: libc::c_int) {
        super::request_stop();
    }

    pub(super) fn install_stop_handler() {
        // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
        unsafe {
            libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGHUP, on_signal as *const () as libc::sighandler_t);
        }
    }

    pub(super) fn attach_console() {}

    pub(super) fn raise_timer_resolution() {}
}

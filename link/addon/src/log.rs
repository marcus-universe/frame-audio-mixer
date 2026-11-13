//! A small log file next to the settings. SteamVR starts this app without a
//! console, so this is where a problem leaves its trace.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The log starts over once it is this big.
const MAX_BYTES: u64 = 512 * 1024;

static FILE: Mutex<()> = Mutex::new(());

/// Appends one line with a timestamp, and echoes it to stderr when there is one.
pub fn line(text: &str) {
    eprintln!("frame-audio-link: {text}");
    let Ok(_guard) = FILE.lock() else { return };
    let dir = crate::config::dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("frame-audio-link.log");
    let too_big = std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_BYTES);
    let file = OpenOptions::new().create(true).append(!too_big).write(true).truncate(too_big).open(&path);
    if let Ok(mut file) = file {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
        let _ = writeln!(file, "{seconds} {text}");
    }
}

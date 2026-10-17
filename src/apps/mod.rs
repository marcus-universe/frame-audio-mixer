//! The "Frame apps by sasaken@" list.
//!
//! This is a Rust port of frame-apps 0.3.1 from Frame Mic Tuner. The bundled
//! catalog and icons are the same. When update checks are on, the list is
//! refreshed from frame.sasaken1102s.net into the shared `~/.cache/frame-apps/`
//! folder, at most once an hour.

mod catalog;
mod manager;

pub use catalog::{bundled, embedded_png, parse_catalog, self_test as catalog_test, AppInfo};
pub use manager::{AppState, Entry, LaunchError, Manager};

pub fn self_test() -> Result<(), String> {
    catalog_test()?;
    manager::self_test()?;
    Ok(())
}

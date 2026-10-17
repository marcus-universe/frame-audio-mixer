//! In-app updater. GitHub is contacted with ureq and rustls. The install runs
//! from a copied binary so it survives `install.sh` restarting the panel.
//! Files live in `~/.cache/frame-audio-mixer/`, the same places Frame Mic Tuner uses.

mod check;
mod install;

pub use check::{check, current_view, url_allowed, version_core, vercmp, Asset, UpdateState, UpdateView, ASSET, REPO, VERSION};
pub use install::{detach, path_allowed, run};

pub fn self_test() -> Result<(), String> {
    check::self_test()?;
    install::self_test()?;
    Ok(())
}

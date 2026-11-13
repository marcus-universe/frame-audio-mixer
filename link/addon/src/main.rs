//! Frame Audio Link: the PC side of Frame Audio Mixer's link.
//!
//! SteamVR starts this add-on with the headset session. It gives the PC four
//! more streaming microphones (B1–B4 of the headset mixer) and four outputs
//! (PC1–PC4) that the headset mixer can take in as channels. Its dashboard
//! overlay switches each stream on or off.

// SteamVR starts the add-on in the background; a console window would pop up
// with every VR session. Commands typed in a terminal attach to it instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod audio;
mod config;
#[cfg(windows)]
mod devices;
mod log;
mod manifest;
mod model;
mod net;
mod platform;
mod ui;
mod vr;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;

use crate::app::App;
use crate::config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
frame-audio-link [COMMAND]

  (none)        Run with the SteamVR dashboard overlay. SteamVR starts it this way.
  headless      Run without SteamVR: only the audio link.
  install       Register with SteamVR, and on Windows create the sound devices.
  uninstall     Undo `install`.
  register      Only register with SteamVR. `unregister` undoes it.
  devices       Windows: `devices status`, `devices install`, `devices remove`.
  status        Print what the link and the sound devices are doing, for ten seconds.
  preview DIR   Write pictures of the overlay in its main states into DIR.
  version       Print the version.";

/// Set when the process is asked to end. The loops below watch it.
static STOP: AtomicBool = AtomicBool::new(false);

fn main() -> ExitCode {
    platform::attach_console();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map_or("", String::as_str);
    let result = match command {
        "" | "run" => run(true),
        "headless" => run(false),
        "install" => install(),
        "uninstall" => uninstall(),
        "register" => manifest::install().context("registering with SteamVR"),
        "unregister" => manifest::uninstall().context("removing the SteamVR registration"),
        "devices" => devices(args.get(1).map_or("status", String::as_str)),
        "status" => status(),
        "preview" => ui::write_previews(std::path::Path::new(args.get(1).map_or(".", String::as_str))),
        "version" | "--version" | "-V" => {
            println!("frame-audio-link {VERSION}");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => Err(anyhow::anyhow!("unknown command `{other}`\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::line(&format!("{err:#}"));
            ExitCode::FAILURE
        }
    }
}

/// The add-on itself. One instance per user: a second one would open the same
/// devices and fight the first for the headset.
fn run(with_overlay: bool) -> anyhow::Result<()> {
    let Some(_instance) = platform::single_instance().context("checking for a running instance")? else {
        log::line("already running");
        return Ok(());
    };
    platform::on_stop_request(&STOP);
    platform::raise_timer_resolution();
    let path = config::path();
    let mut app = App::start(Config::load(&path), path)?;
    log::line(&format!("started {VERSION}"));
    if with_overlay {
        vr::run(&mut app, &STOP)
    } else {
        while !STOP.load(Ordering::Relaxed) {
            app.tick();
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }
}

/// Registers first: the add-on is useful with other virtual cables too, so a
/// PC on which the sound devices cannot be created still gets its overlay.
fn install() -> anyhow::Result<()> {
    manifest::install().context("registering with SteamVR")?;
    println!("Registered with SteamVR. It starts Frame Audio Link with the next VR session.");
    #[cfg(windows)]
    devices::install().context(
        "creating the sound devices. The add-on is registered and runs without them; \
         run `frame-audio-link devices install` to try again, or name other virtual cables in the settings",
    )?;
    println!("Frame Audio Link is installed.");
    Ok(())
}

fn uninstall() -> anyhow::Result<()> {
    manifest::uninstall().context("removing the SteamVR registration")?;
    #[cfg(windows)]
    devices::remove().context("removing the sound devices")?;
    println!("Frame Audio Link is uninstalled. Its settings stay in {}.", config::dir().display());
    Ok(())
}

#[cfg(windows)]
fn devices(action: &str) -> anyhow::Result<()> {
    match action {
        "status" => devices::print_status(),
        "install" => devices::install(),
        "remove" => devices::remove(),
        other => Err(anyhow::anyhow!("unknown devices action `{other}`: use status, install or remove")),
    }
}

#[cfg(not(windows))]
fn devices(_action: &str) -> anyhow::Result<()> {
    println!("On Linux the sound devices exist while the add-on runs; there is nothing to install.");
    Ok(())
}

/// Runs the link without the overlay for a moment and prints what it sees.
fn status() -> anyhow::Result<()> {
    let Some(_instance) = platform::single_instance().context("checking for a running instance")? else {
        println!("Frame Audio Link is running. Its overlay in the SteamVR dashboard shows the status.");
        return Ok(());
    };
    let path = config::path();
    let mut app = App::start(Config::load(&path), path)?;
    for _ in 0..10 {
        std::thread::sleep(Duration::from_secs(1));
        app.tick();
        let snap = app.snapshot();
        println!(
            "link {:?} peer `{}` {} rtt {:.1} ms | headset wants PC outputs {:04b}, offers microphones {:04b} | devices: mics {:?} outputs {:?}",
            snap.link.state,
            snap.link.peer,
            snap.link.peer_addr,
            snap.link.rtt_ms,
            snap.link.remote_recv,
            snap.link.remote_send,
            snap.audio.mics,
            snap.audio.outputs
        );
    }
    Ok(())
}

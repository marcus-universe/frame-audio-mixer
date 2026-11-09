//! Frame VST host: loads one VST3 plugin in its own process.
//!
//! Built twice: for Windows x64, where it runs under Proton ARM64 with FEX
//! emulating the plugin, and for aarch64 Linux, for native plugins. A plugin
//! that crashes takes this process with it, never the mix engine.
//!
//!   frame-vst-host probe <bundle>
//!   frame-vst-host serve <region> <bundle> <class id|-> <state file> <title>
//!   frame-vst-host self-test

mod com;
mod gui;
mod module;
#[allow(dead_code)] // Also compiled into the engine, which uses the rest.
mod shared;
mod serve;

use std::process::ExitCode;

const USAGE: &str = "usage: frame-vst-host probe <bundle> | serve <region> <bundle> <cid|-> <state> <title> | self-test";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |index: usize| args.get(index).map(String::as_str);
    let result = match arg(0) {
        Some("probe") => arg(1).ok_or_else(|| USAGE.to_string()).and_then(module::probe),
        Some("serve") => match (arg(1), arg(2), arg(3), arg(4), arg(5)) {
            (Some(region), Some(plugin), Some(cid), Some(state), Some(title)) => serve::serve(&serve::Args { region, plugin, cid, state, title }),
            _ => Err(USAGE.to_string()),
        },
        Some("self-test") => module::self_test().map(|()| println!("self-test ok")),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("frame-vst-host: {err}");
            ExitCode::FAILURE
        }
    }
}


//! Running one plugin for the engine. The audio thread polls the shared region;
//! the main thread owns the plugin, its editor window and the commands the
//! engine writes to stdin, one per line: `gui`, `save`, `quit`.

use std::io::BufRead;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::module::{parse_cid, AudioPort, Module, Plugin};
use crate::shared::{Plane, Region, MAX_FRAMES, STATUS_FAILED, STATUS_RUNNING};

const AUTOSAVE: Duration = Duration::from_secs(5);

pub struct Args<'a> {
    pub region: &'a str,
    pub plugin: &'a str,
    pub cid: &'a str,
    pub state: &'a str,
    pub title: &'a str,
}

pub fn serve(args: &Args) -> Result<(), String> {
    let region = Arc::new(Region::open(Path::new(args.region)).map_err(|err| format!("region {}: {err}", args.region))?);
    let started = start(args, &region);
    let (module, plugin) = match started {
        Ok(pair) => pair,
        Err(err) => {
            region.header().status.store(STATUS_FAILED, Ordering::Release);
            return Err(err);
        }
    };
    region.header().status.store(STATUS_RUNNING, Ordering::Release);
    eprintln!("frame-vst-host: running {}", args.title);

    let stop = Arc::new(AtomicBool::new(false));
    let audio = {
        let (region, stop, port) = (region.clone(), stop.clone(), AudioPort::of(&plugin));
        std::thread::Builder::new().name("vst-audio".into()).spawn(move || audio_loop(&region, &port, &stop)).map_err(|err| err.to_string())?
    };
    let commands = read_commands();
    let mut editor: Option<crate::gui::Editor> = None;
    let mut saved = plugin.save();
    let mut saved_at = Instant::now();
    loop {
        crate::gui::pump();
        let mut quit = false;
        while let Ok(command) = commands.try_recv() {
            match command.as_str() {
                "gui" => match &editor {
                    Some(open) => open.raise(),
                    None => match crate::gui::Editor::open(&plugin, args.title) {
                        Ok(open) => editor = Some(open),
                        Err(err) => eprintln!("frame-vst-host: editor: {err}"),
                    },
                },
                "save" => saved_at = Instant::now() - AUTOSAVE,
                "quit" => quit = true,
                other => eprintln!("frame-vst-host: unknown command {other:?}"),
            }
        }
        if editor.as_ref().is_some_and(crate::gui::Editor::closed) {
            editor = None;
            saved_at = Instant::now() - AUTOSAVE;
        }
        if quit {
            break;
        }
        if saved_at.elapsed() >= AUTOSAVE {
            saved_at = Instant::now();
            let state = plugin.save();
            if state != saved {
                write_state(args.state, &state);
                saved = state;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // The editor must close before the plugin it shows is released.
    #[allow(clippy::drop_non_drop)]
    drop(editor);
    write_state(args.state, &plugin.save());
    stop.store(true, Ordering::Release);
    let _ = audio.join();
    drop(plugin);
    drop(module);
    Ok(())
}

fn start(args: &Args, region: &Region) -> Result<(Module, Plugin), String> {
    let _ = region;
    let module = Module::load(args.plugin)?;
    let cid = match parse_cid(args.cid) {
        Some(cid) => cid,
        None => module.audio_classes().first().map(|(cid, _)| *cid).ok_or("the plugin has no audio effect")?,
    };
    let saved = std::fs::read(args.state).ok();
    let plugin = Plugin::start(&module, &cid, saved.as_deref())?;
    Ok((module, plugin))
}

fn write_state(path: &str, state: &[u8]) {
    let temp = format!("{path}.tmp");
    if std::fs::write(&temp, state).is_ok() {
        let _ = std::fs::rename(&temp, path);
    }
}

/// Lines from the engine. A closed stdin means the engine is gone.
fn read_commands() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line.trim().to_string()).is_err() {
                return;
            }
        }
        let _ = tx.send("quit".into());
    });
    rx
}

fn audio_loop(region: &Region, port: &AudioPort, stop: &AtomicBool) {
    crate::gui::raise_thread_priority();
    let header = region.header();
    let mut buffers = [vec![0.0f32; MAX_FRAMES], vec![0.0f32; MAX_FRAMES], vec![0.0f32; MAX_FRAMES], vec![0.0f32; MAX_FRAMES]];
    let mut last = header.input_seq.load(Ordering::Acquire);
    while !stop.load(Ordering::Acquire) {
        let seq = header.input_seq.load(Ordering::Acquire);
        if seq == last {
            std::thread::sleep(Duration::from_micros(250));
            continue;
        }
        last = seq;
        let frames = (header.frames.load(Ordering::Acquire) as usize).min(MAX_FRAMES);
        let [in_l, in_r, out_l, out_r] = &mut buffers;
        region.read(Plane::InLeft, &mut in_l[..frames]);
        region.read(Plane::InRight, &mut in_r[..frames]);
        out_l[..frames].fill(0.0);
        out_r[..frames].fill(0.0);
        port.process([&mut in_l[..frames], &mut in_r[..frames]], [&mut out_l[..frames], &mut out_r[..frames]]);
        region.write(Plane::OutLeft, &out_l[..frames]);
        region.write(Plane::OutRight, &out_r[..frames]);
        header.output_seq.store(seq, Ordering::Release);
    }
}

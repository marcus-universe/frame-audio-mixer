//! Frame Audio Mixer. The mix sits after Frame Mic Tuner: the default microphone
//! (already filtered) is mixed with named app channels, and only VRLink is moved
//! onto that mix.

mod apps;
mod brand;
mod config;
mod draw;
mod graph;
mod i18n;
mod icons;
mod meter;
mod overlay;
mod panel;
mod updater;
mod vulkan;
mod worker;

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use panel::Action;
use worker::Job;

static SHOW: AtomicBool = AtomicBool::new(false);

extern "C" fn on_usr1(_: i32) {
    SHOW.store(true, Ordering::Relaxed);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("frame-audio-mixer {}", updater::VERSION);
        return;
    }
    if args.iter().any(|arg| arg == "--update-install") {
        if let Err(err) = updater::run() {
            eprintln!("frame-audio-mixer: {err}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--help") {
        println!("frame-audio-mixer [--version] [--self-test] [--dump-png PATH] [--language CODE] [--update-install]");
        return;
    }
    if args.iter().any(|arg| arg == "--self-test") {
        match self_test() {
            Ok(()) => println!("self-test ok"),
            Err(err) => {
                eprintln!("self-test failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some(pos) = args.iter().position(|arg| arg == "--dump-png") {
        let path = args.get(pos + 1).map(String::as_str).unwrap_or("/tmp/frame-audio-mixer.png");
        let lang = language_arg(&args);
        let fonts = match draw::Fonts::load() {
            Ok(fonts) => fonts,
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        };
        if let Err(err) = panel::render_png(&fonts, lang, std::path::Path::new(path)) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        println!("{path}");
        return;
    }

    let _lock = match instance_lock() {
        Ok(file) => file,
        Err(3) => std::process::exit(3),
        Err(code) => std::process::exit(code),
    };
    install_signals();
    let mut config = config::load(&config::config_path());
    if let Some(lang) = explicit_language(&args) {
        config.language = lang;
    }
    let worker = worker::Worker::start(config.clone());
    worker.send(Job::Apply(config.clone()));
    let fonts = match draw::Fonts::load() {
        Ok(fonts) => fonts,
        Err(err) => {
            eprintln!("frame-audio-mixer: {err}");
            std::process::exit(1);
        }
    };
    let mut ui = panel::Panel::new(config);
    let mut levels = meter::Meter::new();
    let mut session: Option<overlay::Session> = None;
    // Accent of the dashboard icon SteamVR has now. A color change sends the icon again.
    let mut thumb_accent: Option<String> = None;
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    loop {
        if session.is_none() {
            match overlay::Session::connect() {
                Ok(connected) => {
                    eprintln!("frame-audio-mixer: overlay ready");
                    session = Some(connected);
                }
                Err(err) => {
                    eprintln!("frame-audio-mixer: {err}");
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            }
        }
        let Some(session) = session.as_mut() else { continue };
        if SHOW.swap(false, Ordering::Relaxed) {
            session.show();
        }
        let (panel_w, panel_h) = ui.pixel_size();
        if let Err(err) = session.fit(panel_w, panel_h) {
            eprintln!("frame-audio-mixer: {err}");
        }
        let events = session.poll(panel_h as f64);
        let mut dirty = false;
        if ui.keyboard_live() {
            let text = session.keyboard_text();
            if ui.apply_keyboard_text(&text, "", false) {
                dirty = true;
            }
        }
        for event in events {
            if event.kind == overlay::QUIT || event.kind == overlay::OVERLAY_CLOSED {
                std::process::exit(3);
            }
            if event.kind == overlay::KEYBOARD_CHAR || event.kind == overlay::KEYBOARD_DONE || event.kind == overlay::KEYBOARD_CLOSED {
                let text = session.keyboard_text();
                let finished = event.kind != overlay::KEYBOARD_CHAR;
                if ui.apply_keyboard_text(&text, &event.text, finished) {
                    dirty = true;
                }
                dirty = true;
                continue;
            }
            let action = ui.pointer(event.kind, event.x, event.y, event.button);
            if matches!(action, Action::Quit) {
                std::process::exit(3);
            }
            if let Action::ShowKeyboard { description, existing } = &action {
                session.show_keyboard(description, existing);
            }
            apply(&worker, &mut ui, action);
            if matches!(event.kind, overlay::MOUSE_DOWN | overlay::MOUSE_UP | overlay::SCROLL_DISCRETE | overlay::SCROLL_SMOOTH) || ui.interacting() {
                dirty = true;
            }
        }
        ui.tick();
        let visible = session.dashboard_visible();
        let snap = worker.snapshot();
        if visible && ui.wants_meters() {
            levels.sync(&ui.meter_targets(&snap));
        } else {
            levels.sync(&[]);
        }
        let meters_live = visible && ui.wants_meters() && !ui.meter_targets(&snap).is_empty();
        let interval = if meters_live { Duration::from_millis(33) } else { Duration::from_millis(250) };
        if visible && (dirty || last_draw.elapsed() >= interval) {
            let (panel_w, panel_h) = ui.pixel_size();
            if let Err(err) = session.fit(panel_w, panel_h) {
                eprintln!("frame-audio-mixer: {err}");
            }
            let pixels = {
                let peaks = levels.peaks();
                ui.render(&fonts, &snap, peaks)
            };
            let accent = ui.config.accent.clone();
            let thumb = if thumb_accent.as_deref() == Some(accent.as_str()) { None } else { Some(ui.thumbnail(&fonts)) };
            if let Err(err) = session.submit(&pixels, thumb.as_deref()) {
                eprintln!("frame-audio-mixer: {err}");
            } else {
                thumb_accent = Some(accent);
            }
            last_draw = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn apply(worker: &worker::Worker, ui: &mut panel::Panel, action: Action) {
    match action {
        Action::None => {}
        Action::Save => {
            let _ = config::save(&config::config_path(), &ui.config);
            worker.send(Job::Apply(ui.config.clone()));
        }
        Action::Quit => std::process::exit(3),
        Action::ShowKeyboard { description, existing } => {
            // The session is needed; the caller handles the keyboard when it sees this
            // only from the event loop, which has the session. Re-sent below.
            let _ = (description, existing);
        }
        Action::Record => worker.send(Job::Record),
        Action::StopCheck => worker.send(Job::StopCheck),
        Action::PlayClip(id) => worker.send(Job::PlayClip(id)),
        Action::CheckUpdates => {
            let _ = config::save(&config::config_path(), &ui.config);
            worker.send(Job::CheckUpdates);
        }
        Action::InstallUpdate => worker.send(Job::InstallUpdate),
        Action::OpenInstaller(key) => worker.send(Job::OpenInstaller(key)),
        Action::Autostart(on) => worker.send(Job::Autostart(on)),
    }
}

fn self_test() -> Result<(), String> {
    config::self_test()?;
    graph::self_test()?;
    i18n::self_test()?;
    draw::self_test_paths()?;
    panel::self_test()?;
    icons::self_test()?;
    meter::self_test()?;
    worker::self_test()?;
    updater::self_test()?;
    apps::self_test()?;
    let mut seen = std::collections::HashMap::new();
    let stream = graph::AudioNode {
        id: 9,
        serial: 90,
        name: "zen".into(),
        description: String::new(),
        media_class: "Stream/Output/Audio".into(),
        media_name: String::new(),
        app_name: "Zen".into(),
        binary: String::new(),
        is_virtual: false,
        target: String::new(),
    };
    if !worker::pin_once(&mut seen, &stream) || worker::pin_once(&mut seen, &stream) {
        return Err("pin once".into());
    }
    let mut graph = graph::Graph::default();
    if panel::resolved_source("", &graph).is_some() {
        return Err("empty default".into());
    }
    graph.nodes.push(graph::AudioNode {
        id: 4,
        serial: 77,
        name: graph::Graph::BUILTIN_SOURCE.into(),
        description: "Built-in".into(),
        media_class: "Audio/Source".into(),
        media_name: String::new(),
        app_name: String::new(),
        binary: String::new(),
        is_virtual: false,
        target: String::new(),
    });
    if panel::resolved_source("", &graph).as_deref() != Some(graph::Graph::BUILTIN_SOURCE) {
        return Err("system default fallback".into());
    }
    if graph.link_target(graph::Graph::BUILTIN_SOURCE) != "77" {
        return Err("mic serial".into());
    }
    Ok(())
}

fn language_arg(args: &[String]) -> i18n::Lang {
    explicit_language(args).and_then(|code| i18n::parse_code(&code)).unwrap_or(i18n::Lang::En)
}

fn explicit_language(args: &[String]) -> Option<String> {
    args.iter().position(|arg| arg == "--language").and_then(|pos| args.get(pos + 1)).cloned()
}

fn lock_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    PathBuf::from(data).join("frame-audio-mixer").join("panel.lock")
}

fn instance_lock() -> Result<std::fs::File, i32> {
    let path = lock_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let mut file = OpenOptions::new().read(true).write(true).create(true).open(&path).map_err(|_| 1)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let mut pid_text = String::new();
        let _ = file.read_to_string(&mut pid_text);
        let pid = pid_text.trim().parse::<i32>().unwrap_or(0);
        if started_by_service() {
            eprintln!("frame-audio-mixer: already running (pid {pid})");
            return Err(3);
        }
        if pid > 0 {
            unsafe { libc::kill(pid, libc::SIGUSR1) };
        }
        return Err(0);
    }
    let _ = file.set_len(0);
    let _ = file.seek(SeekFrom::Start(0));
    let _ = write!(file, "{}", std::process::id());
    Ok(file)
}

fn started_by_service() -> bool {
    fs::read_to_string("/proc/self/cgroup").map(|text| text.contains("frame-audio-mixer.service")).unwrap_or(false)
}

fn install_signals() {
    unsafe {
        libc::signal(libc::SIGUSR1, on_usr1 as libc::sighandler_t);
    }
}

//! Installed and running state, the hourly fetch, and Konsole.

use super::catalog::{self, embedded_png, parse_catalog, AppInfo, SITE};
use crate::updater::url_allowed;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Unknown,
    Missing,
    Installed,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub key: String,
    pub mono: String,
    pub desc_ja: String,
    pub desc_en: String,
    pub state: AppState,
    pub related: bool,
    pub busy: bool,
    pub icon_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    Busy,
    NoDisplay,
    NoKonsole,
    Failed,
}

pub struct Manager {
    catalog: Vec<AppInfo>,
    entries: Vec<Entry>,
    konsoles: HashMap<String, std::process::Child>,
    last_refresh: u64,
}

impl Manager {
    pub fn new() -> Self {
        let catalog = load_catalog();
        let mut manager = Self { catalog, entries: Vec::new(), konsoles: HashMap::new(), last_refresh: 0 };
        manager.rebuild();
        manager
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn menu_busy(&self) -> bool {
        self.konsoles.contains_key("")
    }

    /// Look at units again, reap Konsole, and fetch the list when it is due.
    pub fn tick(&mut self, fetch_enabled: bool) {
        self.reap();
        let now = unix_now();
        if now.saturating_sub(self.last_refresh) >= 5 {
            self.rebuild();
            self.last_refresh = now;
        }
        if fetch_enabled {
            let _ = fetch_if_due();
            let loaded = load_catalog();
            if loaded.iter().map(|app| app.name.as_str()).collect::<Vec<_>>() != self.catalog.iter().map(|app| app.name.as_str()).collect::<Vec<_>>() {
                self.catalog = loaded;
                self.rebuild();
            }
        }
    }

    pub fn launch(&mut self, key: &str) -> Result<(), LaunchError> {
        if self.konsoles.contains_key(key) {
            return Err(LaunchError::Busy);
        }
        if !key.is_empty() && !self.catalog.iter().any(|app| app.key == key) {
            return Err(LaunchError::Failed);
        }
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Err(LaunchError::NoDisplay);
        }
        if which("konsole").is_none() {
            return Err(LaunchError::NoKonsole);
        }
        let script = konsole_script(key);
        let child = Command::new("konsole")
            .args(["--separate", "-e", "sh", "-c", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| LaunchError::Failed)?;
        self.konsoles.insert(key.to_string(), child);
        self.mark_busy();
        Ok(())
    }

    fn reap(&mut self) {
        let finished: Vec<_> = self
            .konsoles
            .iter_mut()
            .filter_map(|(key, child)| child.try_wait().ok().flatten().map(|_| key.clone()))
            .collect();
        for key in finished {
            self.konsoles.remove(&key);
        }
        self.mark_busy();
    }

    fn mark_busy(&mut self) {
        for entry in &mut self.entries {
            entry.busy = self.konsoles.contains_key(&entry.key);
        }
    }

    fn rebuild(&mut self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
        let states = unit_states(&self.catalog);
        let mut entries = Vec::new();
        // Frame Mic Tuner is the companion: it owns echo cancel and noise suppression.
        push_related(&mut entries, &self.catalog, &states, &home, &config);
        for info in &self.catalog {
            if entries.iter().any(|entry: &Entry| entry.name == info.name) {
                continue;
            }
            entries.push(make_entry(info, false, &states, &home, &config));
        }
        for entry in &mut entries {
            entry.busy = self.konsoles.contains_key(&entry.key);
        }
        self.entries = entries;
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        for (_, mut child) in self.konsoles.drain() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn push_related(entries: &mut Vec<Entry>, catalog: &[AppInfo], states: &HashMap<String, bool>, home: &str, config: &str) {
    if let Some(info) = catalog.iter().find(|info| info.name == "frame-mic-tuner") {
        entries.push(make_entry(info, true, states, home, config));
    }
}

fn make_entry(info: &AppInfo, related: bool, states: &HashMap<String, bool>, home: &str, config: &str) -> Entry {
    let installed = states.contains_key(&info.unit)
        || Path::new(home).join(&info.main_file).is_file()
        || Path::new(config).join("systemd/user").join(&info.unit).is_file();
    let running = states.get(&info.unit).copied().unwrap_or(false);
    let state = if running {
        AppState::Running
    } else if installed {
        AppState::Installed
    } else {
        AppState::Missing
    };
    Entry {
        name: info.name.clone(),
        key: info.key.clone(),
        mono: info.mono.clone(),
        desc_ja: info.desc_ja.clone(),
        desc_en: info.desc_en.clone(),
        state,
        related,
        busy: false,
        icon_path: cached_icon(info).unwrap_or_default(),
    }
}

fn unit_states(catalog: &[AppInfo]) -> HashMap<String, bool> {
    let units: Vec<_> = catalog.iter().map(|info| info.unit.clone()).collect();
    if units.is_empty() {
        return HashMap::new();
    }
    let output = Command::new("systemctl").arg("--user").arg("is-active").args(&units).output();
    let Ok(output) = output else { return HashMap::new() };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut states = HashMap::new();
    for (unit, line) in units.iter().zip(text.lines()) {
        states.insert(unit.clone(), line.trim() == "active");
    }
    states
}

pub fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{home}/.cache"));
    PathBuf::from(base).join("frame-apps")
}

fn load_catalog() -> Vec<AppInfo> {
    let path = cache_dir().join("apps.json");
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(apps) = parse_catalog(&text) {
            return apps;
        }
    }
    catalog::bundled()
}

fn fetch_if_due() -> Result<(), String> {
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = dir.join("last-fetch");
    if let Ok(meta) = fs::metadata(&stamp) {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = modified.elapsed() {
                if age.as_secs() < 3600 {
                    return Ok(());
                }
            }
        }
    }
    let url = format!("{SITE}/apps/apps.json");
    if !url_allowed(&url) && !site_allowed(&url) {
        return Err("list address is not allowed".into());
    }
    let body = http_bytes(&url, 64 * 1024)?;
    let text = String::from_utf8(body.clone()).map_err(|e| e.to_string())?;
    let apps = parse_catalog(&text)?;
    fs::write(dir.join("apps.json"), &text).map_err(|e| e.to_string())?;
    let icons = dir.join("icons");
    fs::create_dir_all(&icons).map_err(|e| e.to_string())?;
    for app in &apps {
        if cached_icon(app).is_some() {
            continue;
        }
        let icon_url = format!("{SITE}/apps/{}", app.icon);
        if let Ok(bytes) = http_bytes(&icon_url, 64 * 1024) {
            if bytes.len() <= 64 * 1024 && catalog::sha256_hex(&bytes) == app.icon_sha256 {
                let _ = fs::write(icons.join(&app.icon), &bytes);
            }
        }
    }
    let _ = fs::write(&stamp, format!("{}\n", unix_now()));
    Ok(())
}

fn site_allowed(url: &str) -> bool {
    url.starts_with("https://frame.sasaken1102s.net/")
}

fn cached_icon(app: &AppInfo) -> Option<String> {
    let path = cache_dir().join("icons").join(&app.icon);
    let bytes = fs::read(&path).ok()?;
    if catalog::sha256_hex(&bytes) == app.icon_sha256 {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn http_bytes(url: &str, max: u64) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("bad-url".into());
    }
    let response = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(30))
        .redirects(0)
        .build()
        .get(url)
        .set("User-Agent", "frame-audio-mixer")
        .call()
        .map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    response.into_reader().take(max).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

pub fn konsole_script(key: &str) -> String {
    let command = if key.is_empty() {
        format!("curl -fsSL {SITE} | sh")
    } else {
        format!("curl -fsSL {SITE} | sh -s -- install {key}")
    };
    format!("{command}\necho\necho 'Done. Press Enter to close.'\nread\n")
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|path| path.is_file())
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn self_test() -> Result<(), String> {
    let script = konsole_script("mic");
    if !script.contains("install mic") || !script.contains(SITE) {
        return Err(script);
    }
    let menu = konsole_script("");
    if menu.contains("install") {
        return Err("menu command".into());
    }
    // The embedded icon is what the panel draws when nothing was fetched.
    if embedded_png("frame-mic-tuner").is_none() {
        return Err("embedded".into());
    }
    Ok(())
}

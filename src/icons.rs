//! Desktop icons for the Add dialog.
//!
//! The index is built from `.desktop` files. Icons are local PNGs (hicolor,
//! pixmaps, or a Flatpak appstream cache). Results, including misses, are cached
//! so the panel does not rescan while it is open.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    pub icon: String,
    pub exec: String,
    pub wm_class: String,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct IconIndex {
    entries: Vec<DesktopEntry>,
    roots: Vec<PathBuf>,
    cache: std::collections::HashMap<String, Option<PathBuf>>,
}

impl IconIndex {
    pub fn lookup(&mut self, app_name: &str, binary: &str) -> Option<PathBuf> {
        let key = format!("{}|{}", app_name.to_lowercase(), binary.to_lowercase());
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }
        let found = self
            .entries
            .iter()
            .find(|entry| entry_matches(entry, app_name, binary))
            .and_then(|entry| resolve_icon(&entry.icon, &self.roots));
        self.cache.insert(key, found.clone());
        found
    }
}

/// Scan in the background. The panel takes the index when it arrives.
pub fn spawn() -> mpsc::Receiver<IconIndex> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(build(&data_roots()));
    });
    rx
}

pub fn build(roots: &[PathBuf]) -> IconIndex {
    let mut entries = Vec::new();
    for root in roots {
        let apps = root.join("applications");
        let Ok(dir) = fs::read_dir(&apps) else { continue };
        for item in dir.flatten() {
            let path = item.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else { continue };
            let mut entry = parse_desktop(&text);
            if entry.name.is_empty() {
                continue;
            }
            if entry.id.is_empty() {
                entry.id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            }
            entries.push(entry);
        }
    }
    IconIndex { entries, roots: roots.to_vec(), cache: std::collections::HashMap::new() }
}

pub fn data_roots() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut roots = Vec::new();
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        if !data.is_empty() {
            roots.push(PathBuf::from(data));
        }
    } else if !home.is_empty() {
        roots.push(PathBuf::from(&home).join(".local/share"));
    }
    if !home.is_empty() {
        roots.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share"));
    }
    roots.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    if let Ok(dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in dirs.split(':') {
            if !dir.is_empty() {
                roots.push(PathBuf::from(dir));
            }
        }
    } else {
        roots.push(PathBuf::from("/usr/local/share"));
        roots.push(PathBuf::from("/usr/share"));
    }
    roots
}

/// Read the `[Desktop Entry]` group. Localized `Name[xx]` keys are ignored.
pub fn parse_desktop(text: &str) -> DesktopEntry {
    let mut entry = DesktopEntry {
        name: String::new(),
        icon: String::new(),
        exec: String::new(),
        wm_class: String::new(),
        id: String::new(),
    };
    let mut in_group = false;
    let mut seen_group = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_group = line.eq_ignore_ascii_case("[Desktop Entry]");
            if in_group {
                seen_group = true;
            } else if seen_group {
                break;
            }
            continue;
        }
        if !in_group {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key {
            "Name" => entry.name = value.to_string(),
            "Icon" => entry.icon = value.to_string(),
            "Exec" => entry.exec = value.to_string(),
            "StartupWMClass" => entry.wm_class = value.to_string(),
            "X-Flatpak" => entry.id = value.to_string(),
            _ => {}
        }
    }
    entry
}

fn entry_matches(entry: &DesktopEntry, app_name: &str, binary: &str) -> bool {
    let app = app_name.trim();
    let bin = binary.trim();
    if !app.is_empty() && eq_ignore(entry.name.as_str(), app) {
        return true;
    }
    if !app.is_empty() && (eq_ignore(&entry.wm_class, app) || eq_ignore(&entry.id, app)) {
        return true;
    }
    let exec_base = exec_basename(&entry.exec);
    if !bin.is_empty() && eq_ignore(&exec_base, bin) {
        return true;
    }
    if !app.is_empty() && eq_ignore(&exec_base, app) {
        return true;
    }
    false
}

fn eq_ignore(left: &str, right: &str) -> bool {
    !left.is_empty() && left.eq_ignore_ascii_case(right)
}

fn exec_basename(exec: &str) -> String {
    let token = exec.split_whitespace().next().unwrap_or("");
    Path::new(token)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(token)
        .to_string()
}

const SIZES: [&str; 5] = ["128x128", "64x64", "48x48", "256x256", "512x512"];

pub fn resolve_icon(icon: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if icon.is_empty() {
        return None;
    }
    let direct = Path::new(icon);
    if direct.is_absolute() && is_png(direct) {
        return Some(direct.to_path_buf());
    }
    let file = if icon.to_ascii_lowercase().ends_with(".png") { icon.to_string() } else { format!("{icon}.png") };
    for root in roots {
        for size in SIZES {
            let path = root.join("icons/hicolor").join(size).join("apps").join(&file);
            if is_png(&path) {
                return Some(path);
            }
        }
        let pixmap = root.join("pixmaps").join(&file);
        if is_png(&pixmap) {
            return Some(pixmap);
        }
    }
    // Flatpak keeps a second copy under the appstream icon cache.
    let home = std::env::var("HOME").unwrap_or_default();
    let mut caches = vec![PathBuf::from("/var/lib/flatpak/appstream")];
    if !home.is_empty() {
        caches.push(PathBuf::from(home).join(".local/share/flatpak/appstream"));
    }
    for cache in caches {
        let Ok(remotes) = fs::read_dir(&cache) else { continue };
        for remote in remotes.flatten() {
            for size in SIZES {
                let path = remote.path().join("icons").join(size).join(&file);
                if is_png(&path) {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn is_png(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("png")).unwrap_or(false)
}

pub fn self_test() -> Result<(), String> {
    let text = "[Desktop Entry]\nName=Zen\nName[ja]=ゼン\nIcon=zen\nExec=/usr/bin/zen %u\nStartupWMClass=zen\n";
    let entry = parse_desktop(text);
    if entry.name != "Zen" || entry.icon != "zen" || exec_basename(&entry.exec) != "zen" {
        return Err(format!("desktop {entry:?}"));
    }
    let dir = std::env::temp_dir().join(format!("frame-amix-icons-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let apps = dir.join("applications");
    let png_dir = dir.join("icons/hicolor/128x128/apps");
    fs::create_dir_all(&apps).map_err(|e| e.to_string())?;
    fs::create_dir_all(&png_dir).map_err(|e| e.to_string())?;
    fs::write(apps.join("zen.desktop"), text).map_err(|e| e.to_string())?;
    fs::write(png_dir.join("zen.png"), b"not-a-real-png").map_err(|e| e.to_string())?;
    let mut index = build(&[dir.clone()]);
    let found = index.lookup("playback", "zen").ok_or("missed zen by binary")?;
    if found != png_dir.join("zen.png") {
        return Err(format!("resolved {found:?}"));
    }
    if index.lookup("nope", "missing").is_some() {
        return Err("unexpected icon".into());
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

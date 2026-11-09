//! VST3 plugins. They never load into this process: each one runs in its own
//! `frame-vst-host`, natively for aarch64 Linux builds and under Proton ARM64
//! (which emulates x64 through FEX) for Windows builds. Audio crosses over in
//! a shared region; see `vst-host/src/shared.rs`.

mod scan;

pub use scan::{load_cache, save_cache, scan, self_test as scan_self_test};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[path = "../../vst-host/src/shared.rs"]
pub mod shared;

/// Which host can run a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VstArch {
    /// Built for aarch64 Linux: loads natively.
    Native,
    /// Built for Windows x64 (or Arm64EC): runs under Proton ARM64.
    Windows,
    /// Only built for x64 Linux. Nothing on the Frame can run it.
    Unsupported,
}

impl VstArch {
    pub fn runs(self) -> bool {
        self != VstArch::Unsupported
    }
}

/// One effect found by a scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VstInfo {
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// The `.vst3` bundle, or the single `.vst3` file older Windows plugins ship as.
    pub path: String,
    /// 32 hex digits from moduleinfo.json. Empty: the host takes the first effect.
    #[serde(default)]
    pub cid: String,
    pub arch: VstArch,
}

/// A plugin placed in an effect chain. `instance` names its state file and its
/// audio region, so two copies of one plugin keep separate settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VstRef {
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    pub path: String,
    #[serde(default)]
    pub cid: String,
    pub arch: VstArch,
    pub instance: String,
}

impl VstRef {
    pub fn place(info: &VstInfo) -> Self {
        Self {
            name: info.name.clone(),
            vendor: info.vendor.clone(),
            path: info.path.clone(),
            cid: info.cid.clone(),
            arch: info.arch,
            instance: new_instance_id(),
        }
    }
}

fn new_instance_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or(0);
    format!("{:x}{:02x}", nanos & 0xffff_ffff_ffff, COUNTER.fetch_add(1, Ordering::Relaxed) & 0xff)
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

/// `~/` expanded, so the folder setting can be typed the short way.
pub fn expand(folder: &str) -> PathBuf {
    match folder.trim().strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if folder.trim() == "~" => home(),
        None => PathBuf::from(folder.trim()),
    }
}

/// Where the engine and a host exchange audio. Instance ids are hex, so the
/// name needs no escaping on either side.
pub fn region_path(instance: &str) -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("frame-audio-mixer").join(format!("vst-{instance}.shm"))
}

pub fn state_path(instance: &str) -> PathBuf {
    crate::config::config_path().with_file_name("vst").join(format!("{instance}.state"))
}

pub fn cache_path() -> PathBuf {
    let cache = std::env::var("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|_| home().join(".cache"));
    cache.join("frame-audio-mixer").join("vst-plugins.json")
}

fn lib_dir() -> PathBuf {
    home().join(".local/lib/frame-audio-mixer")
}

/// Proton ARM64 builds in the home Steam library, newest first.
fn proton_dirs() -> Vec<PathBuf> {
    let common = home().join(".local/share/Steam/steamapps/common");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&common)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    name.starts_with("Proton") && name.contains("ARM64") && path.join("files/bin-arm64/wine").is_file()
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found.reverse();
    found
}

/// A Linux path as Wine sees it, through the `Z:` drive.
pub fn wine_path(path: &Path) -> String {
    format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
}

/// The command that runs a plugin's host, ready to spawn. Proton's own `wine`
/// is called directly: `proton run` goes through a launcher that drops stdin.
pub fn host_command(plugin: &VstRef) -> Result<std::process::Command, String> {
    let region = region_path(&plugin.instance);
    let state = state_path(&plugin.instance);
    let cid = if plugin.cid.is_empty() { "-".to_string() } else { plugin.cid.clone() };
    match plugin.arch {
        VstArch::Unsupported => Err("x64 Linux plugins cannot run on the Frame".into()),
        VstArch::Native => {
            let host = lib_dir().join("frame-vst-host");
            if !host.is_file() {
                return Err(format!("{} is missing; run install.sh again", host.display()));
            }
            let mut command = std::process::Command::new(host);
            command.arg("serve").arg(&region).arg(&plugin.path).arg(cid).arg(&state).arg(&plugin.name);
            Ok(command)
        }
        VstArch::Windows => {
            let host = lib_dir().join("frame-vst-host.exe");
            if !host.is_file() {
                return Err(format!("{} is missing; the release archive ships it", host.display()));
            }
            let proton = proton_dirs().into_iter().next().ok_or("Proton ARM64 is not installed in Steam")?;
            let files = proton.join("files");
            let mut command = std::process::Command::new(files.join("bin-arm64/wine"));
            command
                .env("WINELOADER", files.join("bin-arm64/wine"))
                .env("WINESERVER", files.join("bin-arm64/wineserver"))
                .env("WINEPREFIX", home().join(".local/share/frame-audio-mixer/proton/pfx"))
                .env("WINEDLLPATH", format!("{}:{}", files.join("lib/wine").display(), files.join("lib/vkd3d").display()))
                .env("WINEDEBUG", "-all")
                .env("WINEDLLOVERRIDES", "winemenubuilder.exe=d")
                .env("LD_LIBRARY_PATH", files.join("lib/aarch64-linux-gnu"))
                .arg(&host)
                .arg("serve")
                .arg(wine_path(&region))
                .arg(wine_path(Path::new(&plugin.path)))
                .arg(cid)
                .arg(wine_path(&state))
                .arg(&plugin.name);
            Ok(command)
        }
    }
}

pub fn self_test() -> Result<(), String> {
    scan_self_test()?;
    if wine_path(Path::new("/run/user/1000/a b.shm")) != "Z:\\run\\user\\1000\\a b.shm" {
        return Err("wine path".into());
    }
    let (a, b) = (new_instance_id(), new_instance_id());
    if a == b || !a.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("instance ids {a} {b}"));
    }
    let region = std::env::temp_dir().join(format!("famx-region-{}.shm", std::process::id()));
    let made = shared::Region::create(&region).map_err(|err| err.to_string())?;
    made.write(shared::Plane::InLeft, &[0.5, 0.25]);
    let opened = shared::Region::open(&region).map_err(|err| err.to_string())?;
    let mut back = [0.0f32; 2];
    opened.read(shared::Plane::InLeft, &mut back);
    let _ = std::fs::remove_file(&region);
    if back != [0.5, 0.25] {
        return Err("region round trip".into());
    }
    Ok(())
}

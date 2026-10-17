//! Download a release, check SHA256SUMS, and run its install.sh.
//! The panel starts this as `frame-audio-mixer --update-install` inside a
//! short-lived systemd user unit, from a copy of the binary.

use super::check::{self, download, sha256_file, url_allowed, version_core, vercmp, Asset, ASSET, REPO, VERSION};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct StateFile {
    pub state: String,
    pub step: String,
    pub version: String,
    pub error: String,
    pub message: String,
    pub updated_at: u64,
    pub pid: u32,
}

pub fn detach() -> Result<(), String> {
    if lock_busy() {
        return Err("another update is already running".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = check::cache_dir().join("update");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let copy = dir.join("frame-audio-mixer-updater");
    fs::copy(&exe, &copy).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    write_state("running", "start", "", "", "")?;
    let mut cmd = Command::new("systemd-run");
    cmd.args([
        "--user",
        "--unit=frame-audio-mixer-update",
        "--collect",
        "--quiet",
        &format!("--description=Update frame-audio-mixer"),
    ]);
    for key in ["HOME", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "PATH"] {
        if let Ok(value) = std::env::var(key) {
            cmd.arg(format!("--setenv={key}={value}"));
        }
    }
    cmd.arg("--").arg(&copy).arg("--update-install");
    let status = cmd.status().map_err(|e| format!("detach-failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        write_state("failed", "", "", "detach-failed", "could not start the update unit")?;
        Err("could not start the update unit".into())
    }
}

/// Runs inside the copied binary. Writes progress to update-state.json.
pub fn run() -> Result<(), String> {
    if !acquire_lock() {
        return Err("another update is already running".into());
    }
    let log_path = check::cache_dir().join("update.log");
    let _ = fs::create_dir_all(check::cache_dir());
    write_state("running", "start", "", "", "")?;
    log(&log_path, &format!("updating frame-audio-mixer from {VERSION}"));
    let result = run_inner(&log_path);
    release_lock();
    if let Err(err) = &result {
        let _ = write_state("failed", "", "", "install-failed", err);
        log(&log_path, err);
    }
    result
}

fn run_inner(log_path: &Path) -> Result<(), String> {
    let asset = fetch().map_err(|e| e)?;
    let latest = version_core(&asset.version).ok_or_else(|| format!("bad version {}", asset.tag))?;
    let current = version_core(VERSION).unwrap_or_else(|| VERSION.into());
    if vercmp(&latest, &current) != 1 {
        return Err(format!("the latest release {} is not newer", asset.version));
    }
    if asset.asset_url.is_empty() {
        return Err(format!("release {} has no {}", asset.tag, asset.asset_name));
    }
    if asset.sums_url.is_empty() {
        return Err("release has no SHA256SUMS; update by hand".into());
    }
    let dir = check::cache_dir().join("update").join(&latest);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("files")).map_err(|e| e.to_string())?;
    write_state("running", "download", &asset.version, "", "")?;
    let archive = dir.join(&asset.asset_name);
    download(&asset.asset_url, &archive, MAX_ARCHIVE, 600)?;
    let sums_path = dir.join("SHA256SUMS");
    download(&asset.sums_url, &sums_path, 1024 * 1024, 60)?;
    write_state("running", "verify", &asset.version, "", "")?;
    let sums = fs::read_to_string(&sums_path).map_err(|e| e.to_string())?;
    let expected = check::parse_sums(&sums, &asset.asset_name).ok_or_else(|| format!("SHA256SUMS has no entry for {}", asset.asset_name))?;
    let actual = sha256_file(&archive)?;
    if actual != expected {
        return Err(format!("{actual} does not match SHA256SUMS"));
    }
    log(log_path, &format!("SHA-256 ok: {actual}"));
    write_state("running", "extract", &asset.version, "", "")?;
    let files = dir.join("files");
    extract_safe(&archive, &files)?;
    let installer = find_installer(&files).ok_or_else(|| "archive has no install.sh".to_string())?;
    write_state("running", "install", &asset.version, "", "")?;
    let args = install_args();
    log(log_path, &format!("running install.sh {}", args.join(" ")));
    let status = Command::new("sh")
        .arg(&installer)
        .args(&args)
        .current_dir(installer.parent().unwrap_or(Path::new(".")))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("install.sh exited with {}", status.code().unwrap_or(-1)));
    }
    write_state("done", "", &asset.version, "", "")?;
    log(log_path, &format!("updated to {}", asset.version));
    Ok(())
}

fn fetch() -> Result<Asset, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    if !url_allowed(&url) {
        return Err("bad-url".into());
    }
    let response = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(30))
        .redirects(5)
        .build()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", &format!("frame-audio-mixer/{VERSION}"))
        .call()
        .map_err(|e| e.to_string())?;
    if !url_allowed(response.get_url()) {
        return Err("bad-url".into());
    }
    let mut body = String::new();
    response.into_reader().take(1_048_576).read_to_string(&mut body).map_err(|e| e.to_string())?;
    let asset = check::parse_release(&body, ASSET)?;
    if asset.draft || asset.prerelease {
        return Err("the latest release is a draft or prerelease".into());
    }
    Ok(asset)
}

pub fn path_allowed(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    path.components().all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn extract_safe(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    let entries = tar.entries().map_err(|e| e.to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if !path_allowed(&path) {
            return Err(format!("unsafe path in archive: {}", path.display()));
        }
        match entry.header().entry_type() {
            tar::EntryType::Regular | tar::EntryType::Continuous | tar::EntryType::Directory => {}
            _ => return Err(format!("refusing special file {}", path.display())),
        }
        entry.unpack_in(dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_installer(files: &Path) -> Option<PathBuf> {
    let direct = files.join("install.sh");
    if direct.is_file() {
        return Some(direct);
    }
    let dir = fs::read_dir(files).ok()?;
    let mut children: Vec<_> = dir.flatten().map(|item| item.path()).filter(|path| path.is_dir()).collect();
    if children.len() == 1 && children[0].join("install.sh").is_file() {
        return Some(children.remove(0).join("install.sh"));
    }
    None
}

fn install_args() -> Vec<String> {
    let path = check::config_dir().join("install-args");
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| arg_allowed(line))
        .map(str::to_string)
        .collect()
}

fn arg_allowed(arg: &str) -> bool {
    let mut chars = arg.chars();
    chars.next() == Some('-')
        && chars.next() == Some('-')
        && arg.len() <= 80
        && arg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '='))
}

fn log(path: &Path, line: &str) {
    let stamp = now();
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{stamp} {line}");
    }
}

pub(crate) fn read_state_file() -> Option<StateFile> {
    let text = fs::read_to_string(check::cache_dir().join("update-state.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_state(state: &str, step: &str, version: &str, error: &str, message: &str) -> Result<(), String> {
    let dir = check::cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = StateFile {
        state: state.into(),
        step: step.into(),
        version: version.into(),
        error: error.into(),
        message: message.into(),
        updated_at: now(),
        pid: std::process::id(),
    };
    fs::write(dir.join("update-state.json"), serde_json::to_string(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn lock_path() -> PathBuf {
    check::cache_dir().join("update.lock")
}

fn lock_busy() -> bool {
    let path = lock_path();
    if !path.is_dir() {
        return false;
    }
    let pid = fs::read_to_string(path.join("pid")).ok().and_then(|text| text.trim().parse::<i32>().ok()).unwrap_or(0);
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

fn acquire_lock() -> bool {
    if lock_busy() {
        return false;
    }
    let path = lock_path();
    let _ = fs::remove_dir_all(&path);
    if fs::create_dir(&path).is_err() {
        return false;
    }
    let _ = fs::write(path.join("pid"), format!("{}\n", std::process::id()));
    true
}

fn release_lock() {
    let _ = fs::remove_dir_all(lock_path());
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A ustar member the `tar` crate refuses to build, so the test can still hand it a `..` path.
fn ustar_member(name: &str, body: &[u8]) -> Vec<u8> {
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..108].copy_from_slice(b"0000644\0");
    let size = format!("{:011o}\0", body.len());
    header[124..136].copy_from_slice(size.as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[148..156].copy_from_slice(b"        ");
    header[156] = b'0';
    header[257..262].copy_from_slice(b"ustar");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    let cksum = format!("{:06o}\0 ", sum);
    header[148..156].copy_from_slice(cksum.as_bytes());
    let mut bytes = header.to_vec();
    bytes.extend_from_slice(body);
    bytes.resize(bytes.len() + (512 - body.len() % 512) % 512, 0);
    bytes.resize(bytes.len() + 1024, 0);
    bytes
}

pub fn self_test() -> Result<(), String> {
    if path_allowed(Path::new("../etc/passwd")) || path_allowed(Path::new("/tmp/x")) || path_allowed(Path::new("a/../../b")) {
        return Err("unsafe path accepted".into());
    }
    if !path_allowed(Path::new("install.sh")) || !path_allowed(Path::new("frame-audio-mixer/install.sh")) {
        return Err("safe path rejected".into());
    }
    let dir = std::env::temp_dir().join(format!("frame-amix-tar-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let archive = dir.join("ok.tar.gz");
    {
        let file = File::create(&archive).map_err(|e| e.to_string())?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        let body = b"echo ok\n";
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "install.sh", &body[..]).map_err(|e| e.to_string())?;
        builder.finish().map_err(|e| e.to_string())?;
    }
    extract_safe(&archive, &dir.join("out")).map_err(|e| e.to_string())?;
    if fs::read_to_string(dir.join("out/install.sh")).ok().as_deref() != Some("echo ok\n") {
        return Err("extract".into());
    }
    let bad = dir.join("bad.tar.gz");
    {
        let file = File::create(&bad).map_err(|e| e.to_string())?;
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &ustar_member("../evil", b"x")).map_err(|e| e.to_string())?;
        encoder.finish().map_err(|e| e.to_string())?;
    }
    if extract_safe(&bad, &dir.join("bad-out")).is_ok() {
        return Err("unsafe archive extracted".into());
    }
    let _ = fs::remove_dir_all(&dir);
    if !arg_allowed("--autostart") || arg_allowed("--autostart;rm") || arg_allowed("-x") {
        return Err("install args".into());
    }
    Ok(())
}

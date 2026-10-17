//! Ask GitHub for the latest release and remember the answer.

use super::install::read_state_file;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "marcus-universe/frame-audio-mixer";
pub const ASSET: &str = "frame-audio-mixer-{version}.tar.gz";

const CHECK_TTL: u64 = 86_400;
const ERROR_TTL: u64 = 3_600;
const MAX_JSON: u64 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    Unknown,
    UpToDate,
    Available,
    Installing,
    Installed,
    CheckFailed,
    InstallFailed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateView {
    pub state: UpdateState,
    pub latest: String,
    pub message: String,
    pub installable: bool,
    pub step: String,
    pub version: String,
}

impl Default for UpdateView {
    fn default() -> Self {
        Self {
            state: UpdateState::Unknown,
            latest: String::new(),
            message: String::new(),
            installable: false,
            step: String::new(),
            version: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub tag: String,
    pub version: String,
    pub page: String,
    pub asset_name: String,
    pub asset_url: String,
    pub sums_url: String,
    pub draft: bool,
    pub prerelease: bool,
}

pub fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{home}/.cache"));
    PathBuf::from(base).join("frame-audio-mixer")
}

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(base).join("frame-audio-mixer")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `force` ignores a fresh cached answer and asks GitHub again.
pub fn check(force: bool) -> UpdateView {
    if !force {
        if let Some(view) = fresh_cache() {
            return apply_install(view);
        }
    }
    let view = match fetch_release() {
        Ok(asset) => view_from_asset(&asset),
        Err(err) => UpdateView {
            state: UpdateState::CheckFailed,
            message: err.message,
            version: err.code,
            ..UpdateView::default()
        },
    };
    let _ = write_cache(&view);
    apply_install(view)
}

/// The last answer on disk, plus an install that is running or just finished.
/// This does not use the network.
pub fn current_view() -> UpdateView {
    apply_install(fresh_cache().unwrap_or_default())
}

fn apply_install(mut view: UpdateView) -> UpdateView {
    let Some(state) = read_state_file() else { return view };
    let age = now().saturating_sub(state.updated_at);
    match state.state.as_str() {
        "running" => {
            let alive = state.pid > 0 && unsafe { libc::kill(state.pid as i32, 0) } == 0;
            if alive || (state.pid == 0 && age < 60) {
                view.state = UpdateState::Installing;
                view.step = state.step;
                view.version = state.version;
            } else {
                view.state = UpdateState::InstallFailed;
                view.message = "the update was interrupted".into();
                view.version = state.version;
            }
        }
        "failed" => {
            view.state = UpdateState::InstallFailed;
            view.message = state.message;
            view.version = state.version;
        }
        "done" if age < CHECK_TTL => {
            view.state = UpdateState::Installed;
            view.latest = state.version.clone();
            view.version = state.version;
        }
        _ => {}
    }
    view
}

struct CheckError {
    code: String,
    message: String,
}

fn fetch_release() -> Result<Asset, CheckError> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    if !url_allowed(&url) {
        return Err(fail("bad-url", "refusing to contact the update server"));
    }
    let response = agent(30)
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", &format!("frame-audio-mixer/{VERSION}"))
        .call();
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Err(fail("not-found", "no published release")),
        Err(ureq::Error::Status(code, _)) if code == 403 || code == 429 => {
            return Err(fail("rate-limited", "GitHub refused the request"));
        }
        Err(err) => return Err(fail("network", &err.to_string())),
    };
    if !url_allowed(response.get_url()) {
        return Err(fail("bad-url", "redirected away from GitHub"));
    }
    let mut body = String::new();
    response.into_reader().take(MAX_JSON).read_to_string(&mut body).map_err(|e| fail("network", &e.to_string()))?;
    let asset = parse_release(&body, ASSET).map_err(|message| fail("bad-response", &message))?;
    if asset.draft || asset.prerelease {
        return Err(fail("not-found", "the latest release is a draft or prerelease"));
    }
    Ok(asset)
}

fn fail(code: &str, message: &str) -> CheckError {
    CheckError { code: code.into(), message: message.into() }
}

fn view_from_asset(asset: &Asset) -> UpdateView {
    let Some(latest_core) = version_core(&asset.version) else {
        return UpdateView { state: UpdateState::CheckFailed, message: "bad version".into(), ..UpdateView::default() };
    };
    let current = version_core(VERSION).unwrap_or_else(|| VERSION.into());
    if vercmp(&latest_core, &current) != 1 {
        return UpdateView { state: UpdateState::UpToDate, latest: asset.version.clone(), ..UpdateView::default() };
    }
    let installable = !asset.asset_url.is_empty() && !asset.sums_url.is_empty();
    let message = if asset.asset_url.is_empty() {
        "no-asset".into()
    } else if asset.sums_url.is_empty() {
        "no-checksums".into()
    } else {
        String::new()
    };
    UpdateView {
        state: UpdateState::Available,
        latest: asset.version.clone(),
        message,
        installable,
        version: asset.version.clone(),
        step: String::new(),
    }
}

pub fn parse_release(text: &str, pattern: &str) -> Result<Asset, String> {
    let release: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let tag = release.get("tag_name").and_then(Value::as_str).unwrap_or("").to_string();
    if tag.is_empty() {
        return Err("missing tag".into());
    }
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    let asset_name = pattern.replace("{version}", &version);
    let mut asset_url = String::new();
    let mut sums_url = String::new();
    for item in release.get("assets").and_then(Value::as_array).into_iter().flatten() {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        let url = item.get("browser_download_url").and_then(Value::as_str).unwrap_or("");
        if name == asset_name {
            asset_url = url.to_string();
        }
        if name == "SHA256SUMS" {
            sums_url = url.to_string();
        }
    }
    Ok(Asset {
        tag,
        version,
        page: release.get("html_url").and_then(Value::as_str).unwrap_or("").to_string(),
        asset_name,
        asset_url,
        sums_url,
        draft: release.get("draft").and_then(Value::as_bool).unwrap_or(false),
        prerelease: release.get("prerelease").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// Numeric core of a version. "v0.4.0-rc1" is "0.4.0". Empty or junk is None.
pub fn version_core(version: &str) -> Option<String> {
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next().unwrap_or("");
    if core.is_empty() || core.split('.').count() > 3 {
        return None;
    }
    if core.split('.').any(|part| part.is_empty() || part.len() > 9 || !part.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some(core.to_string())
}

/// Compare two version cores. -1, 0 or 1. Missing parts count as 0.
pub fn vercmp(left: &str, right: &str) -> i32 {
    let mut a = left.split('.');
    let mut b = right.split('.');
    for _ in 0..3 {
        let x: u64 = a.next().unwrap_or("0").parse().unwrap_or(0);
        let y: u64 = b.next().unwrap_or("0").parse().unwrap_or(0);
        if x > y {
            return 1;
        }
        if x < y {
            return -1;
        }
    }
    0
}

pub fn url_allowed(url: &str) -> bool {
    let insecure = std::env::var("FRAME_UPDATE_ALLOW_INSECURE").ok().as_deref() == Some("1");
    if insecure {
        return url.starts_with("https://") || url.starts_with("http://");
    }
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return false;
    }
    matches!(host, "github.com" | "api.github.com" | "codeload.github.com") || host.ends_with(".githubusercontent.com")
}

pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let hex = parts.next().unwrap_or("").to_ascii_lowercase();
        let file = parts.next().unwrap_or("").trim_start_matches('*');
        if file == name && hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(hex);
        }
    }
    None
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .redirects(5)
        .build()
}

pub(crate) fn download(url: &str, dest: &Path, max_bytes: u64, timeout_secs: u64) -> Result<(), String> {
    if !url_allowed(url) {
        return Err("bad-url".into());
    }
    let response = agent(timeout_secs)
        .get(url)
        .set("User-Agent", &format!("frame-audio-mixer/{VERSION}"))
        .call()
        .map_err(|e| e.to_string())?;
    if !url_allowed(response.get_url()) {
        return Err("bad-url".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut reader = response.into_reader().take(max_bytes + 1);
    let copied = std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    if copied > max_bytes {
        let _ = fs::remove_file(dest);
        return Err("too-large".into());
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    state: String,
    latest: String,
    message: String,
    installable: bool,
    checked_at: u64,
    error: bool,
}

use serde::Serialize;

fn write_cache(view: &UpdateView) -> Result<(), String> {
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = CacheFile {
        state: format!("{:?}", view.state),
        latest: view.latest.clone(),
        message: view.message.clone(),
        installable: view.installable,
        checked_at: now(),
        error: view.state == UpdateState::CheckFailed,
    };
    // Debug format of the enum is stable enough for our own reader; store a word instead.
    let file = CacheFile { state: state_word(view.state).into(), ..file };
    fs::write(dir.join("update-check.json"), serde_json::to_string(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn state_word(state: UpdateState) -> &'static str {
    match state {
        UpdateState::UpToDate => "up-to-date",
        UpdateState::Available => "available",
        UpdateState::CheckFailed => "error",
        UpdateState::Installing => "installing",
        UpdateState::Installed => "installed",
        UpdateState::InstallFailed => "install-failed",
        UpdateState::Unknown => "unknown",
    }
}

fn fresh_cache() -> Option<UpdateView> {
    let text = fs::read_to_string(cache_dir().join("update-check.json")).ok()?;
    let file: CacheFile = serde_json::from_str(&text).ok()?;
    let age = now().saturating_sub(file.checked_at);
    let limit = if file.error { ERROR_TTL } else { CHECK_TTL };
    if age > limit {
        return None;
    }
    let state = match file.state.as_str() {
        "up-to-date" => UpdateState::UpToDate,
        "available" => UpdateState::Available,
        "error" => UpdateState::CheckFailed,
        _ => UpdateState::Unknown,
    };
    Some(UpdateView {
        state,
        latest: file.latest,
        message: file.message,
        installable: file.installable,
        step: String::new(),
        version: String::new(),
    })
}

pub fn self_test() -> Result<(), String> {
    if version_core("v0.4.0-rc1").as_deref() != Some("0.4.0") || version_core("nope").is_some() {
        return Err("version core".into());
    }
    if vercmp("0.2.0", "0.1.9") != 1 || vercmp("1.0", "1.0.0") != 0 || vercmp("0.1.0", "0.2.0") != -1 {
        return Err("vercmp".into());
    }
    if !url_allowed("https://api.github.com/repos/x") || !url_allowed("https://objects.githubusercontent.com/a") {
        return Err("allow github".into());
    }
    if url_allowed("http://api.github.com/x") || url_allowed("https://evil.example/x") {
        return Err("blocked a url".into());
    }
    let json = r#"{"tag_name":"v0.2.0","draft":false,"prerelease":false,"html_url":"https://github.com/x","assets":[
        {"name":"frame-audio-mixer-0.2.0.tar.gz","browser_download_url":"https://github.com/a"},
        {"name":"SHA256SUMS","browser_download_url":"https://github.com/b"}]}"#;
    let asset = parse_release(json, ASSET)?;
    if asset.version != "0.2.0" || asset.asset_name != "frame-audio-mixer-0.2.0.tar.gz" || asset.sums_url.is_empty() {
        return Err(format!("release {asset:?}"));
    }
    let sums = "abcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabca  frame-audio-mixer-0.2.0.tar.gz\n";
    // 64 hex chars: count them in the test string below instead.
    let hex = "a".repeat(64);
    let sums = format!("{hex}  frame-audio-mixer-0.2.0.tar.gz\n{hex} *SHA256SUMS\n");
    if parse_sums(&sums, "frame-audio-mixer-0.2.0.tar.gz").as_deref() != Some(hex.as_str()) {
        return Err("sums".into());
    }
    if parse_sums("zz  file\n", "file").is_some() {
        return Err("bad sums accepted".into());
    }
    let _ = sums;
    Ok(())
}

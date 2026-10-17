//! Talks to the vendored frame-update.sh. The release asset is
//! frame-audio-mixer-{version}.tar.gz plus a SHA256SUMS file.

use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "marcus-universe/frame-audio-mixer";
pub const ASSET: &str = "frame-audio-mixer-{version}.tar.gz";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateView {
    pub status: String,
    pub latest: String,
    pub message: String,
    pub installable: bool,
}

#[derive(Deserialize)]
struct CheckJson {
    status: Option<String>,
    latest: Option<String>,
    message: Option<String>,
    installable: Option<bool>,
    error: Option<String>,
}

pub fn script_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    PathBuf::from(data).join("frame-audio-mixer").join("frame-update.sh")
}

pub fn check() -> UpdateView {
    run(&["check", "--force"]).unwrap_or_else(|message| UpdateView {
        status: "error".into(),
        message,
        ..UpdateView::default()
    })
}

pub fn install() -> Result<(), String> {
    let path = script_path();
    if !path.exists() {
        return Err("updater is not installed".into());
    }
    let status = Command::new(&path)
        .args([
            "--app",
            "frame-audio-mixer",
            "--repo",
            REPO,
            "--current",
            VERSION,
            "--asset",
            ASSET,
            "--detach",
            "install",
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("updater exit {}", status.code().unwrap_or(-1)))
    }
}

fn run(extra: &[&str]) -> Result<UpdateView, String> {
    let path = script_path();
    if !path.exists() {
        return Err("updater is not installed".into());
    }
    let mut cmd = Command::new(&path);
    cmd.args(["--app", "frame-audio-mixer", "--repo", REPO, "--current", VERSION, "--asset", ASSET]);
    cmd.args(extra);
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().last().unwrap_or("").trim();
    if line.is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let parsed: CheckJson = serde_json::from_str(line).map_err(|e| format!("{e}: {line}"))?;
    Ok(UpdateView {
        status: parsed.status.unwrap_or_else(|| parsed.error.clone().unwrap_or_default()),
        latest: parsed.latest.unwrap_or_default(),
        message: parsed.message.or(parsed.error).unwrap_or_default(),
        installable: parsed.installable.unwrap_or(false),
    })
}

pub fn tuner_version() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin = PathBuf::from(home).join(".local/bin/frame-mic-tuner");
    if !bin.exists() {
        return String::new();
    }
    let out = Command::new(&bin).arg("--version").output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).split_whitespace().last().unwrap_or("").to_string(),
        Err(_) => "installed".into(),
    }
}

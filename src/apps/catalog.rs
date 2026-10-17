//! The built-in app list, the rules for a fetched list, and the embedded icons.

use serde_json::Value;
use sha2::{Digest, Sha256};

pub const SITE: &str = "https://frame.sasaken1102s.net";
const MAX_CATALOG: usize = 64 * 1024;
const MAX_APPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    pub key: String,
    pub mono: String,
    pub desc_ja: String,
    pub desc_en: String,
    pub unit: String,
    pub main_file: String,
    pub icon: String,
    pub icon_sha256: String,
}

pub fn bundled() -> Vec<AppInfo> {
    vec![
        app("frameeyeosc", "eye", "Eye", "視線とまぶたを OSC で VRChat に送る", "Sends eye gaze and eyelids to VRChat over OSC", "frameeyeosc.service", ".local/bin/frameeyeosc", "frameeyeosc-128.png", "d4f6f9bff0b1fb1d6fcd8141c470ebd2c2af0f7e4a982837a4f472646c9a520b"),
        app("frame-jp-keyboard", "keyboard", "Kb", "VR キーボードにフリック入力とかな漢字変換", "Japanese flick input and kana-kanji conversion for the VR keyboard", "frame-jp-keyboard.service", ".local/share/frame-jp-keyboard/bundle.js", "frame-jp-keyboard-128.png", "3c5b5f32339cefcbef5c828e06a13e3fee0dddb469634c6abc042d0439115d66"),
        app("frame-mic-tuner", "mic", "Mic", "マイクのエコー除去・ノイズ除去を切り替える", "Switches the mic echo cancellation and noise suppression", "frame-mic-tuner.service", ".local/bin/frame-mic-tuner", "frame-mic-tuner-128.png", "8f14b06d0bc26cd8a01e31fe66c5b2e88b758cfb760bbc8d3dd0ce25069b4c9a"),
        app("frame-perf-overlay", "perf", "Perf", "フレームレート・温度などを視界の隅に出す", "Shows frame rate, temperatures and more in a corner of the view", "frame-perf-overlay.service", ".local/bin/frame-perf-overlay", "frame-perf-overlay-128.png", "910cd676a02b54b466db980dc1b29b8164753fb2c8d04e2006f6b59f97e69975"),
        app("frame-aux-shortcuts", "aux", "Aux", "aux ボタンを、押し方ごとのショートカットにする", "Shortcuts on the aux button, one for each way you press it", "frame-aux-shortcuts.service", ".local/bin/frame-aux-shortcuts", "frame-aux-shortcuts-128.png", "cc6c47f8b55200cfacaf76a5b16844f513738b2fc02cfd92cc0f07d2c5a6f062"),
    ]
}

fn app(
    name: &str,
    key: &str,
    mono: &str,
    desc_ja: &str,
    desc_en: &str,
    unit: &str,
    main_file: &str,
    icon: &str,
    sha: &str,
) -> AppInfo {
    AppInfo {
        name: name.into(),
        key: key.into(),
        mono: mono.into(),
        desc_ja: desc_ja.into(),
        desc_en: desc_en.into(),
        unit: unit.into(),
        main_file: main_file.into(),
        icon: icon.into(),
        icon_sha256: sha.into(),
    }
}

pub fn is_valid_app(app: &AppInfo) -> bool {
    is_key(&app.key)
        && is_name(&app.name)
        && is_mono(&app.mono)
        && is_unit(&app.unit)
        && is_main_file(&app.main_file)
        && is_desc(&app.desc_ja)
        && is_desc(&app.desc_en)
        && is_icon_name(&app.icon)
        && is_sha256(&app.icon_sha256)
}

fn is_key(text: &str) -> bool {
    (1..=16).contains(&text.len()) && text.chars().all(|c| c.is_ascii_lowercase())
}
fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && text.len() <= 40
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
fn is_mono(text: &str) -> bool {
    (1..=4).contains(&text.chars().count()) && text.chars().all(|c| c.is_ascii_alphanumeric())
}
fn is_unit(text: &str) -> bool {
    text.ends_with(".service") && is_name(text.trim_end_matches(".service")) && text.len() <= 48
}
fn is_main_file(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 120
        && !text.starts_with('/')
        && text.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
}
fn is_desc(text: &str) -> bool {
    let count = text.chars().count();
    (1..=120).contains(&count) && !text.chars().any(|c| c.is_control())
}
fn is_icon_name(text: &str) -> bool {
    text.ends_with("-128.png") && is_name(text.trim_end_matches("-128.png"))
}
fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub fn parse_catalog(text: &str) -> Result<Vec<AppInfo>, String> {
    if text.len() > MAX_CATALOG {
        return Err("catalog is too large".into());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("catalog version".into());
    }
    let Some(items) = value.get("apps").and_then(Value::as_array) else {
        return Err("catalog apps".into());
    };
    if items.len() > MAX_APPS {
        return Err("too many apps".into());
    }
    let mut apps = Vec::new();
    for item in items {
        let info = AppInfo {
            name: field(item, "name"),
            key: field(item, "key"),
            mono: field(item, "mono"),
            desc_ja: field(item, "desc_ja"),
            desc_en: field(item, "desc_en"),
            unit: field(item, "unit"),
            main_file: field(item, "main_file"),
            icon: field(item, "icon"),
            icon_sha256: field(item, "icon_sha256"),
        };
        if is_valid_app(&info) && !apps.iter().any(|other: &AppInfo| other.name == info.name || other.key == info.key) {
            apps.push(info);
        }
    }
    if apps.is_empty() {
        return Err("no usable apps".into());
    }
    Ok(apps)
}

fn field(item: &Value, key: &str) -> String {
    item.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn embedded_png(name: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match name {
        "frameeyeosc" => (include_bytes!("../../assets/frame-apps/frameeyeosc-128.png"), "d4f6f9bff0b1fb1d6fcd8141c470ebd2c2af0f7e4a982837a4f472646c9a520b"),
        "frame-jp-keyboard" => (include_bytes!("../../assets/frame-apps/frame-jp-keyboard-128.png"), "3c5b5f32339cefcbef5c828e06a13e3fee0dddb469634c6abc042d0439115d66"),
        "frame-mic-tuner" => (include_bytes!("../../assets/frame-apps/frame-mic-tuner-128.png"), "8f14b06d0bc26cd8a01e31fe66c5b2e88b758cfb760bbc8d3dd0ce25069b4c9a"),
        "frame-perf-overlay" => (include_bytes!("../../assets/frame-apps/frame-perf-overlay-128.png"), "910cd676a02b54b466db980dc1b29b8164753fb2c8d04e2006f6b59f97e69975"),
        "frame-aux-shortcuts" => (include_bytes!("../../assets/frame-apps/frame-aux-shortcuts-128.png"), "cc6c47f8b55200cfacaf76a5b16844f513738b2fc02cfd92cc0f07d2c5a6f062"),
        _ => return None,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn self_test() -> Result<(), String> {
    for info in bundled() {
        if !is_valid_app(&info) {
            return Err(format!("bundled {}", info.name));
        }
        let (bytes, sha) = embedded_png(&info.name).ok_or_else(|| format!("icon {}", info.name))?;
        if sha256_hex(bytes) != sha || sha != info.icon_sha256 {
            return Err(format!("hash {}", info.name));
        }
    }
    let bad = r#"{"version":1,"apps":[{"name":"Bad Name","key":"x","mono":"X","desc_ja":"あ","desc_en":"a","unit":"bad.service","main_file":".local/bin/x","icon":"bad-128.png","icon_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#;
    if parse_catalog(bad).is_ok() {
        return Err("bad catalog accepted".into());
    }
    let one = bundled().remove(0);
    let json = format!(
        r#"{{"version":1,"apps":[{{"name":"{}","key":"{}","mono":"{}","desc_ja":"{}","desc_en":"{}","unit":"{}","main_file":"{}","icon":"{}","icon_sha256":"{}"}}]}}"#,
        one.name, one.key, one.mono, one.desc_ja, one.desc_en, one.unit, one.main_file, one.icon, one.icon_sha256
    );
    let parsed = parse_catalog(&json)?;
    if parsed.len() != 1 || parsed[0].name != "frameeyeosc" {
        return Err("catalog parse".into());
    }
    Ok(())
}

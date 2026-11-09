//! Finding plugins without loading them. A VST3 bundle names its builds by
//! folder (`Contents/x86_64-win`, `Contents/aarch64-linux`, ...), and newer
//! ones describe their classes in `Contents/Resources/moduleinfo.json`.

use std::path::Path;

use serde_json::Value;

use super::{VstArch, VstInfo};

const MAX_DEPTH: usize = 6;

/// Every effect under `folder`, sorted by name. Instruments are left out.
pub fn scan(folder: &Path) -> Vec<VstInfo> {
    let mut found = Vec::new();
    walk(folder, 0, &mut found);
    found.sort_by_key(|info| (info.name.to_lowercase(), info.path.clone()));
    found.dedup_by(|a, b| a.path == b.path && a.cid == b.cid);
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<VstInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_vst3 = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("vst3"));
        if is_vst3 && path.is_dir() {
            found.extend(read_bundle(&path));
        } else if is_vst3 && path.is_file() {
            found.extend(read_single_file(&path));
        } else if path.is_dir() && depth < MAX_DEPTH {
            walk(&path, depth + 1, found);
        }
    }
}

fn arch_of(bundle: &Path) -> VstArch {
    let has = |name: &str| bundle.join("Contents").join(name).is_dir();
    if has("aarch64-linux") {
        VstArch::Native
    } else if has("x86_64-win") || has("arm64ec-win") || has("arm64x-win") {
        VstArch::Windows
    } else {
        VstArch::Unsupported
    }
}

fn read_bundle(bundle: &Path) -> Vec<VstInfo> {
    let arch = arch_of(bundle);
    let path = bundle.to_string_lossy().into_owned();
    let stem = bundle.file_stem().and_then(|s| s.to_str()).unwrap_or("Plugin").to_string();
    let info = std::fs::read_to_string(bundle.join("Contents/Resources/moduleinfo.json")).ok().and_then(|text| parse_lenient(&text));
    let Some(info) = info else {
        return vec![VstInfo { name: stem, vendor: String::new(), path, cid: String::new(), arch }];
    };
    let vendor = info.pointer("/Factory Info/Vendor").and_then(Value::as_str).unwrap_or("").to_string();
    let classes: Vec<VstInfo> = info
        .get("Classes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|class| class.get("Category").and_then(Value::as_str) == Some("Audio Module Class"))
        .filter(|class| is_effect(class))
        .map(|class| VstInfo {
            name: class.get("Name").and_then(Value::as_str).unwrap_or(&stem).to_string(),
            vendor: class.get("Vendor").and_then(Value::as_str).filter(|v| !v.is_empty()).unwrap_or(&vendor).to_string(),
            path: path.clone(),
            cid: class.get("CID").and_then(Value::as_str).unwrap_or("").to_string(),
            arch,
        })
        .collect();
    if classes.is_empty() {
        return Vec::new();
    }
    classes
}

/// moduleinfo lists sub categories such as `Fx|Reverb` or `Instrument|Synth`.
fn is_effect(class: &Value) -> bool {
    let subs: Vec<&str> = match class.get("Sub Categories") {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
        Some(Value::String(text)) => text.split('|').collect(),
        _ => return true,
    };
    let subs: Vec<&str> = subs.iter().flat_map(|s| s.split('|')).collect();
    subs.is_empty() || subs.contains(&"Fx") || !subs.contains(&"Instrument")
}

/// A pre-bundle Windows plugin: one DLL named `.vst3`.
fn read_single_file(file: &Path) -> Vec<VstInfo> {
    let mut magic = [0u8; 2];
    let is_pe = std::fs::File::open(file).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic)).is_ok() && &magic == b"MZ";
    let name = file.file_stem().and_then(|s| s.to_str()).unwrap_or("Plugin").to_string();
    let arch = if is_pe { VstArch::Windows } else { VstArch::Unsupported };
    vec![VstInfo { name, vendor: String::new(), path: file.to_string_lossy().into_owned(), cid: String::new(), arch }]
}

/// moduleinfo.json is JSON5 in practice: trailing commas are common.
fn parse_lenient(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str(text) {
        return Some(value);
    }
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    let chars: Vec<char> = text.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        }
        if c == ',' {
            let next = chars[index + 1..].iter().find(|n| !n.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(c);
    }
    serde_json::from_str(&out).ok()
}

pub fn load_cache() -> Vec<VstInfo> {
    std::fs::read_to_string(super::cache_path()).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn save_cache(found: &[VstInfo]) {
    let path = super::cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(found) {
        let _ = std::fs::write(path, text);
    }
}

pub fn self_test() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("famx-vst-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let windows = root.join("Vendor/Verb.vst3");
    std::fs::create_dir_all(windows.join("Contents/x86_64-win")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(windows.join("Contents/Resources")).map_err(|e| e.to_string())?;
    std::fs::write(
        windows.join("Contents/Resources/moduleinfo.json"),
        r#"{ "Name": "Verb", "Factory Info": { "Vendor": "Acme", },
            "Classes": [
              { "CID": "0123456789ABCDEF0123456789ABCDEF", "Category": "Audio Module Class", "Name": "Big Verb", "Sub Categories": ["Fx", "Reverb"], },
              { "CID": "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", "Category": "Audio Module Class", "Name": "Synth", "Sub Categories": ["Instrument"] },
              { "CID": "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE", "Category": "Component Controller Class", "Name": "Big Verb" },
            ], }"#,
    )
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("Arm.vst3/Contents/aarch64-linux")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("Old.vst3/Contents/x86_64-linux")).map_err(|e| e.to_string())?;
    std::fs::write(root.join("Legacy.vst3"), b"MZ\x90\x00").map_err(|e| e.to_string())?;
    let found = scan(&root);
    let _ = std::fs::remove_dir_all(&root);
    let names: Vec<(&str, VstArch)> = found.iter().map(|info| (info.name.as_str(), info.arch)).collect();
    let expect = [("Arm", VstArch::Native), ("Big Verb", VstArch::Windows), ("Legacy", VstArch::Windows), ("Old", VstArch::Unsupported)];
    if names != expect {
        return Err(format!("scan {names:?}"));
    }
    let verb = &found[1];
    if verb.vendor != "Acme" || verb.cid != "0123456789ABCDEF0123456789ABCDEF" {
        return Err(format!("moduleinfo {verb:?}"));
    }
    Ok(())
}

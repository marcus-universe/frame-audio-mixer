//! Registration with SteamVR, so the add-on shows up under "Startup overlay
//! apps" and starts with every VR session.
//!
//! SteamVR learns about an application from a manifest file. Three things are
//! written, all in the user's own folders:
//!
//! - `manifest.vrmanifest` next to the settings, naming this executable;
//! - its path in the `manifest_paths` list of SteamVR's `appconfig.json`;
//! - `vrappconfig/<app key>.vrappconfig` with `autolaunch` switched on.
//!
//! A running SteamVR is told as well, so the entry appears at once and
//! SteamVR's own copy of the list agrees with the file.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde_json::{json, Map, Value};

/// The key SteamVR knows this add-on by. The dashboard overlay uses it too.
pub(crate) const APP_KEY: &str = "marcus-universe.frame-audio-link";

const MANIFEST_FILE: &str = "manifest.vrmanifest";
const APPCONFIG_FILE: &str = "appconfig.json";
/// Written once, before this add-on first changes `appconfig.json`.
const APPCONFIG_BACKUP: &str = "appconfig.json.frame-audio-link.bak";

/// Where SteamVR keeps its files, read from `openvrpaths.vrpath`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct OpenVrPaths {
    pub config: Vec<PathBuf>,
    pub runtime: Vec<PathBuf>,
}

/// SteamVR's folders, or `None` when SteamVR was never run for this user.
pub(crate) fn openvr_paths() -> Option<OpenVrPaths> {
    let text = std::fs::read_to_string(vrpath_file()?).ok()?;
    parse_vrpath(&text).ok()
}

fn vrpath_file() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    Some(base?.join("openvr").join("openvrpaths.vrpath"))
}

fn parse_vrpath(text: &str) -> anyhow::Result<OpenVrPaths> {
    let value: Value = serde_json::from_str(text).context("openvrpaths.vrpath is not JSON")?;
    let list = |key: &str| -> Vec<PathBuf> {
        value.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(PathBuf::from).collect()).unwrap_or_default()
    };
    Ok(OpenVrPaths { config: list("config"), runtime: list("runtime") })
}

/// Registers the add-on as a SteamVR startup overlay application.
///
/// # Errors
/// When SteamVR's configuration cannot be found or written.
pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("finding this executable")?;
    let manifest = crate::config::dir().join(MANIFEST_FILE);
    write_atomic(&manifest, &manifest_json(&exe)).context("writing the application manifest")?;
    let config_dir = steamvr_config_dir()?;
    register_in(&config_dir, &manifest)?;
    // The files above are what counts; a running SteamVR merely learns sooner.
    if let Err(err) = crate::vr::announce_manifest(&manifest, true) {
        crate::log::line(&format!("the running SteamVR was not told about the add-on: {err:#}"));
    }
    Ok(())
}

/// Removes the registration made by `install`.
///
/// # Errors
/// When SteamVR's configuration cannot be written.
pub fn uninstall() -> anyhow::Result<()> {
    let manifest = crate::config::dir().join(MANIFEST_FILE);
    if let Err(err) = crate::vr::announce_manifest(&manifest, false) {
        crate::log::line(&format!("the running SteamVR was not told about the removal: {err:#}"));
    }
    // Without SteamVR there is no registration to undo, only the manifest.
    if let Ok(config_dir) = steamvr_config_dir() {
        unregister_in(&config_dir, &manifest)?;
    }
    remove_if_present(&manifest)
}

fn steamvr_config_dir() -> anyhow::Result<PathBuf> {
    let paths = openvr_paths().context("SteamVR is not set up for this user (no openvrpaths.vrpath). Start SteamVR once, then try again")?;
    paths.config.into_iter().next().context("openvrpaths.vrpath names no config folder")
}

/// Adds the manifest to SteamVR's list and switches autolaunch on.
fn register_in(config_dir: &Path, manifest: &Path) -> anyhow::Result<()> {
    let appconfig = config_dir.join(APPCONFIG_FILE);
    let current = read_optional(&appconfig)?;
    if let Some(updated) = with_manifest_path(current.as_deref(), &manifest.to_string_lossy())? {
        back_up_once(&appconfig, &config_dir.join(APPCONFIG_BACKUP))?;
        write_atomic(&appconfig, &updated).with_context(|| format!("writing {}", appconfig.display()))?;
    }
    let launch = autolaunch_file(config_dir);
    let updated = with_autolaunch(read_optional(&launch)?.as_deref())?;
    write_atomic(&launch, &updated).with_context(|| format!("writing {}", launch.display()))
}

/// Takes the manifest off SteamVR's list and removes the autolaunch file.
fn unregister_in(config_dir: &Path, manifest: &Path) -> anyhow::Result<()> {
    let appconfig = config_dir.join(APPCONFIG_FILE);
    if let Some(current) = read_optional(&appconfig)? {
        if let Some(updated) = without_manifest_path(&current, &manifest.to_string_lossy())? {
            write_atomic(&appconfig, &updated).with_context(|| format!("writing {}", appconfig.display()))?;
        }
    }
    remove_if_present(&autolaunch_file(config_dir))
}

fn autolaunch_file(config_dir: &Path) -> PathBuf {
    config_dir.join("vrappconfig").join(format!("{APP_KEY}.vrappconfig"))
}

/// The application manifest. SteamVR starts `exe` and lists the add-on under
/// its startup overlay applications.
fn manifest_json(exe: &Path) -> String {
    let binary_key = if cfg!(windows) { "binary_path_windows" } else { "binary_path_linux" };
    let manifest = json!({
        "source": "builtin",
        "applications": [{
            "app_key": APP_KEY,
            "launch_type": "binary",
            binary_key: exe.to_string_lossy(),
            "is_dashboard_overlay": true,
            "strings": {
                "en_us": {
                    "name": "Frame Audio Link",
                    "description": "Four more streaming microphones and four PC outputs for Frame Audio Mixer on the Steam Frame."
                },
                "de_de": {
                    "name": "Frame Audio Link",
                    "description": "Vier weitere Streaming-Mikrofone und vier PC-Ausgänge für Frame Audio Mixer auf dem Steam Frame."
                }
            }
        }]
    });
    pretty(&manifest)
}

/// `appconfig.json` with `manifest` in its list, or `None` when it is already
/// there. A missing file starts a new one. Every other entry and key is kept.
fn with_manifest_path(appconfig: Option<&str>, manifest: &str) -> anyhow::Result<Option<String>> {
    let mut root = match appconfig {
        Some(text) => parse_object(text, APPCONFIG_FILE)?,
        None => Map::new(),
    };
    let paths = root.entry("manifest_paths").or_insert_with(|| Value::Array(Vec::new()));
    let Value::Array(paths) = paths else { bail!("`manifest_paths` in {APPCONFIG_FILE} is not a list") };
    if paths.iter().filter_map(Value::as_str).any(|path| same_path(path, manifest)) {
        return Ok(None);
    }
    paths.push(Value::String(manifest.to_string()));
    Ok(Some(pretty(&Value::Object(root))))
}

/// `appconfig.json` without `manifest` in its list, or `None` when it was not there.
fn without_manifest_path(appconfig: &str, manifest: &str) -> anyhow::Result<Option<String>> {
    let mut root = parse_object(appconfig, APPCONFIG_FILE)?;
    let Some(Value::Array(paths)) = root.get_mut("manifest_paths") else { return Ok(None) };
    let before = paths.len();
    paths.retain(|path| !path.as_str().is_some_and(|path| same_path(path, manifest)));
    if paths.len() == before {
        return Ok(None);
    }
    Ok(Some(pretty(&Value::Object(root))))
}

/// The per-application settings with autolaunch on. Whatever else SteamVR
/// stored there (the last launch time) is kept.
fn with_autolaunch(existing: Option<&str>) -> anyhow::Result<String> {
    let mut root = match existing {
        Some(text) => parse_object(text, "the vrappconfig file")?,
        None => Map::new(),
    };
    root.insert("autolaunch".into(), Value::Bool(true));
    root.entry("last_launch_time").or_insert_with(|| Value::String("0".into()));
    Ok(pretty(&Value::Object(root)))
}

/// A file SteamVR wrote must be a JSON object. Anything else is left alone:
/// replacing a file this add-on cannot read could lose the user's other apps.
fn parse_object(text: &str, what: &str) -> anyhow::Result<Map<String, Value>> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => bail!("{what} is not a JSON object; it was left untouched"),
        Err(err) => bail!("{what} is not readable JSON ({err}); it was left untouched"),
    }
}

/// Windows paths differ only by case and slash direction; elsewhere a path is itself.
fn same_path(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        let fold = |path: &str| path.replace('/', "\\").to_lowercase();
        fold(left) == fold(right)
    } else {
        left == right
    }
}

fn pretty(value: &Value) -> String {
    // Serializing a `Value` cannot fail; the compact form is still valid JSON.
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// The file's text, or `None` when there is no such file.
fn read_optional(path: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes through a temporary file, so a crash mid-write cannot leave SteamVR
/// with half a configuration.
fn write_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".frame-audio-link.tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
    std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))
}

fn back_up_once(file: &Path, backup: &Path) -> anyhow::Result<()> {
    if file.exists() && !backup.exists() {
        std::fs::copy(file, backup).with_context(|| format!("backing up {}", file.display()))?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("removing {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAMVR_APPCONFIG: &str = r#"{
   "manifest_paths" : [
      "H:\\Software\\Steam\\config\\steamapps.vrmanifest",
      "C:\\SteamLibrary\\steamapps\\common\\VRCFaceTracking\\app.vrmanifest"
   ],
   "future_key" : 7
}"#;
    const OURS: &str = "C:\\Users\\me\\AppData\\Roaming\\frame-audio-link\\manifest.vrmanifest";

    fn paths_of(appconfig: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(appconfig).unwrap_or_default();
        value["manifest_paths"].as_array().map(|paths| paths.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
    }

    /// A SteamVR config folder in the temp directory, removed when dropped.
    struct TempConfig(PathBuf);

    impl TempConfig {
        fn new(name: &str) -> anyhow::Result<Self> {
            let dir = std::env::temp_dir().join(format!("frame-audio-link-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)?;
            Ok(Self(dir))
        }

        fn appconfig(&self) -> PathBuf {
            self.0.join(APPCONFIG_FILE)
        }

        fn manifest(&self) -> PathBuf {
            self.0.join("ours").join(MANIFEST_FILE)
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn vrpath_should_yield_the_config_and_runtime_folders() -> anyhow::Result<()> {
        let paths = parse_vrpath(r#"{"config":["H:\\Steam\\config"],"external_drivers":null,"runtime":["H:\\Steam\\steamapps\\common\\SteamVR"],"version":1}"#)?;

        let expected = OpenVrPaths { config: vec![PathBuf::from("H:\\Steam\\config")], runtime: vec![PathBuf::from("H:\\Steam\\steamapps\\common\\SteamVR")] };
        assert_eq!(paths, expected);
        Ok(())
    }

    #[test]
    fn manifest_should_name_this_app_as_a_dashboard_overlay() -> anyhow::Result<()> {
        let manifest: Value = serde_json::from_str(&manifest_json(Path::new("/opt/fal/frame-audio-link")))?;
        let app = &manifest["applications"][0];

        assert_eq!((app["app_key"].as_str(), app["is_dashboard_overlay"].as_bool()), (Some(APP_KEY), Some(true)));
        Ok(())
    }

    #[test]
    fn manifest_should_point_at_the_executable() -> anyhow::Result<()> {
        let manifest: Value = serde_json::from_str(&manifest_json(Path::new("/opt/fal/frame-audio-link")))?;
        let key = if cfg!(windows) { "binary_path_windows" } else { "binary_path_linux" };

        assert_eq!(manifest["applications"][0][key].as_str(), Some("/opt/fal/frame-audio-link"));
        Ok(())
    }

    #[test]
    fn adding_should_append_the_manifest_after_the_existing_ones() -> anyhow::Result<()> {
        let updated = with_manifest_path(Some(STEAMVR_APPCONFIG), OURS)?.unwrap_or_default();

        let expected = vec![
            "H:\\Software\\Steam\\config\\steamapps.vrmanifest".to_string(),
            "C:\\SteamLibrary\\steamapps\\common\\VRCFaceTracking\\app.vrmanifest".to_string(),
            OURS.to_string(),
        ];
        assert_eq!(paths_of(&updated), expected);
        Ok(())
    }

    #[test]
    fn adding_should_keep_keys_it_does_not_know() -> anyhow::Result<()> {
        let updated = with_manifest_path(Some(STEAMVR_APPCONFIG), OURS)?.unwrap_or_default();
        let value: Value = serde_json::from_str(&updated)?;

        assert_eq!(value["future_key"], json!(7));
        Ok(())
    }

    #[test]
    fn adding_twice_should_change_nothing_the_second_time() -> anyhow::Result<()> {
        let once = with_manifest_path(Some(STEAMVR_APPCONFIG), OURS)?.unwrap_or_default();

        assert_eq!(with_manifest_path(Some(&once), OURS)?, None);
        Ok(())
    }

    #[test]
    fn adding_without_a_file_should_start_a_new_list() -> anyhow::Result<()> {
        let created = with_manifest_path(None, OURS)?.unwrap_or_default();

        assert_eq!(paths_of(&created), vec![OURS.to_string()]);
        Ok(())
    }

    #[test]
    fn adding_to_unreadable_json_should_be_refused() {
        assert!(with_manifest_path(Some("{ not json"), OURS).is_err());
    }

    #[test]
    fn adding_to_a_list_that_is_not_a_list_should_be_refused() {
        assert!(with_manifest_path(Some(r#"{"manifest_paths":"oops"}"#), OURS).is_err());
    }

    #[test]
    fn removing_should_restore_the_original_list() -> anyhow::Result<()> {
        let added = with_manifest_path(Some(STEAMVR_APPCONFIG), OURS)?.unwrap_or_default();
        let removed = without_manifest_path(&added, OURS)?.unwrap_or_default();

        assert_eq!(paths_of(&removed), paths_of(STEAMVR_APPCONFIG));
        Ok(())
    }

    #[test]
    fn removing_what_is_not_there_should_change_nothing() -> anyhow::Result<()> {
        assert_eq!(without_manifest_path(STEAMVR_APPCONFIG, OURS)?, None);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_should_match_across_case_and_slashes() {
        assert!(same_path("C:/Users/Me/manifest.vrmanifest", "c:\\users\\me\\MANIFEST.vrmanifest"));
    }

    #[test]
    fn autolaunch_should_be_switched_on_in_a_new_file() -> anyhow::Result<()> {
        let value: Value = serde_json::from_str(&with_autolaunch(None)?)?;

        assert_eq!(value, json!({"autolaunch": true, "last_launch_time": "0"}));
        Ok(())
    }

    #[test]
    fn autolaunch_should_keep_what_steamvr_stored() -> anyhow::Result<()> {
        let existing = r#"{"autolaunch": false, "last_launch_time": "1731400000"}"#;
        let value: Value = serde_json::from_str(&with_autolaunch(Some(existing))?)?;

        assert_eq!(value, json!({"autolaunch": true, "last_launch_time": "1731400000"}));
        Ok(())
    }

    #[test]
    fn registering_should_list_the_manifest_in_appconfig() -> anyhow::Result<()> {
        let config = TempConfig::new("register")?;
        std::fs::write(config.appconfig(), STEAMVR_APPCONFIG)?;
        register_in(&config.0, &config.manifest())?;

        let listed = paths_of(&std::fs::read_to_string(config.appconfig())?);
        assert_eq!(listed.last(), Some(&config.manifest().to_string_lossy().into_owned()));
        Ok(())
    }

    #[test]
    fn registering_should_back_up_the_original_appconfig() -> anyhow::Result<()> {
        let config = TempConfig::new("backup")?;
        std::fs::write(config.appconfig(), STEAMVR_APPCONFIG)?;
        register_in(&config.0, &config.manifest())?;

        assert_eq!(std::fs::read_to_string(config.0.join(APPCONFIG_BACKUP))?, STEAMVR_APPCONFIG);
        Ok(())
    }

    #[test]
    fn registering_should_write_the_autolaunch_file() -> anyhow::Result<()> {
        let config = TempConfig::new("autolaunch")?;
        register_in(&config.0, &config.manifest())?;
        let value: Value = serde_json::from_str(&std::fs::read_to_string(autolaunch_file(&config.0))?)?;

        assert_eq!(value["autolaunch"], json!(true));
        Ok(())
    }

    #[test]
    fn registering_twice_should_leave_one_entry() -> anyhow::Result<()> {
        let config = TempConfig::new("twice")?;
        std::fs::write(config.appconfig(), STEAMVR_APPCONFIG)?;
        register_in(&config.0, &config.manifest())?;
        register_in(&config.0, &config.manifest())?;

        assert_eq!(paths_of(&std::fs::read_to_string(config.appconfig())?).len(), 3);
        Ok(())
    }

    #[test]
    fn unregistering_should_leave_the_other_manifests() -> anyhow::Result<()> {
        let config = TempConfig::new("unregister")?;
        std::fs::write(config.appconfig(), STEAMVR_APPCONFIG)?;
        register_in(&config.0, &config.manifest())?;
        unregister_in(&config.0, &config.manifest())?;

        assert_eq!(paths_of(&std::fs::read_to_string(config.appconfig())?), paths_of(STEAMVR_APPCONFIG));
        Ok(())
    }

    #[test]
    fn unregistering_should_remove_the_autolaunch_file() -> anyhow::Result<()> {
        let config = TempConfig::new("unlaunch")?;
        register_in(&config.0, &config.manifest())?;
        unregister_in(&config.0, &config.manifest())?;

        assert!(!autolaunch_file(&config.0).exists());
        Ok(())
    }

    #[test]
    fn registering_over_a_broken_appconfig_should_leave_it_untouched() -> anyhow::Result<()> {
        let config = TempConfig::new("broken")?;
        std::fs::write(config.appconfig(), "{ not json")?;
        let _ = register_in(&config.0, &config.manifest());

        assert_eq!(std::fs::read_to_string(config.appconfig())?, "{ not json");
        Ok(())
    }
}

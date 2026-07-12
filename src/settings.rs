use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::dchu::HardwareSnapshot;
use crate::fan_curve::FanCurveSettings;
use crate::model::{default_zones, normalize_zones, LightingConfig, Mode, Rgb, ZoneId};
use crate::preferences::{LanguagePreference, ThemeColor};

pub const APP_ID: &str = "clevo-control-center";
const LEGACY_APP_ID: &str = "clevo-keyboard-led";
const SETTINGS_FILE: &str = "settings.json";
const SERVICE_PID_FILE: &str = "clevo-control-center.pid";
const SERVICE_LOCK_FILE: &str = "clevo-control-center.lock";
const SERVICE_LOG_FILE: &str = "clevo-control-center.service.log";
const HARDWARE_SNAPSHOT_FILE: &str = "hardware-status.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Mode,
    #[serde(default = "default_brightness_percent")]
    pub brightness: u8,
    pub f0_color: Rgb,
    #[serde(default = "default_zones")]
    pub zones: Vec<ZoneId>,
    #[serde(default)]
    pub fan_curves: FanCurveSettings,
    #[serde(default)]
    pub language: LanguagePreference,
    #[serde(default)]
    pub theme_color: ThemeColor,
    pub window_pos: Option<[f32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Custom,
            brightness: default_brightness_percent(),
            f0_color: Rgb::WHITE,
            zones: default_zones(),
            fan_curves: FanCurveSettings::default(),
            language: LanguagePreference::default(),
            theme_color: ThemeColor::default(),
            window_pos: None,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        self.brightness = self.brightness.clamp(1, 100);
        self.zones = normalize_zones(&self.zones);
        self.fan_curves = self.fan_curves.sanitized();
        if let Some([x, y]) = self.window_pos {
            if !x.is_finite() || !y.is_finite() {
                self.window_pos = None;
            }
        }
        self
    }

    pub fn lighting_config(&self) -> LightingConfig {
        LightingConfig {
            mode: self.mode,
            brightness_percent: self.brightness,
            color: self.f0_color,
            zones: normalize_zones(&self.zones),
        }
    }
}

const fn default_brightness_percent() -> u8 {
    100
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn uid_string() -> String {
    Command::new("id")
        .args(["-u"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|uid| !uid.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join(APP_ID)
}

pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp").join(format!("{APP_ID}-{}", uid_string())))
        .join(APP_ID)
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/state"))
        .join(APP_ID)
}

pub fn settings_path() -> PathBuf {
    let path = config_dir().join(SETTINGS_FILE);
    migrate_legacy_settings(&path);
    path
}

pub fn settings_path_and_first_run() -> (PathBuf, bool) {
    let path = settings_path();
    let first_run = is_first_run(&path);
    (path, first_run)
}

fn is_first_run(path: &Path) -> bool {
    !path.exists()
}

pub fn service_pid_path() -> PathBuf {
    runtime_dir().join(SERVICE_PID_FILE)
}

pub fn service_lock_path() -> PathBuf {
    runtime_dir().join(SERVICE_LOCK_FILE)
}

pub fn service_log_path() -> PathBuf {
    state_dir().join(SERVICE_LOG_FILE)
}

pub fn hardware_snapshot_path() -> PathBuf {
    runtime_dir().join(HARDWARE_SNAPSHOT_FILE)
}

fn migrate_legacy_settings(target: &Path) {
    if target.exists() {
        return;
    }

    let legacy_config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join(LEGACY_APP_ID)
        .join(SETTINGS_FILE);
    let legacy_cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SETTINGS_FILE);
    let legacy = [legacy_config, legacy_cwd]
        .into_iter()
        .find(|path| path.exists());
    let Some(legacy) = legacy else {
        return;
    };

    if let Some(parent) = target.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Failed to create settings directory {}: {err}",
                parent.display()
            );
            return;
        }
    }
    if let Err(err) = fs::copy(&legacy, target) {
        eprintln!(
            "Failed to migrate settings from {} to {}: {err}",
            legacy.display(),
            target.display()
        );
    }
}

pub fn load_settings(path: &Path) -> Settings {
    match fs::read_to_string(path)
        .ok()
        .and_then(|text| parse_settings(&text))
    {
        Some(settings) => settings.sanitized(),
        None => Settings::default(),
    }
}

fn parse_settings(text: &str) -> Option<Settings> {
    let mut value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let object = value.as_object_mut()?;
    if !object.contains_key("brightness") {
        let legacy_level = object
            .remove("brightness_level")
            .and_then(|value| value.as_u64())
            .unwrap_or(4)
            .clamp(1, 4);
        object.insert("brightness".to_owned(), (legacy_level * 25).into());
    }
    serde_json::from_value(value).ok()
}

pub fn file_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

pub fn atomic_write_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&tmp_path, format!("{json}\n"))?;
    fs::rename(tmp_path, path)
}

pub fn load_hardware_snapshot(path: &Path) -> Option<HardwareSnapshot> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<HardwareSnapshot>(&text).ok())
}

pub fn atomic_write_hardware_snapshot(path: &Path, snapshot: &HardwareSnapshot) -> io::Result<()> {
    let json = serde_json::to_string(snapshot).map_err(io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&tmp_path, format!("{json}\n"))?;
    fs::rename(tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{default_zones, Rgb, ZoneId};

    #[test]
    fn settings_json_roundtrips() {
        let settings = Settings {
            mode: Mode::Wave,
            brightness: 64,
            f0_color: Rgb {
                r: 12,
                g: 34,
                b: 56,
            },
            zones: vec![ZoneId::F0, ZoneId::F3, ZoneId::F6],
            fan_curves: FanCurveSettings {
                enabled: true,
                selected_profile: Some(1),
                ..FanCurveSettings::default()
            },
            language: LanguagePreference::English,
            theme_color: ThemeColor::Cyan,
            window_pos: Some([100.0, 200.0]),
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed = serde_json::from_str::<Settings>(&json).unwrap();

        assert_eq!(parsed.mode, Mode::Wave);
        assert_eq!(parsed.brightness, 64);
        assert_eq!(
            parsed.f0_color,
            Rgb {
                r: 12,
                g: 34,
                b: 56
            }
        );
        assert_eq!(parsed.zones, vec![ZoneId::F0, ZoneId::F3, ZoneId::F6]);
        assert!(parsed.fan_curves.enabled);
        assert_eq!(parsed.fan_curves.selected_profile, Some(1));
        assert_eq!(parsed.language, LanguagePreference::English);
        assert_eq!(parsed.theme_color, ThemeColor::Cyan);
        assert_eq!(parsed.window_pos, Some([100.0, 200.0]));
    }

    #[test]
    fn settings_sanitize_clamps_brightness_and_drops_bad_position() {
        let settings = Settings {
            mode: Mode::Custom,
            brightness: 0,
            f0_color: Rgb::WHITE,
            zones: Vec::new(),
            fan_curves: FanCurveSettings {
                enabled: false,
                selected_profile: Some(2),
                ..FanCurveSettings::default()
            },
            language: LanguagePreference::System,
            theme_color: ThemeColor::Amber,
            window_pos: Some([f32::NAN, 10.0]),
        }
        .sanitized();

        assert_eq!(settings.brightness, 1);
        assert_eq!(settings.zones, default_zones());
        assert_eq!(settings.fan_curves.selected_profile, None);
        assert_eq!(settings.window_pos, None);
    }

    #[test]
    fn legacy_lighting_fields_keep_continuous_brightness() {
        let json = r#"{
            "mode": "chase",
            "speed": 80,
            "brightness": 80,
            "running": true,
            "f0_color": { "r": 1, "g": 2, "b": 3 },
            "zones": ["f0"],
            "window_pos": null
        }"#;

        let settings = parse_settings(json).unwrap().sanitized();

        assert_eq!(settings.mode, Mode::Wave);
        assert_eq!(settings.brightness, 80);
        assert_eq!(settings.zones, vec![ZoneId::F0]);
    }

    #[test]
    fn api_four_brightness_level_migrates_to_percent() {
        let json = r#"{
            "mode": "cycle",
            "brightness_level": 3,
            "f0_color": { "r": 1, "g": 2, "b": 3 },
            "zones": ["f0", "f1", "f2"],
            "window_pos": null
        }"#;

        let settings = parse_settings(json).unwrap().sanitized();

        assert_eq!(settings.brightness, 75);
        assert_eq!(settings.language, LanguagePreference::System);
        assert_eq!(settings.theme_color, ThemeColor::Amber);
    }

    #[test]
    fn missing_settings_file_is_treated_as_first_run() {
        let path = std::env::temp_dir().join(format!(
            "clevo-control-center-first-run-{}-missing.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        assert!(is_first_run(&path));
        assert_eq!(load_settings(&path).mode, Settings::default().mode);
    }

    #[test]
    fn existing_settings_file_skips_first_run() {
        let path = std::env::temp_dir().join(format!(
            "clevo-control-center-first-run-{}-existing.json",
            std::process::id()
        ));
        fs::write(&path, "{}\n").unwrap();

        assert!(!is_first_run(&path));

        fs::remove_file(path).unwrap();
    }
}

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::dchu::HardwareSnapshot;
use crate::model::{default_zones, normalize_zones, Mode, Rgb, ZoneId};

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
    pub speed: u8,
    pub brightness: u8,
    pub running: bool,
    pub f0_color: Rgb,
    #[serde(default = "default_zones")]
    pub zones: Vec<ZoneId>,
    pub window_pos: Option<[f32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Custom,
            speed: 36,
            brightness: 100,
            running: false,
            f0_color: Rgb::WHITE,
            zones: default_zones(),
            window_pos: None,
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        self.speed = self.speed.clamp(1, 100);
        self.brightness = self.brightness.clamp(1, 100);
        self.zones = normalize_zones(&self.zones);
        if self.mode == Mode::Custom {
            self.running = false;
        }
        if let Some([x, y]) = self.window_pos {
            if !x.is_finite() || !y.is_finite() {
                self.window_pos = None;
            }
        }
        self
    }
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
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::copy(legacy, target);
}

pub fn load_settings(path: &Path) -> Settings {
    match fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
    {
        Some(settings) => settings.sanitized(),
        None => Settings::default(),
    }
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
            mode: Mode::Chase,
            speed: 72,
            brightness: 64,
            running: true,
            f0_color: Rgb {
                r: 12,
                g: 34,
                b: 56,
            },
            zones: vec![ZoneId::F0, ZoneId::F3, ZoneId::F6],
            window_pos: Some([100.0, 200.0]),
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed = serde_json::from_str::<Settings>(&json).unwrap();

        assert_eq!(parsed.mode, Mode::Chase);
        assert_eq!(parsed.speed, 72);
        assert_eq!(parsed.brightness, 64);
        assert!(parsed.running);
        assert_eq!(
            parsed.f0_color,
            Rgb {
                r: 12,
                g: 34,
                b: 56
            }
        );
        assert_eq!(parsed.zones, vec![ZoneId::F0, ZoneId::F3, ZoneId::F6]);
        assert_eq!(parsed.window_pos, Some([100.0, 200.0]));
    }

    #[test]
    fn settings_sanitize_clamps_speed_and_drops_bad_position() {
        let settings = Settings {
            mode: Mode::Custom,
            speed: 0,
            brightness: 0,
            running: true,
            f0_color: Rgb::WHITE,
            zones: Vec::new(),
            window_pos: Some([f32::NAN, 10.0]),
        }
        .sanitized();

        assert_eq!(settings.speed, 1);
        assert_eq!(settings.brightness, 1);
        assert!(!settings.running);
        assert_eq!(settings.zones, default_zones());
        assert_eq!(settings.window_pos, None);
    }
}

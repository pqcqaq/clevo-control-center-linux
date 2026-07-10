use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::fan_curve::{
    FanCurve, FanCurvePoint, FAN_CURVE_MAX_DUTY, FAN_CURVE_MAX_TEMP, FAN_CURVE_MIN_DUTY,
    FAN_CURVE_MIN_TEMP, FAN_CURVE_POINT_COUNT,
};

const DEFAULT_DCHU_CONTROL_PROC_PATH: &str = "/proc/clevo_dchu_control";
const DEFAULT_DCHU_CONFIG_PROC_PATH: &str = "/proc/clevo_dchu_config";
const DEFAULT_DCHU_STATUS_PROC_PATH: &str = "/proc/clevo_dchu_status";
// Clevo EC reports fan tach period counters; higher real RPM means smaller raw values.
const FAN_RPM_DIVISOR: u32 = 2_156_220;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FanModeOption {
    pub label: &'static str,
    pub value: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerModeOption {
    pub label: &'static str,
    pub value: &'static str,
}

const FAN_MODE_AUTO: FanModeOption = FanModeOption {
    label: "自动",
    value: "auto",
};
const FAN_MODE_MAX: FanModeOption = FanModeOption {
    label: "最大",
    value: "max",
};
const FAN_MODE_SILENT: FanModeOption = FanModeOption {
    label: "静音",
    value: "silent",
};
const FAN_MODE_MAXQ: FanModeOption = FanModeOption {
    label: "MaxQ",
    value: "maxq",
};

const FALLBACK_FAN_MODES: [FanModeOption; 2] = [FAN_MODE_AUTO, FAN_MODE_MAX];

const POWER_MODE_OPTIONS: [PowerModeOption; 4] = [
    PowerModeOption {
        label: "安静",
        value: "0",
    },
    PowerModeOption {
        label: "省电",
        value: "1",
    },
    PowerModeOption {
        label: "性能",
        value: "2",
    },
    PowerModeOption {
        label: "娱乐",
        value: "3",
    },
];
const NO_POWER_MODES: [PowerModeOption; 0] = [];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanStatus {
    pub label: String,
    #[serde(default)]
    pub raw_tach: u16,
    pub rpm: u32,
    #[serde(default)]
    pub temperature_celsius: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemperatureSensor {
    pub label: String,
    pub offset: usize,
    pub raw: u8,
    #[serde(default)]
    pub celsius: Option<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DchuConfig {
    #[serde(default)]
    pub fanq: Option<u8>,
    #[serde(default)]
    pub mode_status: Option<u8>,
    #[serde(default)]
    pub kbtp: Option<u8>,
    #[serde(default)]
    pub psf1: Option<u32>,
    #[serde(default)]
    pub psf2: Option<u32>,
    #[serde(default)]
    pub psf4: Option<u32>,
    #[serde(default)]
    pub psf5: Option<u32>,
    #[serde(default)]
    pub bios_feature_version: Option<u16>,
    #[serde(default)]
    pub bios_feature_offset18: Option<u8>,
    #[serde(default)]
    pub gpu_mux_current: Option<u8>,
    #[serde(default)]
    pub gpu_mux_options: Option<u8>,
    #[serde(default)]
    pub app_power_mode: Option<u8>,
    #[serde(default)]
    pub app_fan_mode: Option<u8>,
    #[serde(default)]
    pub raw_config: Vec<u8>,
}

impl DchuConfig {
    pub fn fan_count(&self) -> Option<u8> {
        self.raw_config.get(0x0c).copied().or(self.fanq)
    }

    pub fn init_fan_mode(&self) -> Option<u8> {
        self.raw_config.get(0x0e).copied().or(self.mode_status)
    }

    pub fn power_mode_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 0)
    }

    pub fn fan_speed_setting_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 7)
    }

    pub fn silent_fan_capability(&self) -> Option<bool> {
        capability_bit(self.psf2, 15)
    }

    pub fn maxq_fan_capability(&self) -> Option<bool> {
        self.init_fan_mode().map(|mode| mode == 5)
    }

    pub fn custom_fan_table_capability(&self) -> Option<bool> {
        let fan_setting = self.fan_speed_setting_capability()?;
        let fan_count = self.fan_count()?;
        let custom_disabled = self.custom_fan_disabled_by_config()?;
        Some(fan_setting && fan_count > 1 && !custom_disabled)
    }

    pub fn legacy_gpu_mux_capability(&self) -> Option<bool> {
        capability_bit(self.psf2, 20)
    }

    pub fn new_gpu_mux_capability(&self) -> Option<bool> {
        self.bios_feature_offset18.map(|value| value & 0x01 != 0)
    }

    pub fn gpu_mux_capability(&self) -> Option<bool> {
        any_known_capability(&[
            self.legacy_gpu_mux_capability(),
            self.new_gpu_mux_capability(),
        ])
    }

    pub fn gpu_oc_capability(&self) -> Option<bool> {
        any_known_capability(&[
            capability_bit(self.psf5, 5),
            capability_bit(self.psf2, 26),
            capability_bit(self.psf2, 27),
        ])
    }

    pub fn cpu_oc_capability(&self) -> Option<bool> {
        any_known_capability(&[capability_bit(self.psf5, 6), capability_bit(self.psf2, 23)])
    }

    pub fn xmp_capability(&self) -> Option<bool> {
        capability_bit(self.psf2, 24)
    }

    pub fn energy_save_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 8)
    }

    pub fn battery_utility_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 9)
    }

    pub fn anti_dust_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 7)
    }

    pub fn fan_offset_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 10).map(|disabled| !disabled)
    }

    pub fn dtt_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 12)
    }

    fn supports_power_mode_ui(&self) -> bool {
        self.power_mode_capability().unwrap_or(true)
    }

    fn supports_fan_mode_ui(&self) -> bool {
        self.fan_speed_setting_capability().unwrap_or(true)
    }

    fn custom_fan_disabled_by_config(&self) -> Option<bool> {
        self.raw_config
            .get(0x2b)
            .map(|value| ((value >> 1) & 1) == 1)
    }
}

fn capability_bit(value: Option<u32>, bit: u32) -> Option<bool> {
    value.map(|value| (value & (1u32 << bit)) != 0)
}

fn any_known_capability(values: &[Option<bool>]) -> Option<bool> {
    if values.iter().any(|value| *value == Some(true)) {
        Some(true)
    } else if values.iter().all(Option::is_some) {
        Some(false)
    } else {
        None
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub fans: Vec<FanStatus>,
    #[serde(default)]
    pub temperature_sensors: Vec<TemperatureSensor>,
    #[serde(default)]
    pub raw_status: Vec<u8>,
    #[serde(default)]
    pub dchu_config: Option<DchuConfig>,
    pub battery_voltage_raw: u16,
    pub battery_rate_raw: u16,
    pub thermal_raw: [u8; 4],
    pub updated_unix_secs: u64,
}

impl HardwareSnapshot {
    pub fn from_status_bytes(bytes: &[u8]) -> Self {
        let mut fans = ["CPU 风扇", "GPU 风扇"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| status_fan(bytes, index, label))
            .collect::<Vec<_>>();
        let pch_tach = get_be_u16(bytes, 0x06);
        if pch_tach > 0 {
            fans.push(status_fan(bytes, 2, "PCH 风扇"));
        }

        Self {
            fans,
            temperature_sensors: temperature_sensors(bytes),
            raw_status: bytes.to_vec(),
            dchu_config: None,
            battery_voltage_raw: get_be_u16(bytes, 0x08),
            battery_rate_raw: get_be_u16(bytes, 0x0e),
            thermal_raw: [
                bytes.get(0x10).copied().unwrap_or_default(),
                bytes.get(0x11).copied().unwrap_or_default(),
                bytes.get(0x12).copied().unwrap_or_default(),
                bytes.get(0x13).copied().unwrap_or_default(),
            ],
            updated_unix_secs: unix_secs_now(),
        }
    }
}

fn status_fan(bytes: &[u8], index: usize, label: &str) -> FanStatus {
    let raw_tach = get_be_u16(bytes, 0x02 + index * 2);
    FanStatus {
        label: label.to_owned(),
        raw_tach,
        rpm: fan_rpm_from_tach(raw_tach),
        temperature_celsius: fan_temperature(bytes, index),
    }
}

fn unix_secs_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn dchu_control_proc_path() -> std::path::PathBuf {
    std::env::var_os("CLEVO_DCHU_CONTROL_PROC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DCHU_CONTROL_PROC_PATH))
}

fn dchu_config_proc_path() -> std::path::PathBuf {
    std::env::var_os("CLEVO_DCHU_CONFIG_PROC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DCHU_CONFIG_PROC_PATH))
}

fn dchu_status_proc_path() -> std::path::PathBuf {
    std::env::var_os("CLEVO_DCHU_STATUS_PROC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DCHU_STATUS_PROC_PATH))
}

fn dchu_control_write(command: &str) -> Result<(), String> {
    let path = dchu_control_proc_path();
    fs::write(&path, format!("{command}\n"))
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

pub fn parse_hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let compact = value
        .split_whitespace()
        .collect::<String>()
        .replace('_', "")
        .replace(':', "");
    if compact.len() % 2 != 0 {
        return Err("hex byte list length must be even".to_owned());
    }

    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| format!("invalid hex byte near offset {index}"))
        })
        .collect()
}

pub fn parse_dchu_buffer_reply(text: &str) -> Result<Vec<u8>, String> {
    let mut lines = text.lines();
    let Some(first) = lines.next().map(str::trim) else {
        return Err("empty DCHU reply".to_owned());
    };

    if first.starts_with("buffer ") {
        let hex = lines.collect::<Vec<_>>().join(" ");
        return parse_hex_bytes(&hex);
    }

    Err(format!("DCHU status did not return buffer: {first}"))
}

pub fn parse_dchu_config_reply(text: &str) -> Result<DchuConfig, String> {
    let mut config = DchuConfig::default();
    let mut hex_lines = Vec::new();

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("config_0d buffer ") {
            continue;
        }
        if let Some(value) = line.strip_prefix("psf1_52 integer 0x") {
            config.psf1 = Some(parse_u32_hex(value, "psf1_52")?);
            continue;
        }
        if let Some(value) = line.strip_prefix("psf2_7a integer 0x") {
            config.psf2 = Some(parse_u32_hex(value, "psf2_7a")?);
            continue;
        }
        if let Some(value) = line.strip_prefix("psf4_60 integer 0x") {
            config.psf4 = Some(parse_u32_hex(value, "psf4_60")?);
            continue;
        }
        if let Some(value) = line.strip_prefix("psf5_10 integer 0x") {
            config.psf5 = Some(parse_u32_hex(value, "psf5_10")?);
            continue;
        }
        if let Some(value) = line.strip_prefix("bios_feature_04_08_version ") {
            config.bios_feature_version =
                parse_optional_u16_hex(value, "bios_feature_04_08_version")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("bios_feature_04_08_offset18 ") {
            config.bios_feature_offset18 =
                parse_optional_u8_hex(value, "bios_feature_04_08_offset18")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("gpu_mux_04_15_current ") {
            config.gpu_mux_current = parse_optional_u8_hex(value, "gpu_mux_04_15_current")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("gpu_mux_04_15_options ") {
            config.gpu_mux_options = parse_optional_u8_hex(value, "gpu_mux_04_15_options")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("app_power_mode ") {
            config.app_power_mode = parse_optional_u8(value, "app_power_mode")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("app_fan_mode ") {
            config.app_fan_mode = parse_optional_u8(value, "app_fan_mode")?;
            continue;
        }
        if line.starts_with("psf") {
            continue;
        }
        hex_lines.push(line);
    }

    if hex_lines.is_empty() {
        return Err("DCHU config did not return config_0d buffer".to_owned());
    }

    config.raw_config = parse_hex_bytes(&hex_lines.join(" "))?;
    config.fanq = config.raw_config.get(0x0c).copied();
    config.mode_status = config.raw_config.get(0x0e).copied();
    config.kbtp = config.raw_config.get(0x0f).copied();
    Ok(config)
}

fn parse_u32_hex(value: &str, label: &str) -> Result<u32, String> {
    u32::from_str_radix(value.trim(), 16).map_err(|_| format!("invalid {label} integer"))
}

fn parse_optional_u16_hex(value: &str, label: &str) -> Result<Option<u16>, String> {
    let Some(value) = value.trim().strip_prefix("integer 0x") else {
        if value.trim() == "unknown" {
            return Ok(None);
        }
        return Err(format!("invalid {label} value"));
    };
    u16::from_str_radix(value, 16)
        .map(Some)
        .map_err(|_| format!("invalid {label} integer"))
}

fn parse_optional_u8_hex(value: &str, label: &str) -> Result<Option<u8>, String> {
    let Some(value) = value.trim().strip_prefix("integer 0x") else {
        if value.trim() == "unknown" {
            return Ok(None);
        }
        return Err(format!("invalid {label} value"));
    };
    u8::from_str_radix(value, 16)
        .map(Some)
        .map_err(|_| format!("invalid {label} integer"))
}

fn parse_optional_u8(value: &str, label: &str) -> Result<Option<u8>, String> {
    let value = value.trim();
    if value == "unknown" {
        return Ok(None);
    }
    value
        .parse::<u8>()
        .map(Some)
        .map_err(|_| format!("invalid {label} value"))
}

fn dchu_status_buffer() -> Result<Vec<u8>, String> {
    let path = dchu_status_proc_path();
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_buffer_reply(&text)
}

fn read_dchu_config() -> Result<DchuConfig, String> {
    let path = dchu_config_proc_path();
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_config_reply(&text)
}

pub fn read_hardware_snapshot() -> Result<HardwareSnapshot, String> {
    let status = dchu_status_buffer()?;
    let mut snapshot = HardwareSnapshot::from_status_bytes(&status);
    snapshot.dchu_config = read_dchu_config().ok();
    Ok(snapshot)
}

fn get_be_u16(bytes: &[u8], offset: usize) -> u16 {
    let hi = bytes.get(offset).copied().unwrap_or_default() as u16;
    let lo = bytes.get(offset + 1).copied().unwrap_or_default() as u16;
    (hi << 8) | lo
}

fn print_status(bytes: &[u8]) {
    println!("DCHU 0x0C status");
    println!("rpm1: {}", fan_rpm_from_tach(get_be_u16(bytes, 0x02)));
    println!("rpm2: {}", fan_rpm_from_tach(get_be_u16(bytes, 0x04)));
    println!("rpm3: {}", fan_rpm_from_tach(get_be_u16(bytes, 0x06)));
    println!(
        "cpu_temperature_celsius: {}",
        fan_temperature(bytes, 0)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "--".to_owned())
    );
    println!(
        "gpu_temperature_celsius: {}",
        fan_temperature(bytes, 1)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "--".to_owned())
    );
    if get_be_u16(bytes, 0x06) > 0 {
        println!(
            "pch_temperature_celsius: {}",
            fan_temperature(bytes, 2)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "--".to_owned())
        );
    }
    println!("battery_voltage_raw: {}", get_be_u16(bytes, 0x08));
    println!("battery_rate_raw: {}", get_be_u16(bytes, 0x0e));
    println!(
        "thermal_raw_10: 0x{:02x}",
        bytes.get(0x10).copied().unwrap_or_default()
    );
    println!(
        "thermal_raw_11: 0x{:02x}",
        bytes.get(0x11).copied().unwrap_or_default()
    );
    println!(
        "thermal_raw_12: 0x{:02x}",
        bytes.get(0x12).copied().unwrap_or_default()
    );
    println!(
        "thermal_raw_13: 0x{:02x}",
        bytes.get(0x13).copied().unwrap_or_default()
    );
}

pub fn available_fan_modes(snapshot: Option<&HardwareSnapshot>) -> Vec<FanModeOption> {
    let Some(config) = snapshot.and_then(|snapshot| snapshot.dchu_config.as_ref()) else {
        return FALLBACK_FAN_MODES.to_vec();
    };

    if !config.supports_fan_mode_ui() {
        return Vec::new();
    }

    let mut modes = vec![FAN_MODE_AUTO, FAN_MODE_MAX];
    if config.silent_fan_capability().unwrap_or(false) {
        modes.push(FAN_MODE_SILENT);
    }
    if config.maxq_fan_capability().unwrap_or(false) {
        modes.push(FAN_MODE_MAXQ);
    }
    modes
}

pub fn available_power_modes(snapshot: Option<&HardwareSnapshot>) -> &'static [PowerModeOption] {
    let Some(config) = snapshot.and_then(|snapshot| snapshot.dchu_config.as_ref()) else {
        return &POWER_MODE_OPTIONS;
    };

    if config.supports_power_mode_ui() {
        &POWER_MODE_OPTIONS
    } else {
        &NO_POWER_MODES
    }
}

pub fn selected_fan_mode_from_snapshot(
    snapshot: Option<&HardwareSnapshot>,
) -> Option<&'static str> {
    match snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .and_then(|config| config.app_fan_mode)
    {
        Some(0) => Some("auto"),
        Some(1) => Some("max"),
        Some(3) => Some("silent"),
        Some(5) => Some("maxq"),
        Some(6) => Some("custom"),
        _ => None,
    }
}

pub fn selected_power_mode_from_snapshot(
    snapshot: Option<&HardwareSnapshot>,
) -> Option<&'static str> {
    match snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .and_then(|config| config.app_power_mode)
    {
        Some(0) => Some("0"),
        Some(1) => Some("1"),
        Some(2) => Some("2"),
        Some(3) => Some("3"),
        _ => None,
    }
}

fn require_danger_flag(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--i-understand") {
        Ok(())
    } else {
        Err("dangerous write requires --i-understand".to_owned())
    }
}

pub fn print_dchu_usage() {
    println!("Usage:");
    println!("  clevo-control-center dchu status");
    println!("  clevo-control-center dchu app-settings");
    println!(
        "  clevo-control-center dchu fan-mode <auto|max|silent|maxq|custom|0|1|3|5|6> --i-understand"
    );
    println!("  clevo-control-center dchu power-mode <0..3> --i-understand");
    println!(
        "  clevo-control-center dchu fan-curve <cpu t:d,t:d,t:d,t:d> <gpu t:d,t:d,t:d,t:d> --i-understand"
    );
}

pub fn parse_fan_mode(value: &str) -> Result<u32, String> {
    match value {
        "auto" => Ok(0),
        "max" => Ok(1),
        "silent" => Ok(3),
        "maxq" => Ok(5),
        "custom" => Ok(6),
        _ => {
            let mode = value
                .parse::<u32>()
                .map_err(|_| "fan mode must be a known name or decimal value".to_owned())?;
            match mode {
                0 | 1 | 3 | 5 | 6 => Ok(mode),
                _ => Err(
                    "fan mode must be one of auto/max/silent/maxq/custom or 0/1/3/5/6".to_owned(),
                ),
            }
        }
    }
}

pub fn fan_rpm_from_tach(raw_tach: u16) -> u32 {
    if raw_tach == 0 {
        0
    } else {
        FAN_RPM_DIVISOR / raw_tach as u32
    }
}

fn fan_temperature(bytes: &[u8], index: usize) -> Option<u8> {
    let value = match index {
        0 => bytes.get(0x11).copied().unwrap_or_default(),
        1 => bytes.get(0x12).copied().unwrap_or_default(),
        2 => bytes.get(0x13).copied().unwrap_or_default(),
        _ => 0,
    };

    plausible_temperature(value)
}

fn temperature_sensors(bytes: &[u8]) -> Vec<TemperatureSensor> {
    (0x10..=0x15)
        .map(|offset| {
            let raw = bytes.get(offset).copied().unwrap_or_default();
            TemperatureSensor {
                label: temperature_sensor_label(offset).to_owned(),
                offset,
                raw,
                celsius: plausible_temperature(raw),
            }
        })
        .collect()
}

fn temperature_sensor_label(offset: usize) -> &'static str {
    match offset {
        0x11 => "CPU 温度",
        0x12 => "GPU 温度",
        0x13 => "第三路温度/PCH 候选",
        0x10 | 0x14 | 0x15 => "EC 温度传感器",
        _ => "未知温度传感器",
    }
}

fn plausible_temperature(value: u8) -> Option<u8> {
    match value {
        1..=125 => Some(value),
        _ => None,
    }
}

fn run_fan_mode(value: &str) -> Result<(), String> {
    parse_fan_mode(value)?;
    dchu_control_write(&format!("fan-mode {value}"))?;
    println!("fan mode set via /proc/clevo_dchu_control");
    Ok(())
}

fn run_power_mode(value: &str) -> Result<(), String> {
    parse_power_mode(value)?;
    dchu_control_write(&format!("power-mode {value}"))?;
    println!("power mode set via /proc/clevo_dchu_control");
    Ok(())
}

fn run_fan_curve(cpu_value: &str, gpu_value: &str) -> Result<(), String> {
    let cpu_points = parse_fan_curve_points_arg(cpu_value)?;
    let gpu_points = parse_fan_curve_points_arg(gpu_value)?;
    let cpu_arg = format_fan_curve_points_arg(&cpu_points);
    let gpu_arg = format_fan_curve_points_arg(&gpu_points);

    dchu_control_write(&format!("fan-curve {cpu_arg} {gpu_arg}"))?;
    println!("fan curve set via /proc/clevo_dchu_control");
    Ok(())
}

fn print_app_settings() -> Result<(), String> {
    let config = read_dchu_config()?;
    println!(
        "power_mode: {}",
        config
            .app_power_mode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    println!(
        "fan_mode: {}",
        config
            .app_fan_mode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    Ok(())
}

pub fn parse_power_mode(value: &str) -> Result<u32, String> {
    let mode = value
        .parse::<u32>()
        .map_err(|_| "power-mode must be 0..3".to_owned())?;
    if mode > 3 {
        return Err("power-mode must be 0..3".to_owned());
    }
    Ok(mode)
}

pub fn fan_curve_points_arg(curve: &FanCurve) -> Result<String, String> {
    let curve = curve.clone().sanitized();
    validate_fan_curve_points(&curve.points)?;
    Ok(format_fan_curve_points_arg(&curve.points))
}

fn parse_fan_curve_points_arg(value: &str) -> Result<Vec<FanCurvePoint>, String> {
    let points = value
        .split(',')
        .map(parse_fan_curve_point_arg)
        .collect::<Result<Vec<_>, _>>()?;
    validate_fan_curve_points(&points)?;
    Ok(points)
}

fn parse_fan_curve_point_arg(value: &str) -> Result<FanCurvePoint, String> {
    let Some((temp, duty)) = value.split_once(':') else {
        return Err("fan curve point must use temp:duty".to_owned());
    };
    let temp_celsius = temp
        .parse::<u8>()
        .map_err(|_| "fan curve temperature must be decimal 30..100".to_owned())?;
    let duty_percent = duty
        .parse::<u8>()
        .map_err(|_| "fan curve duty must be decimal 0..100".to_owned())?;

    Ok(FanCurvePoint {
        temp_celsius,
        duty_percent,
    })
}

fn validate_fan_curve_points(points: &[FanCurvePoint]) -> Result<(), String> {
    if points.len() != FAN_CURVE_POINT_COUNT {
        return Err(format!(
            "fan curve must contain exactly {FAN_CURVE_POINT_COUNT} points"
        ));
    }

    for (index, point) in points.iter().enumerate() {
        if !(FAN_CURVE_MIN_TEMP..=FAN_CURVE_MAX_TEMP).contains(&point.temp_celsius) {
            return Err(format!(
                "fan curve point {} temperature is out of range",
                index + 1
            ));
        }
        if !(FAN_CURVE_MIN_DUTY..=FAN_CURVE_MAX_DUTY).contains(&point.duty_percent) {
            return Err(format!(
                "fan curve point {} duty is out of range",
                index + 1
            ));
        }
        if let Some(previous) = index.checked_sub(1).and_then(|prev| points.get(prev)) {
            if point.temp_celsius <= previous.temp_celsius {
                return Err("fan curve temperatures must increase from left to right".to_owned());
            }
            if point.duty_percent < previous.duty_percent {
                return Err("fan curve duty must not decrease".to_owned());
            }
        }
    }

    Ok(())
}

fn format_fan_curve_points_arg(points: &[FanCurvePoint]) -> String {
    points
        .iter()
        .map(|point| format!("{}:{}", point.temp_celsius, point.duty_percent))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn run_dchu_cli(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_dchu_usage();
        return Ok(());
    };

    match command {
        "status" => print_status(&dchu_status_buffer()?),
        "app-settings" => print_app_settings()?,
        "fan-mode" => {
            require_danger_flag(args)?;
            run_fan_mode(args.get(1).ok_or("fan-mode requires <mode>")?)?;
        }
        "power-mode" => {
            require_danger_flag(args)?;
            run_power_mode(args.get(1).ok_or("power-mode requires <0..3>")?)?;
        }
        "fan-curve" => {
            require_danger_flag(args)?;
            run_fan_curve(
                args.get(1).ok_or("fan-curve requires <cpu points>")?,
                args.get(2).ok_or("fan-curve requires <gpu points>")?,
            )?;
        }
        "help" | "--help" | "-h" => print_dchu_usage(),
        _ => return Err(format!("unknown dchu command: {command}")),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_snapshot_uses_two_primary_fans() {
        let mut bytes = vec![0; 0x20];
        bytes[0x02] = 0x03;
        bytes[0x03] = 0xde;
        bytes[0x04] = 0x04;
        bytes[0x05] = 0x0e;

        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

        assert_eq!(snapshot.fans.len(), 2);
        assert_eq!(snapshot.fans[0].raw_tach, 990);
        assert_eq!(snapshot.fans[0].rpm, 2178);
        assert_eq!(snapshot.fans[1].raw_tach, 1038);
        assert_eq!(snapshot.fans[1].rpm, 2077);
    }

    #[test]
    fn fan_rpm_uses_inverse_tach_counter_formula() {
        assert_eq!(fan_rpm_from_tach(0), 0);
        assert_eq!(fan_rpm_from_tach(990), 2178);
        assert!(fan_rpm_from_tach(800) > fan_rpm_from_tach(1200));
    }

    #[test]
    fn hardware_snapshot_maps_cpu_and_gpu_temperatures() {
        let mut bytes = vec![0; 0x20];
        bytes[0x10] = 81;
        bytes[0x11] = 43;
        bytes[0x12] = 47;
        bytes[0x13] = 82;
        bytes[0x14] = 48;
        bytes[0x15] = 49;

        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

        assert_eq!(snapshot.fans[0].temperature_celsius, Some(43));
        assert_eq!(snapshot.fans[1].temperature_celsius, Some(47));
        assert_eq!(snapshot.raw_status, bytes);
        assert_eq!(snapshot.temperature_sensors.len(), 6);
        assert_eq!(snapshot.temperature_sensors[1].label, "CPU 温度");
        assert_eq!(snapshot.temperature_sensors[1].offset, 0x11);
        assert_eq!(snapshot.temperature_sensors[1].raw, 43);
        assert_eq!(snapshot.temperature_sensors[1].celsius, Some(43));
        assert_eq!(snapshot.temperature_sensors[5].offset, 0x15);
        assert_eq!(snapshot.temperature_sensors[5].celsius, Some(49));
    }

    #[test]
    fn hardware_snapshot_adds_pch_fan_only_when_third_tach_has_data() {
        let mut bytes = vec![0; 0x20];
        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);
        assert_eq!(snapshot.fans.len(), 2);

        bytes[0x06] = 0x07;
        bytes[0x07] = 0x08;
        bytes[0x13] = 51;
        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

        assert_eq!(snapshot.fans.len(), 3);
        assert_eq!(snapshot.fans[2].label, "PCH 风扇");
        assert_eq!(snapshot.fans[2].rpm, 1197);
        assert_eq!(snapshot.fans[2].temperature_celsius, Some(51));
    }

    #[test]
    fn parses_dchu_fan_modes() {
        assert_eq!(parse_fan_mode("auto").unwrap(), 0);
        assert_eq!(parse_fan_mode("max").unwrap(), 1);
        assert_eq!(parse_fan_mode("silent").unwrap(), 3);
        assert_eq!(parse_fan_mode("maxq").unwrap(), 5);
        assert_eq!(parse_fan_mode("custom").unwrap(), 6);
        assert_eq!(parse_fan_mode("0").unwrap(), 0);
        assert_eq!(parse_fan_mode("3").unwrap(), 3);
        assert!(parse_fan_mode("2").is_err());
        assert!(parse_fan_mode("7").is_err());
        assert!(parse_fan_mode("turbo").is_err());
        assert!(parse_fan_mode("0x1").is_err());
    }

    #[test]
    fn parses_dchu_config_reply() {
        let config = parse_dchu_config_reply(
            "config_0d buffer 32\n\
             00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 06\n\
             10 20 30 40 50 60 70 80 00 00 00 00 00 00 00 00\n\
             psf5_10 integer 0x93\n\
             psf1_52 integer 0x4680025\n\
             psf4_60 integer 0x21c\n\
             psf2_7a integer 0x70020053\n\
             bios_feature_04_08_version integer 0x0100\n\
             bios_feature_04_08_offset18 integer 0x01\n\
             gpu_mux_04_15_current integer 0x03\n\
             gpu_mux_04_15_options integer 0x07\n\
             app_power_mode 2\n\
             app_fan_mode 3\n",
        )
        .unwrap();

        assert_eq!(config.fanq, Some(0x02));
        assert_eq!(config.mode_status, Some(0x00));
        assert_eq!(config.kbtp, Some(0x06));
        assert_eq!(config.psf5, Some(0x93));
        assert_eq!(config.psf1, Some(0x0468_0025));
        assert_eq!(config.psf4, Some(0x021c));
        assert_eq!(config.psf2, Some(0x7002_0053));
        assert_eq!(config.bios_feature_version, Some(0x0100));
        assert_eq!(config.bios_feature_offset18, Some(0x01));
        assert_eq!(config.gpu_mux_current, Some(0x03));
        assert_eq!(config.gpu_mux_options, Some(0x07));
        assert_eq!(config.app_power_mode, Some(2));
        assert_eq!(config.app_fan_mode, Some(3));
        assert_eq!(config.raw_config.len(), 32);
    }

    #[test]
    fn derives_oem_capabilities_from_psf_and_config_buffer() {
        let config = dchu_config_with_raw(0x0000_03e1, 0x0d90_8000, 0x0000_1480, 2, 5, 0x00);

        assert_eq!(config.fan_count(), Some(2));
        assert_eq!(config.init_fan_mode(), Some(5));
        assert_eq!(config.power_mode_capability(), Some(true));
        assert_eq!(config.fan_speed_setting_capability(), Some(true));
        assert_eq!(config.silent_fan_capability(), Some(true));
        assert_eq!(config.maxq_fan_capability(), Some(true));
        assert_eq!(config.custom_fan_table_capability(), Some(true));
        assert_eq!(config.legacy_gpu_mux_capability(), Some(true));
        assert_eq!(config.gpu_mux_capability(), Some(true));
        assert_eq!(config.cpu_oc_capability(), Some(true));
        assert_eq!(config.xmp_capability(), Some(true));
        assert_eq!(config.gpu_oc_capability(), Some(true));
        assert_eq!(config.energy_save_capability(), Some(true));
        assert_eq!(config.battery_utility_capability(), Some(true));
        assert_eq!(config.anti_dust_capability(), Some(true));
        assert_eq!(config.fan_offset_capability(), Some(false));
        assert_eq!(config.dtt_capability(), Some(true));
    }

    #[test]
    fn fan_modes_follow_oem_visibility_bits() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(
            0x0000_0081,
            0x0000_8000,
            0,
            2,
            5,
            0x00,
        ));

        assert_eq!(
            fan_mode_values(&available_fan_modes(Some(&snapshot))),
            vec!["auto", "max", "silent", "maxq"]
        );
    }

    #[test]
    fn silent_fan_mode_is_hidden_without_fanless_capability() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(
            0x0000_0081,
            0x0000_0000,
            0,
            2,
            5,
            0x00,
        ));

        assert_eq!(
            fan_mode_values(&available_fan_modes(Some(&snapshot))),
            vec!["auto", "max", "maxq"]
        );
    }

    #[test]
    fn maxq_fan_mode_is_hidden_without_init_fan_mode_five() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(
            0x0000_0081,
            0x0000_8000,
            0,
            2,
            0,
            0x00,
        ));

        assert_eq!(
            fan_mode_values(&available_fan_modes(Some(&snapshot))),
            vec!["auto", "max", "silent"]
        );
    }

    #[test]
    fn custom_fan_table_capability_does_not_create_write_mode_button() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(
            0x0000_0081,
            0x0000_8000,
            0,
            2,
            5,
            0x00,
        ));

        assert_eq!(
            snapshot
                .dchu_config
                .as_ref()
                .unwrap()
                .custom_fan_table_capability(),
            Some(true)
        );
        assert!(!fan_mode_values(&available_fan_modes(Some(&snapshot))).contains(&"custom"));
    }

    #[test]
    fn fan_mode_controls_hide_when_fan_setting_capability_is_absent() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(
            0x0000_0001,
            0x0000_8000,
            0,
            2,
            5,
            0x00,
        ));

        assert!(available_fan_modes(Some(&snapshot)).is_empty());
    }

    #[test]
    fn power_mode_controls_hide_when_power_capability_is_absent() {
        let snapshot = snapshot_with_config(dchu_config_with_raw(0x0000_0080, 0, 0, 2, 5, 0x00));

        assert!(available_power_modes(Some(&snapshot)).is_empty());
    }

    #[test]
    fn unavailable_config_keeps_safe_control_fallbacks() {
        assert_eq!(
            fan_mode_values(&available_fan_modes(None)),
            vec!["auto", "max"]
        );
        assert_eq!(
            power_mode_values(available_power_modes(None)),
            vec!["0", "1", "2", "3"]
        );
    }

    #[test]
    fn parses_limited_power_modes() {
        assert_eq!(parse_power_mode("0").unwrap(), 0);
        assert_eq!(parse_power_mode("3").unwrap(), 3);
        assert!(parse_power_mode("4").is_err());
        assert!(parse_power_mode("0x2").is_err());
        assert!(parse_power_mode("raw-data").is_err());
    }

    #[test]
    fn formats_and_parses_fan_curve_points_arg() {
        let curve = FanCurve {
            points: vec![
                FanCurvePoint {
                    temp_celsius: 40,
                    duty_percent: 28,
                },
                FanCurvePoint {
                    temp_celsius: 58,
                    duty_percent: 42,
                },
                FanCurvePoint {
                    temp_celsius: 78,
                    duty_percent: 72,
                },
                FanCurvePoint {
                    temp_celsius: 100,
                    duty_percent: 100,
                },
            ],
        };

        let value = fan_curve_points_arg(&curve).unwrap();
        let parsed = parse_fan_curve_points_arg(&value).unwrap();

        assert_eq!(value, "40:28,58:42,78:72,100:100");
        assert_eq!(parsed, curve.points);
    }

    #[test]
    fn rejects_invalid_fan_curve_points_arg() {
        assert!(parse_fan_curve_points_arg("40:20,58:30,80:70").is_err());
        assert!(parse_fan_curve_points_arg("40:20,58:30,58:70,100:100").is_err());
        assert!(parse_fan_curve_points_arg("40:20,58:30,80:10,100:100").is_err());
        assert!(parse_fan_curve_points_arg("40:20,58:30,80:70,120:100").is_err());
    }

    #[test]
    fn selects_fan_mode_from_app_settings_only() {
        let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x10),
            ..DchuConfig::default()
        });
        assert_eq!(selected_fan_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x08),
            app_fan_mode: Some(3),
            ..DchuConfig::default()
        });
        assert_eq!(
            selected_fan_mode_from_snapshot(Some(&snapshot)),
            Some("silent")
        );

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x02),
            app_fan_mode: Some(5),
            ..DchuConfig::default()
        });
        assert_eq!(
            selected_fan_mode_from_snapshot(Some(&snapshot)),
            Some("maxq")
        );
    }

    #[test]
    fn selects_power_mode_from_app_settings_only() {
        let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x80),
            ..DchuConfig::default()
        });
        assert_eq!(selected_power_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x08),
            app_power_mode: Some(2),
            ..DchuConfig::default()
        });
        assert_eq!(
            selected_power_mode_from_snapshot(Some(&snapshot)),
            Some("2")
        );

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x02),
            app_power_mode: Some(3),
            ..DchuConfig::default()
        });
        assert_eq!(
            selected_power_mode_from_snapshot(Some(&snapshot)),
            Some("3")
        );
    }

    #[test]
    fn parses_status_buffer_reply_only() {
        let parsed = parse_dchu_buffer_reply("buffer 4\n01 02 0a ff\n").unwrap();
        assert_eq!(parsed, vec![0x01, 0x02, 0x0a, 0xff]);
        assert!(parse_dchu_buffer_reply("integer 0x79\n").is_err());
    }

    fn dchu_config_with_raw(
        psf5: u32,
        psf2: u32,
        psf4: u32,
        fan_count: u8,
        init_fan_mode: u8,
        custom_flags: u8,
    ) -> DchuConfig {
        let mut raw_config = vec![0; 0x2c];
        raw_config[0x0c] = fan_count;
        raw_config[0x0e] = init_fan_mode;
        raw_config[0x2b] = custom_flags;
        DchuConfig {
            fanq: Some(fan_count),
            mode_status: Some(init_fan_mode),
            psf2: Some(psf2),
            psf4: Some(psf4),
            psf5: Some(psf5),
            raw_config,
            ..DchuConfig::default()
        }
    }

    fn snapshot_with_config(config: DchuConfig) -> HardwareSnapshot {
        let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
        snapshot.dchu_config = Some(config);
        snapshot
    }

    fn fan_mode_values(modes: &[FanModeOption]) -> Vec<&'static str> {
        modes.iter().map(|mode| mode.value).collect()
    }

    fn power_mode_values(modes: &[PowerModeOption]) -> Vec<&'static str> {
        modes.iter().map(|mode| mode.value).collect()
    }
}

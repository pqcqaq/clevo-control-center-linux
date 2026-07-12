use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{DchuConfig, FanStatus, HardwareSnapshot, TemperatureSensor};
use crate::fan_curve::{
    FanCurve, FanCurvePoint, FAN_CURVE_MAX_DUTY, FAN_CURVE_MAX_TEMP, FAN_CURVE_MIN_DUTY,
    FAN_CURVE_MIN_TEMP, FAN_CURVE_POINT_COUNT,
};

const DEFAULT_DCHU_CONTROL_PROC_PATH: &str = "/proc/clevo_dchu_control";
const DEFAULT_DCHU_CONFIG_PROC_PATH: &str = "/proc/clevo_dchu_config";
const DEFAULT_DCHU_STATUS_PROC_PATH: &str = "/proc/clevo_dchu_status";
// Clevo EC reports fan tach period counters; higher real RPM means smaller raw values.
const FAN_RPM_DIVISOR: u32 = 2_156_220;

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
            system_battery: None,
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

pub(crate) fn dchu_control_write(command: &str) -> Result<(), String> {
    let path = dchu_control_proc_path();
    fs::write(&path, format!("{command}\n"))
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

pub(crate) fn fan_curve_points_arg(curve: &FanCurve) -> Result<String, String> {
    let curve = curve.clone().sanitized();
    validate_fan_curve_points(&curve.points)?;
    Ok(curve
        .points
        .iter()
        .map(|point| format!("{}:{}", point.temp_celsius, point.duty_percent))
        .collect::<Vec<_>>()
        .join(","))
}

pub(super) fn validate_fan_curve_points(points: &[FanCurvePoint]) -> Result<(), String> {
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

fn parse_hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let compact = value
        .split_whitespace()
        .collect::<String>()
        .replace(['_', ':'], "");
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

pub(super) fn parse_dchu_buffer_reply(text: &str) -> Result<Vec<u8>, String> {
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

pub(super) fn parse_dchu_config_reply(text: &str) -> Result<DchuConfig, String> {
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
        if let Some(value) = line.strip_prefix("bios_feature_04_08_offset15 ") {
            config.bios_feature_offset15 =
                parse_optional_u8_hex(value, "bios_feature_04_08_offset15")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("bios_feature_04_08_offset16 ") {
            config.bios_feature_offset16 =
                parse_optional_u8_hex(value, "bios_feature_04_08_offset16")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("bios_feature_04_08_offset17 ") {
            config.bios_feature_offset17 =
                parse_optional_u8_hex(value, "bios_feature_04_08_offset17")?;
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
        if let Some(value) = line.strip_prefix("battery_saver_04_0d_status ") {
            config.battery_saver_status =
                parse_optional_u8_hex(value, "battery_saver_04_0d_status")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_version ") {
            config.battery_info_version = parse_optional_u16_hex(value, "battery_info_07_version")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_manufacture_date ") {
            config.battery_manufacture_date_raw =
                parse_optional_u16_hex(value, "battery_info_07_manufacture_date")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_cycle_count ") {
            config.battery_cycle_count =
                parse_optional_u16_hex(value, "battery_info_07_cycle_count")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_full_charge_capacity ") {
            config.battery_full_charge_capacity =
                parse_optional_u16_hex(value, "battery_info_07_full_charge_capacity")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_design_capacity ") {
            config.battery_design_capacity =
                parse_optional_u16_hex(value, "battery_info_07_design_capacity")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_status ") {
            config.battery_status = parse_optional_u16_hex(value, "battery_info_07_status")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_pf_status ") {
            config.battery_pf_status = parse_optional_u32_hex(value, "battery_info_07_pf_status")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_operation_status ") {
            config.battery_operation_status =
                parse_optional_u32_hex(value, "battery_info_07_operation_status")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("battery_info_07_stop_charging_threshold ") {
            config.battery_stop_charging_threshold =
                parse_optional_u8_hex(value, "battery_info_07_stop_charging_threshold")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("energy_save_11_default_charge_limit ") {
            config.energy_save_default_charge_limit =
                parse_optional_u8_hex(value, "energy_save_11_default_charge_limit")?;
            continue;
        }
        if let Some(value) = line.strip_prefix("energy_save_11_default_discharge_limit ") {
            config.energy_save_default_discharge_limit =
                parse_optional_u8_hex(value, "energy_save_11_default_discharge_limit")?;
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

fn parse_optional_u32_hex(value: &str, label: &str) -> Result<Option<u32>, String> {
    let Some(value) = value.trim().strip_prefix("integer 0x") else {
        if value.trim() == "unknown" {
            return Ok(None);
        }
        return Err(format!("invalid {label} value"));
    };
    u32::from_str_radix(value, 16)
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

pub(super) fn dchu_status_buffer() -> Result<Vec<u8>, String> {
    let path = dchu_status_proc_path();
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_buffer_reply(&text)
}

pub(super) fn read_dchu_config() -> Result<DchuConfig, String> {
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

pub(super) fn print_status(bytes: &[u8]) {
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

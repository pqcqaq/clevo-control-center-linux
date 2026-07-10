use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

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

const SAFE_FAN_MODES: [FanModeOption; 3] = [
    FanModeOption {
        label: "自动",
        value: "auto",
    },
    FanModeOption {
        label: "最大",
        value: "max",
    },
    FanModeOption {
        label: "MaxQ",
        value: "maxq",
    },
];

const CONFIGURED_FAN_MODES: [FanModeOption; 4] = [
    FanModeOption {
        label: "自动",
        value: "auto",
    },
    FanModeOption {
        label: "最大",
        value: "max",
    },
    FanModeOption {
        label: "静音",
        value: "silent",
    },
    FanModeOption {
        label: "MaxQ",
        value: "maxq",
    },
];

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
    pub raw_config: Vec<u8>,
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

pub fn available_fan_modes(snapshot: Option<&HardwareSnapshot>) -> &'static [FanModeOption] {
    if snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .is_some()
    {
        &CONFIGURED_FAN_MODES
    } else {
        &SAFE_FAN_MODES
    }
}

pub fn selected_fan_mode_from_snapshot(
    _snapshot: Option<&HardwareSnapshot>,
) -> Option<&'static str> {
    // OEM UI reads fan selection from DCHU AppSettings page 4 offset 5.
    // The 0x0D[0x0E] EC flag is coupled with power mode writes, so it is not a safe selection source.
    None
}

pub fn selected_power_mode_from_snapshot(
    _snapshot: Option<&HardwareSnapshot>,
) -> Option<&'static str> {
    // OEM UI reads power selection from DCHU AppSettings page 1 offset 1.
    // Do not infer it from the coupled EC flag exposed in config_0d[0x0E].
    None
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
    println!(
        "  clevo-control-center dchu fan-mode <auto|max|silent|maxq|custom|0|1|3|5|6> --i-understand"
    );
    println!("  clevo-control-center dchu power-mode <0..3> --i-understand");
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

pub fn parse_power_mode(value: &str) -> Result<u32, String> {
    let mode = value
        .parse::<u32>()
        .map_err(|_| "power-mode must be 0..3".to_owned())?;
    if mode > 3 {
        return Err("power-mode must be 0..3".to_owned());
    }
    Ok(mode)
}

pub fn run_dchu_cli(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_dchu_usage();
        return Ok(());
    };

    match command {
        "status" => print_status(&dchu_status_buffer()?),
        "fan-mode" => {
            require_danger_flag(args)?;
            run_fan_mode(args.get(1).ok_or("fan-mode requires <mode>")?)?;
        }
        "power-mode" => {
            require_danger_flag(args)?;
            run_power_mode(args.get(1).ok_or("power-mode requires <0..3>")?)?;
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
             psf2_7a integer 0x70020053\n",
        )
        .unwrap();

        assert_eq!(config.fanq, Some(0x02));
        assert_eq!(config.mode_status, Some(0x00));
        assert_eq!(config.kbtp, Some(0x06));
        assert_eq!(config.psf5, Some(0x93));
        assert_eq!(config.psf1, Some(0x0468_0025));
        assert_eq!(config.psf4, Some(0x021c));
        assert_eq!(config.psf2, Some(0x7002_0053));
        assert_eq!(config.raw_config.len(), 32);
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
    fn does_not_select_fan_mode_from_coupled_ec_flag() {
        let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x10),
            ..DchuConfig::default()
        });
        assert_eq!(selected_fan_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x08),
            ..DchuConfig::default()
        });
        assert_eq!(selected_fan_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x02),
            ..DchuConfig::default()
        });
        assert_eq!(selected_fan_mode_from_snapshot(Some(&snapshot)), None);
    }

    #[test]
    fn does_not_select_power_mode_from_coupled_ec_flag() {
        let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x80),
            ..DchuConfig::default()
        });
        assert_eq!(selected_power_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x08),
            ..DchuConfig::default()
        });
        assert_eq!(selected_power_mode_from_snapshot(Some(&snapshot)), None);

        snapshot.dchu_config = Some(DchuConfig {
            mode_status: Some(0x02),
            ..DchuConfig::default()
        });
        assert_eq!(selected_power_mode_from_snapshot(Some(&snapshot)), None);
    }

    #[test]
    fn parses_status_buffer_reply_only() {
        let parsed = parse_dchu_buffer_reply("buffer 4\n01 02 0a ff\n").unwrap();
        assert_eq!(parsed, vec![0x01, 0x02, 0x0a, 0xff]);
        assert!(parse_dchu_buffer_reply("integer 0x79\n").is_err());
    }
}

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_DCHU_CONTROL_PROC_PATH: &str = "/proc/clevo_dchu_control";
const DEFAULT_DCHU_STATUS_PROC_PATH: &str = "/proc/clevo_dchu_status";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanStatus {
    pub label: String,
    pub rpm: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub fans: Vec<FanStatus>,
    pub battery_voltage_raw: u16,
    pub battery_rate_raw: u16,
    pub thermal_raw: [u8; 4],
    pub updated_unix_secs: u64,
}

impl HardwareSnapshot {
    pub fn from_status_bytes(bytes: &[u8]) -> Self {
        let fans = ["CPU 风扇", "GPU 风扇"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| FanStatus {
                label: label.to_owned(),
                rpm: get_be_u16(bytes, 0x02 + index * 2),
            })
            .collect();

        Self {
            fans,
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

fn dchu_status_buffer() -> Result<Vec<u8>, String> {
    let path = dchu_status_proc_path();
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_buffer_reply(&text)
}

pub fn read_hardware_snapshot() -> Result<HardwareSnapshot, String> {
    let status = dchu_status_buffer()?;
    Ok(HardwareSnapshot::from_status_bytes(&status))
}

fn get_be_u16(bytes: &[u8], offset: usize) -> u16 {
    let hi = bytes.get(offset).copied().unwrap_or_default() as u16;
    let lo = bytes.get(offset + 1).copied().unwrap_or_default() as u16;
    (hi << 8) | lo
}

fn print_status(bytes: &[u8]) {
    println!("DCHU 0x0C status");
    println!("rpm1: {}", get_be_u16(bytes, 0x02));
    println!("rpm2: {}", get_be_u16(bytes, 0x04));
    println!("rpm3: {}", get_be_u16(bytes, 0x06));
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
        "  clevo-control-center dchu fan-mode <auto|max|silent|maxq|custom|turbo|0|1|3|5|6|7> --i-understand"
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
        "turbo" => Ok(7),
        _ => {
            let mode = value
                .parse::<u32>()
                .map_err(|_| "fan mode must be a known name or decimal value".to_owned())?;
            match mode {
                0 | 1 | 3 | 5 | 6 | 7 => Ok(mode),
                _ => Err(
                    "fan mode must be one of auto/max/silent/maxq/custom/turbo or 0/1/3/5/6/7"
                        .to_owned(),
                ),
            }
        }
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
        bytes[0x02] = 0x05;
        bytes[0x03] = 0xdc;
        bytes[0x04] = 0x06;
        bytes[0x05] = 0x40;
        bytes[0x06] = 0x07;
        bytes[0x07] = 0x08;

        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

        assert_eq!(snapshot.fans.len(), 2);
        assert_eq!(snapshot.fans[0].rpm, 1500);
        assert_eq!(snapshot.fans[1].rpm, 1600);
    }

    #[test]
    fn parses_dchu_fan_modes() {
        assert_eq!(parse_fan_mode("auto").unwrap(), 0);
        assert_eq!(parse_fan_mode("max").unwrap(), 1);
        assert_eq!(parse_fan_mode("silent").unwrap(), 3);
        assert_eq!(parse_fan_mode("maxq").unwrap(), 5);
        assert_eq!(parse_fan_mode("custom").unwrap(), 6);
        assert_eq!(parse_fan_mode("turbo").unwrap(), 7);
        assert_eq!(parse_fan_mode("0").unwrap(), 0);
        assert!(parse_fan_mode("2").is_err());
        assert!(parse_fan_mode("0x1").is_err());
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
    fn parses_status_buffer_reply_only() {
        let parsed = parse_dchu_buffer_reply("buffer 4\n01 02 0a ff\n").unwrap();
        assert_eq!(parsed, vec![0x01, 0x02, 0x0a, 0xff]);
        assert!(parse_dchu_buffer_reply("integer 0x79\n").is_err());
    }
}

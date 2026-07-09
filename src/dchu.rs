use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_DCHU_PROC_PATH: &str = "/proc/clevo_dchu";
const DEFAULT_DCHU_STATUS_PROC_PATH: &str = "/proc/clevo_dchu_status";
const CAPS_FUNCTIONS: [u32; 4] = [0x10, 0x52, 0x60, 0x7a];

#[derive(Debug)]
pub enum DchuReply {
    Integer(u64),
    Buffer(Vec<u8>),
    Other(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanStatus {
    pub label: String,
    pub rpm: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityReply {
    pub function: u32,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub fans: Vec<FanStatus>,
    pub battery_voltage_raw: u16,
    pub battery_rate_raw: u16,
    pub thermal_raw: [u8; 4],
    pub caps: Vec<CapabilityReply>,
    pub errors: Vec<String>,
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
            caps: Vec::new(),
            errors: Vec::new(),
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

pub fn dchu_proc_path() -> std::path::PathBuf {
    std::env::var_os("CLEVO_DCHU_PROC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DCHU_PROC_PATH))
}

fn dchu_status_proc_path() -> std::path::PathBuf {
    std::env::var_os("CLEVO_DCHU_STATUS_PROC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DCHU_STATUS_PROC_PATH))
}

pub fn parse_u32_arg(value: &str) -> Result<u32, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).map_err(|_| format!("invalid number: {value}"))
    } else {
        trimmed
            .parse::<u32>()
            .or_else(|_| u32::from_str_radix(trimmed, 16))
            .map_err(|_| format!("invalid number: {value}"))
    }
}

pub fn parse_hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let compact = value
        .split_whitespace()
        .collect::<String>()
        .replace('_', "")
        .replace(':', "");
    if compact.len() % 2 != 0 {
        return Err("hex payload length must be even".to_owned());
    }

    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| format!("invalid hex byte near offset {index}"))
        })
        .collect()
}

fn bytes_to_proc_payload(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn dchu_query(function: u32, payload: Option<&[u8]>) -> Result<DchuReply, String> {
    let path = dchu_proc_path();
    let command = match payload {
        Some(bytes) => format!("write {function:02x} {}\n", bytes_to_proc_payload(bytes)),
        None => format!("read {function:02x}\n"),
    };

    fs::write(&path, command).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_reply(&text)
}

pub fn parse_dchu_reply(text: &str) -> Result<DchuReply, String> {
    let mut lines = text.lines();
    let Some(first) = lines.next().map(str::trim) else {
        return Err("empty DCHU reply".to_owned());
    };

    if let Some(value) = first.strip_prefix("integer ") {
        return parse_u32_arg(value).map(|value| DchuReply::Integer(value as u64));
    }

    if first.starts_with("buffer ") {
        let hex = lines.collect::<Vec<_>>().join(" ");
        return parse_hex_bytes(&hex).map(DchuReply::Buffer);
    }

    Ok(DchuReply::Other(text.trim().to_owned()))
}

pub fn dchu_buffer(function: u32) -> Result<Vec<u8>, String> {
    match dchu_query(function, None)? {
        DchuReply::Buffer(bytes) => Ok(bytes),
        other => Err(format!(
            "function 0x{function:02x} did not return buffer: {other:?}"
        )),
    }
}

fn dchu_status_buffer() -> Result<Vec<u8>, String> {
    let path = dchu_status_proc_path();
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    match parse_dchu_reply(&text)? {
        DchuReply::Buffer(bytes) => Ok(bytes),
        other => Err(format!("DCHU status did not return buffer: {other:?}")),
    }
}

fn dchu_debug_is_writable() -> bool {
    fs::OpenOptions::new()
        .write(true)
        .open(dchu_proc_path())
        .is_ok()
}

pub fn read_hardware_snapshot() -> Result<HardwareSnapshot, String> {
    let status = dchu_status_buffer().or_else(|_| dchu_buffer(0x0c))?;
    let mut snapshot = HardwareSnapshot::from_status_bytes(&status);

    if dchu_debug_is_writable() {
        for function in CAPS_FUNCTIONS {
            match dchu_query(function, None) {
                Ok(reply) => snapshot.caps.push(CapabilityReply {
                    function,
                    summary: format_reply_summary(&reply),
                }),
                Err(err) => snapshot.errors.push(format!("0x{function:02x}: {err}")),
            }
        }
    }

    Ok(snapshot)
}

fn format_reply_summary(reply: &DchuReply) -> String {
    match reply {
        DchuReply::Integer(value) => format!("integer 0x{value:x} ({value})"),
        DchuReply::Buffer(bytes) => format!("buffer {} bytes", bytes.len()),
        DchuReply::Other(text) => text.to_owned(),
    }
}

fn print_dchu_raw(reply: DchuReply) {
    match reply {
        DchuReply::Integer(value) => println!("integer 0x{value:x} ({value})"),
        DchuReply::Buffer(bytes) => {
            println!("buffer {} bytes", bytes.len());
            for (row, chunk) in bytes.chunks(16).enumerate() {
                print!("{:04x}: ", row * 16);
                for byte in chunk {
                    print!("{byte:02x} ");
                }
                println!();
            }
        }
        DchuReply::Other(text) => println!("{text}"),
    }
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

fn print_fan_curve(label: &str, bytes: &[u8], offset: usize) {
    println!("{label}:");
    for step in 0..4 {
        let temp = bytes.get(offset + step * 2).copied().unwrap_or_default();
        let duty = bytes
            .get(offset + step * 2 + 1)
            .copied()
            .unwrap_or_default();
        let percent = ((duty as u16 * 100) + 127) / 255;
        println!(
            "  step{}: temp={} duty={} ({}%)",
            step + 1,
            temp,
            duty,
            percent
        );
    }
}

fn print_fan_table(bytes: &[u8]) {
    println!("DCHU 0x0D fan table");
    println!(
        "keyboard left:   #{:02x}{:02x}{:02x}",
        bytes.get(0x02).copied().unwrap_or_default(),
        bytes.get(0x03).copied().unwrap_or_default(),
        bytes.get(0x04).copied().unwrap_or_default()
    );
    println!(
        "keyboard middle: #{:02x}{:02x}{:02x}",
        bytes.get(0x05).copied().unwrap_or_default(),
        bytes.get(0x06).copied().unwrap_or_default(),
        bytes.get(0x07).copied().unwrap_or_default()
    );
    println!(
        "keyboard right:  #{:02x}{:02x}{:02x}",
        bytes.get(0x08).copied().unwrap_or_default(),
        bytes.get(0x09).copied().unwrap_or_default(),
        bytes.get(0x0a).copied().unwrap_or_default()
    );
    println!(
        "keyboard_brightness_raw: {}",
        bytes.get(0x0b).copied().unwrap_or_default()
    );
    println!(
        "fanq: 0x{:02x}",
        bytes.get(0x0c).copied().unwrap_or_default()
    );
    println!(
        "d7_fbuf: 0x{:02x}",
        bytes.get(0x0e).copied().unwrap_or_default()
    );
    println!(
        "kbtp: 0x{:02x}",
        bytes.get(0x0f).copied().unwrap_or_default()
    );
    print_fan_curve("fan1", bytes, 0x10);
    print_fan_curve("fan2", bytes, 0x18);
    print_fan_curve("fan3", bytes, 0x20);
    println!(
        "kpcr: 0x{:02x}",
        bytes.get(0x2b).copied().unwrap_or_default()
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
    println!("  clevo-control-center dchu fan-table");
    println!("  clevo-control-center dchu caps");
    println!("  clevo-control-center dchu raw-get <function>");
    println!("  clevo-control-center dchu raw-set <function> <hex-bytes> --i-understand");
    println!("  clevo-control-center dchu raw-set-dword <function> <u32> --i-understand");
    println!("  clevo-control-center dchu kbd-brightness <0..9> --i-understand");
    println!("  clevo-control-center dchu fan-curve-set <30-or-32-hex-bytes> --i-understand");
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
            let mode = parse_u32_arg(value)?;
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

pub fn run_dchu_cli(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_dchu_usage();
        return Ok(());
    };

    match command {
        "status" => print_status(&dchu_status_buffer().or_else(|_| dchu_buffer(0x0c))?),
        "fan-table" => print_fan_table(&dchu_buffer(0x0d)?),
        "caps" => {
            for function in CAPS_FUNCTIONS {
                print!("0x{function:02x}: ");
                print_dchu_raw(dchu_query(function, None)?);
            }
        }
        "raw-get" => {
            let function = args.get(1).ok_or("raw-get requires <function>")?;
            print_dchu_raw(dchu_query(parse_u32_arg(function)?, None)?);
        }
        "raw-set" => {
            require_danger_flag(args)?;
            let function = args.get(1).ok_or("raw-set requires <function>")?;
            let payload = args.get(2).ok_or("raw-set requires <hex-bytes>")?;
            let bytes = parse_hex_bytes(payload)?;
            print_dchu_raw(dchu_query(parse_u32_arg(function)?, Some(&bytes))?);
        }
        "raw-set-dword" => {
            require_danger_flag(args)?;
            let function = args.get(1).ok_or("raw-set-dword requires <function>")?;
            let value = args.get(2).ok_or("raw-set-dword requires <u32>")?;
            let bytes = parse_u32_arg(value)?.to_le_bytes();
            print_dchu_raw(dchu_query(parse_u32_arg(function)?, Some(&bytes))?);
        }
        "kbd-brightness" => {
            require_danger_flag(args)?;
            let level = args
                .get(1)
                .ok_or("kbd-brightness requires <0..9>")?
                .parse::<u32>()
                .map_err(|_| "keyboard brightness must be 0..9".to_owned())?;
            if level > 9 {
                return Err("keyboard brightness must be 0..9".to_owned());
            }
            let payload = (0x0d_u32 << 28) | (level << 12);
            println!("writing SCMD 0x67 brightness payload 0x{payload:08x}");
            print_dchu_raw(dchu_query(0x67, Some(&payload.to_le_bytes()))?);
        }
        "fan-curve-set" => {
            require_danger_flag(args)?;
            let payload = args
                .get(1)
                .ok_or("fan-curve-set requires <30-or-32-hex-bytes>")?;
            let mut bytes = parse_hex_bytes(payload)?;
            match bytes.len() {
                30 => {
                    let mut with_prefix = vec![0, 0];
                    with_prefix.extend(bytes);
                    bytes = with_prefix;
                }
                32 => {}
                len => {
                    return Err(format!(
                        "fan curve payload must be 30 bytes without offset prefix or 32 bytes with 00 00 prefix, got {len}"
                    ));
                }
            }
            print_dchu_raw(dchu_query(0x0e, Some(&bytes))?);
        }
        "fan-mode" => {
            require_danger_flag(args)?;
            let mode = parse_fan_mode(args.get(1).ok_or("fan-mode requires <mode>")?)?;
            let payload = (0x01_u32 << 24) | mode;
            println!("writing SCMD 0x79 fan-mode payload 0x{payload:08x}");
            print_dchu_raw(dchu_query(0x79, Some(&payload.to_le_bytes()))?);
        }
        "power-mode" => {
            require_danger_flag(args)?;
            let mode = args
                .get(1)
                .ok_or("power-mode requires <0..3>")?
                .parse::<u32>()
                .map_err(|_| "power-mode must be 0..3".to_owned())?;
            if mode > 3 {
                return Err("power-mode must be 0..3".to_owned());
            }
            let payload = (0x19_u32 << 24) | mode;
            println!("writing SCMD 0x79 power-mode payload 0x{payload:08x}");
            print_dchu_raw(dchu_query(0x79, Some(&payload.to_le_bytes()))?);
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
    }
}

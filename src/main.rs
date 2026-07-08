use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{
    self, pos2, vec2, Align2, Area, Button, CentralPanel, Color32, ComboBox, Context,
    FontData, FontDefinitions, FontFamily, FontId, Frame, Id, Order, RichText, Sense, Slider,
    Stroke, Ui, ViewportBuilder, ViewportClass, ViewportCommand, ViewportId,
};
use serde::{Deserialize, Serialize};

const DEFAULT_PROC_PATH: &str = "/proc/clevo_kbd_led";
const DEFAULT_DCHU_PROC_PATH: &str = "/proc/clevo_dchu";
const APP_ID: &str = "clevo-control-center";
const LEGACY_APP_ID: &str = "clevo-keyboard-led";
const SETTINGS_FILE: &str = "settings.json";
const SERVICE_PID_FILE: &str = "clevo-control-center.pid";
const SERVICE_LOCK_FILE: &str = "clevo-control-center.lock";
const SERVICE_LOG_FILE: &str = "clevo-control-center.service.log";
const BASE_ZONES: [ZoneId; 3] = [ZoneId::F0, ZoneId::F1, ZoneId::F2];
const ALL_ZONES: [ZoneId; 7] = [
    ZoneId::F0,
    ZoneId::F1,
    ZoneId::F2,
    ZoneId::F3,
    ZoneId::F4,
    ZoneId::F5,
    ZoneId::F6,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
enum ZoneId {
    #[serde(rename = "f0")]
    F0,
    #[serde(rename = "f1")]
    F1,
    #[serde(rename = "f2")]
    F2,
    #[serde(rename = "f3")]
    F3,
    #[serde(rename = "f4")]
    F4,
    #[serde(rename = "f5")]
    F5,
    #[serde(rename = "f6")]
    F6,
}

impl ZoneId {
    fn proc_code(self) -> &'static str {
        match self {
            Self::F0 => "f0",
            Self::F1 => "f1",
            Self::F2 => "f2",
            Self::F3 => "f3",
            Self::F4 => "f4",
            Self::F5 => "f5",
            Self::F6 => "f6",
        }
    }

    fn label(self) -> &'static str {
        self.proc_code()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    fn color32(self) -> Color32 {
        Color32::from_rgb(self.r, self.g, self.b)
    }

    fn hex_lower(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Mode {
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "cycle")]
    Cycle,
    #[serde(rename = "chase")]
    Chase,
    #[serde(rename = "blink")]
    Blink,
    #[serde(rename = "breathing")]
    Breathing,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Custom => "自定义",
            Self::Cycle => "循环",
            Self::Chase => "追逐",
            Self::Blink => "闪烁",
            Self::Breathing => "呼吸",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Custom,
            Self::Cycle,
            Self::Chase,
            Self::Blink,
            Self::Breathing,
        ]
    }
}

#[derive(Clone)]
struct ZoneColor {
    zone: ZoneId,
    rgb: Rgb,
}

struct LedWriter {
    proc_path: PathBuf,
}

impl LedWriter {
    fn new() -> Self {
        let proc_path = std::env::var_os("CLEVO_KBD_LED_PROC")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PROC_PATH));
        Self { proc_path }
    }

    fn ready(&self) -> bool {
        self.proc_path.exists()
            && fs::OpenOptions::new()
                .write(true)
                .open(&self.proc_path)
                .is_ok()
    }

    fn write(&self, colors: &[ZoneColor]) -> io::Result<()> {
        if colors.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no zones"));
        }

        for command in commands_for_colors(colors) {
            fs::write(&self.proc_path, command)?;
        }
        Ok(())
    }

    fn proc_path(&self) -> &Path {
        &self.proc_path
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Settings {
    mode: Mode,
    speed: u8,
    brightness: u8,
    running: bool,
    f0_color: Rgb,
    #[serde(default = "default_zones")]
    zones: Vec<ZoneId>,
    window_pos: Option<[f32; 2]>,
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
    fn sanitized(mut self) -> Self {
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

fn default_zones() -> Vec<ZoneId> {
    BASE_ZONES.to_vec()
}

fn normalize_zones(zones: &[ZoneId]) -> Vec<ZoneId> {
    let normalized = ALL_ZONES
        .into_iter()
        .filter(|zone| zones.contains(zone))
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        default_zones()
    } else {
        normalized
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

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join(APP_ID)
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp").join(format!("{APP_ID}-{}", uid_string())))
        .join(APP_ID)
}

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/state"))
        .join(APP_ID)
}

fn settings_path() -> PathBuf {
    let path = config_dir().join(SETTINGS_FILE);
    migrate_legacy_settings(&path);
    path
}

fn service_pid_path() -> PathBuf {
    runtime_dir().join(SERVICE_PID_FILE)
}

fn service_lock_path() -> PathBuf {
    runtime_dir().join(SERVICE_LOCK_FILE)
}

fn service_log_path() -> PathBuf {
    state_dir().join(SERVICE_LOG_FILE)
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

fn load_settings(path: &Path) -> Settings {
    match fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
    {
        Some(settings) => settings.sanitized(),
        None => Settings::default(),
    }
}

fn file_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|metadata| metadata.modified()).ok()
}

fn atomic_write_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&tmp_path, format!("{json}\n"))?;
    fs::rename(tmp_path, path)
}

fn commands_for_colors(colors: &[ZoneColor]) -> Vec<String> {
    if colors.len() == 3
        && BASE_ZONES
            .iter()
            .all(|zone| colors.iter().any(|color| color.zone == *zone))
        && colors.iter().all(|color| color.rgb == colors[0].rgb)
    {
        return vec![format!("{}\n", colors[0].rgb.hex_lower())];
    }

    colors
        .iter()
        .map(|color| format!("{} {}\n", color.zone.proc_code(), color.rgb.hex_lower()))
        .collect()
}

#[derive(Debug)]
enum DchuReply {
    Integer(u64),
    Buffer(Vec<u8>),
    Other(String),
}

fn dchu_proc_path() -> PathBuf {
    std::env::var_os("CLEVO_DCHU_PROC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DCHU_PROC_PATH))
}

fn parse_u32_arg(value: &str) -> Result<u32, String> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|_| format!("invalid number: {value}"))
    } else {
        trimmed
            .parse::<u32>()
            .or_else(|_| u32::from_str_radix(trimmed, 16))
            .map_err(|_| format!("invalid number: {value}"))
    }
}

fn parse_hex_bytes(value: &str) -> Result<Vec<u8>, String> {
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

fn dchu_query(function: u32, payload: Option<&[u8]>) -> Result<DchuReply, String> {
    let path = dchu_proc_path();
    let command = match payload {
        Some(bytes) => format!("write {function:02x} {}\n", bytes_to_proc_payload(bytes)),
        None => format!("read {function:02x}\n"),
    };

    fs::write(&path, command).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    let text =
        fs::read_to_string(&path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    parse_dchu_reply(&text)
}

fn parse_dchu_reply(text: &str) -> Result<DchuReply, String> {
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

fn dchu_buffer(function: u32) -> Result<Vec<u8>, String> {
    match dchu_query(function, None)? {
        DchuReply::Buffer(bytes) => Ok(bytes),
        other => Err(format!("function 0x{function:02x} did not return buffer: {other:?}")),
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
    println!("thermal_raw_10: 0x{:02x}", bytes.get(0x10).copied().unwrap_or_default());
    println!("thermal_raw_11: 0x{:02x}", bytes.get(0x11).copied().unwrap_or_default());
    println!("thermal_raw_12: 0x{:02x}", bytes.get(0x12).copied().unwrap_or_default());
    println!("thermal_raw_13: 0x{:02x}", bytes.get(0x13).copied().unwrap_or_default());
}

fn print_fan_curve(label: &str, bytes: &[u8], offset: usize) {
    println!("{label}:");
    for step in 0..4 {
        let temp = bytes.get(offset + step * 2).copied().unwrap_or_default();
        let duty = bytes.get(offset + step * 2 + 1).copied().unwrap_or_default();
        let percent = ((duty as u16 * 100) + 127) / 255;
        println!("  step{}: temp={} duty={} ({}%)", step + 1, temp, duty, percent);
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
    println!("keyboard_brightness_raw: {}", bytes.get(0x0b).copied().unwrap_or_default());
    println!("fanq: 0x{:02x}", bytes.get(0x0c).copied().unwrap_or_default());
    println!("d7_fbuf: 0x{:02x}", bytes.get(0x0e).copied().unwrap_or_default());
    println!("kbtp: 0x{:02x}", bytes.get(0x0f).copied().unwrap_or_default());
    print_fan_curve("fan1", bytes, 0x10);
    print_fan_curve("fan2", bytes, 0x18);
    print_fan_curve("fan3", bytes, 0x20);
    println!("kpcr: 0x{:02x}", bytes.get(0x2b).copied().unwrap_or_default());
}

fn require_danger_flag(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--i-understand") {
        Ok(())
    } else {
        Err("dangerous write requires --i-understand".to_owned())
    }
}

fn print_dchu_usage() {
    println!("Usage:");
    println!("  clevo-control-center dchu status");
    println!("  clevo-control-center dchu fan-table");
    println!("  clevo-control-center dchu caps");
    println!("  clevo-control-center dchu raw-get <function>");
    println!("  clevo-control-center dchu raw-set <function> <hex-bytes> --i-understand");
    println!("  clevo-control-center dchu raw-set-dword <function> <u32> --i-understand");
    println!("  clevo-control-center dchu kbd-brightness <0..9> --i-understand");
    println!("  clevo-control-center dchu fan-curve-set <30-or-32-hex-bytes> --i-understand");
    println!("  clevo-control-center dchu fan-mode <auto|max|silent|maxq|custom|turbo|0|1|3|5|6|7> --i-understand");
    println!("  clevo-control-center dchu power-mode <0..3> --i-understand");
}

fn parse_fan_mode(value: &str) -> Result<u32, String> {
    match value.to_ascii_lowercase().as_str() {
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
                _ => Err("fan mode must be one of auto/max/silent/maxq/custom/turbo or 0/1/3/5/6/7".to_owned()),
            }
        }
    }
}

fn run_dchu_cli(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_dchu_usage();
        return Ok(());
    };

    match command {
        "status" => print_status(&dchu_buffer(0x0c)?),
        "fan-table" => print_fan_table(&dchu_buffer(0x0d)?),
        "caps" => {
            for function in [0x10, 0x52, 0x60, 0x7a] {
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
                .map_err(|_| "power mode must be 0..3".to_owned())?;
            if mode > 3 {
                return Err("power mode must be 0..3".to_owned());
            }
            let payload = (0x19_u32 << 24) | mode;
            println!("writing SCMD 0x79 payload 0x{payload:08x}");
            print_dchu_raw(dchu_query(0x79, Some(&payload.to_le_bytes()))?);
        }
        "help" | "--help" | "-h" => print_dchu_usage(),
        _ => return Err(format!("unknown dchu command: {command}")),
    }

    Ok(())
}

struct ClevoLedApp {
    writer: LedWriter,
    settings_path: PathBuf,
    settings_mtime: Option<SystemTime>,
    mode: Mode,
    speed: u8,
    brightness: u8,
    running: bool,
    f0_color: Rgb,
    zones: Vec<ZoneId>,
    settings_open: bool,
    menu_open: bool,
    last_error: Option<String>,
    window_pos: Option<[f32; 2]>,
    dirty_settings: bool,
    dirty_window_position: bool,
    last_settings_save: Instant,
}

impl ClevoLedApp {
    fn new(settings_path: PathBuf, settings: Settings) -> Self {
        let writer = LedWriter::new();
        if !writer.ready() {
            eprintln!("Keyboard RGB interface is not writable: {}", writer.proc_path().display());
        }
        let settings_mtime = file_modified(&settings_path);

        Self {
            writer,
            settings_path,
            settings_mtime,
            mode: settings.mode,
            speed: settings.speed,
            brightness: settings.brightness,
            running: settings.running,
            f0_color: settings.f0_color,
            zones: settings.zones,
            settings_open: false,
            menu_open: false,
            last_error: None,
            window_pos: settings.window_pos,
            dirty_settings: false,
            dirty_window_position: false,
            last_settings_save: Instant::now(),
        }
    }

    fn toggle(&mut self) {
        if self.mode == Mode::Custom {
            return;
        }

        self.running = !self.running;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    fn selected_zones(&self) -> Vec<ZoneId> {
        normalize_zones(&self.zones)
    }

    fn set_zone_enabled(&mut self, zone: ZoneId, enabled: bool) {
        if enabled {
            if !self.zones.contains(&zone) {
                self.zones.push(zone);
            }
        } else if self.zones.len() > 1 {
            self.zones.retain(|item| *item != zone);
        }
        self.zones = normalize_zones(&self.zones);
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
        if self.mode == Mode::Custom {
            self.write_selected_color(self.f0_color);
        }
    }

    fn write_selected_color(&mut self, rgb: Rgb) {
        let colors = self
            .selected_zones()
            .into_iter()
            .map(|zone| ZoneColor { zone, rgb })
            .collect::<Vec<_>>();

        if let Err(err) = self.writer.write(&colors) {
            self.last_error = Some(err.to_string());
            self.running = false;
            eprintln!("Failed to write selected color: {err}");
        }
    }

    fn mark_settings_dirty(&mut self) {
        self.dirty_settings = true;
    }

    fn apply_external_settings(&mut self, settings: Settings) {
        self.mode = settings.mode;
        self.speed = settings.speed;
        self.brightness = settings.brightness;
        self.running = settings.running;
        self.f0_color = settings.f0_color;
        self.zones = settings.zones;
    }

    fn sync_external_settings(&mut self) {
        if self.dirty_settings {
            return;
        }

        let mtime = file_modified(&self.settings_path);
        if mtime.is_none() || mtime == self.settings_mtime {
            return;
        }

        let settings = load_settings(&self.settings_path);
        self.apply_external_settings(settings);
        self.settings_mtime = mtime;
    }

    fn update_window_position(&mut self, ctx: &Context) {
        let position = ctx.input(|input| {
            input
                .viewport()
                .outer_rect
                .map(|rect| [rect.min.x, rect.min.y])
        });

        if let Some(position) = position {
            let changed = self
                .window_pos
                .map(|old| {
                    (old[0] - position[0]).abs() > 0.5
                        || (old[1] - position[1]).abs() > 0.5
                })
                .unwrap_or(true);
            if changed {
                self.window_pos = Some(position);
                self.dirty_window_position = true;
            }
        }
    }

    fn persist_settings_if_due(&mut self, force: bool) {
        if !self.dirty_settings && !self.dirty_window_position && !force {
            return;
        }
        if !force && self.last_settings_save.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.persist_settings();
    }

    fn persist_settings(&mut self) {
        let mut settings = if self.dirty_settings {
            Settings {
                mode: self.mode,
                speed: self.speed,
                brightness: self.brightness,
                running: self.running && self.mode != Mode::Custom,
                f0_color: self.f0_color,
                zones: self.selected_zones(),
                window_pos: self.window_pos,
            }
        } else {
            load_settings(&self.settings_path)
        };

        settings.window_pos = self.window_pos;
        settings = settings.sanitized();

        let should_apply_local_state = self.dirty_settings;

        match serde_json::to_string_pretty(&settings)
            .map_err(io::Error::other)
            .and_then(|_| atomic_write_settings(&self.settings_path, &settings))
        {
            Ok(()) => {
                self.dirty_settings = false;
                self.dirty_window_position = false;
                self.last_settings_save = Instant::now();
                self.settings_mtime = file_modified(&self.settings_path);
                if !should_apply_local_state {
                    self.apply_external_settings(settings);
                }
            }
            Err(err) => {
                eprintln!("Failed to write {}: {err}", self.settings_path.display());
                self.last_settings_save = Instant::now();
            }
        }
    }

}

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.sync_external_settings();
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(16, 18, 22)))
            .show(ctx, |ui| {
                custom_title_bar(ui, ctx, self);
                ui.add_space(2.0);
                main_controls(ui, self);
            });

        settings_viewport(ctx, self);
        self.persist_settings_if_due(false);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_settings_if_due(true);
    }
}

fn custom_title_bar(ui: &mut Ui, ctx: &Context, app: &mut ClevoLedApp) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click_and_drag());
    if response.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(16, 18, 22));

    let logo_rect = egui::Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 3.0), vec2(24.0, 20.0));
    let logo_response = ui.interact(logo_rect, ui.id().with("logo"), Sense::click());
    let logo_fill = if logo_response.hovered() {
        Color32::from_rgb(54, 60, 70)
    } else {
        Color32::from_rgb(34, 38, 45)
    };
    painter.circle_filled(logo_rect.center(), 9.0, logo_fill);
    painter.circle_filled(logo_rect.center() + vec2(-3.0, -2.5), 2.5, Color32::from_rgb(246, 92, 92));
    painter.circle_filled(logo_rect.center() + vec2(3.0, -2.5), 2.5, Color32::from_rgb(93, 204, 132));
    painter.circle_filled(logo_rect.center() + vec2(0.0, 3.0), 2.5, Color32::from_rgb(92, 151, 247));
    if logo_response.clicked() {
        app.menu_open = !app.menu_open;
    }

    if app.menu_open {
        Area::new(Id::new("logo_menu"))
            .order(Order::Foreground)
            .fixed_pos(pos2(rect.left() + 6.0, rect.bottom() + 2.0))
            .show(ui.ctx(), |ui| {
                Frame::none()
                    .fill(Color32::from_rgb(31, 35, 42))
                    .rounding(6.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(67, 74, 84)))
                    .inner_margin(egui::Margin::symmetric(6.0, 6.0))
                    .show(ui, |ui| {
                        if ui
                            .add_sized(vec2(68.0, 24.0), Button::new(RichText::new("设置").size(13.0)))
                            .clicked()
                        {
                            app.menu_open = false;
                            app.settings_open = true;
                        }
                    });
            });
    }

    let close_rect = egui::Rect::from_min_size(pos2(rect.right() - 34.0, rect.top()), vec2(30.0, 26.0));
    let close_response = ui.interact(close_rect, ui.id().with("close"), Sense::click());
    let close_fill = if close_response.hovered() {
        Color32::from_rgb(92, 36, 42)
    } else {
        Color32::from_rgb(31, 34, 40)
    };
    painter.circle_filled(close_rect.center(), 10.0, close_fill);
    painter.text(
        close_rect.center(),
        Align2::CENTER_CENTER,
        "x",
        FontId::proportional(15.0),
        Color32::from_rgb(230, 230, 230),
    );
    if close_response.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

fn main_controls(ui: &mut Ui, app: &mut ClevoLedApp) {
    let custom_mode = app.mode == Mode::Custom;
    Frame::none()
        .fill(Color32::from_rgb(21, 24, 29))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_height(86.0);
            ui.horizontal_centered(|ui| {
                color_swatch(ui, app);

                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ComboBox::from_id_salt("mode")
                        .width(220.0)
                        .selected_text(app.mode.label())
                        .show_ui(ui, |ui| {
                            for mode in Mode::all() {
                                let old_mode = app.mode;
                                let clicked = ui.selectable_value(&mut app.mode, *mode, mode.label()).clicked();
                                if app.mode != old_mode {
                                    if app.mode == Mode::Custom {
                                        app.running = false;
                                        app.write_selected_color(app.f0_color);
                                    }
                                    app.mark_settings_dirty();
                                    app.persist_settings_if_due(true);
                                }
                                if clicked && app.running && app.mode == Mode::Custom {
                                    app.write_selected_color(app.f0_color);
                                }
                            }
                        });

                    ui.add_space(8.0);
                    let speed_response = ui.horizontal(|ui| {
                        ui.set_width(220.0);
                        ui.label(RichText::new("速度").size(12.0));
                        ui.add_enabled_ui(!custom_mode, |ui| {
                            ui.add_sized(
                                vec2(174.0, 18.0),
                                Slider::new(&mut app.speed, 1..=100).show_value(false),
                            )
                        }).inner
                    }).inner;
                    if speed_response.changed() {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }

                    ui.add_space(6.0);
                    let brightness_response = ui.horizontal(|ui| {
                        ui.set_width(220.0);
                        ui.label(RichText::new("亮度").size(12.0));
                        ui.add_enabled_ui(!custom_mode, |ui| {
                            ui.add_sized(
                                vec2(174.0, 18.0),
                                Slider::new(&mut app.brightness, 1..=100).show_value(false),
                            )
                        }).inner
                    }).inner;
                    if brightness_response.changed() {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }
                });

                ui.add_space(4.0);
                let label = if app.running { "结束" } else { "开始" };
                if ui
                    .add_enabled(!custom_mode, Button::new(RichText::new(label).size(16.0)).min_size(vec2(64.0, 42.0)))
                    .clicked()
                {
                    app.toggle();
                }
            });
        });
}

fn settings_viewport(ctx: &Context, app: &mut ClevoLedApp) {
    if !app.settings_open {
        return;
    }

    let viewport_id = ViewportId::from_hash_of("settings");
    let builder = ViewportBuilder::default()
        .with_title("设置")
        .with_inner_size([238.0, 172.0])
        .with_min_inner_size([238.0, 172.0])
        .with_max_inner_size([238.0, 172.0])
        .with_resizable(false);

    let mut close_requested = false;
    let mut changed_zones = Vec::new();

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        if ctx.input(|input| input.viewport().close_requested()) {
            close_requested = true;
        }

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(18, 20, 24)))
            .show(ctx, |ui| {
                let content = |ui: &mut Ui| {
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                        for zone in ALL_ZONES {
                            let mut enabled = app.zones.contains(&zone);
                            let is_last_enabled = enabled && app.zones.len() == 1;
                            let response = ui
                                .add_enabled_ui(!is_last_enabled, |ui| {
                                    ui.add_sized(
                                        vec2(50.0, 28.0),
                                        egui::Checkbox::new(&mut enabled, zone.label()),
                                    )
                                })
                                .inner;
                            if response.changed() {
                                changed_zones.push((zone, enabled));
                            }
                        }
                    });
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized(vec2(56.0, 26.0), Button::new("关闭")).clicked() {
                            close_requested = true;
                        }
                    });
                };

                if class == ViewportClass::Embedded {
                    egui::Window::new("设置")
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, content);
                } else {
                    Frame::none()
                        .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                        .show(ui, content);
                }
            });
    });

    for (zone, enabled) in changed_zones {
        app.set_zone_enabled(zone, enabled);
    }

    if close_requested {
        app.settings_open = false;
        ctx.send_viewport_cmd_to(viewport_id, ViewportCommand::Close);
    }
}

fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(62.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 23.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(9, 11, 14));
    painter.circle_stroke(center, radius + 4.0, Stroke::new(1.0, Color32::from_rgb(66, 72, 82)));
    painter.circle_filled(center, radius, app.f0_color.color32());

    if response.hovered() && app.mode == Mode::Custom {
        painter.circle_stroke(center, radius + 6.0, Stroke::new(1.5, Color32::from_rgb(118, 190, 255)));
    }

    if response.clicked() && app.mode == Mode::Custom {
        match open_native_color_picker(app.f0_color) {
            Ok(Some(rgb)) => {
                app.f0_color = rgb;
                app.mark_settings_dirty();
                app.persist_settings_if_due(true);
                app.write_selected_color(app.f0_color);
            }
            Ok(None) => {}
            Err(err) => {
                app.last_error = Some(err.to_string());
                eprintln!("Failed to open color picker: {err}");
            }
        }
    }
}

fn open_native_color_picker(current: Rgb) -> io::Result<Option<Rgb>> {
    let current_hex = format!("#{:02x}{:02x}{:02x}", current.r, current.g, current.b);

    if command_exists("zenity") {
        let output = Command::new("zenity")
            .args(["--color-selection", "--show-palette", "--color", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(&output.stdout)));
    }

    if command_exists("kdialog") {
        let output = Command::new("kdialog")
            .args(["--getcolor", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(&output.stdout)));
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "需要安装 zenity 或 kdialog 才能弹出系统调色盘",
    ))
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {command} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn parse_color_picker_output(output: &str) -> Option<Rgb> {
    let text = output.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex_rgb(hex);
    }

    if let Some(body) = text.strip_prefix("rgb(").and_then(|value| value.strip_suffix(')')) {
        let values = body
            .split(',')
            .map(|part| part.trim().parse::<u16>().ok())
            .collect::<Option<Vec<_>>>()?;
        if values.len() == 3 && values.iter().all(|value| *value <= 255) {
            return Some(Rgb {
                r: values[0] as u8,
                g: values[1] as u8,
                b: values[2] as u8,
            });
        }
    }

    None
}

fn parse_hex_rgb(hex: &str) -> Option<Rgb> {
    if hex.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(Rgb {
        r: ((value >> 16) & 0xff) as u8,
        g: ((value >> 8) & 0xff) as u8,
        b: (value & 0xff) as u8,
    })
}

fn ensure_service_running() {
    if active_service_pid().is_some() {
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("Failed to locate executable for service: {err}");
            return;
        }
    };

    let log_path = service_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path);
    let stderr = log
        .ok()
        .map(Stdio::from)
        .unwrap_or_else(Stdio::null);

    match Command::new(exe)
        .arg("--service")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
    {
        Ok(_child) => {}
        Err(err) => eprintln!("Failed to start LED service: {err}"),
    }
}

fn service_pid() -> Option<u32> {
    read_pid_file(&service_pid_path())
}

fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|pid_text| pid_text.trim().parse::<u32>().ok())
}

fn process_is_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn active_service_pid() -> Option<u32> {
    service_pid()
        .filter(|pid| process_is_running(*pid))
        .or_else(|| read_pid_file(&service_lock_path()).filter(|pid| process_is_running(*pid)))
}

struct ServiceLock {
    path: PathBuf,
}

impl ServiceLock {
    fn acquire() -> io::Result<Option<Self>> {
        let path = service_lock_path();
        let pid_path = service_pid_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        for _ in 0..3 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    writeln!(file, "{}", process::id())?;
                    fs::write(&pid_path, format!("{}\n", process::id()))?;
                    return Ok(Some(Self { path }));
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(50));
                    if active_service_pid().is_some() {
                        return Ok(None);
                    }
                    let _ = fs::remove_file(&path);
                    let _ = fs::remove_file(&pid_path);
                }
                Err(err) => return Err(err),
            }
        }

        if active_service_pid().is_some() {
            Ok(None)
        } else {
            Err(io::Error::new(io::ErrorKind::AlreadyExists, "stale service lock"))
        }
    }
}

impl Drop for ServiceLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(service_pid_path());
    }
}

fn service_loop(settings_path: PathBuf) -> ! {
    let _lock = match ServiceLock::acquire() {
        Ok(Some(lock)) => lock,
        Ok(None) => process::exit(0),
        Err(err) => {
            eprintln!("Failed to acquire service lock: {err}");
            process::exit(1);
        }
    };

    let writer = LedWriter::new();
    let mut phase = 0.0_f32;
    let mut last_frame = Instant::now();
    let mut last_static_color: Option<Rgb> = None;
    let mut last_static_zones: Vec<ZoneId> = Vec::new();

    loop {
        let settings = load_settings(&settings_path);
        if settings.mode == Mode::Custom {
            let zones = normalize_zones(&settings.zones);
            if last_static_color != Some(settings.f0_color) || last_static_zones != zones {
                let colors = colors_for_mode(Mode::Custom, 0.0, &settings);
                let _ = writer.write(&colors);
                last_static_color = Some(settings.f0_color);
                last_static_zones = zones;
            }
            thread::sleep(Duration::from_millis(180));
            continue;
        }

        last_static_color = None;
        last_static_zones.clear();
        if settings.running {
            let now = Instant::now();
            let dt = now.duration_since(last_frame).as_secs_f32().min(0.2);
            last_frame = now;
            phase = (phase + dt * cycles_per_second(settings.speed)).fract();
            let colors = colors_for_mode(settings.mode, phase, &settings);
            if let Err(err) = writer.write(&colors) {
                eprintln!("LED service write failed: {err}");
            }
            thread::sleep(tick_interval(settings.speed));
        } else {
            last_frame = Instant::now();
            thread::sleep(Duration::from_millis(180));
        }
    }
}

fn colors_for_mode(mode: Mode, phase: f32, settings: &Settings) -> Vec<ZoneColor> {
    let brightness = settings.brightness as f32 / 100.0;
    let zones = normalize_zones(&settings.zones);
    match mode {
        Mode::Custom => zones
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: settings.f0_color,
            })
            .collect(),
        Mode::Cycle => {
            let rgb = hsv_rgb(phase, 1.0, brightness);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Chase => {
            let zone_count = zones.len().max(1) as f32;
            zones
            .into_iter()
            .enumerate()
            .map(|(index, zone)| ZoneColor {
                zone,
                rgb: hsv_rgb(phase + index as f32 / zone_count, 1.0, brightness),
            })
            .collect()
        }
        Mode::Blink => {
            let blink_phase = (phase * 5.0).fract();
            let level = if blink_phase < 0.42 {
                1.0
            } else if blink_phase < 0.5 {
                1.0 - smoothstep((blink_phase - 0.42) / 0.08)
            } else if blink_phase < 0.92 {
                0.0
            } else {
                smoothstep((blink_phase - 0.92) / 0.08)
            };
            let rgb = scale_rgb(settings.f0_color, brightness * level);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Breathing => {
            let pulse = 0.12 + 0.88 * ((phase * std::f32::consts::TAU).sin() + 1.0) / 2.0;
            let rgb = scale_rgb(settings.f0_color, pulse * brightness);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
    }
}

fn tick_interval(speed: u8) -> Duration {
    let millis = 28_u64.saturating_sub((speed as u64 * 12) / 100).max(16);
    Duration::from_millis(millis)
}

fn cycles_per_second(speed: u8) -> f32 {
    let t = speed.clamp(1, 100) as f32 / 100.0;
    0.035 + 0.42 * t.powf(1.35)
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn scale_rgb(rgb: Rgb, factor: f32) -> Rgb {
    Rgb {
        r: clamp_u8(rgb.r as f32 * factor),
        g: clamp_u8(rgb.g as f32 * factor),
        b: clamp_u8(rgb.b as f32 * factor),
    }
}

fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn hsv_rgb(hue: f32, saturation: f32, value: f32) -> Rgb {
    let h = hue.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - f * saturation);
    let t = value * (1.0 - (1.0 - f) * saturation);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    Rgb {
        r: clamp_u8(r * 255.0),
        g: clamp_u8(g * 255.0),
        b: clamp_u8(b * 255.0),
    }
}

fn install_cjk_font(ctx: &Context) {
    const FONT_CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    ];

    let Some((path, bytes)) = FONT_CANDIDATES
        .iter()
        .find_map(|path| fs::read(path).ok().map(|bytes| (*path, bytes)))
    else {
        eprintln!("No CJK font found; Chinese text may not render correctly");
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk_fallback".to_owned(), FontData::from_owned(bytes));

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk_fallback".to_owned());
    }

    ctx.set_fonts(fonts);
    eprintln!("Loaded CJK font: {path}");
}

fn main() -> eframe::Result {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("dchu") {
        if let Err(err) = run_dchu_cli(&args[2..]) {
            eprintln!("{err}");
            print_dchu_usage();
            process::exit(2);
        }
        return Ok(());
    }

    if std::env::args().any(|arg| arg == "--service") {
        service_loop(settings_path());
    }

    let settings_path = settings_path();
    let settings = load_settings(&settings_path);
    ensure_service_running();
    let mut viewport = ViewportBuilder::default()
        .with_inner_size([432.0, 134.0])
        .with_min_inner_size([432.0, 134.0])
        .with_max_inner_size([432.0, 134.0])
        .with_decorations(false)
        .with_resizable(false);

    if let Some([x, y]) = settings.window_pos {
        viewport = viewport.with_position(pos2(x, y));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(ClevoLedApp::new(settings_path, settings)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_base_zones_same_color_as_short_command() {
        let colors = BASE_ZONES
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: Rgb { r: 255, g: 0, b: 0 },
            })
            .collect::<Vec<_>>();

        assert_eq!(commands_for_colors(&colors), vec!["ff0000\n"]);
    }

    #[test]
    fn serializes_mixed_zone_commands() {
        let colors = vec![
            ZoneColor {
                zone: ZoneId::F0,
                rgb: Rgb { r: 255, g: 0, b: 0 },
            },
            ZoneColor {
                zone: ZoneId::F2,
                rgb: Rgb { r: 0, g: 0, b: 255 },
            },
        ];

        assert_eq!(
            commands_for_colors(&colors),
            vec!["f0 ff0000\n", "f2 0000ff\n"]
        );
    }

    #[test]
    fn hsv_cycle_starts_at_red() {
        assert_eq!(hsv_rgb(0.0, 1.0, 1.0), Rgb { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn effect_timing_uses_fine_grained_steps() {
        assert!(tick_interval(1) <= Duration::from_millis(28));
        assert!(tick_interval(100) <= Duration::from_millis(16));
        assert!(cycles_per_second(100) < 0.5);
    }

    #[test]
    fn smoothstep_has_stable_edges() {
        assert_eq!(smoothstep(-1.0), 0.0);
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(2.0), 1.0);
    }

    #[test]
    fn settings_json_roundtrips() {
        let settings = Settings {
            mode: Mode::Chase,
            speed: 72,
            brightness: 64,
            running: true,
            f0_color: Rgb { r: 12, g: 34, b: 56 },
            zones: vec![ZoneId::F0, ZoneId::F3, ZoneId::F6],
            window_pos: Some([100.0, 200.0]),
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed = serde_json::from_str::<Settings>(&json).unwrap();

        assert_eq!(parsed.mode, Mode::Chase);
        assert_eq!(parsed.speed, 72);
        assert_eq!(parsed.brightness, 64);
        assert!(parsed.running);
        assert_eq!(parsed.f0_color, Rgb { r: 12, g: 34, b: 56 });
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

    #[test]
    fn parses_native_color_picker_outputs() {
        assert_eq!(
            parse_color_picker_output("#0c2238\n"),
            Some(Rgb { r: 12, g: 34, b: 56 })
        );
        assert_eq!(
            parse_color_picker_output("rgb(12,34,56)\n"),
            Some(Rgb { r: 12, g: 34, b: 56 })
        );
        assert_eq!(parse_color_picker_output(""), None);
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

    #[test]
    fn service_generates_chase_colors_for_base_zones() {
        let settings = Settings {
            mode: Mode::Chase,
            speed: 50,
            brightness: 100,
            running: true,
            f0_color: Rgb::WHITE,
            zones: default_zones(),
            window_pos: None,
        };

        let colors = colors_for_mode(Mode::Chase, 0.0, &settings);

        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0].zone, ZoneId::F0);
        assert_eq!(colors[1].zone, ZoneId::F1);
        assert_eq!(colors[2].zone, ZoneId::F2);
    }

    #[test]
    fn service_uses_selected_zones() {
        let settings = Settings {
            mode: Mode::Cycle,
            speed: 50,
            brightness: 100,
            running: true,
            f0_color: Rgb::WHITE,
            zones: vec![ZoneId::F0, ZoneId::F4, ZoneId::F6],
            window_pos: None,
        };

        let zones = colors_for_mode(Mode::Cycle, 0.0, &settings)
            .into_iter()
            .map(|color| color.zone)
            .collect::<Vec<_>>();

        assert_eq!(zones, vec![ZoneId::F0, ZoneId::F4, ZoneId::F6]);
    }
}

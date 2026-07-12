mod cli;
mod io;

use serde::{Deserialize, Serialize};
#[cfg(any(debug_assertions, test))]
use std::fmt::Write as _;

use crate::fan_curve::{FanCurvePoint, FAN_CURVE_MAX_TEMP};
use crate::preferences::UiLanguage;

pub use cli::{print_dchu_usage, run_dchu_cli};
#[cfg(debug_assertions)]
pub use io::fan_rpm_from_tach;
pub use io::read_hardware_snapshot;
pub(crate) use io::{dchu_control_write, fan_curve_points_arg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanMode {
    Auto,
    Max,
    Silent,
    MaxQ,
    Custom,
}

impl FanMode {
    pub fn localized_label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::Auto => language.pick("自动", "Auto"),
            Self::Max => language.pick("最大", "Maximum"),
            Self::Silent => language.pick("静音", "Silent"),
            Self::MaxQ => "MaxQ",
            Self::Custom => language.pick("自定义", "Custom"),
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Max => "max",
            Self::Silent => "silent",
            Self::MaxQ => "maxq",
            Self::Custom => "custom",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "auto" | "0" => Some(Self::Auto),
            "max" | "1" => Some(Self::Max),
            "silent" | "3" => Some(Self::Silent),
            "maxq" | "5" => Some(Self::MaxQ),
            "custom" | "6" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerMode {
    Quiet,
    PowerSaving,
    Performance,
    Entertainment,
}

impl PowerMode {
    pub fn localized_label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::Quiet => language.pick("安静", "Quiet"),
            Self::PowerSaving => language.pick("省电", "Power saver"),
            Self::Performance => language.pick("性能", "Performance"),
            Self::Entertainment => language.pick("娱乐", "Entertainment"),
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::Quiet => "0",
            Self::PowerSaving => "1",
            Self::Performance => "2",
            Self::Entertainment => "3",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::Quiet),
            "1" => Some(Self::PowerSaving),
            "2" => Some(Self::Performance),
            "3" => Some(Self::Entertainment),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuMuxMode {
    MSHybrid,
    DGpu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardLightingLayout {
    Unsupported,
    White,
    SingleZone,
    ThreeZone,
    PerKey,
    Unknown,
}

impl KeyboardLightingLayout {
    pub fn localized_label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::Unsupported => language.pick("不支持", "Unsupported"),
            Self::White => language.pick("白光键盘", "White backlight"),
            Self::SingleZone => language.pick("单区 RGB", "Single-zone RGB"),
            Self::ThreeZone => language.pick("三区 RGB", "Three-zone RGB"),
            Self::PerKey => language.pick("逐键 RGB", "Per-key RGB"),
            Self::Unknown => language.pick("未知", "Unknown"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardLightingCapabilities {
    pub layout: KeyboardLightingLayout,
    pub logo: Option<bool>,
    pub lightbar: Option<bool>,
}

impl Default for KeyboardLightingCapabilities {
    fn default() -> Self {
        Self {
            layout: KeyboardLightingLayout::Unknown,
            logo: None,
            lightbar: None,
        }
    }
}

impl GpuMuxMode {
    pub fn localized_label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::MSHybrid => language.pick("混合模式", "Hybrid mode"),
            Self::DGpu => language.pick("独显直连", "Discrete GPU"),
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::MSHybrid => "mshybrid",
            Self::DGpu => "dgpu",
        }
    }
}

const GPU_MUX_MODES: [GpuMuxMode; 2] = [GpuMuxMode::DGpu, GpuMuxMode::MSHybrid];

const FALLBACK_FAN_MODES: [FanMode; 2] = [FanMode::Auto, FanMode::Max];

const POWER_MODES: [PowerMode; 4] = [
    PowerMode::Quiet,
    PowerMode::PowerSaving,
    PowerMode::Performance,
    PowerMode::Entertainment,
];
const NO_POWER_MODES: [PowerMode; 0] = [];

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
    pub bios_feature_offset15: Option<u8>,
    #[serde(default)]
    pub bios_feature_offset16: Option<u8>,
    #[serde(default)]
    pub bios_feature_offset17: Option<u8>,
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
    pub battery_saver_status: Option<u8>,
    #[serde(default)]
    pub battery_info_version: Option<u16>,
    #[serde(default)]
    pub battery_manufacture_date_raw: Option<u16>,
    #[serde(default)]
    pub battery_cycle_count: Option<u16>,
    #[serde(default)]
    pub battery_full_charge_capacity: Option<u16>,
    #[serde(default)]
    pub battery_design_capacity: Option<u16>,
    #[serde(default)]
    pub battery_status: Option<u16>,
    #[serde(default)]
    pub battery_pf_status: Option<u32>,
    #[serde(default)]
    pub battery_operation_status: Option<u32>,
    #[serde(default)]
    pub battery_stop_charging_threshold: Option<u8>,
    #[serde(default)]
    pub energy_save_default_charge_limit: Option<u8>,
    #[serde(default)]
    pub energy_save_default_discharge_limit: Option<u8>,
    #[serde(default)]
    pub raw_config: Vec<u8>,
}

impl DchuConfig {
    pub fn keyboard_type(&self) -> Option<u8> {
        self.raw_config.get(0x0f).copied().or(self.kbtp)
    }

    pub fn keyboard_lighting_capabilities(&self) -> KeyboardLightingCapabilities {
        let layout = match self.keyboard_type() {
            Some(0) => KeyboardLightingLayout::Unsupported,
            Some(1) => KeyboardLightingLayout::White,
            Some(2) => KeyboardLightingLayout::ThreeZone,
            Some(3 | 19 | 35 | 51 | 243) => KeyboardLightingLayout::PerKey,
            Some(6 | 22) => KeyboardLightingLayout::SingleZone,
            Some(_) | None => KeyboardLightingLayout::Unknown,
        };

        KeyboardLightingCapabilities {
            layout,
            logo: capability_bit(self.psf2, 18),
            lightbar: capability_bit(self.psf2, 12),
        }
    }

    pub fn cpu_fan_curve_anchor(&self) -> Option<FanCurvePoint> {
        fan_curve_anchor(&self.raw_config, 16)
    }

    pub fn gpu_fan_curve_anchor(&self) -> Option<FanCurvePoint> {
        fan_curve_anchor(&self.raw_config, 24)
    }

    #[cfg(any(debug_assertions, test))]
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

    #[cfg(any(debug_assertions, test))]
    pub fn custom_fan_table_capability(&self) -> Option<bool> {
        let fan_setting = self.fan_speed_setting_capability()?;
        let fan_count = self.fan_count()?;
        let custom_disabled = self.custom_fan_disabled_by_config()?;
        Some(fan_setting && fan_count > 1 && !custom_disabled)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn legacy_gpu_mux_capability(&self) -> Option<bool> {
        capability_bit(self.psf2, 20)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn new_gpu_mux_capability(&self) -> Option<bool> {
        self.bios_feature_offset18.map(|value| value & 0x01 != 0)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn gpu_mux_capability(&self) -> Option<bool> {
        any_known_capability(&[
            self.legacy_gpu_mux_capability(),
            self.new_gpu_mux_capability(),
        ])
    }

    #[cfg(any(debug_assertions, test))]
    pub fn gpu_oc_capability(&self) -> Option<bool> {
        any_known_capability(&[
            capability_bit(self.psf5, 5),
            capability_bit(self.psf2, 26),
            capability_bit(self.psf2, 27),
        ])
    }

    #[cfg(any(debug_assertions, test))]
    pub fn cpu_oc_capability(&self) -> Option<bool> {
        any_known_capability(&[capability_bit(self.psf5, 6), capability_bit(self.psf2, 23)])
    }

    #[cfg(any(debug_assertions, test))]
    pub fn xmp_capability(&self) -> Option<bool> {
        capability_bit(self.psf2, 24)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn energy_save_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 8)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn battery_utility_capability(&self) -> Option<bool> {
        capability_bit(self.psf5, 9)
    }

    pub fn battery_saver_capability(&self) -> Option<bool> {
        if self.bios_feature_version != Some(0x0100) {
            return None;
        }
        self.bios_feature_offset15
            .map(|value| value & (1 << 2) != 0)
    }

    pub fn battery_saver_enabled(&self) -> Option<bool> {
        if self.battery_saver_capability() != Some(true) {
            return None;
        }
        match self.battery_saver_status? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn battery_info_available(&self) -> bool {
        self.battery_full_charge_capacity.unwrap_or_default() > 0
            && self.battery_design_capacity.unwrap_or_default() > 0
    }

    pub fn battery_health_percent(&self) -> Option<u8> {
        if !self.battery_info_available() {
            return None;
        }
        let full = u32::from(self.battery_full_charge_capacity?);
        let design = u32::from(self.battery_design_capacity?);
        Some(((full * 100 + design / 2) / design).min(100) as u8)
    }

    #[cfg(test)]
    pub fn battery_manufacture_date(&self) -> Option<(u16, u8, u8)> {
        let raw = self.battery_manufacture_date_raw?;
        let day = (raw & 0x1f) as u8;
        let month = ((raw >> 5) & 0x0f) as u8;
        let year = 1980 + (raw >> 9);
        (day > 0 && day <= 31 && month > 0 && month <= 12).then_some((year, month, day))
    }

    #[cfg(any(debug_assertions, test))]
    pub fn anti_dust_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 7)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn fan_offset_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 10).map(|disabled| !disabled)
    }

    #[cfg(any(debug_assertions, test))]
    pub fn dtt_capability(&self) -> Option<bool> {
        capability_bit(self.psf4, 12)
    }

    fn supports_power_mode_ui(&self) -> bool {
        self.power_mode_capability().unwrap_or(true)
    }

    fn supports_fan_mode_ui(&self) -> bool {
        self.fan_speed_setting_capability().unwrap_or(true)
    }

    #[cfg(any(debug_assertions, test))]
    fn custom_fan_disabled_by_config(&self) -> Option<bool> {
        self.raw_config
            .get(0x2b)
            .map(|value| ((value >> 1) & 1) == 1)
    }
}

fn fan_curve_anchor(buffer: &[u8], offset: usize) -> Option<FanCurvePoint> {
    let temp_celsius = *buffer.get(offset)?;
    let duty_raw = *buffer.get(offset + 1)?;
    if temp_celsius >= FAN_CURVE_MAX_TEMP - 2 {
        return None;
    }
    Some(FanCurvePoint {
        temp_celsius,
        duty_percent: ((u16::from(duty_raw) * 100 + 127) / 255) as u8,
    })
}

fn capability_bit(value: Option<u32>, bit: u32) -> Option<bool> {
    value.map(|value| (value & (1u32 << bit)) != 0)
}

#[cfg(any(debug_assertions, test))]
fn any_known_capability(values: &[Option<bool>]) -> Option<bool> {
    if values.contains(&Some(true)) {
        Some(true)
    } else if values.iter().all(Option::is_some) {
        Some(false)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BatteryCapacityUnit {
    #[serde(rename = "Mah")]
    MilliampHours,
    #[serde(rename = "Mwh")]
    MilliwattHours,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BatteryChargeStatus {
    #[serde(rename = "Charging")]
    Charging,
    #[serde(rename = "Discharging")]
    Discharging,
    #[serde(rename = "Full")]
    Full,
    #[serde(rename = "Not charging")]
    NotCharging,
    #[serde(rename = "Unknown")]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemBatteryInfo {
    #[serde(default)]
    pub charge_percent: Option<u8>,
    #[serde(default)]
    pub status: Option<BatteryChargeStatus>,
    #[serde(default)]
    pub full_capacity: Option<u64>,
    #[serde(default)]
    pub design_capacity: Option<u64>,
    #[serde(default)]
    pub capacity_unit: Option<BatteryCapacityUnit>,
}

impl SystemBatteryInfo {
    pub(crate) fn has_data(&self) -> bool {
        self.charge_percent.is_some()
            || self.status.is_some()
            || self.full_capacity.is_some()
            || self.design_capacity.is_some()
    }

    pub fn health_percent(&self) -> Option<u8> {
        let full = u128::from(self.full_capacity?);
        let design = u128::from(self.design_capacity?);
        if design == 0 {
            return None;
        }
        Some(((full * 100 + design / 2) / design).min(100) as u8)
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
    #[serde(default)]
    pub system_battery: Option<SystemBatteryInfo>,
    pub battery_voltage_raw: u16,
    pub battery_rate_raw: u16,
    pub thermal_raw: [u8; 4],
    pub updated_unix_secs: u64,
}

impl HardwareSnapshot {
    #[cfg(any(debug_assertions, test))]
    pub fn diagnostic_report(&self) -> String {
        let mut report = String::from("DCHU hardware snapshot\n");
        for fan in &self.fans {
            let _ = writeln!(
                report,
                "{}: {} RPM, raw_tach={}, temperature={} C",
                fan.label,
                fan.rpm,
                fan.raw_tach,
                fan.temperature_celsius
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "--".to_owned())
            );
        }
        let _ = writeln!(report, "battery_voltage_raw: {}", self.battery_voltage_raw);
        let _ = writeln!(report, "battery_rate_raw: {}", self.battery_rate_raw);
        let _ = writeln!(
            report,
            "thermal_raw: {:02x} {:02x} {:02x} {:02x}",
            self.thermal_raw[0], self.thermal_raw[1], self.thermal_raw[2], self.thermal_raw[3]
        );
        let _ = writeln!(report, "status_buffer:");
        for (row, chunk) in self.raw_status.chunks(16).enumerate() {
            let _ = write!(report, "{:04x}: ", row * 16);
            for byte in chunk {
                let _ = write!(report, "{byte:02x} ");
            }
            report.push('\n');
        }
        report
    }
}

pub fn available_fan_modes(snapshot: Option<&HardwareSnapshot>) -> Vec<FanMode> {
    let Some(config) = snapshot.and_then(|snapshot| snapshot.dchu_config.as_ref()) else {
        return FALLBACK_FAN_MODES.to_vec();
    };

    if !config.supports_fan_mode_ui() {
        return Vec::new();
    }

    let mut modes = vec![FanMode::Auto, FanMode::Max];
    if config.silent_fan_capability().unwrap_or(false) {
        modes.push(FanMode::Silent);
    }
    if config.maxq_fan_capability().unwrap_or(false) {
        modes.push(FanMode::MaxQ);
    }
    modes
}

pub fn available_power_modes(snapshot: Option<&HardwareSnapshot>) -> &'static [PowerMode] {
    let Some(config) = snapshot.and_then(|snapshot| snapshot.dchu_config.as_ref()) else {
        return &POWER_MODES;
    };

    if config.supports_power_mode_ui() {
        &POWER_MODES
    } else {
        &NO_POWER_MODES
    }
}

pub fn selected_fan_mode_from_snapshot(snapshot: Option<&HardwareSnapshot>) -> Option<FanMode> {
    match snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .and_then(|config| config.app_fan_mode)
    {
        Some(0) => Some(FanMode::Auto),
        Some(1) => Some(FanMode::Max),
        Some(3) => Some(FanMode::Silent),
        Some(5) => Some(FanMode::MaxQ),
        Some(6) => Some(FanMode::Custom),
        _ => None,
    }
}

pub fn selected_power_mode_from_snapshot(snapshot: Option<&HardwareSnapshot>) -> Option<PowerMode> {
    match snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .and_then(|config| config.app_power_mode)
    {
        Some(0) => Some(PowerMode::Quiet),
        Some(1) => Some(PowerMode::PowerSaving),
        Some(2) => Some(PowerMode::Performance),
        Some(3) => Some(PowerMode::Entertainment),
        _ => None,
    }
}

pub fn available_gpu_mux_modes(_snapshot: Option<&HardwareSnapshot>) -> Vec<GpuMuxMode> {
    // Clevo BIOS metadata for MUX support is inconsistent across models; the
    // write path only accepts known official target values and reports errors.
    GPU_MUX_MODES.to_vec()
}

pub fn selected_gpu_mux_mode_from_snapshot(
    snapshot: Option<&HardwareSnapshot>,
) -> Option<GpuMuxMode> {
    match snapshot
        .and_then(|snapshot| snapshot.dchu_config.as_ref())
        .and_then(|config| config.gpu_mux_current)
    {
        Some(2) => Some(GpuMuxMode::DGpu),
        Some(3) => Some(GpuMuxMode::MSHybrid),
        _ => None,
    }
}
#[cfg(test)]
mod tests;

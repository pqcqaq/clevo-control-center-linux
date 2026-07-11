mod cli;
mod io;

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

pub use cli::{print_dchu_usage, run_dchu_cli};
pub(crate) use io::{dchu_control_write, fan_curve_points_arg};
pub use io::{fan_rpm_from_tach, read_hardware_snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanMode {
    Auto,
    Max,
    Silent,
    MaxQ,
    Custom,
}

impl FanMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动",
            Self::Max => "最大",
            Self::Silent => "静音",
            Self::MaxQ => "MaxQ",
            Self::Custom => "自定义",
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "安静",
            Self::PowerSaving => "省电",
            Self::Performance => "性能",
            Self::Entertainment => "娱乐",
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

impl GpuMuxMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::MSHybrid => "混合模式",
            Self::DGpu => "独显直连",
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
    if values.contains(&Some(true)) {
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

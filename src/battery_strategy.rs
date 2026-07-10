use serde::{Deserialize, Serialize};

pub const CHARGE_START_MIN: u8 = 40;
pub const CHARGE_START_MAX: u8 = 95;
pub const CHARGE_STOP_MIN: u8 = 50;
pub const CHARGE_STOP_MAX: u8 = 100;
pub const LOW_BATTERY_MIN: u8 = 5;
pub const LOW_BATTERY_MAX: u8 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BatteryStrategyPreset {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "care")]
    Care,
    #[serde(rename = "endurance")]
    Endurance,
}

impl BatteryStrategyPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "标准",
            Self::Care => "保养",
            Self::Endurance => "续航",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Standard => "接近原厂默认，尽量保持满电。",
            Self::Care => "降低长期满电时间，适合长期插电。",
            Self::Endurance => "更激进地限制充电上限，偏向电池寿命。",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Standard, Self::Care, Self::Endurance]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatteryStrategySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_preset")]
    pub preset: BatteryStrategyPreset,
    #[serde(default = "default_charge_start")]
    pub charge_start_percent: u8,
    #[serde(default = "default_charge_stop")]
    pub charge_stop_percent: u8,
    #[serde(default)]
    pub energy_save_on_battery: bool,
    #[serde(default = "default_low_battery_protection")]
    pub low_battery_protection: bool,
    #[serde(default = "default_low_battery_threshold")]
    pub low_battery_threshold_percent: u8,
    #[serde(default)]
    pub reduce_keyboard_brightness: bool,
}

impl Default for BatteryStrategySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: BatteryStrategyPreset::Standard,
            charge_start_percent: default_charge_start(),
            charge_stop_percent: default_charge_stop(),
            energy_save_on_battery: false,
            low_battery_protection: true,
            low_battery_threshold_percent: default_low_battery_threshold(),
            reduce_keyboard_brightness: false,
        }
    }
}

impl BatteryStrategySettings {
    pub fn sanitized(mut self) -> Self {
        self.charge_start_percent = self
            .charge_start_percent
            .clamp(CHARGE_START_MIN, CHARGE_START_MAX);
        self.charge_stop_percent = self
            .charge_stop_percent
            .clamp(CHARGE_STOP_MIN, CHARGE_STOP_MAX);
        if self.charge_start_percent >= self.charge_stop_percent {
            self.charge_start_percent = self.charge_stop_percent.saturating_sub(5);
        }
        self.charge_start_percent = self
            .charge_start_percent
            .clamp(CHARGE_START_MIN, CHARGE_START_MAX);
        self.low_battery_threshold_percent = self
            .low_battery_threshold_percent
            .clamp(LOW_BATTERY_MIN, LOW_BATTERY_MAX);
        self
    }

    pub fn apply_preset(&mut self, preset: BatteryStrategyPreset) {
        *self = match preset {
            BatteryStrategyPreset::Standard => Self {
                enabled: self.enabled,
                preset,
                charge_start_percent: 95,
                charge_stop_percent: 100,
                energy_save_on_battery: false,
                low_battery_protection: true,
                low_battery_threshold_percent: 20,
                reduce_keyboard_brightness: false,
            },
            BatteryStrategyPreset::Care => Self {
                enabled: self.enabled,
                preset,
                charge_start_percent: 45,
                charge_stop_percent: 80,
                energy_save_on_battery: false,
                low_battery_protection: true,
                low_battery_threshold_percent: 25,
                reduce_keyboard_brightness: true,
            },
            BatteryStrategyPreset::Endurance => Self {
                enabled: self.enabled,
                preset,
                charge_start_percent: 40,
                charge_stop_percent: 70,
                energy_save_on_battery: true,
                low_battery_protection: true,
                low_battery_threshold_percent: 30,
                reduce_keyboard_brightness: true,
            },
        }
    }
}

fn default_preset() -> BatteryStrategyPreset {
    BatteryStrategyPreset::Standard
}

fn default_charge_start() -> u8 {
    95
}

fn default_charge_stop() -> u8 {
    100
}

fn default_low_battery_protection() -> bool {
    true
}

fn default_low_battery_threshold() -> u8 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_strategy_sanitizes_thresholds() {
        let settings = BatteryStrategySettings {
            charge_start_percent: 120,
            charge_stop_percent: 60,
            low_battery_threshold_percent: 1,
            ..BatteryStrategySettings::default()
        }
        .sanitized();

        assert_eq!(settings.charge_start_percent, 55);
        assert_eq!(settings.charge_stop_percent, 60);
        assert_eq!(settings.low_battery_threshold_percent, LOW_BATTERY_MIN);
    }

    #[test]
    fn battery_strategy_presets_keep_enabled_state() {
        let mut settings = BatteryStrategySettings {
            enabled: true,
            ..BatteryStrategySettings::default()
        };

        settings.apply_preset(BatteryStrategyPreset::Care);

        assert!(settings.enabled);
        assert_eq!(settings.preset, BatteryStrategyPreset::Care);
        assert_eq!(settings.charge_stop_percent, 80);
        assert!(settings.reduce_keyboard_brightness);
    }
}

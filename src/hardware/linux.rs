use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::HardwareBackend;
use crate::dchu::{
    self, BatteryCapacityUnit, BatteryChargeStatus, FanMode, GpuMuxMode, HardwareSnapshot,
    PowerMode, SystemBatteryInfo,
};
use crate::fan_curve::FanCurveProfile;
use crate::model::{LightingFrame, ZoneColor, BASE_ZONES};

const DEFAULT_LIGHTING_PROC_PATH: &str = "/proc/clevo_control_center_led";

pub(super) struct LinuxHardwareBackend {
    lighting_path: PathBuf,
}

impl LinuxHardwareBackend {
    pub(super) fn new() -> Self {
        Self {
            lighting_path: std::env::var_os("CLEVO_KBD_LED_PROC")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_LIGHTING_PROC_PATH)),
        }
    }

    fn apply_lighting_commands(&self, commands: Vec<String>) -> io::Result<()> {
        for command in commands {
            fs::write(&self.lighting_path, command)?;
        }
        Ok(())
    }
}

impl HardwareBackend for LinuxHardwareBackend {
    fn lighting_ready(&self) -> bool {
        self.lighting_path.exists()
            && fs::OpenOptions::new()
                .write(true)
                .open(&self.lighting_path)
                .is_ok()
    }

    fn set_lighting_brightness(&self, percent: u8) -> Result<(), String> {
        self.apply_lighting_commands(vec![format!("brightness {percent}\n")])
            .map_err(|err| err.to_string())
    }

    fn apply_lighting_frame(&self, frame: &LightingFrame) -> Result<(), String> {
        self.apply_lighting_commands(color_commands(&frame.colors))
            .map_err(|err| err.to_string())
    }

    fn read_snapshot(&self) -> Result<HardwareSnapshot, String> {
        let mut snapshot = dchu::read_hardware_snapshot()?;
        snapshot.system_battery = read_system_battery_info();
        Ok(snapshot)
    }

    fn set_fan_mode(&self, mode: FanMode) -> Result<(), String> {
        dchu::dchu_control_write(&format!("fan-mode {}", mode.value()))
    }

    fn set_power_mode(&self, mode: PowerMode) -> Result<(), String> {
        dchu::dchu_control_write(&format!("power-mode {}", mode.value()))
    }

    fn set_fan_curve(&self, profile: &FanCurveProfile) -> Result<(), String> {
        let cpu = dchu::fan_curve_points_arg(&profile.cpu)?;
        let gpu = dchu::fan_curve_points_arg(&profile.gpu)?;
        dchu::dchu_control_write(&format!("fan-curve {cpu} {gpu}"))
    }

    fn set_gpu_mux(&self, mode: GpuMuxMode) -> Result<(), String> {
        dchu::dchu_control_write(&format!("gpu-mux {}", mode.value()))
    }

    fn set_battery_saver(&self, enabled: bool) -> Result<(), String> {
        dchu::dchu_control_write(if enabled {
            "battery-saver on"
        } else {
            "battery-saver off"
        })
    }
}

fn read_system_battery_info() -> Option<SystemBatteryInfo> {
    let root = std::env::var_os("CLEVO_POWER_SUPPLY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/sys/class/power_supply"));
    read_system_battery_info_from(&root)
}

fn read_system_battery_info_from(root: &Path) -> Option<SystemBatteryInfo> {
    let mut supply_paths = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    supply_paths.sort();
    let battery_path = supply_paths.into_iter().find(|path| {
        fs::read_to_string(path.join("type"))
            .is_ok_and(|supply_type| supply_type.trim() == "Battery")
    })?;
    let read_text = |name: &str| {
        fs::read_to_string(battery_path.join(name))
            .ok()
            .map(|value| value.trim().to_owned())
    };
    let read_u64 = |name: &str| read_text(name)?.parse::<u64>().ok();

    let (full_capacity, design_capacity, capacity_unit) =
        match (read_u64("charge_full"), read_u64("charge_full_design")) {
            (Some(full), Some(design)) if full > 0 && design > 0 => (
                Some(full / 1000),
                Some(design / 1000),
                Some(BatteryCapacityUnit::MilliampHours),
            ),
            _ => match (read_u64("energy_full"), read_u64("energy_full_design")) {
                (Some(full), Some(design)) if full > 0 && design > 0 => (
                    Some(full / 1000),
                    Some(design / 1000),
                    Some(BatteryCapacityUnit::MilliwattHours),
                ),
                _ => (None, None, None),
            },
        };

    let info = SystemBatteryInfo {
        charge_percent: read_u64("capacity")
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| *value <= 100),
        status: read_text("status").and_then(|value| match value.as_str() {
            "Charging" => Some(BatteryChargeStatus::Charging),
            "Discharging" => Some(BatteryChargeStatus::Discharging),
            "Full" => Some(BatteryChargeStatus::Full),
            "Not charging" => Some(BatteryChargeStatus::NotCharging),
            "Unknown" => Some(BatteryChargeStatus::Unknown),
            _ => None,
        }),
        full_capacity,
        design_capacity,
        capacity_unit,
    };
    info.has_data().then_some(info)
}

fn color_commands(colors: &[ZoneColor]) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Rgb, ZoneId};

    fn temporary_power_supply_root(test_name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clevo-control-center-{test_name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create power_supply test root");
        root
    }

    #[test]
    fn reads_charge_capacity_from_first_battery_supply() {
        let root = temporary_power_supply_root("charge-battery");
        let adapter = root.join("AC");
        let battery = root.join("BAT0");
        fs::create_dir_all(&adapter).expect("create adapter directory");
        fs::create_dir_all(&battery).expect("create battery directory");
        fs::write(adapter.join("type"), "Mains\n").expect("write adapter type");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");
        fs::write(battery.join("capacity"), "64\n").expect("write charge percent");
        fs::write(battery.join("status"), "Discharging\n").expect("write battery status");
        fs::write(battery.join("charge_full"), "2197000\n").expect("write full capacity");
        fs::write(battery.join("charge_full_design"), "3410000\n").expect("write design capacity");

        let info = read_system_battery_info_from(&root).expect("read battery info");
        assert_eq!(info.charge_percent, Some(64));
        assert_eq!(info.status, Some(BatteryChargeStatus::Discharging));
        assert_eq!(info.full_capacity, Some(2197));
        assert_eq!(info.design_capacity, Some(3410));
        assert_eq!(info.capacity_unit, Some(BatteryCapacityUnit::MilliampHours));

        fs::remove_dir_all(root).expect("remove power_supply test root");
    }

    #[test]
    fn falls_back_to_energy_capacity_and_rejects_invalid_percent() {
        let root = temporary_power_supply_root("energy-battery");
        let battery = root.join("BAT1");
        fs::create_dir_all(&battery).expect("create battery directory");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");
        fs::write(battery.join("capacity"), "101\n").expect("write invalid percent");
        fs::write(battery.join("energy_full"), "45000000\n").expect("write full energy");
        fs::write(battery.join("energy_full_design"), "60000000\n").expect("write design energy");

        let info = read_system_battery_info_from(&root).expect("read battery info");
        assert_eq!(info.charge_percent, None);
        assert_eq!(info.full_capacity, Some(45000));
        assert_eq!(info.design_capacity, Some(60000));
        assert_eq!(
            info.capacity_unit,
            Some(BatteryCapacityUnit::MilliwattHours)
        );

        fs::remove_dir_all(root).expect("remove power_supply test root");
    }

    #[test]
    fn ignores_battery_supply_without_readable_data() {
        let root = temporary_power_supply_root("empty-battery");
        let battery = root.join("BAT0");
        fs::create_dir_all(&battery).expect("create battery directory");
        fs::write(battery.join("type"), "Battery\n").expect("write battery type");

        assert!(read_system_battery_info_from(&root).is_none());

        fs::remove_dir_all(root).expect("remove power_supply test root");
    }

    #[test]
    fn serializes_base_zone_frame_with_same_color_as_one_proc_write() {
        let colors = BASE_ZONES
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: Rgb { r: 255, g: 0, b: 0 },
            })
            .collect::<Vec<_>>();
        assert_eq!(color_commands(&colors), vec!["ff0000\n"]);
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

        assert_eq!(color_commands(&colors), vec!["f0 ff0000\n", "f2 0000ff\n"]);
    }

    #[test]
    fn empty_frame_emits_no_proc_writes() {
        assert!(color_commands(&[]).is_empty());
    }
}

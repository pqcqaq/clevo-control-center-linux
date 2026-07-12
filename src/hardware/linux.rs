use std::fs;
use std::io;
use std::path::PathBuf;

use super::HardwareBackend;
use crate::dchu::{self, FanMode, GpuMuxMode, HardwareSnapshot, PowerMode};
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
        dchu::read_hardware_snapshot()
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

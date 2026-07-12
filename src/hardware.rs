mod linux;

use crate::dchu::{FanMode, GpuMuxMode, HardwareSnapshot, PowerMode};
use crate::fan_curve::FanCurveProfile;
use crate::model::LightingFrame;

pub trait HardwareBackend: Send + Sync {
    fn lighting_ready(&self) -> bool;
    fn set_lighting_brightness(&self, percent: u8) -> Result<(), String>;
    fn apply_lighting_frame(&self, frame: &LightingFrame) -> Result<(), String>;
    fn read_snapshot(&self) -> Result<HardwareSnapshot, String>;
    fn set_fan_mode(&self, mode: FanMode) -> Result<(), String>;
    fn set_power_mode(&self, mode: PowerMode) -> Result<(), String>;
    fn set_fan_curve(&self, profile: &FanCurveProfile) -> Result<(), String>;
    fn set_gpu_mux(&self, mode: GpuMuxMode) -> Result<(), String>;
    fn set_battery_saver(&self, enabled: bool) -> Result<(), String>;
}

pub fn native_backend() -> Box<dyn HardwareBackend> {
    Box::new(linux::LinuxHardwareBackend::new())
}

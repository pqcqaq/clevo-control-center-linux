mod persistence;
mod window;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use crate::battery_strategy::BatteryStrategySettings;
use crate::dchu::{self, FanMode, GpuMuxMode, HardwareSnapshot, PowerMode};
use crate::fan_curve::{
    default_fan_curve_profiles, FanCurveSelection, FanCurveSettings, FAN_CURVE_COUNT,
};
use crate::hardware::HardwareBackend;
use crate::model::{normalize_zones, AdvancedTab, ControlPage, Mode, Rgb, ZoneColor, ZoneId};
use crate::settings::{
    atomic_write_hardware_snapshot, file_modified, hardware_snapshot_path, Settings,
};

pub struct ClevoLedApp {
    hardware_backend: Box<dyn HardwareBackend>,
    settings_path: PathBuf,
    settings_mtime: Option<SystemTime>,
    hardware_snapshot_path: PathBuf,
    hardware_snapshot_mtime: Option<SystemTime>,
    pub(super) hardware: Option<HardwareSnapshot>,
    pub(super) hardware_status: Option<String>,
    last_hardware_sync: Instant,
    pub(super) active_page: ControlPage,
    pub(super) advanced_tab: AdvancedTab,
    pub(super) mode: Mode,
    pub(super) speed: u8,
    pub(super) brightness: u8,
    pub(super) running: bool,
    pub(super) f0_color: Rgb,
    pub(super) zones: Vec<ZoneId>,
    pub(super) fan_curves: FanCurveSettings,
    pub(super) fan_curve_draft: FanCurveSettings,
    pub(super) fan_curve_tab: usize,
    pub(super) fan_curve_selection: Option<FanCurveSelection>,
    pub(super) battery_strategy: BatteryStrategySettings,
    pub(super) last_error: Option<String>,
    pub(super) command_output: String,
    pub(super) command_status: Option<String>,
    pub(super) pending_gpu_mux_mode: Option<GpuMuxMode>,
    window_pos: Option<[f32; 2]>,
    dirty_settings: bool,
    dirty_window_position: bool,
    last_settings_save: Instant,
}

impl ClevoLedApp {
    pub fn new(
        settings_path: PathBuf,
        settings: Settings,
        hardware_backend: Box<dyn HardwareBackend>,
    ) -> Self {
        if !hardware_backend.lighting_ready() {
            eprintln!("Keyboard RGB interface is not writable");
        }

        let hardware_snapshot_path = hardware_snapshot_path();
        let hardware = persistence::wait_for_hardware_snapshot(
            &hardware_snapshot_path,
            Duration::from_millis(1200),
        );
        let hardware_snapshot_mtime = file_modified(&hardware_snapshot_path);
        let hardware_status = if hardware.is_none() {
            Some("正在等待风扇数据".to_owned())
        } else {
            None
        };
        let mut app = Self {
            hardware_backend,
            settings_path,
            settings_mtime: None,
            hardware_snapshot_path,
            hardware_snapshot_mtime,
            hardware,
            hardware_status,
            last_hardware_sync: Instant::now() - Duration::from_secs(2),
            active_page: ControlPage::Overview,
            advanced_tab: AdvancedTab::Fans,
            mode: settings.mode,
            speed: settings.speed,
            brightness: settings.brightness,
            running: settings.running,
            f0_color: settings.f0_color,
            zones: settings.zones,
            fan_curves: settings.fan_curves.clone(),
            fan_curve_draft: settings.fan_curves,
            fan_curve_tab: 0,
            fan_curve_selection: None,
            battery_strategy: settings.battery_strategy,
            last_error: None,
            command_output: String::new(),
            command_status: None,
            pending_gpu_mux_mode: None,
            window_pos: settings.window_pos,
            dirty_settings: false,
            dirty_window_position: false,
            last_settings_save: Instant::now(),
        };
        app.settings_mtime = file_modified(&app.settings_path);
        app
    }

    pub(super) fn toggle(&mut self) {
        if self.mode == Mode::Custom {
            return;
        }

        self.running = !self.running;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn selected_zones(&self) -> Vec<ZoneId> {
        normalize_zones(&self.zones)
    }

    pub(super) fn set_zone_enabled(&mut self, zone: ZoneId, enabled: bool) {
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

    pub(super) fn set_fan_curve_enabled(&mut self, enabled: bool) {
        self.fan_curves.enabled = enabled;
        self.fan_curve_draft.enabled = enabled;
        if !enabled {
            self.fan_curves.selected_profile = None;
            self.fan_curve_draft.selected_profile = None;
        }
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn select_fan_curve_profile(&mut self, index: usize) {
        if !self.fan_curves.enabled || index >= FAN_CURVE_COUNT {
            return;
        }
        let Some(profile) = self.fan_curves.profiles.get(index).cloned() else {
            return;
        };
        if let Err(err) = self.hardware_backend.set_fan_curve(&profile) {
            self.command_status = Some("自定义曲线应用失败".to_owned());
            self.command_output = err;
            return;
        }

        self.refresh_hardware_snapshot(false);
        self.fan_curves.selected_profile = Some(index);
        self.fan_curve_draft.selected_profile = Some(index);
        self.command_status = Some(format!("已应用 {}", FanCurveSettings::profile_label(index)));
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn clear_selected_fan_curve_profile(&mut self) {
        if self.fan_curves.selected_profile.is_some() {
            self.fan_curves.selected_profile = None;
            self.fan_curve_draft.selected_profile = None;
            self.mark_settings_dirty();
            self.persist_settings_if_due(true);
        }
    }

    pub(super) fn save_fan_curve_draft(&mut self) {
        self.fan_curve_draft = self.fan_curve_draft.clone().sanitized();
        self.fan_curves = self.fan_curve_draft.clone();
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn restore_fan_curve_draft(&mut self) {
        self.fan_curve_draft = self.fan_curves.clone();
        self.fan_curve_selection = None;
    }

    pub(super) fn reset_current_fan_curve_profile(&mut self) {
        if self.fan_curve_tab >= FAN_CURVE_COUNT {
            return;
        }
        let defaults = default_fan_curve_profiles();
        if let Some(profile) = defaults.get(self.fan_curve_tab) {
            self.fan_curve_draft.profiles[self.fan_curve_tab] = profile.clone();
            self.fan_curve_selection = None;
        }
    }

    pub(super) fn set_battery_strategy_enabled(&mut self, enabled: bool) {
        self.battery_strategy.enabled = enabled;
        self.save_battery_strategy();
    }

    pub(super) fn save_battery_strategy(&mut self) {
        self.battery_strategy = self.battery_strategy.clone().sanitized();
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn write_selected_color(&mut self, rgb: Rgb) {
        let colors = self
            .selected_zones()
            .into_iter()
            .map(|zone| ZoneColor { zone, rgb })
            .collect::<Vec<_>>();

        if let Err(err) = self.hardware_backend.write_lighting(&colors) {
            self.last_error = Some(err.clone());
            self.running = false;
            eprintln!("Failed to write selected color: {err}");
        }
    }

    pub(super) fn show_hardware_diagnostics(&mut self) {
        match self.hardware_backend.read_snapshot() {
            Ok(snapshot) => {
                if let Err(err) =
                    atomic_write_hardware_snapshot(&self.hardware_snapshot_path, &snapshot)
                {
                    eprintln!(
                        "Failed to write {}: {err}",
                        self.hardware_snapshot_path.display()
                    );
                }
                self.command_status = Some("硬件状态读取完成".to_owned());
                self.command_output = snapshot.diagnostic_report();
                self.hardware = Some(snapshot);
                self.hardware_status = None;
                self.hardware_snapshot_mtime = file_modified(&self.hardware_snapshot_path);
            }
            Err(err) => {
                self.command_status = Some("硬件状态读取失败".to_owned());
                self.command_output = err;
            }
        }
    }

    pub(super) fn set_power_mode(&mut self, mode: PowerMode) {
        match self.hardware_backend.set_power_mode(mode) {
            Ok(()) => {
                self.command_status = Some(format!("已切换到{}模式", mode.label()));
                self.command_output.clear();
                self.refresh_hardware_snapshot(false);
            }
            Err(err) => {
                self.command_status = Some("电源模式切换失败".to_owned());
                self.command_output = err;
            }
        }
    }

    pub(super) fn set_fan_mode(&mut self, mode: FanMode) {
        match self.hardware_backend.set_fan_mode(mode) {
            Ok(()) => {
                self.command_status = Some(format!("已切换到{}模式", mode.label()));
                self.command_output.clear();
                self.refresh_hardware_snapshot(false);
            }
            Err(err) => {
                self.command_status = Some("风扇模式切换失败".to_owned());
                self.command_output = err;
            }
        }
    }

    pub(super) fn request_gpu_mux_switch(&mut self, mode: GpuMuxMode) {
        if dchu::selected_gpu_mux_mode_from_snapshot(self.hardware.as_ref()) == Some(mode) {
            self.pending_gpu_mux_mode = None;
            self.command_status = None;
            self.command_output.clear();
            return;
        }
        self.command_status = None;
        self.command_output.clear();
        self.pending_gpu_mux_mode = Some(mode);
    }

    pub(super) fn cancel_gpu_mux_switch(&mut self) {
        self.pending_gpu_mux_mode = None;
        self.command_status = None;
        self.command_output.clear();
    }

    pub(super) fn confirm_gpu_mux_switch_and_reboot(&mut self) {
        let Some(mode) = self.pending_gpu_mux_mode.take() else {
            return;
        };
        if let Err(err) = self.hardware_backend.set_gpu_mux(mode) {
            self.command_status = Some("显卡模式写入失败".to_owned());
            self.command_output = err;
            return;
        }

        self.command_status = None;
        self.command_output.clear();
        if let Err(err) = request_system_reboot() {
            self.command_status = Some("重启命令失败".to_owned());
            self.command_output = err;
        }
    }

    pub(super) fn refresh_hardware_snapshot(&mut self, user_visible: bool) {
        match self.hardware_backend.read_snapshot() {
            Ok(snapshot) => {
                if let Err(err) =
                    atomic_write_hardware_snapshot(&self.hardware_snapshot_path, &snapshot)
                {
                    eprintln!(
                        "Failed to write {}: {err}",
                        self.hardware_snapshot_path.display()
                    );
                }
                self.hardware = Some(snapshot);
                self.hardware_snapshot_mtime = file_modified(&self.hardware_snapshot_path);
                if user_visible {
                    self.hardware_status = Some("硬件状态已更新".to_owned());
                } else {
                    self.hardware_status = None;
                }
            }
            Err(err) => {
                if user_visible || self.hardware.is_none() {
                    self.hardware_status = Some(format!("硬件状态暂不可用: {err}"));
                }
            }
        }
    }
}

fn request_system_reboot() -> Result<(), String> {
    match Command::new("systemctl").arg("reboot").spawn() {
        Ok(_) => Ok(()),
        Err(systemctl_err) => Command::new("shutdown")
            .args(["-r", "now"])
            .spawn()
            .map(|_| ())
            .map_err(|shutdown_err| {
                format!(
                    "systemctl reboot failed: {systemctl_err}; shutdown -r now failed: {shutdown_err}"
                )
            }),
    }
}

mod persistence;
mod window;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use crate::battery_strategy::BatteryStrategySettings;
use crate::dchu::{
    self, FanMode, GpuMuxMode, HardwareSnapshot, KeyboardLightingCapabilities,
    KeyboardLightingLayout, PowerMode,
};
use crate::fan_curve::{
    default_fan_curve_profiles, FanCurveSelection, FanCurveSettings, FAN_CURVE_COUNT,
};
use crate::hardware::HardwareBackend;
#[cfg(debug_assertions)]
use crate::model::AdvancedTab;
use crate::model::{normalize_zones, ControlPage, Mode, Rgb, ZoneId, BASE_ZONES};
use crate::module_loader::ModuleState;
use crate::preferences::{LanguagePreference, ThemeColor, UiLanguage};
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
    #[cfg(debug_assertions)]
    pub(super) advanced_tab: AdvancedTab,
    pub(super) mode: Mode,
    pub(super) brightness: u8,
    pub(super) f0_color: Rgb,
    pub(super) zones: Vec<ZoneId>,
    pub(super) fan_curves: FanCurveSettings,
    pub(super) fan_curve_draft: FanCurveSettings,
    pub(super) fan_curve_tab: usize,
    pub(super) fan_curve_selection: Option<FanCurveSelection>,
    pub(super) battery_strategy: BatteryStrategySettings,
    pub(super) language_preference: LanguagePreference,
    pub(super) language: UiLanguage,
    pub(super) theme_color: ThemeColor,
    pub(super) command_output: String,
    pub(super) command_status: Option<String>,
    pub(super) pending_gpu_mux_mode: Option<GpuMuxMode>,
    pub(super) color_picker_open: bool,
    pub(super) color_picker_draft: Rgb,
    pub(super) module_prompt: Option<ModuleState>,
    pub(super) module_error: Option<String>,
    window_pos: Option<[f32; 2]>,
    dirty_settings: bool,
    dirty_window_position: bool,
    last_settings_save: Instant,
    pub(super) first_run_pending: bool,
    hardware_runtime_started: bool,
    pub(super) first_run_error: Option<String>,
}

impl ClevoLedApp {
    pub fn new(
        settings_path: PathBuf,
        settings: Settings,
        hardware_backend: Box<dyn HardwareBackend>,
        first_run: bool,
    ) -> Self {
        let language = settings.language.resolved();
        let hardware_snapshot_path = hardware_snapshot_path();
        let initial_module_state = (!first_run).then(crate::module_loader::module_state);
        let hardware_runtime_started = matches!(initial_module_state, Some(ModuleState::Ready));
        if hardware_runtime_started {
            crate::service::ensure_service_running();
        }
        let hardware = if first_run {
            None
        } else {
            persistence::wait_for_hardware_snapshot(
                &hardware_snapshot_path,
                Duration::from_millis(1200),
            )
        };
        let hardware_snapshot_mtime = file_modified(&hardware_snapshot_path);
        let hardware_status = if hardware.is_none() {
            Some(
                language
                    .pick("正在等待风扇数据", "Waiting for fan data")
                    .to_owned(),
            )
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
            #[cfg(debug_assertions)]
            advanced_tab: AdvancedTab::Fans,
            mode: settings.mode,
            brightness: settings.brightness,
            f0_color: settings.f0_color,
            zones: settings.zones,
            fan_curves: settings.fan_curves.clone(),
            fan_curve_draft: settings.fan_curves,
            fan_curve_tab: 0,
            fan_curve_selection: None,
            battery_strategy: settings.battery_strategy,
            language_preference: settings.language,
            language,
            theme_color: settings.theme_color,
            command_output: String::new(),
            command_status: None,
            pending_gpu_mux_mode: None,
            color_picker_open: false,
            color_picker_draft: settings.f0_color,
            module_prompt: initial_module_state.filter(|state| *state != ModuleState::Ready),
            module_error: None,
            window_pos: settings.window_pos,
            dirty_settings: false,
            dirty_window_position: false,
            last_settings_save: Instant::now(),
            first_run_pending: first_run,
            hardware_runtime_started,
            first_run_error: None,
        };
        app.settings_mtime = file_modified(&app.settings_path);
        app.sync_fan_curve_firmware_anchors();
        app
    }

    fn start_hardware_runtime(&mut self) {
        if self.hardware_runtime_started {
            return;
        }
        match crate::module_loader::module_state() {
            ModuleState::Ready => {
                self.module_prompt = None;
                self.module_error = None;
            }
            state => {
                self.module_prompt = Some(state);
                return;
            }
        }
        if !self.hardware_backend.lighting_ready() {
            eprintln!("Keyboard RGB interface is not writable");
        }
        crate::service::ensure_service_running();
        self.hardware_runtime_started = true;
    }

    pub(super) fn process_module_request(&mut self) {
        self.module_error = None;
        match crate::module_loader::load_module_with_auth(self.language) {
            Ok(()) if crate::module_loader::module_state() == ModuleState::Ready => {
                self.start_hardware_runtime();
                self.refresh_hardware_snapshot(false);
            }
            Ok(()) => {
                let state = crate::module_loader::module_state();
                self.module_prompt = Some(state);
                self.module_error = Some(
                    self.language
                        .pick(
                            "处理命令已完成，但模块仍不可用或版本过旧。请检查内核头文件后重试。",
                            "The command completed, but the module is still unavailable or outdated. Check the kernel headers and try again.",
                        )
                        .to_owned(),
                );
            }
            Err(err) => {
                self.module_error = Some(match self.language {
                    UiLanguage::SimplifiedChinese => format!("模块加载或更新失败：{err}"),
                    UiLanguage::English => format!("Module loading or update failed: {err}"),
                });
            }
        }
    }

    pub(super) fn open_color_picker(&mut self) {
        self.color_picker_draft = self.f0_color;
        self.color_picker_open = true;
    }

    pub(super) fn cancel_color_picker(&mut self) {
        self.color_picker_open = false;
        self.color_picker_draft = self.f0_color;
    }

    pub(super) fn apply_color_picker(&mut self) {
        self.f0_color = self.color_picker_draft;
        self.color_picker_open = false;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn accept_first_run_disclaimer(&mut self) {
        let settings = Settings {
            mode: self.mode,
            brightness: self.brightness,
            f0_color: self.f0_color,
            zones: self.selected_zones(),
            fan_curves: self.fan_curves.clone(),
            battery_strategy: self.battery_strategy.clone(),
            language: self.language_preference,
            theme_color: self.theme_color,
            window_pos: self.window_pos,
        }
        .sanitized();

        match crate::settings::atomic_write_settings(&self.settings_path, &settings) {
            Ok(()) => {
                self.settings_mtime = file_modified(&self.settings_path);
                self.first_run_pending = false;
                self.first_run_error = None;
                self.start_hardware_runtime();
            }
            Err(err) => {
                self.first_run_error = Some(match self.language {
                    UiLanguage::SimplifiedChinese => {
                        format!("无法保存首次启动确认：{err}")
                    }
                    UiLanguage::English => {
                        format!("Could not save first-run confirmation: {err}")
                    }
                });
            }
        }
    }

    pub(super) fn selected_zones(&self) -> Vec<ZoneId> {
        let capabilities = self.keyboard_lighting_capabilities();
        let mut zones = match capabilities.layout {
            KeyboardLightingLayout::SingleZone => vec![ZoneId::F0],
            KeyboardLightingLayout::ThreeZone | KeyboardLightingLayout::Unknown => BASE_ZONES
                .into_iter()
                .filter(|zone| self.zones.contains(zone))
                .collect(),
            KeyboardLightingLayout::Unsupported
            | KeyboardLightingLayout::White
            | KeyboardLightingLayout::PerKey => normalize_zones(&self.zones),
        };
        if zones.is_empty() {
            zones.extend(BASE_ZONES);
        }
        if capabilities.lightbar == Some(true) && self.zones.contains(&ZoneId::F3) {
            zones.push(ZoneId::F3);
        }
        if capabilities.logo == Some(true) && self.zones.contains(&ZoneId::F6) {
            zones.push(ZoneId::F6);
        }
        zones
    }

    pub(super) fn keyboard_lighting_capabilities(&self) -> KeyboardLightingCapabilities {
        self.hardware
            .as_ref()
            .and_then(|snapshot| snapshot.dchu_config.as_ref())
            .map(|config| config.keyboard_lighting_capabilities())
            .unwrap_or_default()
    }

    pub(super) fn sync_fan_curve_firmware_anchors(&mut self) {
        let Some(config) = self
            .hardware
            .as_ref()
            .and_then(|snapshot| snapshot.dchu_config.as_ref())
        else {
            return;
        };
        let (Some(cpu), Some(gpu)) = (config.cpu_fan_curve_anchor(), config.gpu_fan_curve_anchor())
        else {
            return;
        };

        self.fan_curves.apply_firmware_anchors(cpu, gpu);
        self.fan_curve_draft.apply_firmware_anchors(cpu, gpu);
        if config.app_fan_mode != Some(6) {
            self.fan_curves.selected_profile = None;
            self.fan_curve_draft.selected_profile = None;
        }
    }

    pub(super) fn set_language_preference(&mut self, preference: LanguagePreference) {
        if self.language_preference == preference {
            return;
        }
        self.language_preference = preference;
        self.language = preference.resolved();
        if self.hardware.is_none() {
            self.hardware_status = Some(
                self.language
                    .pick("正在等待风扇数据", "Waiting for fan data")
                    .to_owned(),
            );
        }
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    pub(super) fn set_theme_color(&mut self, theme_color: ThemeColor) {
        if self.theme_color == theme_color {
            return;
        }
        self.theme_color = theme_color;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
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
            self.command_status = Some(
                self.language
                    .pick("自定义曲线应用失败", "Could not apply custom curve")
                    .to_owned(),
            );
            self.command_output = err;
            return;
        }

        self.refresh_hardware_snapshot(false);
        self.fan_curves.selected_profile = Some(index);
        self.fan_curve_draft.selected_profile = Some(index);
        self.command_status = Some(match self.language {
            UiLanguage::SimplifiedChinese => format!(
                "已应用 {}",
                FanCurveSettings::localized_profile_label(index, self.language)
            ),
            UiLanguage::English => format!(
                "Applied {}",
                FanCurveSettings::localized_profile_label(index, self.language)
            ),
        });
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
        self.sync_fan_curve_firmware_anchors();
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
            self.sync_fan_curve_firmware_anchors();
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

    #[cfg(debug_assertions)]
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
                self.command_status = Some(
                    self.language
                        .pick("硬件状态读取完成", "Hardware status read successfully")
                        .to_owned(),
                );
                self.command_output = snapshot.diagnostic_report();
                self.hardware = Some(snapshot);
                self.hardware_status = None;
                self.hardware_snapshot_mtime = file_modified(&self.hardware_snapshot_path);
            }
            Err(err) => {
                self.command_status = Some(
                    self.language
                        .pick("硬件状态读取失败", "Could not read hardware status")
                        .to_owned(),
                );
                self.command_output = err;
            }
        }
    }

    pub(super) fn set_power_mode(&mut self, mode: PowerMode) {
        match self.hardware_backend.set_power_mode(mode) {
            Ok(()) => {
                self.fan_curves.selected_profile = None;
                self.fan_curve_draft.selected_profile = None;
                self.mark_settings_dirty();
                self.persist_settings_if_due(true);
                self.command_status = Some(match self.language {
                    UiLanguage::SimplifiedChinese => {
                        format!("已切换到{}模式", mode.localized_label(self.language))
                    }
                    UiLanguage::English => {
                        format!("Switched to {} mode", mode.localized_label(self.language))
                    }
                });
                self.command_output.clear();
                self.refresh_hardware_snapshot(false);
            }
            Err(err) => {
                self.command_status = Some(
                    self.language
                        .pick("电源模式切换失败", "Could not switch power mode")
                        .to_owned(),
                );
                self.command_output = err;
            }
        }
    }

    pub(super) fn set_fan_mode(&mut self, mode: FanMode) {
        match self.hardware_backend.set_fan_mode(mode) {
            Ok(()) => {
                self.command_status = Some(match self.language {
                    UiLanguage::SimplifiedChinese => {
                        format!("已切换到{}模式", mode.localized_label(self.language))
                    }
                    UiLanguage::English => {
                        format!("Switched to {} mode", mode.localized_label(self.language))
                    }
                });
                self.command_output.clear();
                self.refresh_hardware_snapshot(false);
            }
            Err(err) => {
                self.command_status = Some(
                    self.language
                        .pick("风扇模式切换失败", "Could not switch fan mode")
                        .to_owned(),
                );
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
            self.command_status = Some(
                self.language
                    .pick("显卡模式写入失败", "Could not write graphics mode")
                    .to_owned(),
            );
            self.command_output = err;
            return;
        }

        self.command_status = None;
        self.command_output.clear();
        if let Err(err) = request_system_reboot() {
            self.command_status = Some(
                self.language
                    .pick("重启命令失败", "Could not start system restart")
                    .to_owned(),
            );
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
                self.sync_fan_curve_firmware_anchors();
                self.hardware_snapshot_mtime = file_modified(&self.hardware_snapshot_path);
                if user_visible {
                    self.hardware_status = Some(
                        self.language
                            .pick("硬件状态已更新", "Hardware status refreshed")
                            .to_owned(),
                    );
                } else {
                    self.hardware_status = None;
                }
            }
            Err(err) => {
                if user_visible || self.hardware.is_none() {
                    self.hardware_status = Some(match self.language {
                        UiLanguage::SimplifiedChinese => {
                            format!("硬件状态暂不可用: {err}")
                        }
                        UiLanguage::English => {
                            format!("Hardware status is unavailable: {err}")
                        }
                    });
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

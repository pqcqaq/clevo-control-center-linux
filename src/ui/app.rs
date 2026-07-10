use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{
    pos2, vec2, Align2, Button, CentralPanel, Color32, Context, FontId, Frame, Rect, RichText,
    Sense, Stroke, Ui, ViewportCommand,
};

use super::layout;
use crate::dchu::{self, HardwareSnapshot};
use crate::fan_curve::{
    default_fan_curve_profiles, FanCurveSelection, FanCurveSettings, FAN_CURVE_COUNT,
};
use crate::led::LedWriter;
use crate::model::{normalize_zones, AdvancedTab, ControlPage, Mode, Rgb, ZoneColor, ZoneId};
use crate::settings::{
    atomic_write_hardware_snapshot, atomic_write_settings, file_modified, hardware_snapshot_path,
    load_hardware_snapshot, load_settings, Settings,
};

const BODY_HORIZONTAL_MARGIN: f32 = 12.0;

pub struct ClevoLedApp {
    writer: LedWriter,
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
    pub(super) last_error: Option<String>,
    pub(super) command_output: String,
    pub(super) command_status: Option<String>,
    window_pos: Option<[f32; 2]>,
    dirty_settings: bool,
    dirty_window_position: bool,
    last_settings_save: Instant,
}

impl ClevoLedApp {
    pub fn new(settings_path: PathBuf, settings: Settings) -> Self {
        let writer = LedWriter::new();
        if !writer.ready() {
            eprintln!(
                "Keyboard RGB interface is not writable: {}",
                writer.proc_path().display()
            );
        }

        let hardware_snapshot_path = hardware_snapshot_path();
        let hardware =
            wait_for_hardware_snapshot(&hardware_snapshot_path, Duration::from_millis(1200));
        let hardware_snapshot_mtime = file_modified(&hardware_snapshot_path);
        let hardware_status = if hardware.is_none() {
            Some("正在等待风扇数据".to_owned())
        } else {
            None
        };
        let mut app = Self {
            writer,
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
            last_error: None,
            command_output: String::new(),
            command_status: None,
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
        self.fan_curves.selected_profile = Some(index);
        self.fan_curve_draft.selected_profile = Some(index);
        self.command_status = Some(format!(
            "已选择 {}（本地配置，未写入 EC）",
            FanCurveSettings::profile_label(index)
        ));
        self.command_output.clear();
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

    pub(super) fn write_selected_color(&mut self, rgb: Rgb) {
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

    pub(super) fn run_dchu_read(&mut self, command: &str) {
        self.run_dchu_command(&["dchu", command]);
    }

    pub(super) fn run_dchu_write(&mut self, args: &[&str]) {
        let mut command_args = vec!["dchu"];
        command_args.extend_from_slice(args);
        self.run_dchu_command(&command_args);
    }

    fn run_dchu_command(&mut self, args: &[&str]) {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(err) => {
                self.command_status = Some("无法定位程序".to_owned());
                self.command_output = err.to_string();
                return;
            }
        };

        let output = Command::new(exe).args(args).output();
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.command_output = format!("{stdout}{stderr}");
                self.command_status = Some(if output.status.success() {
                    "命令执行完成".to_owned()
                } else {
                    format!("命令失败: {}", output.status)
                });
                if output.status.success() {
                    self.refresh_hardware_snapshot(false);
                }
            }
            Err(err) => {
                self.command_status = Some("命令启动失败".to_owned());
                self.command_output = err.to_string();
            }
        }
    }

    pub(super) fn refresh_hardware_snapshot(&mut self, user_visible: bool) {
        match dchu::read_hardware_snapshot() {
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

    fn sync_hardware_snapshot(&mut self) {
        if self.last_hardware_sync.elapsed() < Duration::from_millis(600) {
            return;
        }
        self.last_hardware_sync = Instant::now();

        let mtime = file_modified(&self.hardware_snapshot_path);
        if mtime.is_none() || mtime == self.hardware_snapshot_mtime {
            return;
        }

        if let Some(snapshot) = load_hardware_snapshot(&self.hardware_snapshot_path) {
            self.hardware = Some(snapshot);
            self.hardware_status = None;
            self.hardware_snapshot_mtime = mtime;
        }
    }

    pub(super) fn mark_settings_dirty(&mut self) {
        self.dirty_settings = true;
    }

    fn apply_external_settings(&mut self, settings: Settings) {
        self.mode = settings.mode;
        self.speed = settings.speed;
        self.brightness = settings.brightness;
        self.running = settings.running;
        self.f0_color = settings.f0_color;
        self.zones = settings.zones;
        self.fan_curves = settings.fan_curves.clone();
        self.fan_curve_draft = settings.fan_curves;
        self.fan_curve_selection = None;
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
                .map(|old| (old[0] - position[0]).abs() > 0.5 || (old[1] - position[1]).abs() > 0.5)
                .unwrap_or(true);
            if changed {
                self.window_pos = Some(position);
                self.dirty_window_position = true;
            }
        }
    }

    pub(super) fn persist_settings_if_due(&mut self, force: bool) {
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
                fan_curves: self.fan_curves.clone(),
                window_pos: self.window_pos,
            }
        } else {
            load_settings(&self.settings_path)
        };

        settings.window_pos = self.window_pos;
        settings = settings.sanitized();
        let should_apply_local_state = self.dirty_settings;

        match atomic_write_settings(&self.settings_path, &settings) {
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

fn wait_for_hardware_snapshot(path: &Path, timeout: Duration) -> Option<HardwareSnapshot> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(snapshot) = load_hardware_snapshot(path) {
            return Some(snapshot);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(75));
    }
}

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.sync_external_settings();
        self.sync_hardware_snapshot();
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(20, 20, 18)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    custom_title_bar(ui, ctx);
                    ui.add_space(8.0);
                    body_frame(ui, |ui| layout::control_center(ui, self));
                });
            });

        self.persist_settings_if_due(false);
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_settings_if_due(true);
    }
}

fn body_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .inner_margin(body_margin())
        .show(ui, add_contents);
}

fn body_margin() -> egui::Margin {
    egui::Margin::symmetric(BODY_HORIZONTAL_MARGIN, 0.0)
}

fn custom_title_bar(ui: &mut Ui, ctx: &Context) {
    const TITLE_BAR_HEIGHT: f32 = 38.0;
    const CLOSE_SIZE: f32 = 26.0;

    let width = ui.available_width().max(1.0);
    let (rect, drag_response) =
        ui.allocate_exact_size(vec2(width, TITLE_BAR_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 16));
    painter.line_segment(
        [
            pos2(rect.left(), rect.bottom()),
            pos2(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0, Color32::from_rgb(43, 40, 35)),
    );
    painter.text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Clevo Control Center",
        FontId::proportional(14.0),
        Color32::from_rgb(226, 219, 207),
    );

    let close_rect = Rect::from_min_size(
        pos2(rect.right() - CLOSE_SIZE - 10.0, rect.top() + 6.0),
        vec2(CLOSE_SIZE, CLOSE_SIZE),
    );
    let close_response = ui.put(
        close_rect,
        Button::new(
            RichText::new("x")
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(220, 214, 204)),
        )
        .fill(Color32::from_rgb(40, 37, 32))
        .stroke(Stroke::new(1.0, Color32::from_rgb(62, 56, 47))),
    );

    if close_response.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    } else if drag_response.drag_started() && !close_response.hovered() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_margin_only_adds_horizontal_padding() {
        let margin = body_margin();
        assert_eq!(margin.left, 12.0);
        assert_eq!(margin.right, 12.0);
        assert_eq!(margin.top, 0.0);
        assert_eq!(margin.bottom, 0.0);
    }
}

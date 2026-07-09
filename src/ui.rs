use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, pos2, vec2, Align2, Button, CentralPanel, Color32, ComboBox, Context, FontData,
    FontDefinitions, FontFamily, FontId, Frame, RichText, ScrollArea, Sense, Slider, Stroke, Ui,
    ViewportCommand,
};

use crate::dchu::{self, FanStatus, HardwareSnapshot};
use crate::led::LedWriter;
use crate::model::{normalize_zones, ControlPage, Mode, Rgb, ZoneColor, ALL_ZONES};
use crate::settings::{
    atomic_write_hardware_snapshot, atomic_write_settings, file_modified, hardware_snapshot_path,
    load_hardware_snapshot, load_settings, Settings,
};

pub struct ClevoLedApp {
    writer: LedWriter,
    settings_path: PathBuf,
    settings_mtime: Option<SystemTime>,
    hardware_snapshot_path: PathBuf,
    hardware_snapshot_mtime: Option<SystemTime>,
    hardware: Option<HardwareSnapshot>,
    hardware_status: Option<String>,
    last_hardware_sync: Instant,
    active_page: ControlPage,
    mode: Mode,
    speed: u8,
    brightness: u8,
    running: bool,
    f0_color: Rgb,
    zones: Vec<crate::model::ZoneId>,
    last_error: Option<String>,
    command_output: String,
    command_status: Option<String>,
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
        let hardware = load_hardware_snapshot(&hardware_snapshot_path);
        let hardware_snapshot_mtime = file_modified(&hardware_snapshot_path);
        let mut app = Self {
            writer,
            settings_path,
            settings_mtime: None,
            hardware_snapshot_path,
            hardware_snapshot_mtime,
            hardware,
            hardware_status: None,
            last_hardware_sync: Instant::now() - Duration::from_secs(2),
            active_page: ControlPage::Overview,
            mode: settings.mode,
            speed: settings.speed,
            brightness: settings.brightness,
            running: settings.running,
            f0_color: settings.f0_color,
            zones: settings.zones,
            last_error: None,
            command_output: String::new(),
            command_status: None,
            window_pos: settings.window_pos,
            dirty_settings: false,
            dirty_window_position: false,
            last_settings_save: Instant::now(),
        };
        app.settings_mtime = file_modified(&app.settings_path);
        if app.hardware.is_none() {
            app.refresh_hardware_snapshot(false);
        }
        app
    }

    fn toggle(&mut self) {
        if self.mode == Mode::Custom {
            return;
        }

        self.running = !self.running;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    fn selected_zones(&self) -> Vec<crate::model::ZoneId> {
        normalize_zones(&self.zones)
    }

    fn set_zone_enabled(&mut self, zone: crate::model::ZoneId, enabled: bool) {
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

    fn run_dchu_read(&mut self, command: &str) {
        self.run_dchu_command(&["dchu", command]);
    }

    fn run_dchu_write(&mut self, args: &[&str]) {
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

    fn refresh_hardware_snapshot(&mut self, user_visible: bool) {
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
                .map(|old| (old[0] - position[0]).abs() > 0.5 || (old[1] - position[1]).abs() > 0.5)
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

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.sync_external_settings();
        self.sync_hardware_snapshot();
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(14, 14, 13)))
            .show(ctx, |ui| {
                custom_title_bar(ui, ctx);
                control_center(ui, self);
            });

        self.persist_settings_if_due(false);
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_settings_if_due(true);
    }
}

fn custom_title_bar(ui: &mut Ui, ctx: &Context) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 42.0), Sense::click_and_drag());
    if response.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 14, 13));
    painter.text(
        pos2(rect.left() + 18.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Clevo Control Center",
        FontId::proportional(15.0),
        Color32::from_rgb(230, 226, 216),
    );

    let close_rect = egui::Rect::from_min_size(
        pos2(rect.right() - 42.0, rect.top() + 5.0),
        vec2(34.0, 32.0),
    );
    let close_response = ui.interact(close_rect, ui.id().with("close"), Sense::click());
    let close_fill = if close_response.hovered() {
        Color32::from_rgb(92, 45, 36)
    } else {
        Color32::from_rgb(32, 31, 29)
    };
    painter.circle_filled(close_rect.center(), 12.0, close_fill);
    painter.text(
        close_rect.center(),
        Align2::CENTER_CENTER,
        "x",
        FontId::proportional(15.0),
        Color32::from_rgb(230, 226, 216),
    );
    if close_response.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

fn control_center(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal(|ui| {
        sidebar(ui, app);
        ui.add_space(12.0);
        Frame::none()
            .fill(Color32::from_rgb(22, 22, 20))
            .rounding(10.0)
            .inner_margin(egui::Margin::same(16.0))
            .show(ui, |ui| {
                ui.set_min_width(730.0);
                ui.set_min_height(510.0);
                match app.active_page {
                    ControlPage::Overview => overview_page(ui, app),
                    ControlPage::Lighting => lighting_page(ui, app),
                    ControlPage::Performance => performance_page(ui, app),
                    ControlPage::Diagnostics => diagnostics_page(ui, app),
                    ControlPage::Settings => settings_page(ui, app),
                }
            });
    });
}

fn sidebar(ui: &mut Ui, app: &mut ClevoLedApp) {
    Frame::none()
        .fill(Color32::from_rgb(17, 17, 16))
        .inner_margin(egui::Margin::symmetric(12.0, 14.0))
        .show(ui, |ui| {
            ui.set_width(170.0);
            ui.add_space(4.0);
            ui.label(
                RichText::new("蓝天控制中心")
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(239, 234, 223)),
            );
            ui.label(
                RichText::new("Linux Edition")
                    .size(12.0)
                    .color(Color32::from_rgb(139, 133, 122)),
            );
            ui.add_space(20.0);
            for page in ControlPage::all() {
                let selected = app.active_page == *page;
                let fill = if selected {
                    Color32::from_rgb(55, 43, 27)
                } else {
                    Color32::from_rgb(27, 27, 25)
                };
                let text = if selected {
                    Color32::from_rgb(252, 235, 207)
                } else {
                    Color32::from_rgb(187, 180, 168)
                };
                if ui
                    .add_sized(
                        vec2(146.0, 34.0),
                        Button::new(RichText::new(page.label()).size(14.0).color(text)).fill(fill),
                    )
                    .clicked()
                {
                    app.active_page = *page;
                }
                ui.add_space(6.0);
            }
        });
}

fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(title)
            .size(24.0)
            .strong()
            .color(Color32::from_rgb(239, 234, 223)),
    );
    ui.label(
        RichText::new(subtitle)
            .size(13.0)
            .color(Color32::from_rgb(151, 145, 135)),
    );
    ui.add_space(14.0);
}

fn info_tile(ui: &mut Ui, title: &str, value: &str, accent: Color32) {
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_size(vec2(156.0, 72.0));
            ui.label(
                RichText::new(title)
                    .size(12.0)
                    .color(Color32::from_rgb(145, 138, 127)),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(value).size(20.0).strong().color(accent));
        });
}

fn overview_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "总览", "当前风扇转速和灯效配置");
    ui.horizontal(|ui| {
        info_tile(
            ui,
            "灯效模式",
            app.mode.label(),
            Color32::from_rgb(226, 184, 112),
        );
        info_tile(
            ui,
            "灯效状态",
            if app.running { "运行" } else { "停止" },
            Color32::from_rgb(166, 205, 141),
        );
        info_tile(
            ui,
            "亮度",
            &format!("{}%", app.brightness),
            Color32::from_rgb(232, 206, 149),
        );
        info_tile(
            ui,
            "分区",
            &format!("{} 个", app.selected_zones().len()),
            Color32::from_rgb(204, 176, 132),
        );
    });

    ui.add_space(18.0);
    let fans = overview_fans(app.hardware.as_ref());
    ui.columns(2, |columns| {
        fan_card(&mut columns[0], &fans[0]);
        fan_card(&mut columns[1], &fans[1]);
    });

    ui.add_space(14.0);
    if let Some(snapshot) = &app.hardware {
        ui.label(
            RichText::new(snapshot_age_text(snapshot))
                .size(12.0)
                .color(Color32::from_rgb(126, 120, 110)),
        );
    } else if let Some(status) = &app.hardware_status {
        ui.label(
            RichText::new(status)
                .size(12.0)
                .color(Color32::from_rgb(214, 157, 105)),
        );
    } else {
        ui.label(
            RichText::new("正在等待硬件状态")
                .size(12.0)
                .color(Color32::from_rgb(126, 120, 110)),
        );
    }
}

fn overview_fans(snapshot: Option<&HardwareSnapshot>) -> [FanStatus; 2] {
    let mut fans = [
        FanStatus {
            label: "CPU 风扇".to_owned(),
            rpm: 0,
        },
        FanStatus {
            label: "GPU 风扇".to_owned(),
            rpm: 0,
        },
    ];

    if let Some(snapshot) = snapshot {
        for (target, source) in fans.iter_mut().zip(snapshot.fans.iter()) {
            *target = source.clone();
        }
    }

    fans
}

fn fan_card(ui: &mut Ui, fan: &FanStatus) {
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(58, 53, 45)))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_min_size(vec2(300.0, 156.0));
            ui.label(
                RichText::new(&fan.label)
                    .size(15.0)
                    .strong()
                    .color(Color32::from_rgb(236, 230, 218)),
            );
            ui.add_space(14.0);
            ui.label(
                RichText::new(if fan.rpm == 0 {
                    "-- RPM".to_owned()
                } else {
                    format!("{} RPM", fan.rpm)
                })
                .size(36.0)
                .strong()
                .color(Color32::from_rgb(231, 176, 96)),
            );
            ui.add_space(12.0);
            let width = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(width, 12.0), Sense::hover());
            let fill_width = if fan.rpm == 0 {
                0.0
            } else {
                (fan.rpm as f32 / 5200.0).clamp(0.08, 1.0) * rect.width()
            };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 6.0, Color32::from_rgb(20, 20, 18));
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, vec2(fill_width, rect.height())),
                6.0,
                Color32::from_rgb(184, 126, 58),
            );
        });
}

fn lighting_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "灯光", "控制键盘 RGB 动态模式、速度和亮度");
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                color_swatch(ui, app);
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.set_width(330.0);
                    ComboBox::from_id_salt("mode")
                        .width(280.0)
                        .selected_text(app.mode.label())
                        .show_ui(ui, |ui| {
                            for mode in Mode::all() {
                                let old_mode = app.mode;
                                let clicked = ui
                                    .selectable_value(&mut app.mode, *mode, mode.label())
                                    .clicked();
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

                    ui.add_space(12.0);
                    lighting_slider(ui, "速度", &mut app.speed, app.mode != Mode::Custom);
                    ui.add_space(8.0);
                    lighting_slider(ui, "亮度", &mut app.brightness, app.mode != Mode::Custom);
                    if ui.ctx().input(|input| input.pointer.any_released()) {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }
                });
                ui.add_space(18.0);
                let label = if app.running {
                    "停止灯效"
                } else {
                    "启动灯效"
                };
                if ui
                    .add_enabled(
                        app.mode != Mode::Custom,
                        Button::new(RichText::new(label).size(15.0)).min_size(vec2(112.0, 42.0)),
                    )
                    .clicked()
                {
                    app.toggle();
                }
            });
        });

    if let Some(err) = &app.last_error {
        ui.add_space(12.0);
        ui.label(
            RichText::new(err)
                .size(12.0)
                .color(Color32::from_rgb(221, 126, 93)),
        );
    }
}

fn lighting_slider(ui: &mut Ui, label: &str, value: &mut u8, enabled: bool) {
    ui.horizontal(|ui| {
        ui.set_width(330.0);
        ui.label(
            RichText::new(label)
                .size(13.0)
                .color(Color32::from_rgb(193, 186, 173)),
        );
        ui.add_enabled_ui(enabled, |ui| {
            ui.add_sized(
                vec2(250.0, 20.0),
                Slider::new(value, 1..=100).show_value(true),
            );
        });
    });
}

fn performance_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "性能", "DCHU 电源档位和风扇策略");
    ui.horizontal(|ui| {
        control_group(
            ui,
            "电源模式",
            &[("安静", "0"), ("省电", "1"), ("性能", "2"), ("娱乐", "3")],
            |mode| {
                app.run_dchu_write(&["power-mode", mode, "--i-understand"]);
            },
        );
        ui.add_space(12.0);
        control_group(
            ui,
            "风扇模式",
            &[
                ("自动", "auto"),
                ("最大", "max"),
                ("静音", "silent"),
                ("MaxQ", "maxq"),
                ("Turbo", "turbo"),
            ],
            |mode| {
                app.run_dchu_write(&["fan-mode", mode, "--i-understand"]);
            },
        );
    });
    ui.add_space(12.0);
    command_panel(ui, app);
}

fn control_group<F: FnMut(&str)>(ui: &mut Ui, title: &str, items: &[(&str, &str)], mut action: F) {
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_width(330.0);
            ui.label(
                RichText::new(title)
                    .size(16.0)
                    .strong()
                    .color(Color32::from_rgb(236, 230, 218)),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for (label, value) in items {
                    if ui
                        .add_sized(vec2(86.0, 32.0), Button::new(*label))
                        .clicked()
                    {
                        action(value);
                    }
                }
            });
        });
}

fn diagnostics_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "诊断", "读取 DCHU 状态、能力位和原始风扇数据");
    ui.horizontal(|ui| {
        if ui
            .add_sized(vec2(120.0, 34.0), Button::new("状态"))
            .clicked()
        {
            app.run_dchu_read("status");
        }
        if ui
            .add_sized(vec2(120.0, 34.0), Button::new("风扇表"))
            .clicked()
        {
            app.run_dchu_read("fan-table");
        }
        if ui
            .add_sized(vec2(120.0, 34.0), Button::new("能力位"))
            .clicked()
        {
            app.run_dchu_read("caps");
        }
    });
    ui.add_space(12.0);
    command_panel(ui, app);
}

fn settings_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "设置", "选择键盘生效分区并查看硬件读回");
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("键盘分区")
                    .size(15.0)
                    .strong()
                    .color(Color32::from_rgb(236, 230, 218)),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for zone in ALL_ZONES {
                    let mut enabled = app.zones.contains(&zone);
                    let is_last_enabled = enabled && app.zones.len() == 1;
                    let response = ui
                        .add_enabled_ui(!is_last_enabled, |ui| {
                            ui.add_sized(
                                vec2(72.0, 30.0),
                                egui::Checkbox::new(&mut enabled, zone.label()),
                            )
                        })
                        .inner;
                    if response.changed() {
                        app.set_zone_enabled(zone, enabled);
                    }
                }
            });
        });

    ui.add_space(14.0);
    Frame::none()
        .fill(Color32::from_rgb(31, 31, 28))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("硬件读回")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(236, 230, 218)),
                );
                ui.add_space(10.0);
                if ui
                    .add_sized(vec2(118.0, 30.0), Button::new("更新状态"))
                    .clicked()
                {
                    app.refresh_hardware_snapshot(true);
                }
            });
            ui.add_space(10.0);
            hardware_details(ui, app);
        });
}

fn hardware_details(ui: &mut Ui, app: &ClevoLedApp) {
    if let Some(snapshot) = &app.hardware {
        ui.label(
            RichText::new(snapshot_age_text(snapshot))
                .size(12.0)
                .color(Color32::from_rgb(151, 145, 135)),
        );
        ui.add_space(8.0);
        for fan in &snapshot.fans {
            ui.label(format!("{}: {} RPM", fan.label, fan.rpm));
        }
        ui.label(format!(
            "battery_voltage_raw: {}",
            snapshot.battery_voltage_raw
        ));
        ui.label(format!("battery_rate_raw: {}", snapshot.battery_rate_raw));
        ui.label(format!(
            "thermal_raw: {:02x} {:02x} {:02x} {:02x}",
            snapshot.thermal_raw[0],
            snapshot.thermal_raw[1],
            snapshot.thermal_raw[2],
            snapshot.thermal_raw[3]
        ));
        if !snapshot.caps.is_empty() {
            ui.add_space(8.0);
            ui.label("caps:");
            for cap in &snapshot.caps {
                ui.label(format!("0x{:02x}: {}", cap.function, cap.summary));
            }
        }
        for err in &snapshot.errors {
            ui.label(
                RichText::new(err)
                    .size(12.0)
                    .color(Color32::from_rgb(221, 126, 93)),
            );
        }
    } else if let Some(status) = &app.hardware_status {
        ui.label(
            RichText::new(status)
                .size(12.0)
                .color(Color32::from_rgb(214, 157, 105)),
        );
    } else {
        ui.label("暂无硬件读回");
    }
}

fn command_panel(ui: &mut Ui, app: &mut ClevoLedApp) {
    if let Some(status) = &app.command_status {
        ui.label(
            RichText::new(status)
                .size(13.0)
                .color(Color32::from_rgb(226, 184, 112)),
        );
    }
    if !app.command_output.is_empty() {
        Frame::none()
            .fill(Color32::from_rgb(12, 12, 11))
            .rounding(8.0)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    ui.monospace(&app.command_output);
                });
            });
    }
}

fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(62.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 23.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(12, 12, 11));
    painter.circle_stroke(
        center,
        radius + 4.0,
        Stroke::new(1.0, Color32::from_rgb(70, 64, 54)),
    );
    painter.circle_filled(center, radius, rgb_color32(app.f0_color));

    if response.hovered() && app.mode == Mode::Custom {
        painter.circle_stroke(
            center,
            radius + 6.0,
            Stroke::new(1.5, Color32::from_rgb(214, 157, 92)),
        );
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

fn rgb_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn snapshot_age_text(snapshot: &HardwareSnapshot) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let age = now.saturating_sub(snapshot.updated_unix_secs);
    format!("硬件状态更新于 {age} 秒前")
}

fn open_native_color_picker(current: Rgb) -> io::Result<Option<Rgb>> {
    let current_hex = format!("#{:02x}{:02x}{:02x}", current.r, current.g, current.b);

    if command_exists("zenity") {
        let output = Command::new("zenity")
            .args([
                "--color-selection",
                "--show-palette",
                "--color",
                &current_hex,
            ])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(
            &output.stdout,
        )));
    }

    if command_exists("kdialog") {
        let output = Command::new("kdialog")
            .args(["--getcolor", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(
            &output.stdout,
        )));
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

    if let Some(body) = text
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
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

pub fn install_cjk_font(ctx: &Context) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_color_picker_outputs() {
        assert_eq!(
            parse_color_picker_output("#0c2238\n"),
            Some(Rgb {
                r: 12,
                g: 34,
                b: 56
            })
        );
        assert_eq!(
            parse_color_picker_output("rgb(12,34,56)\n"),
            Some(Rgb {
                r: 12,
                g: 34,
                b: 56
            })
        );
        assert_eq!(parse_color_picker_output(""), None);
    }
}

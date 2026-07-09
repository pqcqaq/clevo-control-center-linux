use eframe::egui::{vec2, Button, Color32, ComboBox, Frame, RichText, Slider, Ui};

use super::app::ClevoLedApp;
use super::widgets::{
    color_swatch, command_panel, control_group, fan_card, hardware_details, info_tile, page_header,
    snapshot_age_text,
};
use crate::dchu::{FanStatus, HardwareSnapshot};
use crate::model::{ControlPage, Mode, ALL_ZONES};

pub(super) fn show_active_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    match app.active_page {
        ControlPage::Overview => overview_page(ui, app),
        ControlPage::Lighting => lighting_page(ui, app),
        ControlPage::Performance => performance_page(ui, app),
        ControlPage::Diagnostics => diagnostics_page(ui, app),
        ControlPage::Settings => settings_page(ui, app),
    }
}

fn overview_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "总览", "风扇转速和当前灯效配置");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
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
    let width = fan_card_width(ui.available_width());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
        for fan in &fans {
            fan_card(ui, fan, width);
        }
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

fn fan_card_width(available_width: f32) -> f32 {
    if available_width >= 560.0 {
        ((available_width - 12.0) / 2.0).max(260.0)
    } else {
        available_width.max(260.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_card_width_keeps_two_cards_visible_when_space_allows() {
        assert_eq!(fan_card_width(700.0), 344.0);
        assert_eq!(fan_card_width(520.0), 520.0);
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

fn lighting_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "灯光", "控制键盘 RGB 动态模式、速度和亮度");
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(18.0, 14.0);
                color_swatch(ui, app);
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
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
        control_group(
            ui,
            "电源模式",
            &[("安静", "0"), ("省电", "1"), ("性能", "2"), ("娱乐", "3")],
            |mode| {
                app.run_dchu_write(&["power-mode", mode, "--i-understand"]);
            },
        );
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

fn diagnostics_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "诊断", "读取 DCHU 状态、能力位和原始风扇数据");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
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
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
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
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                ui.label(
                    RichText::new("硬件读回")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(236, 230, 218)),
                );
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

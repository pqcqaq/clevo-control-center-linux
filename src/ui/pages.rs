use eframe::egui::{vec2, Button, Color32, ComboBox, Frame, RichText, Slider, Ui};

use super::advanced;
use super::app::ClevoLedApp;
use super::widgets::{
    color_swatch, command_panel, control_group, fan_gauge, hardware_details, page_header,
};
use crate::dchu::{FanStatus, HardwareSnapshot};
use crate::model::{AdvancedTab, ControlPage, Mode, ALL_ZONES};

const GAUGE_GAP: f32 = 18.0;
const MIN_GAUGE_WIDTH: f32 = 260.0;

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
    let fans = overview_fans(app.hardware.as_ref());
    let available_width = ui.available_width();
    let columns = overview_gauge_columns(available_width, fans.len());
    let width = overview_gauge_width(available_width, columns);

    if columns > 1 {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, GAUGE_GAP);
            for chunk in fans.chunks(columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(GAUGE_GAP, 0.0);
                    for fan in chunk {
                        fan_gauge(ui, fan, width);
                    }
                });
            }
        });
    } else {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, GAUGE_GAP);
            for fan in &fans {
                fan_gauge(ui, fan, width);
            }
        });
    }

    ui.add_space(26.0);
    overview_controls(ui, app);
    ui.add_space(18.0);
    overview_advanced(ui, app);
}

fn overview_gauge_columns(available_width: f32, fan_count: usize) -> usize {
    let max_columns = fan_count.clamp(1, 3);
    for columns in (1..=max_columns).rev() {
        let required_width = MIN_GAUGE_WIDTH * columns as f32 + GAUGE_GAP * (columns - 1) as f32;
        if available_width >= required_width {
            return columns;
        }
    }
    1
}

fn overview_gauge_width(available_width: f32, columns: usize) -> f32 {
    let columns = columns.max(1);
    let gap_width = GAUGE_GAP * (columns - 1) as f32;
    ((available_width - gap_width) / columns as f32).max(MIN_GAUGE_WIDTH)
}

fn overview_controls(ui: &mut Ui, app: &mut ClevoLedApp) {
    overview_lighting_mode(ui, app);
    ui.add_space(12.0);
    overview_button_row(
        ui,
        "电源模式",
        &[("安静", "0"), ("省电", "1"), ("性能", "2"), ("娱乐", "3")],
        |mode| app.run_dchu_write(&["power-mode", mode, "--i-understand"]),
    );
    ui.add_space(12.0);
    overview_button_row(
        ui,
        "风扇模式",
        &[
            ("自动", "auto"),
            ("最大", "max"),
            ("静音", "silent"),
            ("MaxQ", "maxq"),
            ("Turbo", "turbo"),
        ],
        |mode| app.run_dchu_write(&["fan-mode", mode, "--i-understand"]),
    );
}

fn overview_lighting_mode(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        overview_row_label(ui, "灯光效果");
        ComboBox::from_id_salt("overview-lighting-mode")
            .width(190.0)
            .selected_text(app.mode.label())
            .show_ui(ui, |ui| {
                for mode in Mode::all() {
                    let old_mode = app.mode;
                    ui.selectable_value(&mut app.mode, *mode, mode.label());
                    if app.mode != old_mode {
                        if app.mode == Mode::Custom {
                            app.running = false;
                            app.write_selected_color(app.f0_color);
                        }
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }
                }
            });
    });
}

fn overview_button_row<F: FnMut(&str)>(
    ui: &mut Ui,
    title: &str,
    items: &[(&str, &str)],
    mut action: F,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        overview_row_label(ui, title);
        for (label, value) in items {
            if ui
                .add_sized(vec2(76.0, 30.0), Button::new(*label))
                .clicked()
            {
                action(value);
            }
        }
    });
}

fn overview_row_label(ui: &mut Ui, label: &str) {
    ui.add_sized(
        vec2(78.0, 30.0),
        egui::Label::new(
            RichText::new(label)
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(222, 214, 199)),
        ),
    );
}

fn overview_advanced(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("高级")
                .size(15.0)
                .strong()
                .color(Color32::from_rgb(236, 230, 218)),
        );
        let label = if app.overview_advanced_open {
            "收起"
        } else {
            "展开"
        };
        if ui.add_sized(vec2(74.0, 30.0), Button::new(label)).clicked() {
            app.overview_advanced_open = !app.overview_advanced_open;
        }
    });

    if !app.overview_advanced_open {
        return;
    }

    ui.add_space(8.0);
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for tab in AdvancedTab::all() {
                    let selected = app.overview_advanced_tab == *tab;
                    let fill = if selected {
                        Color32::from_rgb(69, 51, 28)
                    } else {
                        Color32::from_rgb(28, 27, 24)
                    };
                    if ui
                        .add_sized(vec2(104.0, 30.0), Button::new(tab.label()).fill(fill))
                        .clicked()
                    {
                        app.overview_advanced_tab = *tab;
                    }
                }
            });
            ui.add_space(12.0);
            match app.overview_advanced_tab {
                AdvancedTab::Fans => advanced::fan_info(ui, app.hardware.as_ref()),
                AdvancedTab::Temperatures => advanced::temperature_info(ui, app.hardware.as_ref()),
                AdvancedTab::Other => advanced::other_info(ui, app.hardware.as_ref()),
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_gauge_width_keeps_two_gauges_visible_when_space_allows() {
        assert_eq!(overview_gauge_columns(700.0, 2), 2);
        assert_eq!(overview_gauge_width(700.0, 2), 341.0);
        assert_eq!(overview_gauge_columns(520.0, 2), 1);
        assert_eq!(overview_gauge_width(520.0, 1), 520.0);
    }

    #[test]
    fn overview_gauge_columns_supports_optional_pch_fan() {
        assert_eq!(overview_gauge_columns(900.0, 3), 3);
        assert_eq!(overview_gauge_columns(700.0, 3), 2);
        assert_eq!(overview_gauge_columns(520.0, 3), 1);
    }
}

fn overview_fans(snapshot: Option<&HardwareSnapshot>) -> Vec<FanStatus> {
    let mut fans = vec![
        FanStatus {
            label: "CPU 风扇".to_owned(),
            raw_tach: 0,
            rpm: 0,
            temperature_celsius: None,
        },
        FanStatus {
            label: "GPU 风扇".to_owned(),
            raw_tach: 0,
            rpm: 0,
            temperature_celsius: None,
        },
    ];

    if let Some(snapshot) = snapshot.filter(|snapshot| !snapshot.fans.is_empty()) {
        fans = snapshot.fans.clone();
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
    page_header(ui, "诊断", "读取 DCHU 只读硬件状态");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        if ui
            .add_sized(vec2(120.0, 34.0), Button::new("状态"))
            .clicked()
        {
            app.run_dchu_read("status");
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

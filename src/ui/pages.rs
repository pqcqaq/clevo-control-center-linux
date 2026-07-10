use eframe::egui::{
    pos2, vec2, Align2, Button, Color32, ComboBox, FontId, Frame, Pos2, Rect, RichText, Sense,
    Shape, Slider, Stroke, Ui,
};

use super::advanced;
use super::app::ClevoLedApp;
use super::widgets::{
    color_swatch, command_panel, control_group, fan_gauge, hardware_details, page_header,
};
use crate::dchu::{
    available_fan_modes, selected_fan_mode_from_snapshot, selected_power_mode_from_snapshot,
    FanStatus, HardwareSnapshot,
};
use crate::model::{AdvancedTab, ControlPage, Mode, ALL_ZONES};

const GAUGE_GAP: f32 = 8.0;
const GAUGE_EDGE_GUARD: f32 = 8.0;
const MIN_GAUGE_WIDTH: f32 = 224.0;
const MAX_GAUGE_WIDTH: f32 = 236.0;
const OVERVIEW_ACTION_WIDTH: f32 = 88.0;
const OVERVIEW_ACTION_HEIGHT: f32 = 34.0;
const OVERVIEW_ACTION_SKEW: f32 = 10.0;
const OVERVIEW_SECTION_MARGIN: f32 = 14.0;

pub(super) fn show_active_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    match app.active_page {
        ControlPage::Overview => overview_page(ui, app),
        ControlPage::Lighting => lighting_page(ui, app),
        ControlPage::Performance => performance_page(ui, app),
        ControlPage::Diagnostics => diagnostics_page(ui, app),
        ControlPage::Settings => settings_page(ui, app),
        ControlPage::Advanced => advanced_page(ui, app),
    }
}

fn overview_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    overview_section(ui, "风扇阵列", "FAN ARRAY", |ui| {
        overview_gauges(ui, app)
    });
    ui.add_space(14.0);
    overview_section(ui, "控制矩阵", "CONTROL MATRIX", |ui| {
        overview_controls(ui, app)
    });
}

fn overview_section(ui: &mut Ui, title: &str, code: &str, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(Color32::from_rgb(31, 30, 27))
        .stroke(Stroke::new(1.0, Color32::from_rgb(54, 49, 40)))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(OVERVIEW_SECTION_MARGIN))
        .show(ui, |ui| {
            overview_section_title(ui, title, code);
            ui.add_space(10.0);
            add_contents(ui);
        });
}

fn overview_section_title(ui: &mut Ui, title: &str, code: &str) {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let accent = Color32::from_rgb(214, 157, 92);

    painter.text(
        pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        Color32::from_rgb(236, 230, 218),
    );
    painter.text(
        pos2(rect.left() + 88.0, rect.center().y + 0.5),
        Align2::LEFT_CENTER,
        code,
        FontId::proportional(10.0),
        Color32::from_rgb(151, 145, 135),
    );
    painter.line_segment(
        [
            pos2(rect.left() + 178.0, rect.center().y),
            pos2(rect.right() - 36.0, rect.center().y),
        ],
        Stroke::new(1.0, Color32::from_rgb(63, 57, 46)),
    );
    painter.line_segment(
        [
            pos2(rect.right() - 28.0, rect.center().y),
            pos2(rect.right() - 3.0, rect.center().y),
        ],
        Stroke::new(2.0, accent),
    );
}

fn overview_gauges(ui: &mut Ui, app: &ClevoLedApp) {
    let fans = overview_fans(app.hardware.as_ref());
    let available_width = ui.available_width();
    let columns = overview_gauge_columns(available_width, fans.len());
    let width = overview_gauge_width(available_width, columns);

    if columns > 1 {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, GAUGE_GAP);
            for chunk in fans.chunks(columns) {
                let row_width = overview_gauge_row_width(width, chunk.len());
                let leading_space = overview_gauge_leading_space(available_width, row_width);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(GAUGE_GAP, 0.0);
                    ui.add_space(leading_space);
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
                let row_width = overview_gauge_row_width(width, 1);
                let leading_space = overview_gauge_leading_space(available_width, row_width);
                ui.horizontal(|ui| {
                    ui.add_space(leading_space);
                    fan_gauge(ui, fan, width);
                });
            }
        });
    }
}

fn overview_gauge_columns(available_width: f32, fan_count: usize) -> usize {
    let max_columns = fan_count.clamp(1, 3);
    let usable_width = (available_width - GAUGE_EDGE_GUARD).max(0.0);
    for columns in (1..=max_columns).rev() {
        let required_width = MIN_GAUGE_WIDTH * columns as f32 + GAUGE_GAP * (columns - 1) as f32;
        if usable_width >= required_width {
            return columns;
        }
    }
    1
}

fn overview_gauge_width(available_width: f32, columns: usize) -> f32 {
    let columns = columns.max(1);
    let usable_width = (available_width - GAUGE_EDGE_GUARD).max(MIN_GAUGE_WIDTH);
    let gap_width = GAUGE_GAP * (columns - 1) as f32;
    ((usable_width - gap_width) / columns as f32)
        .floor()
        .min(MAX_GAUGE_WIDTH)
        .max(MIN_GAUGE_WIDTH)
}

fn overview_gauge_row_width(gauge_width: f32, columns: usize) -> f32 {
    let columns = columns.max(1);
    gauge_width * columns as f32 + GAUGE_GAP * (columns - 1) as f32
}

fn overview_gauge_leading_space(available_width: f32, row_width: f32) -> f32 {
    ((available_width - row_width) * 0.5).max(0.0)
}

fn overview_controls(ui: &mut Ui, app: &mut ClevoLedApp) {
    let selected_power_mode = selected_power_mode_from_snapshot(app.hardware.as_ref());
    overview_button_row(
        ui,
        "电源模式",
        "POWER",
        &[("安静", "0"), ("省电", "1"), ("性能", "2"), ("娱乐", "3")],
        selected_power_mode,
        |mode| app.run_dchu_write(&["power-mode", mode, "--i-understand"]),
    );
    ui.add_space(14.0);
    overview_control_separator(ui);
    ui.add_space(14.0);
    let fan_modes = overview_fan_mode_items(app.hardware.as_ref());
    let selected_fan_mode = selected_fan_mode_from_snapshot(app.hardware.as_ref());
    overview_button_row(
        ui,
        "风扇模式",
        "FAN",
        &fan_modes,
        selected_fan_mode,
        |mode| app.run_dchu_write(&["fan-mode", mode, "--i-understand"]),
    );
}

fn overview_fan_mode_items(
    snapshot: Option<&HardwareSnapshot>,
) -> Vec<(&'static str, &'static str)> {
    available_fan_modes(snapshot)
        .iter()
        .map(|mode| (mode.label, mode.value))
        .collect()
}

fn overview_button_row<F: FnMut(&str)>(
    ui: &mut Ui,
    title: &str,
    code: &str,
    items: &[(&str, &str)],
    selected_value: Option<&str>,
    mut action: F,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        overview_row_label(ui, title, code);
        for (label, value) in items {
            if overview_action_button(ui, code, label, value, selected_value == Some(*value)) {
                action(value);
            }
        }
    });
}

fn overview_row_label(ui: &mut Ui, label: &str, code: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(96.0, OVERVIEW_ACTION_HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.line_segment(
        [
            pos2(rect.left(), rect.bottom() - 4.0),
            pos2(rect.right() - 16.0, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0, Color32::from_rgb(72, 62, 47)),
    );
    painter.line_segment(
        [
            pos2(rect.right() - 13.0, rect.bottom() - 4.0),
            pos2(rect.right(), rect.bottom() - 12.0),
        ],
        Stroke::new(1.0, Color32::from_rgb(214, 157, 92)),
    );
    painter.text(
        pos2(rect.left(), rect.top() + 10.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        Color32::from_rgb(222, 214, 199),
    );
    painter.text(
        pos2(rect.left(), rect.top() + 27.0),
        Align2::LEFT_CENTER,
        code,
        FontId::proportional(9.0),
        Color32::from_rgb(126, 119, 106),
    );
}

fn overview_action_button(
    ui: &mut Ui,
    group: &str,
    label: &str,
    value: &str,
    selected: bool,
) -> bool {
    let id = ui.make_persistent_id(("overview_action", group, value));
    let (rect, _) = ui.allocate_exact_size(
        vec2(OVERVIEW_ACTION_WIDTH, OVERVIEW_ACTION_HEIGHT),
        Sense::hover(),
    );
    let response = ui.interact(rect, id, Sense::click());
    let hover_t =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    let press_t = ui.ctx().animate_bool_with_time(
        response.id.with("press"),
        response.is_pointer_button_down_on(),
        0.06,
    );
    let selected_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("selected"), selected, 0.16);
    let rect = rect
        .translate(vec2(hover_t * 1.5 - press_t, press_t + selected_t))
        .shrink2(vec2(0.0, 1.0));
    let active_t = hover_t.max(selected_t);
    let fill = overview_mix_color(
        overview_mix_color(
            Color32::from_rgb(28, 27, 24),
            Color32::from_rgb(82, 58, 30),
            selected_t,
        ),
        Color32::from_rgb(72, 54, 32),
        hover_t * 0.6,
    );
    let stroke = overview_mix_color(
        Color32::from_rgb(68, 59, 45),
        Color32::from_rgb(232, 169, 88),
        active_t,
    );
    let text = overview_mix_color(
        Color32::from_rgb(199, 191, 177),
        Color32::from_rgb(255, 236, 200),
        active_t,
    );
    let painter = ui.painter_at(rect.expand(5.0));
    painter.add(Shape::convex_polygon(
        overview_action_points(rect, OVERVIEW_ACTION_SKEW).to_vec(),
        fill,
        Stroke::new(1.0 + active_t * 0.9, stroke),
    ));
    if selected_t > 0.0 {
        painter.add(Shape::convex_polygon(
            overview_action_points(rect.shrink2(vec2(5.0, 5.0)), OVERVIEW_ACTION_SKEW * 0.55)
                .to_vec(),
            Color32::from_rgba_unmultiplied(214, 157, 92, (34.0 * selected_t) as u8),
            Stroke::new(0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 0)),
        ));
    }
    painter.line_segment(
        [
            pos2(rect.left() + OVERVIEW_ACTION_SKEW + 5.0, rect.top() + 5.0),
            pos2(rect.right() - 18.0, rect.top() + 5.0),
        ],
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 229, 180, (28.0 + active_t * 82.0) as u8),
        ),
    );
    painter.line_segment(
        [
            pos2(
                rect.right() - OVERVIEW_ACTION_SKEW - 8.0,
                rect.bottom() - 5.0,
            ),
            pos2(rect.right() - 3.0, rect.bottom() - 5.0),
        ],
        Stroke::new(
            1.0 + selected_t,
            Color32::from_rgba_unmultiplied(214, 157, 92, (90.0 + active_t * 130.0) as u8),
        ),
    );
    painter.text(
        rect.center() + vec2(1.0, 0.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0 + hover_t * 0.5),
        text,
    );

    response.clicked()
}

fn overview_control_separator(ui: &mut Ui) {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 8.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.line_segment(
        [
            pos2(rect.left(), rect.center().y),
            pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, Color32::from_rgb(48, 43, 35)),
    );
    painter.line_segment(
        [
            pos2(rect.left() + 24.0, rect.center().y),
            pos2(rect.left() + 88.0, rect.center().y),
        ],
        Stroke::new(1.5, Color32::from_rgb(214, 157, 92)),
    );
}

fn overview_action_points(rect: Rect, skew: f32) -> [Pos2; 4] {
    [
        pos2(rect.left() + skew, rect.top()),
        pos2(rect.right(), rect.top()),
        pos2(rect.right() - skew, rect.bottom()),
        pos2(rect.left(), rect.bottom()),
    ]
}

fn overview_mix_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
        mix(from.a(), to.a()),
    )
}

fn advanced_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "高级", "DCHU 0x0C 只读硬件状态");
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for tab in AdvancedTab::all() {
                    let selected = app.advanced_tab == *tab;
                    let fill = if selected {
                        Color32::from_rgb(69, 51, 28)
                    } else {
                        Color32::from_rgb(28, 27, 24)
                    };
                    if ui
                        .add_sized(vec2(104.0, 30.0), Button::new(tab.label()).fill(fill))
                        .clicked()
                    {
                        app.advanced_tab = *tab;
                    }
                }
            });
            ui.add_space(12.0);
            match app.advanced_tab {
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
        assert_eq!(overview_gauge_width(700.0, 2), 236.0);
        assert_eq!(overview_gauge_columns(520.0, 2), 2);
        assert_eq!(overview_gauge_width(520.0, 2), 236.0);
    }

    #[test]
    fn overview_gauge_columns_supports_optional_pch_fan() {
        assert_eq!(overview_gauge_columns(900.0, 3), 3);
        assert_eq!(overview_gauge_columns(700.0, 3), 3);
        assert_eq!(overview_gauge_columns(520.0, 3), 2);
    }

    #[test]
    fn overview_gauge_row_leaves_right_edge_guard() {
        let columns = overview_gauge_columns(700.0, 2);
        let width = overview_gauge_width(700.0, columns);
        let occupied = overview_gauge_row_width(width, columns);
        assert!(occupied <= 700.0 - GAUGE_EDGE_GUARD);
    }

    #[test]
    fn overview_gauge_three_fan_row_leaves_right_edge_guard() {
        let columns = overview_gauge_columns(700.0, 3);
        let width = overview_gauge_width(700.0, columns);
        let occupied = overview_gauge_row_width(width, columns);
        assert!(occupied <= 700.0 - GAUGE_EDGE_GUARD);
    }

    #[test]
    fn overview_gauge_row_is_centered_when_space_allows() {
        let width = overview_gauge_width(700.0, 2);
        let row_width = overview_gauge_row_width(width, 2);
        assert_eq!(row_width, 480.0);
        assert_eq!(overview_gauge_leading_space(700.0, row_width), 110.0);
    }

    #[test]
    fn overview_action_shape_uses_slanted_edges() {
        let rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(98.0, 54.0));
        let points = overview_action_points(rect, 10.0);

        assert_eq!(points[0], pos2(20.0, 20.0));
        assert_eq!(points[1], pos2(98.0, 20.0));
        assert_eq!(points[2], pos2(88.0, 54.0));
        assert_eq!(points[3], pos2(10.0, 54.0));
    }

    #[test]
    fn overview_mix_color_clamps_interpolation() {
        let from = Color32::from_rgb(20, 30, 40);
        let to = Color32::from_rgb(120, 130, 140);

        assert_eq!(overview_mix_color(from, to, -1.0), from);
        assert_eq!(overview_mix_color(from, to, 2.0), to);
        assert_eq!(
            overview_mix_color(from, to, 0.5),
            Color32::from_rgb(70, 80, 90)
        );
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
        let fan_modes = overview_fan_mode_items(app.hardware.as_ref());
        control_group(ui, "风扇模式", &fan_modes, |mode| {
            app.run_dchu_write(&["fan-mode", mode, "--i-understand"]);
        });
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

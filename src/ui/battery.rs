use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, DragValue, FontId, Frame, Layout, Rect, RichText,
    Sense, Slider, Stroke, Ui,
};

use super::app::ClevoLedApp;
use super::widgets::page_header;
use crate::battery_strategy::{
    BatteryStrategyPreset, CHARGE_START_MAX, CHARGE_START_MIN, CHARGE_STOP_MAX, CHARGE_STOP_MIN,
    LOW_BATTERY_MAX, LOW_BATTERY_MIN,
};

pub(super) fn battery_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(
        ui,
        app.language.pick("电池", "Battery"),
        app.language.pick(
            "充电窗口与低电量保护",
            "Charge window and low-battery protection",
        ),
    );

    Frame::none()
        .fill(Color32::from_rgb(28, 29, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 57, 54)))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            strategy_overview(ui, app);
            section_divider(ui);

            ui.add_enabled_ui(app.battery_strategy.enabled, |ui| {
                preset_selector(ui, app);
                section_divider(ui);

                let available_width = ui.available_width();
                if available_width >= 620.0 {
                    ui.columns(2, |columns| {
                        columns[0].set_width((available_width - 24.0) * 0.5);
                        columns[1].set_width((available_width - 24.0) * 0.5);
                        threshold_section(&mut columns[0], app);
                        protection_section(&mut columns[1], app);
                    });
                } else {
                    threshold_section(ui, app);
                    section_divider(ui);
                    protection_section(ui, app);
                }
            });

            section_divider(ui);
            capability_status(ui, app);
        });
}

fn strategy_overview(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(app.language.pick("充电窗口", "Charge window"))
                    .size(12.0)
                    .color(Color32::from_rgb(145, 153, 148)),
            );
            ui.label(
                RichText::new(format!(
                    "{}–{}%",
                    app.battery_strategy.charge_start_percent,
                    app.battery_strategy.charge_stop_percent
                ))
                .size(30.0)
                .strong()
                .color(Color32::from_rgb(236, 241, 238)),
            );
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if battery_toggle(ui, app.battery_strategy.enabled) {
                app.set_battery_strategy_enabled(!app.battery_strategy.enabled);
            }
            ui.label(
                RichText::new(if app.battery_strategy.enabled {
                    app.language.pick("策略已启用", "Policy enabled")
                } else {
                    app.language.pick("策略未启用", "Policy disabled")
                })
                .size(13.0)
                .color(if app.battery_strategy.enabled {
                    accent()
                } else {
                    Color32::from_rgb(142, 148, 144)
                }),
            );
        });
    });

    ui.add_space(12.0);
    charge_window_meter(ui, app);
}

fn charge_window_meter(ui: &mut Ui, app: &ClevoLedApp) {
    let language = app.language;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let track = Rect::from_min_max(
        pos2(rect.left() + 2.0, rect.center().y - 5.0),
        pos2(rect.right() - 2.0, rect.center().y + 5.0),
    );
    let start_x =
        track.left() + track.width() * percent_t(app.battery_strategy.charge_start_percent);
    let stop_x = track.left() + track.width() * percent_t(app.battery_strategy.charge_stop_percent);
    let active = Rect::from_min_max(
        pos2(start_x, track.top()),
        pos2(stop_x.max(start_x + 2.0), track.bottom()),
    );
    let active_color = if app.battery_strategy.enabled {
        accent()
    } else {
        Color32::from_rgb(91, 100, 95)
    };

    painter.rect_filled(track, 5.0, Color32::from_rgb(45, 49, 47));
    painter.rect_filled(active, 5.0, active_color);

    for percent in [0_u8, 25, 50, 75, 100] {
        let x = track.left() + track.width() * percent_t(percent);
        painter.line_segment(
            [pos2(x, track.top() - 5.0), pos2(x, track.bottom() + 5.0)],
            Stroke::new(1.0, Color32::from_rgb(71, 78, 74)),
        );
        let alignment = match percent {
            0 => Align2::LEFT_TOP,
            100 => Align2::RIGHT_TOP,
            _ => Align2::CENTER_TOP,
        };
        painter.text(
            pos2(x, track.bottom() + 13.0),
            alignment,
            percent,
            FontId::proportional(10.0),
            Color32::from_rgb(118, 126, 121),
        );
    }

    for (x, label, value, alignment) in [
        (
            start_x,
            language.pick("开始", "Start"),
            app.battery_strategy.charge_start_percent,
            Align2::RIGHT_BOTTOM,
        ),
        (
            stop_x,
            language.pick("停止", "Stop"),
            app.battery_strategy.charge_stop_percent,
            Align2::LEFT_BOTTOM,
        ),
    ] {
        painter.circle_filled(
            pos2(x, track.center().y),
            7.0,
            Color32::from_rgb(24, 27, 25),
        );
        painter.circle_stroke(
            pos2(x, track.center().y),
            7.0,
            Stroke::new(2.0, active_color),
        );
        painter.text(
            pos2(x, track.top() - 9.0),
            alignment,
            format!("{label} {value}%"),
            FontId::proportional(11.0),
            Color32::from_rgb(203, 211, 206),
        );
    }
}

fn preset_selector(ui: &mut Ui, app: &mut ClevoLedApp) {
    section_title(
        ui,
        app.language.pick("策略预设", "Policy presets"),
        app.battery_strategy
            .preset
            .localized_description(app.language),
    );
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let width = ((ui.available_width() - 16.0) / 3.0).max(96.0);
        for preset in BatteryStrategyPreset::all() {
            let selected = app.battery_strategy.preset == *preset;
            let id = ui.make_persistent_id(("battery_preset", *preset));
            let (rect, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
            let response = ui.interact(rect, id, Sense::click());
            let hover_t = ui.ctx().animate_bool_with_time(
                response.id.with("hover"),
                response.hovered(),
                0.12,
            );
            let selected_t =
                ui.ctx()
                    .animate_bool_with_time(response.id.with("selected"), selected, 0.16);
            let painter = ui.painter_at(rect.expand(2.0));
            let fill = mix_color(
                Color32::from_rgb(34, 37, 35),
                Color32::from_rgb(30, 64, 52),
                selected_t,
            );
            let stroke = mix_color(
                Color32::from_rgb(59, 65, 61),
                accent(),
                selected_t.max(hover_t * 0.55),
            );
            painter.rect_filled(rect, 6.0, fill);
            painter.rect_stroke(rect, 6.0, Stroke::new(1.0 + selected_t * 0.4, stroke));
            painter.text(
                pos2(rect.center().x, rect.center().y - 7.0),
                Align2::CENTER_CENTER,
                preset.localized_label(app.language),
                FontId::proportional(13.0),
                Color32::from_rgb(231, 237, 233),
            );
            painter.text(
                pos2(rect.center().x, rect.center().y + 10.0),
                Align2::CENTER_CENTER,
                preset_range(*preset),
                FontId::proportional(11.0),
                if selected {
                    accent()
                } else {
                    Color32::from_rgb(133, 142, 136)
                },
            );

            if response.clicked() {
                app.battery_strategy.apply_preset(*preset);
                app.save_battery_strategy();
            }
        }
    });
}

fn threshold_section(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    section_title(
        ui,
        language.pick("充电阈值", "Charge thresholds"),
        language.pick("控制电池保持区间", "Set the battery maintenance range"),
    );
    ui.add_space(10.0);

    let start_max =
        CHARGE_START_MAX.min(app.battery_strategy.charge_stop_percent.saturating_sub(5));
    let stop_min = CHARGE_STOP_MIN.max(app.battery_strategy.charge_start_percent.saturating_add(5));
    let mut changed = false;
    changed |= threshold_control(
        ui,
        language.pick("开始充电", "Start charging"),
        &mut app.battery_strategy.charge_start_percent,
        CHARGE_START_MIN,
        start_max,
    );
    ui.add_space(10.0);
    changed |= threshold_control(
        ui,
        language.pick("停止充电", "Stop charging"),
        &mut app.battery_strategy.charge_stop_percent,
        stop_min,
        CHARGE_STOP_MAX,
    );

    if changed {
        app.save_battery_strategy();
    }
}

fn threshold_control(ui: &mut Ui, label: &str, value: &mut u8, min: u8, max: u8) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(12.0)
                .color(Color32::from_rgb(196, 204, 199)),
        );
        ui.add_space(6.0);
        changed |= ui
            .add(DragValue::new(value).range(min..=max).suffix("%"))
            .changed();
    });
    let slider_width = ui.available_width().max(140.0);
    ui.spacing_mut().slider_width = slider_width;
    changed |= ui
        .add(Slider::new(value, min..=max).show_value(false))
        .changed();
    changed
}

fn protection_section(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    section_title(
        ui,
        language.pick("保护行为", "Protection behavior"),
        language.pick(
            "电池供电与低电量响应",
            "Battery-power and low-charge response",
        ),
    );
    ui.add_space(8.0);

    let mut changed = false;
    changed |= setting_row(
        ui,
        language.pick("电池供电节能", "Save power on battery"),
        &mut app.battery_strategy.energy_save_on_battery,
    );
    changed |= setting_row(
        ui,
        language.pick("低电量保护", "Low-battery protection"),
        &mut app.battery_strategy.low_battery_protection,
    );

    if app.battery_strategy.low_battery_protection {
        ui.add_space(6.0);
        changed |= threshold_control(
            ui,
            language.pick("触发阈值", "Trigger threshold"),
            &mut app.battery_strategy.low_battery_threshold_percent,
            LOW_BATTERY_MIN,
            LOW_BATTERY_MAX,
        );
        ui.add_space(4.0);
        changed |= setting_row(
            ui,
            language.pick("同步降低键盘亮度", "Reduce keyboard brightness"),
            &mut app.battery_strategy.reduce_keyboard_brightness,
        );
    }

    if changed {
        app.save_battery_strategy();
    }
}

fn setting_row(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(12.0)
                .color(Color32::from_rgb(196, 204, 199)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if battery_toggle(ui, *value) {
                *value = !*value;
                clicked = true;
            }
        });
    });
    ui.add_space(7.0);
    clicked
}

fn battery_toggle(ui: &mut Ui, enabled: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(42.0, 22.0), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("battery_toggle"), enabled, 0.14);
    let hover_t = ui.ctx().animate_bool_with_time(
        response.id.with("battery_toggle_hover"),
        response.hovered(),
        0.1,
    );
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect_filled(
        rect,
        11.0,
        mix_color(
            Color32::from_rgb(46, 51, 48),
            Color32::from_rgb(31, 91, 69),
            t,
        ),
    );
    painter.rect_stroke(
        rect,
        11.0,
        Stroke::new(
            1.0,
            mix_color(
                Color32::from_rgb(70, 77, 73),
                accent(),
                t.max(hover_t * 0.5),
            ),
        ),
    );
    painter.circle_filled(
        pos2(
            rect.left() + 11.0 + (rect.width() - 22.0) * t,
            rect.center().y,
        ),
        7.0,
        Color32::from_rgb(231, 238, 234),
    );
    response.clicked()
}

fn capability_status(ui: &mut Ui, app: &ClevoLedApp) {
    let config = app
        .hardware
        .as_ref()
        .and_then(|snapshot| snapshot.dchu_config.as_ref());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(14.0, 6.0);
        status_item(
            ui,
            app.language.pick("配置", "Configuration"),
            Some(true),
            app.language.pick("本地", "Local"),
        );
        status_item(
            ui,
            "Battery Utility",
            config.and_then(|config| config.battery_utility_capability()),
            "",
        );
        status_item(
            ui,
            "EnergySave",
            config.and_then(|config| config.energy_save_capability()),
            "",
        );
    });
}

fn status_item(ui: &mut Ui, label: &str, state: Option<bool>, suffix: &str) {
    let color = match state {
        Some(true) => accent(),
        Some(false) => Color32::from_rgb(203, 105, 91),
        None => Color32::from_rgb(108, 116, 111),
    };
    let (dot_rect, _) = ui.allocate_exact_size(vec2(8.0, 16.0), Sense::hover());
    ui.painter().circle_filled(dot_rect.center(), 3.0, color);
    ui.label(
        RichText::new(if suffix.is_empty() {
            label.to_owned()
        } else {
            format!("{label} · {suffix}")
        })
        .size(11.0)
        .color(Color32::from_rgb(132, 141, 135)),
    );
}

fn section_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(225, 232, 228)),
        );
        ui.add_space(1.0);
        ui.label(
            RichText::new(subtitle)
                .size(11.0)
                .color(Color32::from_rgb(126, 135, 129)),
        );
    });
}

fn section_divider(ui: &mut Ui) {
    ui.add_space(9.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, Color32::from_rgb(48, 53, 50)),
    );
    ui.add_space(9.0);
}

fn preset_range(preset: BatteryStrategyPreset) -> &'static str {
    match preset {
        BatteryStrategyPreset::Standard => "95–100%",
        BatteryStrategyPreset::Care => "45–80%",
        BatteryStrategyPreset::Endurance => "40–70%",
    }
}

fn accent() -> Color32 {
    Color32::from_rgb(82, 210, 160)
}

fn mix_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn percent_t(value: u8) -> f32 {
    (value as f32 / 100.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_position_is_clamped() {
        assert_eq!(percent_t(0), 0.0);
        assert_eq!(percent_t(100), 1.0);
        assert_eq!(percent_t(150), 1.0);
    }

    #[test]
    fn preset_ranges_match_strategy_defaults() {
        assert_eq!(preset_range(BatteryStrategyPreset::Standard), "95–100%");
        assert_eq!(preset_range(BatteryStrategyPreset::Care), "45–80%");
        assert_eq!(preset_range(BatteryStrategyPreset::Endurance), "40–70%");
    }
}

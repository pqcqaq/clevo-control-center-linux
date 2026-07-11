use eframe::egui::{
    self, pos2, vec2, Align2, Button, Color32, DragValue, FontId, Frame, Rect, RichText, Sense,
    Shape, Slider, Stroke, Ui,
};

use super::app::ClevoLedApp;
use super::widgets::{page_header, toggle_switch};
use crate::battery_strategy::{
    BatteryStrategyPreset, CHARGE_START_MAX, CHARGE_START_MIN, CHARGE_STOP_MAX, CHARGE_STOP_MIN,
    LOW_BATTERY_MAX, LOW_BATTERY_MIN,
};

pub(super) fn battery_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(
        ui,
        "电池",
        "本地电池策略配置；当前版本不写 EC 或系统电源计划",
    );

    capability_panel(ui, app);
    ui.add_space(12.0);

    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            strategy_switch(ui, app);
            ui.add_space(14.0);
            if app.battery_strategy.enabled {
                preset_row(ui, app);
                ui.add_space(14.0);
                threshold_editor(ui, app);
                ui.add_space(14.0);
                behavior_editor(ui, app);
            } else {
                ui.label(
                    RichText::new("电池策略未开启；当前只保留默认系统/固件行为。")
                        .size(13.0)
                        .color(Color32::from_rgb(151, 145, 135)),
                );
            }
        });
}

fn capability_panel(ui: &mut Ui, app: &ClevoLedApp) {
    Frame::none()
        .fill(Color32::from_rgb(26, 25, 22))
        .stroke(Stroke::new(1.0, Color32::from_rgb(57, 51, 42)))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
                ui.label(
                    RichText::new("硬件能力")
                        .size(14.0)
                        .strong()
                        .color(Color32::from_rgb(232, 224, 210)),
                );
                let config = app.hardware.as_ref().and_then(|snapshot| snapshot.dchu_config.as_ref());
                capability_chip(
                    ui,
                    "Battery Utility",
                    config.and_then(|config| config.battery_utility_capability()),
                );
                capability_chip(
                    ui,
                    "EnergySave",
                    config.and_then(|config| config.energy_save_capability()),
                );
                capability_chip(ui, "Battery Saver(v1)", None);
            });
            ui.add_space(8.0);
            battery_meter(ui, app);
            ui.add_space(8.0);
            ui.label(
                RichText::new("这些能力只用于展示。当前页面保存本地策略，不调用原厂 EnergySave/Battery Saver 写接口。")
                    .size(12.0)
                    .color(Color32::from_rgb(151, 145, 135)),
            );
        });
}

fn capability_chip(ui: &mut Ui, label: &str, value: Option<bool>) {
    let (text, fill, stroke) = match value {
        Some(true) => (
            format!("{label}: 支持"),
            Color32::from_rgb(45, 58, 34),
            Color32::from_rgb(126, 174, 90),
        ),
        Some(false) => (
            format!("{label}: 不支持"),
            Color32::from_rgb(57, 38, 31),
            Color32::from_rgb(177, 94, 72),
        ),
        None => (
            format!("{label}: 未知"),
            Color32::from_rgb(43, 40, 34),
            Color32::from_rgb(88, 80, 66),
        ),
    };
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .rounding(999.0)
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(12.0)
                    .color(Color32::from_rgb(232, 224, 210)),
            );
        });
}

fn battery_meter(ui: &mut Ui, app: &ClevoLedApp) {
    let width = ui.available_width().clamp(260.0, 420.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 54.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let shell = Rect::from_min_max(
        pos2(rect.left(), rect.top() + 10.0),
        pos2(rect.right() - 16.0, rect.bottom() - 8.0),
    );
    let cap = Rect::from_min_max(
        pos2(shell.right() + 3.0, shell.center().y - 8.0),
        pos2(rect.right(), shell.center().y + 8.0),
    );
    painter.rect_filled(shell, 8.0, Color32::from_rgb(17, 16, 14));
    painter.rect_stroke(shell, 8.0, Stroke::new(1.0, Color32::from_rgb(68, 61, 50)));
    painter.rect_filled(cap, 4.0, Color32::from_rgb(68, 61, 50));

    let start_t = percent_t(app.battery_strategy.charge_start_percent);
    let stop_t = percent_t(app.battery_strategy.charge_stop_percent);
    let start_x = shell.left() + shell.width() * start_t;
    let stop_x = shell.left() + shell.width() * stop_t;
    let band = Rect::from_min_max(
        pos2(start_x, shell.top() + 5.0),
        pos2(stop_x.max(start_x + 2.0), shell.bottom() - 5.0),
    );
    painter.rect_filled(band, 5.0, Color32::from_rgb(210, 144, 68));
    painter.add(Shape::line(
        vec![
            pos2(start_x, shell.bottom() + 6.0),
            pos2(start_x, shell.bottom() - 2.0),
            pos2(stop_x, shell.bottom() - 2.0),
            pos2(stop_x, shell.bottom() + 6.0),
        ],
        Stroke::new(1.2, Color32::from_rgb(226, 166, 88)),
    ));
    painter.text(
        shell.center(),
        Align2::CENTER_CENTER,
        format!(
            "{}% -> {}%",
            app.battery_strategy.charge_start_percent, app.battery_strategy.charge_stop_percent
        ),
        FontId::proportional(14.0),
        Color32::from_rgb(239, 229, 208),
    );
}

fn percent_t(value: u8) -> f32 {
    (value as f32 / 100.0).clamp(0.0, 1.0)
}

fn strategy_switch(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
        if toggle_switch(ui, app.battery_strategy.enabled) {
            app.set_battery_strategy_enabled(!app.battery_strategy.enabled);
        }
        ui.label(
            RichText::new("启用电池策略")
                .size(15.0)
                .strong()
                .color(Color32::from_rgb(236, 230, 218)),
        );
        ui.label(
            RichText::new("仅保存策略配置；硬件写入待后续验证后再接入。")
                .size(12.0)
                .color(Color32::from_rgb(151, 145, 135)),
        );
    });
}

fn preset_row(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        for preset in BatteryStrategyPreset::all() {
            let selected = app.battery_strategy.preset == *preset;
            let fill = if selected {
                Color32::from_rgb(78, 53, 27)
            } else {
                Color32::from_rgb(27, 26, 23)
            };
            let stroke = if selected {
                Stroke::new(1.3, Color32::from_rgb(226, 166, 88))
            } else {
                Stroke::new(1.0, Color32::from_rgb(64, 58, 48))
            };
            if ui
                .add_sized(
                    vec2(116.0, 34.0),
                    Button::new(preset.label()).fill(fill).stroke(stroke),
                )
                .clicked()
            {
                app.battery_strategy.apply_preset(*preset);
                app.save_battery_strategy();
            }
        }
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(app.battery_strategy.preset.description())
            .size(12.0)
            .color(Color32::from_rgb(151, 145, 135)),
    );
}

fn threshold_editor(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.label(
        RichText::new("充放电阈值")
            .size(14.0)
            .strong()
            .color(Color32::from_rgb(232, 224, 210)),
    );
    ui.add_space(6.0);
    let mut changed = false;
    changed |= percent_slider(
        ui,
        "低于此值开始充电",
        &mut app.battery_strategy.charge_start_percent,
        CHARGE_START_MIN,
        CHARGE_START_MAX,
    );
    changed |= percent_slider(
        ui,
        "达到此值停止充电",
        &mut app.battery_strategy.charge_stop_percent,
        CHARGE_STOP_MIN,
        CHARGE_STOP_MAX,
    );
    if changed {
        app.save_battery_strategy();
    }
}

fn percent_slider(ui: &mut Ui, label: &str, value: &mut u8, min: u8, max: u8) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_width(520.0);
        ui.label(
            RichText::new(label)
                .size(13.0)
                .color(Color32::from_rgb(193, 186, 173)),
        );
        changed |= ui
            .add_sized(
                vec2(260.0, 20.0),
                Slider::new(value, min..=max).show_value(false),
            )
            .changed();
        changed |= ui
            .add(DragValue::new(value).range(min..=max).suffix("%"))
            .changed();
    });
    changed
}

fn behavior_editor(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.label(
        RichText::new("策略动作")
            .size(14.0)
            .strong()
            .color(Color32::from_rgb(232, 224, 210)),
    );
    ui.add_space(8.0);

    let mut changed = false;
    changed |= option_switch(
        ui,
        "使用电池时偏向节能",
        "本地策略标记；当前不切换系统电源计划。",
        &mut app.battery_strategy.energy_save_on_battery,
    );
    changed |= option_switch(
        ui,
        "启用低电量保护",
        "记录低电量阈值和降亮度意图。",
        &mut app.battery_strategy.low_battery_protection,
    );
    if app.battery_strategy.low_battery_protection {
        changed |= percent_slider(
            ui,
            "低电量阈值",
            &mut app.battery_strategy.low_battery_threshold_percent,
            LOW_BATTERY_MIN,
            LOW_BATTERY_MAX,
        );
        changed |= option_switch(
            ui,
            "低电量降低键盘亮度",
            "只保存策略，不直接修改当前灯效亮度。",
            &mut app.battery_strategy.reduce_keyboard_brightness,
        );
    }

    if changed {
        app.save_battery_strategy();
    }
}

fn option_switch(ui: &mut Ui, title: &str, subtitle: &str, value: &mut bool) -> bool {
    let mut clicked = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 4.0);
        if toggle_switch(ui, *value) {
            *value = !*value;
            clicked = true;
        }
        ui.vertical(|ui| {
            ui.label(
                RichText::new(title)
                    .size(13.0)
                    .color(Color32::from_rgb(222, 214, 199)),
            );
            ui.label(
                RichText::new(subtitle)
                    .size(11.0)
                    .color(Color32::from_rgb(151, 145, 135)),
            );
        });
    });
    ui.add_space(8.0);
    clicked
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
}

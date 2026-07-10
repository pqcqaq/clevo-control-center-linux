use eframe::egui::{
    self, pos2, vec2, Align2, Button, Color32, DragValue, FontId, Pos2, Rect, RichText, Sense,
    Shape, Stroke, Ui,
};

use super::app::ClevoLedApp;
use super::widgets::{page_header, toggle_switch};
use crate::fan_curve::{
    default_fan_curve_profiles, FanCurve, FanCurveChannel, FanCurveSelection, FanCurveSettings,
    FAN_CURVE_COUNT, FAN_CURVE_MAX_DUTY, FAN_CURVE_MAX_TEMP, FAN_CURVE_MIN_DUTY,
    FAN_CURVE_MIN_TEMP,
};

const CURVE_PANEL_HEIGHT: f32 = 236.0;
const SELECTED_POINT_EDITOR_HEIGHT: f32 = 34.0;

pub(super) fn fan_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "风扇", "自定义风扇控制曲线");

    fan_curve_switch(ui, app);
    if app.fan_curve_draft.enabled {
        ui.add_space(16.0);
        fan_curve_tabs(ui, app);
        ui.add_space(14.0);
        fan_curve_editor(ui, app);
        ui.add_space(16.0);
        fan_curve_actions(ui, app);
    }
}

fn fan_curve_switch(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
        if toggle_switch(ui, app.fan_curve_draft.enabled) {
            app.set_fan_curve_enabled(!app.fan_curve_draft.enabled);
        }
        ui.label(
            RichText::new("开启自定义风扇曲线")
                .size(15.0)
                .strong()
                .color(Color32::from_rgb(236, 230, 218)),
        );
    });
}

fn fan_curve_tabs(ui: &mut Ui, app: &mut ClevoLedApp) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for index in 0..FAN_CURVE_COUNT {
            let selected = app.fan_curve_tab == index;
            let fill = if selected {
                Color32::from_rgb(74, 52, 27)
            } else {
                Color32::from_rgb(27, 26, 23)
            };
            let stroke = if selected {
                Stroke::new(1.2, Color32::from_rgb(226, 166, 88))
            } else {
                Stroke::new(1.0, Color32::from_rgb(64, 58, 48))
            };
            if ui
                .add_sized(
                    vec2(104.0, 32.0),
                    Button::new(FanCurveSettings::profile_label(index))
                        .fill(fill)
                        .stroke(stroke),
                )
                .clicked()
            {
                app.fan_curve_tab = index;
                app.fan_curve_selection = None;
            }
        }
    });
}

fn fan_curve_editor(ui: &mut Ui, app: &mut ClevoLedApp) {
    if app.fan_curve_tab >= app.fan_curve_draft.profiles.len() {
        return;
    }

    let available_width = ui.available_width();
    let column_gap = 14.0;
    if available_width >= 520.0 {
        let width = ((available_width - column_gap) * 0.5).floor();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(column_gap, 0.0);
            curve_panel(ui, app, FanCurveChannel::Cpu, width);
            curve_panel(ui, app, FanCurveChannel::Gpu, width);
        });
    } else {
        curve_panel(ui, app, FanCurveChannel::Cpu, available_width);
        ui.add_space(14.0);
        curve_panel(ui, app, FanCurveChannel::Gpu, available_width);
    }

    selected_point_editor_slot(ui, app);
}

fn curve_panel(ui: &mut Ui, app: &mut ClevoLedApp, channel: FanCurveChannel, width: f32) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.label(
            RichText::new(channel.label())
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(232, 224, 210)),
        );
        ui.add_space(8.0);
        let (rect, _) =
            ui.allocate_exact_size(vec2(width.max(180.0), CURVE_PANEL_HEIGHT), Sense::hover());
        draw_curve_editor(ui, app, channel, rect);
    });
}

fn draw_curve_editor(ui: &mut Ui, app: &mut ClevoLedApp, channel: FanCurveChannel, rect: Rect) {
    let profile_index = app.fan_curve_tab;
    let points = channel_curve_mut(app, channel).points.clone();
    let painter = ui.painter_at(rect);
    let plot = rect.shrink2(vec2(26.0, 22.0));
    draw_curve_background(&painter, rect, plot);

    let screen_points = points
        .iter()
        .map(|point| curve_point_to_pos(plot, point.temp_celsius, point.duty_percent))
        .collect::<Vec<_>>();
    painter.add(Shape::line(
        screen_points.clone(),
        Stroke::new(2.4, Color32::from_rgb(229, 164, 86)),
    ));

    for (index, point_pos) in screen_points.iter().enumerate() {
        let selected = app.fan_curve_selection
            == Some(FanCurveSelection {
                profile: profile_index,
                channel,
                point: index,
            });
        let point_rect = Rect::from_center_size(*point_pos, vec2(18.0, 18.0));
        let id = ui.make_persistent_id(("fan_curve_point", profile_index, channel, index));
        let response = ui.interact(point_rect, id, Sense::click_and_drag());
        if response.clicked() {
            app.fan_curve_selection = Some(FanCurveSelection {
                profile: profile_index,
                channel,
                point: index,
            });
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let (temp, duty) = pos_to_curve_point(plot, pointer);
                channel_curve_mut(app, channel).set_point(index, temp, duty);
                app.fan_curve_selection = Some(FanCurveSelection {
                    profile: profile_index,
                    channel,
                    point: index,
                });
            }
        }

        let radius = if selected { 6.5 } else { 5.0 };
        painter.circle_filled(*point_pos, radius + 3.0, Color32::from_rgb(14, 13, 12));
        painter.circle_filled(
            *point_pos,
            radius,
            if selected {
                Color32::from_rgb(255, 206, 132)
            } else {
                Color32::from_rgb(198, 143, 80)
            },
        );
    }
}

fn draw_curve_background(painter: &egui::Painter, rect: Rect, plot: Rect) {
    painter.rect_filled(rect, 8.0, Color32::from_rgb(18, 17, 15));
    painter.rect_stroke(plot, 4.0, Stroke::new(1.0, Color32::from_rgb(64, 58, 48)));
    for step in 0..=4 {
        let x = plot.left() + plot.width() * step as f32 / 4.0;
        painter.line_segment(
            [pos2(x, plot.top()), pos2(x, plot.bottom())],
            Stroke::new(1.0, Color32::from_rgb(37, 34, 29)),
        );
        let y = plot.top() + plot.height() * step as f32 / 4.0;
        painter.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, Color32::from_rgb(37, 34, 29)),
        );
    }

    painter.text(
        pos2(plot.left(), rect.bottom() - 5.0),
        Align2::LEFT_BOTTOM,
        format!("{FAN_CURVE_MIN_TEMP}°C"),
        FontId::proportional(10.0),
        Color32::from_rgb(128, 120, 108),
    );
    painter.text(
        pos2(plot.right(), rect.bottom() - 5.0),
        Align2::RIGHT_BOTTOM,
        format!("{FAN_CURVE_MAX_TEMP}°C"),
        FontId::proportional(10.0),
        Color32::from_rgb(128, 120, 108),
    );
    painter.text(
        pos2(rect.left() + 2.0, plot.top()),
        Align2::LEFT_TOP,
        "100%",
        FontId::proportional(10.0),
        Color32::from_rgb(128, 120, 108),
    );
    painter.text(
        pos2(rect.left() + 2.0, plot.bottom()),
        Align2::LEFT_BOTTOM,
        "0%",
        FontId::proportional(10.0),
        Color32::from_rgb(128, 120, 108),
    );
}

fn selected_point_editor(ui: &mut Ui, app: &mut ClevoLedApp, selection: FanCurveSelection) {
    let Some(point) = channel_curve_mut(app, selection.channel)
        .points
        .get(selection.point)
        .copied()
    else {
        return;
    };

    let mut temp = point.temp_celsius;
    let mut duty = point.duty_percent;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
        ui.label(
            RichText::new(format!(
                "{} 点 {}",
                selection.channel.label(),
                selection.point + 1
            ))
            .size(13.0)
            .strong()
            .color(Color32::from_rgb(222, 214, 199)),
        );
        ui.label("温度");
        let temp_changed = ui
            .add(DragValue::new(&mut temp).range(FAN_CURVE_MIN_TEMP..=FAN_CURVE_MAX_TEMP))
            .changed();
        ui.label("°C");
        ui.label("占空比");
        let duty_changed = ui
            .add(DragValue::new(&mut duty).range(FAN_CURVE_MIN_DUTY..=FAN_CURVE_MAX_DUTY))
            .changed();
        ui.label("%");
        if temp_changed || duty_changed {
            channel_curve_mut(app, selection.channel).set_point(selection.point, temp, duty);
        }
    });
}

fn selected_point_editor_slot(ui: &mut Ui, app: &mut ClevoLedApp) {
    let selection = app
        .fan_curve_selection
        .filter(|selection| selection.profile == app.fan_curve_tab);

    ui.add_space(12.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), SELECTED_POINT_EDITOR_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            if let Some(selection) = selection {
                selected_point_editor(ui, app, selection);
            }
        },
    );
}

fn fan_curve_actions(ui: &mut Ui, app: &mut ClevoLedApp) {
    let has_unsaved_changes = fan_curve_has_unsaved_changes(&app.fan_curve_draft, &app.fan_curves);
    let can_reset_current =
        current_fan_curve_differs_from_default(&app.fan_curve_draft, app.fan_curve_tab);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        if ui
            .add_enabled(
                has_unsaved_changes,
                Button::new("恢复").min_size(vec2(100.0, 34.0)),
            )
            .clicked()
        {
            app.restore_fan_curve_draft();
        }
        if ui
            .add_enabled(
                can_reset_current,
                Button::new("重置").min_size(vec2(100.0, 34.0)),
            )
            .clicked()
        {
            app.reset_current_fan_curve_profile();
        }
        if ui
            .add_enabled(
                has_unsaved_changes,
                Button::new("保存").min_size(vec2(100.0, 34.0)),
            )
            .clicked()
        {
            app.save_fan_curve_draft();
        }
    });
}

fn fan_curve_has_unsaved_changes(draft: &FanCurveSettings, saved: &FanCurveSettings) -> bool {
    draft.clone().sanitized() != saved.clone().sanitized()
}

fn current_fan_curve_differs_from_default(draft: &FanCurveSettings, tab: usize) -> bool {
    let Some(current) = draft.profiles.get(tab) else {
        return false;
    };
    let Some(default) = default_fan_curve_profiles().get(tab).cloned() else {
        return false;
    };

    current.clone().sanitized() != default
}

fn channel_curve_mut(app: &mut ClevoLedApp, channel: FanCurveChannel) -> &mut FanCurve {
    let profile = &mut app.fan_curve_draft.profiles[app.fan_curve_tab];
    match channel {
        FanCurveChannel::Cpu => &mut profile.cpu,
        FanCurveChannel::Gpu => &mut profile.gpu,
    }
}

fn curve_point_to_pos(plot: Rect, temp_celsius: u8, duty_percent: u8) -> Pos2 {
    let temp_t = (temp_celsius.saturating_sub(FAN_CURVE_MIN_TEMP)) as f32
        / (FAN_CURVE_MAX_TEMP - FAN_CURVE_MIN_TEMP) as f32;
    let duty_t = (duty_percent.saturating_sub(FAN_CURVE_MIN_DUTY)) as f32
        / (FAN_CURVE_MAX_DUTY - FAN_CURVE_MIN_DUTY) as f32;
    pos2(
        plot.left() + plot.width() * temp_t.clamp(0.0, 1.0),
        plot.bottom() - plot.height() * duty_t.clamp(0.0, 1.0),
    )
}

fn pos_to_curve_point(plot: Rect, pos: Pos2) -> (u8, u8) {
    let temp_t = ((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
    let duty_t = ((plot.bottom() - pos.y) / plot.height()).clamp(0.0, 1.0);
    let temp =
        FAN_CURVE_MIN_TEMP as f32 + (FAN_CURVE_MAX_TEMP - FAN_CURVE_MIN_TEMP) as f32 * temp_t;
    let duty =
        FAN_CURVE_MIN_DUTY as f32 + (FAN_CURVE_MAX_DUTY - FAN_CURVE_MIN_DUTY) as f32 * duty_t;
    (temp.round() as u8, duty.round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_point_mapping_roundtrips_midpoint() {
        let plot = Rect::from_min_max(pos2(10.0, 20.0), pos2(210.0, 120.0));
        let pos = curve_point_to_pos(plot, 65, 50);
        let (temp, duty) = pos_to_curve_point(plot, pos);

        assert_eq!(temp, 65);
        assert_eq!(duty, 50);
    }

    #[test]
    fn fan_curve_unsaved_changes_tracks_draft_against_saved_settings() {
        let saved = FanCurveSettings::default();
        let mut draft = saved.clone();

        assert!(!fan_curve_has_unsaved_changes(&draft, &saved));

        draft.profiles[0].cpu.set_point(1, 61, 63);

        assert!(fan_curve_has_unsaved_changes(&draft, &saved));
        assert!(!fan_curve_has_unsaved_changes(&saved, &saved));
    }

    #[test]
    fn fan_curve_reset_availability_tracks_current_profile_default() {
        let mut draft = FanCurveSettings::default();

        assert!(!current_fan_curve_differs_from_default(&draft, 0));

        draft.profiles[0].gpu.set_point(1, 61, 63);

        assert!(current_fan_curve_differs_from_default(&draft, 0));
        assert!(!current_fan_curve_differs_from_default(&draft, 1));
        assert!(!current_fan_curve_differs_from_default(
            &draft,
            FAN_CURVE_COUNT
        ));
    }
}

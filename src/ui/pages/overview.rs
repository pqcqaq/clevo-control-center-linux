use eframe::egui::{
    pos2, vec2, Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui,
};

use super::super::app::ClevoLedApp;
use super::super::fan_gauge::fan_gauge;
use crate::dchu::{
    available_fan_modes, available_power_modes, selected_fan_mode_from_snapshot,
    selected_power_mode_from_snapshot, FanMode, FanStatus, HardwareSnapshot, PowerMode,
};
use crate::fan_curve::{FanCurveSettings, FAN_CURVE_COUNT};

const GAUGE_GAP: f32 = 8.0;
const GAUGE_EDGE_GUARD: f32 = 8.0;
const MIN_GAUGE_WIDTH: f32 = 224.0;
const MAX_GAUGE_WIDTH: f32 = 236.0;
const OVERVIEW_ACTION_WIDTH: f32 = 88.0;
const OVERVIEW_ACTION_HEIGHT: f32 = 34.0;
const OVERVIEW_ACTION_SKEW: f32 = 10.0;
const OVERVIEW_SECTION_MARGIN: f32 = 14.0;

pub(super) fn overview_page(ui: &mut Ui, app: &mut ClevoLedApp) {
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
        .clamp(MIN_GAUGE_WIDTH, MAX_GAUGE_WIDTH)
}

fn overview_gauge_row_width(gauge_width: f32, columns: usize) -> f32 {
    let columns = columns.max(1);
    gauge_width * columns as f32 + GAUGE_GAP * (columns - 1) as f32
}

fn overview_gauge_leading_space(available_width: f32, row_width: f32) -> f32 {
    ((available_width - row_width) * 0.5).max(0.0)
}

fn overview_controls(ui: &mut Ui, app: &mut ClevoLedApp) {
    let power_modes = overview_power_mode_items(app.hardware.as_ref());
    let selected_power_mode =
        selected_power_mode_from_snapshot(app.hardware.as_ref()).map(PowerMode::value);
    let fan_modes = overview_fan_mode_items(app);
    let selected_fan_mode = selected_fan_mode_value(app);

    if !power_modes.is_empty() {
        overview_button_row(
            ui,
            "电源模式",
            "POWER",
            &power_modes,
            selected_power_mode,
            |mode| {
                if let Some(mode) = PowerMode::from_value(mode) {
                    app.set_power_mode(mode);
                }
            },
        );
    }

    if !power_modes.is_empty() && !fan_modes.is_empty() {
        ui.add_space(14.0);
        overview_control_separator(ui);
        ui.add_space(14.0);
    }

    if !fan_modes.is_empty() {
        overview_button_row(
            ui,
            "风扇模式",
            "FAN",
            &fan_modes,
            selected_fan_mode,
            |mode| apply_fan_mode_selection(app, mode),
        );
    }

    if power_modes.is_empty() && fan_modes.is_empty() {
        ui.label(
            RichText::new("当前固件未报告可写电源或风扇模式能力")
                .size(13.0)
                .color(Color32::from_rgb(151, 145, 135)),
        );
    }
}

fn overview_power_mode_items(
    snapshot: Option<&HardwareSnapshot>,
) -> Vec<(&'static str, &'static str)> {
    available_power_modes(snapshot)
        .iter()
        .map(|mode| (mode.label(), mode.value()))
        .collect()
}

fn overview_fan_mode_items(app: &ClevoLedApp) -> Vec<(&'static str, &'static str)> {
    let mut modes = available_fan_modes(app.hardware.as_ref())
        .iter()
        .map(|mode| (mode.label(), mode.value()))
        .collect::<Vec<_>>();

    if app.fan_curves.enabled {
        modes.extend((0..FAN_CURVE_COUNT).map(|index| {
            (
                FanCurveSettings::profile_label(index),
                FanCurveSettings::mode_value(index),
            )
        }));
    }
    modes
}

fn selected_fan_mode_value(app: &ClevoLedApp) -> Option<&'static str> {
    if app.fan_curves.enabled {
        if let Some(index) = app.fan_curves.selected_profile {
            return Some(FanCurveSettings::mode_value(index));
        }
    }
    selected_fan_mode_from_snapshot(app.hardware.as_ref()).map(FanMode::value)
}

fn apply_fan_mode_selection(app: &mut ClevoLedApp, value: &str) {
    if let Some(index) = FanCurveSettings::mode_index(value) {
        app.select_fan_curve_profile(index);
    } else {
        app.clear_selected_fan_curve_profile();
        if let Some(mode) = FanMode::from_value(value) {
            app.set_fan_mode(mode);
        }
    }
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

use std::f32::consts::PI;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Pos2, RichText, Sense, Shape, Stroke, Ui,
};

use crate::dchu::FanStatus;

pub(super) fn fan_gauge(ui: &mut Ui, fan: &FanStatus, width: f32) {
    const GAUGE_HEIGHT: f32 = 266.0;

    ui.allocate_ui_with_layout(
        vec2(width, GAUGE_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_width(width);
            ui.label(
                RichText::new(&fan.label)
                    .size(15.0)
                    .strong()
                    .color(Color32::from_rgb(236, 230, 218)),
            );
            ui.add_space(4.0);
            let (rect, _) = ui.allocate_exact_size(vec2(width, 210.0), Sense::hover());
            draw_fan_gauge(ui, rect, fan);
            ui.add_space(6.0);
            ui.label(
                RichText::new(fan_temperature_text(fan))
                    .size(13.0)
                    .strong()
                    .color(Color32::from_rgb(222, 214, 199)),
            );
        },
    );
}

fn fan_temperature_text(fan: &FanStatus) -> String {
    match fan.temperature_celsius {
        Some(temp) => format!("温度 {temp}°C"),
        None => "温度 --°C".to_owned(),
    }
}

fn draw_fan_gauge(ui: &mut Ui, rect: egui::Rect, fan: &FanStatus) {
    const START_ANGLE: f32 = PI * 0.82;
    const SWEEP_ANGLE: f32 = PI * 1.36;

    let painter = ui.painter_at(rect);
    let center = pos2(rect.center().x, rect.top() + 125.0);
    let radius = rect.width().min(235.0) * 0.34;
    let progress = fan_load(fan.rpm);
    let accent = fan_accent(progress);

    for step in 0..=10 {
        let angle = START_ANGLE + SWEEP_ANGLE * (step as f32 / 10.0);
        let outer = point_on_circle(center, radius + 7.0, angle);
        let inner = point_on_circle(
            center,
            radius - if step % 5 == 0 { 10.0 } else { 5.0 },
            angle,
        );
        painter.line_segment(
            [inner, outer],
            Stroke::new(
                if step % 5 == 0 { 1.6 } else { 1.0 },
                Color32::from_rgb(98, 87, 68),
            ),
        );
    }

    draw_arc(
        &painter,
        center,
        radius,
        START_ANGLE,
        SWEEP_ANGLE,
        Stroke::new(8.0, Color32::from_rgb(47, 43, 36)),
    );
    if progress > 0.0 {
        draw_arc(
            &painter,
            center,
            radius,
            START_ANGLE,
            SWEEP_ANGLE * progress,
            Stroke::new(8.0, accent),
        );
    }

    let needle_angle = START_ANGLE + SWEEP_ANGLE * progress;
    draw_needle(&painter, center, radius * 0.80, needle_angle, accent);
    painter.circle_filled(center, 11.0, Color32::from_rgb(18, 17, 15));
    painter.circle_filled(center, 5.0, accent);

    draw_gauge_label(&painter, center, radius, START_ANGLE, "0");
    draw_gauge_label(
        &painter,
        center,
        radius,
        START_ANGLE + SWEEP_ANGLE * 0.5,
        "2600",
    );
    draw_gauge_label(&painter, center, radius, START_ANGLE + SWEEP_ANGLE, "5200");

    let rpm_text = if fan.rpm == 0 {
        "--".to_owned()
    } else {
        fan.rpm.to_string()
    };
    painter.text(
        center + vec2(0.0, 32.0),
        Align2::CENTER_CENTER,
        rpm_text,
        FontId::proportional(30.0),
        accent,
    );
    painter.text(
        center + vec2(0.0, 58.0),
        Align2::CENTER_CENTER,
        "RPM",
        FontId::proportional(11.0),
        Color32::from_rgb(145, 138, 127),
    );
}

fn draw_needle(painter: &egui::Painter, center: Pos2, length: f32, angle: f32, color: Color32) {
    let tip = point_on_circle(center, length, angle);
    let base_left = point_on_circle(center, 8.0, angle + PI * 0.5);
    let base_right = point_on_circle(center, 8.0, angle - PI * 0.5);
    painter.add(Shape::convex_polygon(
        vec![tip, base_left, center, base_right],
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 230),
        Stroke::new(1.0, Color32::from_rgb(22, 20, 17)),
    ));
    painter.line_segment(
        [center, tip],
        Stroke::new(2.0, Color32::from_rgb(255, 235, 205)),
    );
}

fn draw_gauge_label(painter: &egui::Painter, center: Pos2, radius: f32, angle: f32, label: &str) {
    painter.text(
        point_on_circle(center, radius - 28.0, angle),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.0),
        Color32::from_rgb(125, 118, 106),
    );
}

fn fan_load(rpm: u32) -> f32 {
    (rpm as f32 / 5200.0).clamp(0.0, 1.0)
}

fn fan_accent(progress: f32) -> Color32 {
    if progress >= 0.54 {
        Color32::from_rgb(225, 126, 88)
    } else if progress >= 0.23 {
        Color32::from_rgb(231, 176, 96)
    } else {
        Color32::from_rgb(154, 194, 132)
    }
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    sweep_angle: f32,
    stroke: Stroke,
) {
    let segments = ((sweep_angle.abs() / (PI * 2.0)) * 96.0).ceil().max(8.0) as usize;
    let points = (0..=segments)
        .map(|index| {
            let t = index as f32 / segments as f32;
            point_on_circle(center, radius, start_angle + sweep_angle * t)
        })
        .collect::<Vec<_>>();
    painter.add(Shape::line(points, stroke));
}

fn point_on_circle(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    center + vec2(angle.cos() * radius, angle.sin() * radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_load_clamps_to_gauge_range() {
        assert_eq!(fan_load(0), 0.0);
        assert!((fan_load(2600) - 0.5).abs() < f32::EPSILON);
        assert_eq!(fan_load(9000), 1.0);
    }

    #[test]
    fn fan_temperature_text_formats_missing_and_present_values() {
        assert_eq!(
            fan_temperature_text(&FanStatus {
                label: "CPU 风扇".to_owned(),
                raw_tach: 0,
                rpm: 900,
                temperature_celsius: Some(43),
            }),
            "温度 43°C"
        );
        assert_eq!(
            fan_temperature_text(&FanStatus {
                label: "GPU 风扇".to_owned(),
                raw_tach: 0,
                rpm: 0,
                temperature_celsius: None,
            }),
            "温度 --°C"
        );
    }
}

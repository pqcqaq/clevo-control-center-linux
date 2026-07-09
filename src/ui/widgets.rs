use std::f32::consts::PI;
use std::fs;
use std::io;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, pos2, vec2, Align2, Button, Color32, Context, FontData, FontDefinitions, FontFamily,
    FontId, Frame, Pos2, RichText, ScrollArea, Sense, Shape, Stroke, Ui,
};

use super::app::ClevoLedApp;
use crate::dchu::{FanStatus, HardwareSnapshot};
use crate::model::{Mode, Rgb};

pub(super) fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
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

pub(super) fn control_group<F: FnMut(&str)>(
    ui: &mut Ui,
    title: &str,
    items: &[(&str, &str)],
    mut action: F,
) {
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
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

pub(super) fn command_panel(ui: &mut Ui, app: &mut ClevoLedApp) {
    if let Some(status) = &app.command_status {
        ui.label(
            RichText::new(status)
                .size(13.0)
                .color(Color32::from_rgb(226, 184, 112)),
        );
    }
    if !app.command_output.is_empty() {
        Frame::none()
            .fill(Color32::from_rgb(15, 14, 13))
            .rounding(10.0)
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    ui.monospace(&app.command_output);
                });
            });
    }
}

pub(super) fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(62.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 23.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(15, 14, 13));
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

pub(super) fn hardware_details(ui: &mut Ui, app: &ClevoLedApp) {
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

fn rgb_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

pub(super) fn snapshot_age_text(snapshot: &HardwareSnapshot) -> String {
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
                rpm: 900,
                temperature_celsius: Some(43),
            }),
            "温度 43°C"
        );
        assert_eq!(
            fan_temperature_text(&FanStatus {
                label: "GPU 风扇".to_owned(),
                rpm: 0,
                temperature_celsius: None,
            }),
            "温度 --°C"
        );
    }
}

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, pos2, vec2, Color32, Context, FontData, FontDefinitions, FontFamily, Frame, RichText,
    ScrollArea, Sense, Stroke, Ui,
};

use super::app::ClevoLedApp;
use crate::dchu::HardwareSnapshot;

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

pub(super) fn toggle_switch(ui: &mut Ui, enabled: bool) -> bool {
    let desired_size = vec2(48.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), enabled, 0.14);
    let hover_t =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    let fill = mix_color(
        Color32::from_rgb(49, 45, 37),
        Color32::from_rgb(138, 88, 33),
        t,
    );
    let stroke = mix_color(
        Color32::from_rgb(72, 64, 52),
        Color32::from_rgb(235, 168, 80),
        t.max(hover_t * 0.5),
    );
    let painter = ui.painter_at(rect.expand(3.0));
    painter.rect_filled(rect, 12.0, fill);
    painter.rect_stroke(rect, 12.0, Stroke::new(1.0 + hover_t * 0.5, stroke));
    let knob_x = rect.left() + 12.0 + (rect.width() - 24.0) * t;
    let knob_center = pos2(knob_x, rect.center().y);
    painter.circle_filled(knob_center, 8.0, Color32::from_rgb(239, 228, 207));
    painter.circle_stroke(
        knob_center,
        8.0,
        Stroke::new(1.0, Color32::from_rgb(34, 30, 25)),
    );
    response.clicked()
}

fn mix_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
        mix(from.a(), to.a()),
    )
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

pub(super) fn snapshot_age_text(snapshot: &HardwareSnapshot) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let age = now.saturating_sub(snapshot.updated_unix_secs);
    format!("硬件状态更新于 {age} 秒前")
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

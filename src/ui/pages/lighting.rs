use eframe::egui::{vec2, Color32, ComboBox, Frame, RichText, Slider, Ui};

use super::super::app::ClevoLedApp;
use super::super::color_picker::color_swatch;
use super::super::widgets::page_header;
use crate::model::{Mode, BASE_ZONES};

pub(super) fn lighting_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "灯光", "控制键盘 RGB 原生灯效、颜色和亮度");
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
                                ui.selectable_value(&mut app.mode, *mode, mode.label());
                                if app.mode != old_mode {
                                    app.mark_settings_dirty();
                                    app.persist_settings_if_due(true);
                                }
                            }
                        });

                    ui.add_space(12.0);
                    if brightness_control(ui, &mut app.brightness) {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }

                    if matches!(app.mode, Mode::Custom | Mode::Breathing) {
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("作用区域")
                                    .size(13.0)
                                    .color(Color32::from_rgb(193, 186, 173)),
                            );
                            for (zone, label) in BASE_ZONES.into_iter().zip(["左", "中", "右"]) {
                                let enabled = app.zones.contains(&zone);
                                if ui.selectable_label(enabled, label).clicked() {
                                    app.set_zone_enabled(zone, !enabled);
                                }
                            }
                        });
                    }
                });
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

fn brightness_control(ui: &mut Ui, value: &mut u8) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_width(330.0);
        ui.label(
            RichText::new("亮度")
                .size(13.0)
                .color(Color32::from_rgb(193, 186, 173)),
        );
        changed = ui
            .add_sized(
                vec2(250.0, 20.0),
                Slider::new(value, 1..=100).integer().suffix("%"),
            )
            .changed();
    });
    changed
}

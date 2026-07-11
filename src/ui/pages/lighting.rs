use eframe::egui::{vec2, Button, Color32, ComboBox, Frame, RichText, Slider, Ui};

use super::super::app::ClevoLedApp;
use super::super::color_picker::color_swatch;
use super::super::widgets::page_header;
use crate::model::Mode;

pub(super) fn lighting_page(ui: &mut Ui, app: &mut ClevoLedApp) {
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

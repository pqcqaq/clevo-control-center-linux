use eframe::egui::{vec2, Color32, ComboBox, Frame, RichText, Slider, Ui};

use super::super::app::ClevoLedApp;
use super::super::color_picker::color_swatch;
use super::super::widgets::page_header;
use crate::dchu::KeyboardLightingLayout;
use crate::model::{Mode, BASE_ZONES};

pub(super) fn lighting_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(
        ui,
        app.language.pick("灯光", "Lighting"),
        app.language.pick(
            "由后台服务以 60 FPS 控制键盘 RGB 动效、颜色和亮度",
            "Control keyboard RGB effects, color, and brightness at 60 FPS in the background service",
        ),
    );
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
                        .selected_text(app.mode.localized_label(app.language))
                        .show_ui(ui, |ui| {
                            for mode in Mode::all() {
                                let old_mode = app.mode;
                                ui.selectable_value(
                                    &mut app.mode,
                                    *mode,
                                    mode.localized_label(app.language),
                                );
                                if app.mode != old_mode {
                                    app.mark_settings_dirty();
                                    app.persist_settings_if_due(true);
                                }
                            }
                        });

                    ui.add_space(12.0);
                    if brightness_control(ui, &mut app.brightness, app.language) {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }

                    if matches!(app.mode, Mode::Custom | Mode::Breathing) {
                        ui.add_space(12.0);
                        match app.keyboard_lighting_capabilities().layout {
                            KeyboardLightingLayout::ThreeZone | KeyboardLightingLayout::Unknown => {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(app.language.pick("作用区域", "Zones"))
                                            .size(13.0)
                                            .color(Color32::from_rgb(193, 186, 173)),
                                    );
                                    let labels = [
                                        app.language.pick("左", "Left"),
                                        app.language.pick("中", "Center"),
                                        app.language.pick("右", "Right"),
                                    ];
                                    for (zone, label) in BASE_ZONES.into_iter().zip(labels) {
                                        let enabled = app.zones.contains(&zone);
                                        if ui.selectable_label(enabled, label).clicked() {
                                            app.set_zone_enabled(zone, !enabled);
                                        }
                                    }
                                });
                            }
                            KeyboardLightingLayout::SingleZone => {
                                ui.label(
                                    RichText::new(
                                        app.language
                                            .pick("作用区域 · 整个键盘", "Zone · Entire keyboard"),
                                    )
                                    .size(13.0)
                                    .color(Color32::from_rgb(193, 186, 173)),
                                );
                            }
                            layout => {
                                ui.label(
                                    RichText::new(layout.localized_label(app.language))
                                        .size(13.0)
                                        .color(Color32::from_rgb(193, 186, 173)),
                                );
                            }
                        }
                        let capabilities = app.keyboard_lighting_capabilities();
                        if capabilities.lightbar == Some(true) || capabilities.logo == Some(true) {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if capabilities.lightbar == Some(true) {
                                    let enabled = app.zones.contains(&crate::model::ZoneId::F3);
                                    if ui
                                        .selectable_label(
                                            enabled,
                                            app.language.pick("灯条", "Light bar"),
                                        )
                                        .clicked()
                                    {
                                        app.set_zone_enabled(crate::model::ZoneId::F3, !enabled);
                                    }
                                }
                                if capabilities.logo == Some(true) {
                                    let enabled = app.zones.contains(&crate::model::ZoneId::F6);
                                    if ui
                                        .selectable_label(
                                            enabled,
                                            app.language.pick("Logo", "Logo"),
                                        )
                                        .clicked()
                                    {
                                        app.set_zone_enabled(crate::model::ZoneId::F6, !enabled);
                                    }
                                }
                            });
                        }
                    }
                });
            });
        });
}

fn brightness_control(
    ui: &mut Ui,
    value: &mut u8,
    language: crate::preferences::UiLanguage,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_width(330.0);
        ui.label(
            RichText::new(language.pick("亮度", "Brightness"))
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

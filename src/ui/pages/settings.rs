use eframe::egui::{vec2, Button, Color32, Frame, RichText, Stroke, Ui};

use super::super::app::ClevoLedApp;
use super::super::theme;
use super::super::widgets::{hardware_details, page_header};
use crate::dchu::KeyboardLightingLayout;
use crate::model::{ZoneId, BASE_ZONES};
use crate::preferences::{LanguagePreference, ThemeColor};

pub(super) fn settings_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    page_header(
        ui,
        language.pick("设置", "Settings"),
        language.pick(
            "界面偏好、键盘分区与硬件状态",
            "Interface, keyboard zones, and hardware status",
        ),
    );

    interface_preferences(ui, app);
    section_divider(ui);
    keyboard_zones(ui, app);
    section_divider(ui);
    hardware_status(ui, app);
}

fn interface_preferences(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    section_heading(
        ui,
        language.pick("外观与语言", "Appearance and language"),
        language.pick(
            "更改会立即应用并自动保存",
            "Changes apply immediately and are saved automatically",
        ),
    );

    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        ui.label(
            RichText::new(language.pick("语言", "Language"))
                .size(13.0)
                .color(Color32::from_rgb(190, 183, 171)),
        );
        for preference in LanguagePreference::ALL {
            let selected = app.language_preference == preference;
            if ui
                .add_sized(
                    vec2(130.0, 30.0),
                    Button::new(preference.label(language)).selected(selected),
                )
                .clicked()
            {
                app.set_language_preference(preference);
            }
        }
    });

    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        ui.label(
            RichText::new(language.pick("主题", "Theme"))
                .size(13.0)
                .color(Color32::from_rgb(190, 183, 171)),
        );
        for color in ThemeColor::ALL {
            let palette = theme::palette(color);
            let selected = app.theme_color == color;
            let text = RichText::new(format!("●  {}", color.label(language)))
                .size(12.0)
                .color(if selected {
                    palette.bright
                } else {
                    palette.accent
                });
            if ui
                .add_sized(
                    vec2(104.0, 30.0),
                    Button::new(text)
                        .selected(selected)
                        .fill(if selected {
                            palette.selected_surface
                        } else {
                            Color32::from_rgb(29, 28, 25)
                        })
                        .stroke(Stroke::new(
                            if selected { 1.4 } else { 1.0 },
                            if selected {
                                palette.border
                            } else {
                                Color32::from_rgb(61, 57, 50)
                            },
                        )),
                )
                .clicked()
            {
                app.set_theme_color(color);
            }
        }
    });
}

fn keyboard_zones(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    let capabilities = app.keyboard_lighting_capabilities();
    section_heading(
        ui,
        language.pick("键盘分区", "Keyboard zones"),
        capabilities.layout.localized_label(language),
    );
    ui.add_space(10.0);
    match capabilities.layout {
        KeyboardLightingLayout::SingleZone => {
            ui.label(language.pick(
                "这台机器只有一个可控键盘区域；每帧只需一次硬件写入。",
                "This machine exposes one keyboard zone and needs one hardware write per frame.",
            ));
        }
        KeyboardLightingLayout::ThreeZone | KeyboardLightingLayout::Unknown => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                let labels = [
                    language.pick("左区", "Left"),
                    language.pick("中区", "Center"),
                    language.pick("右区", "Right"),
                ];
                for (zone, label) in BASE_ZONES.into_iter().zip(labels) {
                    zone_checkbox(ui, app, zone, label);
                }
            });
        }
        KeyboardLightingLayout::PerKey => {
            ui.label(language.pick(
                "检测到逐键 RGB；当前版本尚未开放逐键映射。",
                "Per-key RGB was detected; per-key mapping is not exposed yet.",
            ));
        }
        KeyboardLightingLayout::White => {
            ui.label(language.pick(
                "检测到白光键盘，不提供 RGB 分区选择。",
                "A white-backlight keyboard was detected; RGB zones are unavailable.",
            ));
        }
        KeyboardLightingLayout::Unsupported => {
            ui.label(language.pick(
                "固件未报告键盘灯支持。",
                "Firmware reports no keyboard lighting support.",
            ));
        }
    }

    if capabilities.lightbar == Some(true) || capabilities.logo == Some(true) {
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if capabilities.lightbar == Some(true) {
                zone_checkbox(ui, app, ZoneId::F3, language.pick("灯条", "Light bar"));
            }
            if capabilities.logo == Some(true) {
                zone_checkbox(ui, app, ZoneId::F6, "Logo");
            }
        });
    }
}

fn zone_checkbox(ui: &mut Ui, app: &mut ClevoLedApp, zone: ZoneId, label: &str) {
    let mut enabled = app.zones.contains(&zone);
    let active_zones = app.selected_zones();
    let is_last_enabled = enabled && active_zones.len() == 1;
    let response = ui
        .add_enabled_ui(!is_last_enabled, |ui| {
            ui.add_sized(
                vec2(104.0, 30.0),
                eframe::egui::Checkbox::new(&mut enabled, label),
            )
        })
        .inner;
    if response.changed() {
        app.set_zone_enabled(zone, enabled);
    }
}

fn hardware_status(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        section_heading(
            ui,
            language.pick("硬件读回", "Hardware status"),
            language.pick(
                "后台服务最近一次同步结果",
                "Latest background service snapshot",
            ),
        );
        if ui
            .add_sized(
                vec2(118.0, 30.0),
                Button::new(language.pick("更新状态", "Refresh")),
            )
            .clicked()
        {
            app.refresh_hardware_snapshot(true);
        }
    });
    ui.add_space(8.0);
    Frame::none()
        .fill(Color32::from_rgb(32, 31, 28))
        .rounding(6.0)
        .inner_margin(eframe::egui::Margin::same(10.0))
        .show(ui, |ui| hardware_details(ui, app));
}

fn section_heading(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .size(15.0)
                .strong()
                .color(Color32::from_rgb(236, 230, 218)),
        );
        ui.label(
            RichText::new(subtitle)
                .size(11.0)
                .color(Color32::from_rgb(139, 133, 122)),
        );
    });
}

fn section_divider(ui: &mut Ui) {
    ui.add_space(13.0);
    ui.separator();
    ui.add_space(13.0);
}

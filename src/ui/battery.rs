use eframe::egui::{pos2, vec2, Align, Color32, Frame, Layout, Rect, RichText, Sense, Stroke, Ui};

use super::app::ClevoLedApp;
use super::theme;
use super::widgets::page_header;
use crate::dchu::{BatteryCapacityUnit, BatteryChargeStatus, DchuConfig, SystemBatteryInfo};
use crate::preferences::UiLanguage;

pub(super) fn battery_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    let language = app.language;
    page_header(
        ui,
        language.pick("电池", "Battery"),
        language.pick(
            "电池健康、当前状态与充电保护",
            "Battery health, live status and charge protection",
        ),
    );

    let config = app
        .hardware
        .as_ref()
        .and_then(|snapshot| snapshot.dchu_config.as_ref());
    let system_battery = app
        .hardware
        .as_ref()
        .and_then(|snapshot| snapshot.system_battery.as_ref());
    let palette = theme::palette(app.theme_color);
    let command_status = app.command_status.as_deref();
    let command_output = app.command_output.as_str();
    let mut battery_saver_request = None;

    Frame::none()
        .fill(Color32::from_rgb(28, 29, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 57, 54)))
        .rounding(8.0)
        .inner_margin(eframe::egui::Margin::same(16.0))
        .show(ui, |ui| {
            health_summary(ui, language, config, system_battery);
            section_divider(ui);

            let available_width = ui.available_width();
            if available_width >= 620.0 {
                ui.columns(2, |columns| {
                    columns[0].set_width((available_width - 26.0) * 0.5);
                    columns[1].set_width((available_width - 26.0) * 0.5);
                    health_details(&mut columns[0], language, config, system_battery);
                    battery_saver_request =
                        firmware_protection(&mut columns[1], language, config, palette.accent);
                });
            } else {
                health_details(ui, language, config, system_battery);
                section_divider(ui);
                battery_saver_request = firmware_protection(ui, language, config, palette.accent);
            }
            battery_action_status(ui, command_status, command_output);
        });

    if let Some(enabled) = battery_saver_request {
        app.set_battery_saver_enabled(enabled);
    }
}

fn health_summary(
    ui: &mut Ui,
    language: UiLanguage,
    config: Option<&DchuConfig>,
    system_battery: Option<&SystemBatteryInfo>,
) {
    let oem_health = config.and_then(DchuConfig::battery_health_percent);
    let health = oem_health.or_else(|| system_battery.and_then(SystemBatteryInfo::health_percent));
    let color = health_color(health);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(language.pick("电池健康度", "Battery health"))
                    .size(12.0)
                    .color(Color32::from_rgb(145, 153, 148)),
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(
                        health
                            .map(|value| format!("{value}%"))
                            .unwrap_or_else(|| "--".to_owned()),
                    )
                    .size(30.0)
                    .strong()
                    .color(Color32::from_rgb(236, 241, 238)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(match health {
                        Some(80..=100) => language.pick("状态良好", "Healthy"),
                        Some(60..=79) => language.pick("已有损耗", "Worn"),
                        Some(_) => language.pick("建议检查", "Check battery"),
                        None => language.pick("等待电池数据", "Waiting for battery data"),
                    })
                    .size(12.0)
                    .color(color),
                );
            });
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let oem_telemetry = config.is_some_and(DchuConfig::battery_info_available);
            let system_telemetry = system_battery.is_some();
            ui.label(
                RichText::new(if oem_telemetry {
                    language.pick("固件数据", "Firmware data")
                } else if system_telemetry {
                    language.pick("系统电池数据", "System battery data")
                } else {
                    language.pick("暂时无数据", "No data available")
                })
                .size(11.0)
                .color(if oem_telemetry || system_telemetry {
                    Color32::from_rgb(137, 181, 158)
                } else {
                    Color32::from_rgb(118, 124, 120)
                }),
            );
        });
    });

    ui.add_space(10.0);
    health_meter(ui, health, color);
}

fn health_meter(ui: &mut Ui, health: Option<u8>, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let track = Rect::from_min_max(
        pos2(rect.left() + 2.0, rect.center().y - 5.0),
        pos2(rect.right() - 2.0, rect.center().y + 5.0),
    );
    painter.rect_filled(track, 5.0, Color32::from_rgb(45, 49, 47));
    if let Some(health) = health {
        let fill = Rect::from_min_max(
            track.min,
            pos2(
                track.left() + track.width() * f32::from(health) / 100.0,
                track.bottom(),
            ),
        );
        painter.rect_filled(fill, 5.0, color);
    }
    for percent in [0_u8, 25, 50, 75, 100] {
        let x = track.left() + track.width() * f32::from(percent) / 100.0;
        painter.line_segment(
            [pos2(x, track.top() - 3.0), pos2(x, track.bottom() + 3.0)],
            Stroke::new(1.0, Color32::from_rgb(73, 79, 75)),
        );
    }
}

fn health_details(
    ui: &mut Ui,
    language: UiLanguage,
    config: Option<&DchuConfig>,
    system_battery: Option<&SystemBatteryInfo>,
) {
    let oem_config = config.filter(|item| item.battery_info_available());
    section_title(
        ui,
        language.pick("当前状态", "Live status"),
        language.pick("充电状态与可用容量", "Charge state and usable capacity"),
    );
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(
                system_battery
                    .and_then(|item| item.charge_percent)
                    .map(|value| format!("{value}%"))
                    .unwrap_or_else(|| "--".to_owned()),
            )
            .size(26.0)
            .strong()
            .color(Color32::from_rgb(232, 238, 234)),
        );
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(language.pick("当前电量", "Charge"))
                    .size(10.0)
                    .color(Color32::from_rgb(126, 135, 129)),
            );
            ui.label(
                RichText::new(localized_charge_status(language, system_battery))
                    .size(11.0)
                    .color(Color32::from_rgb(191, 201, 195)),
            );
        });
    });
    ui.add_space(12.0);
    detail_row(
        ui,
        language.pick("可用 / 设计容量", "Usable / design capacity"),
        oem_config
            .and_then(|item| {
                Some(format!(
                    "{} / {} mAh",
                    item.battery_full_charge_capacity?, item.battery_design_capacity?
                ))
            })
            .or_else(|| system_capacity_label(system_battery))
            .unwrap_or_else(|| "--".to_owned()),
    );
}

fn firmware_protection(
    ui: &mut Ui,
    language: UiLanguage,
    config: Option<&DchuConfig>,
    accent: Color32,
) -> Option<bool> {
    section_title(
        ui,
        language.pick("充电保护", "Charge protection"),
        language.pick(
            "降低长期满充带来的电池损耗",
            "Helps reduce wear from prolonged full charge",
        ),
    );
    ui.add_space(10.0);

    let supported = config.and_then(DchuConfig::battery_saver_capability);
    let enabled = config.and_then(DchuConfig::battery_saver_enabled);
    let current_state = enabled.unwrap_or(false);
    let mut requested_state = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match (supported, enabled) {
                    (Some(true), Some(true)) => language.pick("已开启", "On"),
                    (Some(true), Some(false)) => language.pick("已关闭", "Off"),
                    (Some(true), None) => language.pick("暂时不可用", "Unavailable"),
                    (Some(false), _) => {
                        language.pick("此机型暂不支持", "Not available on this model")
                    }
                    (None, _) => language.pick("正在检测", "Checking support"),
                })
                .size(14.0)
                .strong()
                .color(if enabled == Some(true) {
                    accent
                } else {
                    Color32::from_rgb(176, 182, 178)
                }),
            );
            let explanation = match supported {
                Some(false) => language.pick(
                    "固件未开放该控制能力，已自动锁定以保护设备",
                    "Firmware control is unavailable and has been safely locked",
                ),
                Some(true) => language.pick(
                    "切换后会立即确认固件状态",
                    "Firmware state is verified after each change",
                ),
                _ => language.pick(
                    "读取固件能力后即可使用",
                    "Available after firmware detection",
                ),
            };
            ui.label(
                RichText::new(explanation)
                    .size(11.0)
                    .color(Color32::from_rgb(126, 135, 129)),
            );
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let interactive = supported == Some(true) && enabled.is_some();
            ui.add_enabled_ui(interactive, |ui| {
                if battery_toggle(ui, current_state, accent) {
                    requested_state = Some(!current_state);
                }
            });
        });
    });
    requested_state
}

fn system_capacity_label(battery: Option<&SystemBatteryInfo>) -> Option<String> {
    let battery = battery?;
    let full = battery.full_capacity?;
    let design = battery.design_capacity?;
    let unit = match battery.capacity_unit? {
        BatteryCapacityUnit::MilliampHours => "mAh",
        BatteryCapacityUnit::MilliwattHours => "mWh",
    };
    Some(format!("{full} / {design} {unit}"))
}

fn localized_charge_status(
    language: UiLanguage,
    battery: Option<&SystemBatteryInfo>,
) -> &'static str {
    match battery.and_then(|item| item.status) {
        Some(BatteryChargeStatus::Charging) => language.pick("正在充电", "Charging"),
        Some(BatteryChargeStatus::Discharging) => language.pick("电池供电", "On battery"),
        Some(BatteryChargeStatus::Full) => language.pick("已充满", "Fully charged"),
        Some(BatteryChargeStatus::NotCharging) => {
            language.pick("已接通，未充电", "Plugged in, not charging")
        }
        Some(BatteryChargeStatus::Unknown) | None => language.pick("状态未知", "Status unknown"),
    }
}

fn battery_action_status(ui: &mut Ui, status: Option<&str>, command_output: &str) {
    let Some(status) = status else {
        return;
    };
    if !status.contains("电池") && !status.to_ascii_lowercase().contains("battery") {
        return;
    }

    ui.add_space(10.0);
    ui.label(
        RichText::new(status)
            .size(11.0)
            .color(if command_output.is_empty() {
                Color32::from_rgb(137, 181, 158)
            } else {
                Color32::from_rgb(221, 116, 94)
            }),
    );
    if !command_output.is_empty() {
        ui.label(
            RichText::new(command_output)
                .size(10.0)
                .color(Color32::from_rgb(166, 139, 130)),
        );
    }
}

fn detail_row(ui: &mut Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(Color32::from_rgb(139, 148, 142)),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(value)
                .size(11.0)
                .color(Color32::from_rgb(216, 223, 219)),
        );
    });
    ui.add_space(7.0);
}

fn section_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(225, 232, 228)),
        );
        ui.add_space(1.0);
        ui.label(
            RichText::new(subtitle)
                .size(11.0)
                .color(Color32::from_rgb(126, 135, 129)),
        );
    });
}

fn section_divider(ui: &mut Ui) {
    ui.add_space(9.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, Color32::from_rgb(48, 53, 50)),
    );
    ui.add_space(9.0);
}

fn battery_toggle(ui: &mut Ui, enabled: bool, accent: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(42.0, 22.0), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("battery_toggle"), enabled, 0.14);
    let hover_t = ui.ctx().animate_bool_with_time(
        response.id.with("battery_toggle_hover"),
        response.hovered(),
        0.1,
    );
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect_filled(
        rect,
        11.0,
        mix_color(Color32::from_rgb(46, 51, 48), accent, t * 0.65),
    );
    painter.rect_stroke(
        rect,
        11.0,
        Stroke::new(
            1.0,
            mix_color(Color32::from_rgb(70, 77, 73), accent, t.max(hover_t * 0.5)),
        ),
    );
    painter.circle_filled(
        pos2(
            rect.left() + 11.0 + (rect.width() - 22.0) * t,
            rect.center().y,
        ),
        7.0,
        Color32::from_rgb(231, 238, 234),
    );
    response.clicked()
}

fn health_color(health: Option<u8>) -> Color32 {
    match health {
        Some(80..=100) => Color32::from_rgb(82, 210, 160),
        Some(60..=79) => Color32::from_rgb(220, 172, 82),
        Some(_) => Color32::from_rgb(221, 116, 94),
        None => Color32::from_rgb(104, 111, 107),
    }
}

fn mix_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_color_uses_distinct_condition_bands() {
        assert_ne!(health_color(Some(90)), health_color(Some(70)));
        assert_ne!(health_color(Some(70)), health_color(Some(40)));
        assert_ne!(health_color(Some(40)), health_color(None));
    }
}

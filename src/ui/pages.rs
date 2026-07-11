mod gpu;
mod lighting;
mod overview;

use eframe::egui::{vec2, Button, Color32, Frame, RichText, Ui};

#[cfg(debug_assertions)]
use super::advanced;
use super::app::ClevoLedApp;
#[cfg(debug_assertions)]
use super::widgets::command_panel;
use super::widgets::{hardware_details, page_header};
use super::{battery, fan};
#[cfg(debug_assertions)]
use crate::model::AdvancedTab;
use crate::model::{ControlPage, ALL_ZONES};

pub(super) fn show_active_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    match app.active_page {
        ControlPage::Overview => overview::overview_page(ui, app),
        ControlPage::Lighting => lighting::lighting_page(ui, app),
        ControlPage::Fan => fan::fan_page(ui, app),
        ControlPage::Battery => battery::battery_page(ui, app),
        ControlPage::Gpu => gpu::gpu_page(ui, app),
        #[cfg(debug_assertions)]
        ControlPage::Diagnostics => diagnostics_page(ui, app),
        ControlPage::Settings => settings_page(ui, app),
        #[cfg(debug_assertions)]
        ControlPage::Advanced => advanced_page(ui, app),
    }
}

#[cfg(debug_assertions)]
fn advanced_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "高级", "DCHU 0x0C 只读硬件状态");
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for tab in AdvancedTab::all() {
                    let selected = app.advanced_tab == *tab;
                    let fill = if selected {
                        Color32::from_rgb(69, 51, 28)
                    } else {
                        Color32::from_rgb(28, 27, 24)
                    };
                    if ui
                        .add_sized(vec2(104.0, 30.0), Button::new(tab.label()).fill(fill))
                        .clicked()
                    {
                        app.advanced_tab = *tab;
                    }
                }
            });
            ui.add_space(12.0);
            match app.advanced_tab {
                AdvancedTab::Fans => advanced::fan_info(ui, app.hardware.as_ref()),
                AdvancedTab::Temperatures => advanced::temperature_info(ui, app.hardware.as_ref()),
                AdvancedTab::Other => advanced::other_info(ui, app.hardware.as_ref()),
            }
        });
}

#[cfg(debug_assertions)]
fn diagnostics_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "诊断", "读取 DCHU 只读硬件状态");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        if ui
            .add_sized(vec2(120.0, 34.0), Button::new("状态"))
            .clicked()
        {
            app.show_hardware_diagnostics();
        }
    });
    ui.add_space(12.0);
    command_panel(ui, app);
}

fn settings_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(ui, "设置", "选择键盘生效分区并查看硬件读回");
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("键盘分区")
                    .size(15.0)
                    .strong()
                    .color(Color32::from_rgb(236, 230, 218)),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for zone in ALL_ZONES {
                    let mut enabled = app.zones.contains(&zone);
                    let is_last_enabled = enabled && app.zones.len() == 1;
                    let response = ui
                        .add_enabled_ui(!is_last_enabled, |ui| {
                            ui.add_sized(
                                vec2(72.0, 30.0),
                                egui::Checkbox::new(&mut enabled, zone.label()),
                            )
                        })
                        .inner;
                    if response.changed() {
                        app.set_zone_enabled(zone, enabled);
                    }
                }
            });
        });

    ui.add_space(14.0);
    Frame::none()
        .fill(Color32::from_rgb(35, 34, 30))
        .rounding(10.0)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                ui.label(
                    RichText::new("硬件读回")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(236, 230, 218)),
                );
                if ui
                    .add_sized(vec2(118.0, 30.0), Button::new("更新状态"))
                    .clicked()
                {
                    app.refresh_hardware_snapshot(true);
                }
            });
            ui.add_space(10.0);
            hardware_details(ui, app);
        });
}

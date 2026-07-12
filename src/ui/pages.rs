mod gpu;
mod lighting;
mod overview;
mod settings;

use eframe::egui::Ui;
#[cfg(debug_assertions)]
use eframe::egui::{vec2, Button, Color32, Frame};

#[cfg(debug_assertions)]
use super::advanced;
use super::app::ClevoLedApp;
#[cfg(debug_assertions)]
use super::widgets::command_panel;
#[cfg(debug_assertions)]
use super::widgets::page_header;
use super::{battery, fan};
#[cfg(debug_assertions)]
use crate::model::AdvancedTab;
use crate::model::ControlPage;

pub(super) fn show_active_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    match app.active_page {
        ControlPage::Overview => overview::overview_page(ui, app),
        ControlPage::Lighting => lighting::lighting_page(ui, app),
        ControlPage::Fan => fan::fan_page(ui, app),
        ControlPage::Battery => battery::battery_page(ui, app),
        ControlPage::Gpu => gpu::gpu_page(ui, app),
        #[cfg(debug_assertions)]
        ControlPage::Diagnostics => diagnostics_page(ui, app),
        ControlPage::Settings => settings::settings_page(ui, app),
        #[cfg(debug_assertions)]
        ControlPage::Advanced => advanced_page(ui, app),
    }
}

#[cfg(debug_assertions)]
fn advanced_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(
        ui,
        app.language.pick("高级", "Advanced"),
        app.language.pick(
            "DCHU 0x0C 只读硬件状态",
            "Read-only DCHU 0x0C hardware status",
        ),
    );
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
                        .add_sized(
                            vec2(104.0, 30.0),
                            Button::new(tab.localized_label(app.language)).fill(fill),
                        )
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
    page_header(
        ui,
        app.language.pick("诊断", "Diagnostics"),
        app.language.pick(
            "读取 DCHU 只读硬件状态",
            "Read DCHU hardware status without writing",
        ),
    );
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        if ui
            .add_sized(
                vec2(120.0, 34.0),
                Button::new(app.language.pick("状态", "Read status")),
            )
            .clicked()
        {
            app.show_hardware_diagnostics();
        }
    });
    ui.add_space(12.0);
    command_panel(ui, app);
}

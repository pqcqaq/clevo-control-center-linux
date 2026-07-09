use eframe::egui::{vec2, Button, Color32, Frame, RichText, ScrollArea, Ui};

use super::{app::ClevoLedApp, pages};
use crate::model::ControlPage;

const SIDEBAR_WIDTH: f32 = 176.0;
const SHELL_GAP: f32 = 12.0;

pub(super) fn control_center(ui: &mut Ui, app: &mut ClevoLedApp) {
    let available = ui.available_size();
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    ui.horizontal(|ui| {
        sidebar(ui, app, available.y);
        ui.add_space(SHELL_GAP);
        content_panel(ui, app, available.y);
    });
}

fn sidebar(ui: &mut Ui, app: &mut ClevoLedApp, height: f32) {
    Frame::none()
        .fill(Color32::from_rgb(24, 23, 21))
        .rounding(14.0)
        .inner_margin(egui::Margin::symmetric(12.0, 16.0))
        .show(ui, |ui| {
            ui.set_width(SIDEBAR_WIDTH);
            ui.set_min_height(height.max(420.0));
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("蓝天控制中心")
                        .size(18.0)
                        .strong()
                        .color(Color32::from_rgb(239, 234, 223)),
                );
                ui.label(
                    RichText::new("Linux Edition")
                        .size(12.0)
                        .color(Color32::from_rgb(139, 133, 122)),
                );
                ui.add_space(22.0);
                for page in ControlPage::all() {
                    nav_button(ui, app, *page);
                    ui.add_space(7.0);
                }
            });
        });
}

fn nav_button(ui: &mut Ui, app: &mut ClevoLedApp, page: ControlPage) {
    let selected = app.active_page == page;
    let fill = if selected {
        Color32::from_rgb(64, 47, 25)
    } else {
        Color32::from_rgb(32, 31, 28)
    };
    let text = if selected {
        Color32::from_rgb(252, 235, 207)
    } else {
        Color32::from_rgb(187, 180, 168)
    };

    if ui
        .add_sized(
            vec2(SIDEBAR_WIDTH - 24.0, 36.0),
            Button::new(RichText::new(page.label()).size(14.0).color(text)).fill(fill),
        )
        .clicked()
    {
        app.active_page = page;
    }
}

fn content_panel(ui: &mut Ui, app: &mut ClevoLedApp, height: f32) {
    let width = ui.available_width().max(1.0);
    Frame::none()
        .fill(Color32::from_rgb(29, 28, 25))
        .rounding(14.0)
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_min_height(height.max(420.0));
            ui.vertical(|ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| pages::show_active_page(ui, app));
            });
        });
}

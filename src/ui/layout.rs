use eframe::egui::{vec2, Button, Color32, Frame, RichText, ScrollArea, Ui};

use super::{app::ClevoLedApp, pages};
use crate::model::ControlPage;

const SIDEBAR_CONTENT_WIDTH: f32 = 152.0;
const SIDEBAR_HORIZONTAL_MARGIN: f32 = 12.0;
const SHELL_GAP: f32 = 12.0;
const SHELL_HORIZONTAL_MARGIN: f32 = 12.0;
const SHELL_BOTTOM_MARGIN: f32 = 12.0;
const NAV_BUTTON_HEIGHT: f32 = 36.0;
const CONTENT_PANEL_MARGIN: f32 = 18.0;

pub(super) fn control_center(ui: &mut Ui, app: &mut ClevoLedApp) {
    let available = shell_available_size(ui.available_size());
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    ui.horizontal(|ui| {
        ui.add_space(SHELL_HORIZONTAL_MARGIN);
        ui.allocate_ui(available, |ui| {
            ui.horizontal(|ui| {
                sidebar(ui, app, available.y);
                ui.add_space(SHELL_GAP);
                content_panel(ui, app, available.y);
            });
        });
    });
}

fn sidebar(ui: &mut Ui, app: &mut ClevoLedApp, height: f32) {
    Frame::none()
        .fill(Color32::from_rgb(24, 23, 21))
        .rounding(14.0)
        .inner_margin(egui::Margin::symmetric(SIDEBAR_HORIZONTAL_MARGIN, 16.0))
        .show(ui, |ui| {
            ui.set_width(SIDEBAR_CONTENT_WIDTH);
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
            vec2(SIDEBAR_CONTENT_WIDTH, NAV_BUTTON_HEIGHT),
            Button::new(RichText::new(page.label()).size(14.0).color(text)).fill(fill),
        )
        .clicked()
    {
        app.active_page = page;
    }
}

fn content_panel(ui: &mut Ui, app: &mut ClevoLedApp, height: f32) {
    let width = content_panel_inner_width(ui.available_width());
    Frame::none()
        .fill(Color32::from_rgb(29, 28, 25))
        .rounding(14.0)
        .inner_margin(egui::Margin::same(CONTENT_PANEL_MARGIN))
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

fn content_panel_inner_width(available_width: f32) -> f32 {
    (available_width - CONTENT_PANEL_MARGIN * 2.0).max(1.0)
}

fn shell_available_size(available_size: egui::Vec2) -> egui::Vec2 {
    vec2(
        (available_size.x - SHELL_HORIZONTAL_MARGIN * 2.0).max(1.0),
        (available_size.y - SHELL_BOTTOM_MARGIN).max(1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_buttons_fill_content_width() {
        assert_eq!(SIDEBAR_CONTENT_WIDTH, 152.0);
        assert_eq!(
            SIDEBAR_HORIZONTAL_MARGIN * 2.0 + SIDEBAR_CONTENT_WIDTH,
            176.0
        );
    }

    #[test]
    fn content_panel_inner_width_accounts_for_frame_margin() {
        assert_eq!(content_panel_inner_width(772.0), 736.0);
        assert_eq!(content_panel_inner_width(12.0), 1.0);
    }

    #[test]
    fn shell_available_size_keeps_outer_margin() {
        assert_eq!(shell_available_size(vec2(960.0, 554.0)), vec2(936.0, 542.0));
        assert_eq!(shell_available_size(vec2(10.0, 8.0)), vec2(1.0, 1.0));
    }
}

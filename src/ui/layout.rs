use eframe::egui::{
    pos2, vec2, Align2, Color32, FontId, Frame, Pos2, Rect, RichText, ScrollArea, Sense, Shape,
    Stroke, Ui,
};

use super::{app::ClevoLedApp, pages, theme};
use crate::model::ControlPage;

const SIDEBAR_CONTENT_WIDTH: f32 = 152.0;
const SIDEBAR_HORIZONTAL_MARGIN: f32 = 12.0;
const SHELL_GAP: f32 = 12.0;
const NAV_BUTTON_HEIGHT: f32 = 36.0;
const NAV_BUTTON_GAP: f32 = 10.0;
const NAV_BUTTON_SKEW: f32 = 12.0;
const CONTENT_PANEL_MARGIN: f32 = 18.0;

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
    let language = app.language;
    let palette = theme::palette(app.theme_color);
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
                    RichText::new(language.pick("蓝天控制中心", "Clevo Control Center"))
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
                    nav_button(ui, app, *page, palette);
                    ui.add_space(NAV_BUTTON_GAP);
                }
            });
        });
}

fn nav_button(ui: &mut Ui, app: &mut ClevoLedApp, page: ControlPage, palette: theme::Palette) {
    let id = ui.make_persistent_id(("nav_button", page));
    let (rect, _) = ui.allocate_exact_size(
        vec2(SIDEBAR_CONTENT_WIDTH, NAV_BUTTON_HEIGHT),
        Sense::hover(),
    );
    let response = ui.interact(rect, id, Sense::click());

    if response.clicked() {
        app.active_page = page;
    }

    draw_nav_button(
        ui,
        rect,
        &response,
        app.active_page == page,
        page.localized_label(app.language),
        palette,
    );
}

fn draw_nav_button(
    ui: &mut Ui,
    rect: Rect,
    response: &egui::Response,
    selected: bool,
    label: &str,
    palette: theme::Palette,
) {
    let hover_t =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.14);
    let selected_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("selected"), selected, 0.18);
    let press_t = ui.ctx().animate_bool_with_time(
        response.id.with("press"),
        response.is_pointer_button_down_on(),
        0.07,
    );

    let rect = rect
        .translate(vec2(hover_t * 2.5 - press_t * 1.5, press_t * 1.0))
        .shrink2(vec2(0.0, 1.0));
    let painter = ui.painter_at(rect.expand(7.0));
    let lift = hover_t.max(selected_t);
    let fill = theme::mix(
        theme::mix(Color32::from_rgb(30, 29, 26), palette.surface, selected_t),
        Color32::from_rgb(46, 42, 35),
        hover_t * 0.55,
    );
    let stroke = theme::mix(Color32::from_rgb(57, 52, 43), palette.border, lift);
    let text = theme::mix(
        Color32::from_rgb(176, 170, 158),
        palette.text,
        (selected_t + hover_t * 0.55).clamp(0.0, 1.0),
    );
    let points = nav_button_points(rect, NAV_BUTTON_SKEW);
    painter.add(Shape::convex_polygon(
        points.to_vec(),
        fill,
        Stroke::new(1.0 + selected_t, stroke),
    ));

    if lift > 0.0 {
        let glow = Color32::from_rgba_unmultiplied(
            palette.accent.r(),
            palette.accent.g(),
            palette.accent.b(),
            (44.0 * lift) as u8,
        );
        painter.add(Shape::convex_polygon(
            nav_button_points(rect.expand(3.0), NAV_BUTTON_SKEW + 1.5).to_vec(),
            glow,
            Stroke::new(0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 0)),
        ));
    }

    let rail_width = 4.0 + selected_t * 8.0;
    let rail_rect = Rect::from_min_max(
        pos2(rect.left(), rect.top() + 5.0),
        pos2(rect.left() + rail_width, rect.bottom() - 5.0),
    );
    painter.add(Shape::convex_polygon(
        nav_button_points(rail_rect, 3.0).to_vec(),
        Color32::from_rgba_unmultiplied(
            palette.accent.r(),
            palette.accent.g(),
            palette.accent.b(),
            (58.0 + 130.0 * selected_t) as u8,
        ),
        Stroke::new(0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 0)),
    ));

    let top_y = rect.top() + 5.0;
    painter.line_segment(
        [
            pos2(rect.left() + NAV_BUTTON_SKEW + 8.0, top_y),
            pos2(rect.right() - 22.0, top_y),
        ],
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                palette.bright.r(),
                palette.bright.g(),
                palette.bright.b(),
                (22.0 + 48.0 * lift) as u8,
            ),
        ),
    );

    if hover_t > 0.0 {
        let scan_x = rect.left() + 18.0 + (rect.width() - 42.0) * hover_t;
        painter.line_segment(
            [
                pos2(scan_x - 7.0, rect.top() + 7.0),
                pos2(scan_x + 2.0, rect.bottom() - 7.0),
            ],
            Stroke::new(
                1.2,
                Color32::from_rgba_unmultiplied(
                    palette.bright.r(),
                    palette.bright.g(),
                    palette.bright.b(),
                    (94.0 * hover_t) as u8,
                ),
            ),
        );
    }

    let notch_alpha = (70.0 + 150.0 * selected_t + 45.0 * hover_t).clamp(0.0, 255.0) as u8;
    painter.line_segment(
        [
            pos2(rect.right() - NAV_BUTTON_SKEW - 14.0, rect.bottom() - 5.0),
            pos2(rect.right() - 4.0, rect.bottom() - 5.0),
        ],
        Stroke::new(
            1.5,
            Color32::from_rgba_unmultiplied(
                palette.accent.r(),
                palette.accent.g(),
                palette.accent.b(),
                notch_alpha,
            ),
        ),
    );

    painter.text(
        pos2(
            rect.left() + NAV_BUTTON_SKEW + 18.0 + hover_t * 2.0,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0 + selected_t * 0.5),
        text,
    );
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

fn nav_button_points(rect: Rect, skew: f32) -> [Pos2; 4] {
    [
        pos2(rect.left() + skew, rect.top()),
        pos2(rect.right(), rect.top()),
        pos2(rect.right() - skew, rect.bottom()),
        pos2(rect.left(), rect.bottom()),
    ]
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
    fn nav_button_shape_uses_parallelogram_skew() {
        let rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 56.0));
        let points = nav_button_points(rect, 12.0);

        assert_eq!(points[0], pos2(22.0, 20.0));
        assert_eq!(points[1], pos2(110.0, 20.0));
        assert_eq!(points[2], pos2(98.0, 56.0));
        assert_eq!(points[3], pos2(10.0, 56.0));
    }

    #[test]
    fn content_panel_inner_width_accounts_for_frame_margin() {
        assert_eq!(content_panel_inner_width(772.0), 736.0);
        assert_eq!(content_panel_inner_width(12.0), 1.0);
    }
}

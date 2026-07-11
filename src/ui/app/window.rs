use std::time::Duration;

use eframe::egui::{
    pos2, vec2, Align2, Button, CentralPanel, Color32, Context, FontId, Frame, Rect, RichText,
    Sense, Stroke, Ui, ViewportCommand,
};

use super::ClevoLedApp;
use crate::ui::layout;

const BODY_HORIZONTAL_MARGIN: f32 = 12.0;

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.sync_external_settings();
        self.sync_hardware_snapshot();
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(20, 20, 18)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    custom_title_bar(ui, ctx);
                    ui.add_space(8.0);
                    body_frame(ui, |ui| layout::control_center(ui, self));
                });
            });
        gpu_mux_confirm_dialog(ctx, self);

        self.persist_settings_if_due(false);
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_settings_if_due(true);
    }
}

fn body_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .inner_margin(body_margin())
        .show(ui, add_contents);
}

fn body_margin() -> egui::Margin {
    egui::Margin::symmetric(BODY_HORIZONTAL_MARGIN, 0.0)
}

fn gpu_mux_confirm_dialog(ctx: &Context, app: &mut ClevoLedApp) {
    let Some(mode) = app.pending_gpu_mux_mode else {
        return;
    };

    eframe::egui::Window::new("确认重启")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(30, 29, 26))
                .stroke(Stroke::new(1.0, Color32::from_rgb(221, 164, 91)))
                .rounding(12.0)
                .inner_margin(egui::Margin::same(18.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(
                RichText::new(format!("切换到{}", mode.label()))
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(244, 235, 219)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("该设置会写入固件，必须重启后生效。确认后会立即写入并重启。")
                    .size(13.0)
                    .color(Color32::from_rgb(194, 185, 171)),
            );
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(vec2(112.0, 34.0), Button::new("取消"))
                    .clicked()
                {
                    app.cancel_gpu_mux_switch();
                }
                ui.add_space(10.0);
                if ui
                    .add_sized(vec2(150.0, 34.0), Button::new("写入并重启"))
                    .clicked()
                {
                    app.confirm_gpu_mux_switch_and_reboot();
                }
            });
        });
}

fn custom_title_bar(ui: &mut Ui, ctx: &Context) {
    const TITLE_BAR_HEIGHT: f32 = 38.0;
    const CLOSE_SIZE: f32 = 26.0;

    let width = ui.available_width().max(1.0);
    let (rect, drag_response) =
        ui.allocate_exact_size(vec2(width, TITLE_BAR_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 16));
    painter.line_segment(
        [
            pos2(rect.left(), rect.bottom()),
            pos2(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0, Color32::from_rgb(43, 40, 35)),
    );
    painter.text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Clevo Control Center",
        FontId::proportional(14.0),
        Color32::from_rgb(226, 219, 207),
    );

    let close_rect = Rect::from_min_size(
        pos2(rect.right() - CLOSE_SIZE - 10.0, rect.top() + 6.0),
        vec2(CLOSE_SIZE, CLOSE_SIZE),
    );
    let close_response = ui.put(
        close_rect,
        Button::new(
            RichText::new("x")
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(220, 214, 204)),
        )
        .fill(Color32::from_rgb(40, 37, 32))
        .stroke(Stroke::new(1.0, Color32::from_rgb(62, 56, 47))),
    );

    if close_response.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    } else if drag_response.drag_started() && !close_response.hovered() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_margin_only_adds_horizontal_padding() {
        let margin = body_margin();
        assert_eq!(margin.left, 12.0);
        assert_eq!(margin.right, 12.0);
        assert_eq!(margin.top, 0.0);
        assert_eq!(margin.bottom, 0.0);
    }
}

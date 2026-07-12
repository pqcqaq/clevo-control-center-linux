use eframe::egui::{
    self, color_picker, pos2, vec2, Align, Align2, Button, Color32, Context, FontId, Frame, Layout,
    RichText, Sense, Stroke, Ui,
};

use super::app::ClevoLedApp;
use super::theme;
use crate::model::{Mode, Rgb};

const COLOR_PRESETS: [Rgb; 10] = [
    Rgb {
        r: 255,
        g: 70,
        b: 70,
    },
    Rgb {
        r: 255,
        g: 151,
        b: 55,
    },
    Rgb {
        r: 255,
        g: 220,
        b: 82,
    },
    Rgb {
        r: 94,
        g: 224,
        b: 126,
    },
    Rgb {
        r: 55,
        g: 214,
        b: 202,
    },
    Rgb {
        r: 70,
        g: 154,
        b: 255,
    },
    Rgb {
        r: 126,
        g: 105,
        b: 255,
    },
    Rgb {
        r: 221,
        g: 95,
        b: 198,
    },
    Rgb {
        r: 255,
        g: 255,
        b: 255,
    },
    Rgb {
        r: 155,
        g: 164,
        b: 176,
    },
];

pub(super) fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(62.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 23.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(15, 14, 13));
    painter.circle_stroke(
        center,
        radius + 4.0,
        Stroke::new(1.0, Color32::from_rgb(70, 64, 54)),
    );
    painter.circle_filled(center, radius, rgb_color32(app.f0_color));

    let enabled = matches!(app.mode, Mode::Custom | Mode::Breathing);
    if response.hovered() && enabled {
        painter.circle_stroke(
            center,
            radius + 6.0,
            Stroke::new(1.5, theme::palette(app.theme_color).accent),
        );
    }
    if response.clicked() && enabled {
        app.open_color_picker();
    }
}

pub(super) fn color_picker_dialog(ctx: &Context, app: &mut ClevoLedApp) {
    if !app.color_picker_open {
        return;
    }

    let language = app.language;
    let palette = theme::palette(app.theme_color);
    let mut draft = app.color_picker_draft;
    let mut apply = false;
    let mut cancel = ctx.input(|input| input.key_pressed(egui::Key::Escape));

    egui::Window::new("product_color_picker")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 4.0))
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(29, 28, 25))
                .stroke(Stroke::new(1.0, palette.border))
                .rounding(8.0)
                .inner_margin(egui::Margin::same(20.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(500.0);
            ui.horizontal(|ui| {
                let (mark_rect, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
                let painter = ui.painter_at(mark_rect);
                painter.circle_filled(mark_rect.center(), 18.0, rgb_color32(draft));
                painter.circle_stroke(mark_rect.center(), 19.0, Stroke::new(1.5, palette.border));
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(language.pick("键盘灯光", "KEYBOARD LIGHTING"))
                            .size(11.0)
                            .color(palette.accent),
                    );
                    ui.label(
                        RichText::new(language.pick("选择颜色", "Choose color"))
                            .size(22.0)
                            .strong()
                            .color(Color32::from_rgb(244, 236, 221)),
                    );
                });
            });

            ui.add_space(16.0);
            Frame::none()
                .fill(Color32::from_rgb(23, 22, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(55, 51, 44)))
                .rounding(6.0)
                .inner_margin(egui::Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut color = rgb_color32(draft);
                        ui.spacing_mut().slider_width = 250.0;
                        if color_picker::color_picker_color32(
                            ui,
                            &mut color,
                            color_picker::Alpha::Opaque,
                        ) {
                            draft = color32_rgb(color);
                        }

                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| {
                            ui.add_space(14.0);
                            let (preview_rect, _) =
                                ui.allocate_exact_size(vec2(104.0, 104.0), Sense::hover());
                            let painter = ui.painter_at(preview_rect);
                            painter.rect_filled(preview_rect, 6.0, rgb_color32(draft));
                            painter.rect_stroke(
                                preview_rect,
                                6.0,
                                Stroke::new(1.0, palette.border),
                            );
                            painter.text(
                                pos2(preview_rect.center().x, preview_rect.bottom() + 20.0),
                                Align2::CENTER_CENTER,
                                format!("#{:02X}{:02X}{:02X}", draft.r, draft.g, draft.b),
                                FontId::monospace(13.0),
                                Color32::from_rgb(204, 197, 185),
                            );
                        });
                    });
                });

            ui.add_space(14.0);
            ui.label(
                RichText::new(language.pick("快速颜色", "Quick colors"))
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(194, 186, 173)),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (index, preset) in COLOR_PRESETS.into_iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 28.0), Sense::click());
                    let selected = draft == preset;
                    ui.painter()
                        .rect_filled(rect.shrink(3.0), 3.0, rgb_color32(preset));
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            if selected {
                                palette.bright
                            } else {
                                Color32::from_rgb(66, 61, 53)
                            },
                        ),
                    );
                    if response.clicked() {
                        draft = preset;
                    }
                    if index + 1 < COLOR_PRESETS.len() {
                        ui.add_space(8.0);
                    }
                }
            });

            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        vec2(120.0, 36.0),
                        Button::new(
                            RichText::new(language.pick("应用颜色", "Apply color"))
                                .strong()
                                .color(palette.text),
                        )
                        .fill(palette.selected_surface)
                        .stroke(Stroke::new(1.0, palette.border)),
                    )
                    .clicked()
                {
                    apply = true;
                }
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        vec2(92.0, 36.0),
                        Button::new(language.pick("取消", "Cancel")),
                    )
                    .clicked()
                {
                    cancel = true;
                }
            });
        });

    app.color_picker_draft = draft;
    if apply {
        app.apply_color_picker();
    } else if cancel {
        app.cancel_color_picker();
    }
}

fn rgb_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn color32_rgb(color: Color32) -> Rgb {
    Rgb {
        r: color.r(),
        g: color.g(),
        b: color.b(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_conversion_keeps_rgb_channels() {
        let rgb = Rgb {
            r: 12,
            g: 34,
            b: 56,
        };
        assert_eq!(color32_rgb(rgb_color32(rgb)), rgb);
    }
}

use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Frame, Rect, Sense, Shape, Stroke, Ui};

use super::super::app::ClevoLedApp;
use super::super::widgets::page_header;
use crate::dchu::{available_gpu_mux_modes, selected_gpu_mux_mode_from_snapshot, GpuMuxMode};

pub(super) fn gpu_page(ui: &mut Ui, app: &mut ClevoLedApp) {
    page_header(
        ui,
        app.language.pick("显卡", "Graphics"),
        app.language.pick(
            "独显直连与混合模式切换",
            "Switch between discrete and hybrid graphics",
        ),
    );

    let selected_mode = selected_gpu_mux_mode_from_snapshot(app.hardware.as_ref());
    let available_modes = available_gpu_mux_modes(app.hardware.as_ref());
    let current_label = selected_mode
        .map(|mode| mode.localized_label(app.language))
        .unwrap_or_else(|| app.language.pick("未知", "Unknown"));

    Frame::none()
        .fill(Color32::from_rgb(22, 24, 26))
        .rounding(8.0)
        .stroke(Stroke::new(1.0, Color32::from_rgb(58, 66, 70)))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            gpu_mux_status_strip(ui, current_label, app.language);
            ui.add_space(16.0);

            let available_width = ui.available_width();
            let gap = 16.0;
            if available_width >= 620.0 {
                let button_width = ((available_width - gap) * 0.5).floor();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                    gpu_mux_button_slot(
                        ui,
                        app,
                        GpuMuxMode::DGpu,
                        selected_mode,
                        &available_modes,
                        button_width,
                    );
                    gpu_mux_button_slot(
                        ui,
                        app,
                        GpuMuxMode::MSHybrid,
                        selected_mode,
                        &available_modes,
                        button_width,
                    );
                });
            } else {
                gpu_mux_button_slot(
                    ui,
                    app,
                    GpuMuxMode::DGpu,
                    selected_mode,
                    &available_modes,
                    available_width,
                );
                ui.add_space(12.0);
                gpu_mux_button_slot(
                    ui,
                    app,
                    GpuMuxMode::MSHybrid,
                    selected_mode,
                    &available_modes,
                    available_width,
                );
            }
        });
}

fn gpu_mux_status_strip(
    ui: &mut Ui,
    current_label: &str,
    language: crate::preferences::UiLanguage,
) {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 56.0), Sense::hover());
    let painter = ui.painter_at(rect.expand(4.0));
    let bg = Color32::from_rgb(18, 20, 22);
    let line = Color32::from_rgb(67, 79, 82);
    let accent = Color32::from_rgb(68, 210, 206);
    let amber = Color32::from_rgb(232, 169, 88);

    painter.rect_filled(rect, 6.0, bg);
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, line));

    for index in 0..7 {
        let x = rect.left() + 18.0 + index as f32 * 18.0;
        painter.line_segment(
            [pos2(x, rect.top() + 10.0), pos2(x + 7.0, rect.top() + 10.0)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(68, 210, 206, 52)),
        );
    }
    painter.line_segment(
        [
            pos2(rect.left() + 18.0, rect.bottom() - 10.0),
            pos2(rect.right() - 18.0, rect.bottom() - 10.0),
        ],
        Stroke::new(1.0, Color32::from_rgb(43, 51, 54)),
    );
    painter.line_segment(
        [
            pos2(rect.left() + 18.0, rect.bottom() - 10.0),
            pos2(rect.left() + 138.0, rect.bottom() - 10.0),
        ],
        Stroke::new(2.0, accent),
    );

    painter.text(
        pos2(rect.left() + 18.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        language.pick("当前模式", "Current mode"),
        FontId::proportional(13.0),
        Color32::from_rgb(152, 164, 164),
    );
    painter.text(
        pos2(rect.left() + 92.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        current_label,
        FontId::proportional(22.0),
        Color32::from_rgb(238, 245, 238),
    );

    let chip = Rect::from_min_size(
        pos2(rect.right() - 126.0, rect.top() + 14.0),
        vec2(104.0, 28.0),
    );
    painter.rect_filled(chip, 4.0, Color32::from_rgb(30, 35, 35));
    painter.rect_stroke(chip, 4.0, Stroke::new(1.0, amber));
    painter.text(
        chip.center(),
        Align2::CENTER_CENTER,
        "MUX LINK",
        FontId::proportional(12.0),
        Color32::from_rgb(240, 220, 186),
    );
}

fn gpu_mux_button_slot(
    ui: &mut Ui,
    app: &mut ClevoLedApp,
    mode: GpuMuxMode,
    selected_mode: Option<GpuMuxMode>,
    available_modes: &[GpuMuxMode],
    width: f32,
) {
    let enabled = available_modes.contains(&mode);
    let selected = selected_mode == Some(mode);
    if gpu_mux_mode_button(ui, mode, selected, enabled, width, app.language) {
        app.request_gpu_mux_switch(mode);
    }
}

fn gpu_mux_mode_button(
    ui: &mut Ui,
    mode: GpuMuxMode,
    selected: bool,
    enabled: bool,
    width: f32,
    language: crate::preferences::UiLanguage,
) -> bool {
    let height = 236.0;
    let id = ui.make_persistent_id(("gpu_mux_mode", mode.value()));
    let (rect, _) = ui.allocate_exact_size(vec2(width.max(260.0), height), Sense::hover());
    let response = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover_t = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered() && enabled,
        0.14,
    );
    let selected_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("selected"), selected, 0.18);
    let active_t = hover_t.max(selected_t);
    let accent = match mode {
        GpuMuxMode::DGpu => Color32::from_rgb(236, 154, 76),
        GpuMuxMode::MSHybrid => Color32::from_rgb(68, 210, 206),
    };
    let rect = rect.translate(vec2(0.0, -hover_t * 2.0));
    let fill = mix_color(
        Color32::from_rgb(18, 21, 23),
        Color32::from_rgb(34, 42, 43),
        hover_t * 0.5 + selected_t * 0.55,
    );
    let stroke = mix_color(Color32::from_rgb(58, 70, 73), accent, active_t);
    let text = if enabled {
        mix_color(
            Color32::from_rgb(212, 222, 219),
            Color32::from_rgb(250, 245, 230),
            active_t,
        )
    } else {
        Color32::from_rgb(106, 114, 112)
    };

    let painter = ui.painter_at(rect.expand(8.0));
    painter.rect_filled(rect, 8.0, fill);
    painter.rect_stroke(rect, 8.0, Stroke::new(1.0 + active_t * 1.5, stroke));
    gpu_mux_panel_texture(&painter, rect, accent, active_t);
    gpu_mux_corner_brackets(&painter, rect, accent, active_t);
    if selected_t > 0.0 {
        let inset = rect.shrink(7.0);
        painter.rect_stroke(
            inset,
            4.0,
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 150),
            ),
        );
        painter.line_segment(
            [
                pos2(rect.left() + 18.0, rect.top() + 14.0),
                pos2(rect.right() - 18.0, rect.top() + 14.0),
            ],
            Stroke::new(2.0, accent),
        );
    }

    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 22.0, rect.top() + 32.0),
        vec2(rect.width() - 44.0, 118.0),
    );
    match mode {
        GpuMuxMode::DGpu => draw_discrete_gpu_icon(&painter, icon_rect, active_t, enabled),
        GpuMuxMode::MSHybrid => draw_intel_gpu_icon(&painter, icon_rect, active_t, enabled),
    }

    painter.text(
        pos2(rect.left() + 22.0, rect.bottom() - 62.0),
        Align2::LEFT_CENTER,
        mode.localized_label(language),
        FontId::proportional(22.0),
        text,
    );
    painter.text(
        pos2(rect.left() + 22.0, rect.bottom() - 32.0),
        Align2::LEFT_CENTER,
        gpu_mux_mode_description(mode, language),
        FontId::proportional(12.0),
        if enabled {
            Color32::from_rgb(151, 144, 130)
        } else {
            Color32::from_rgb(94, 89, 82)
        },
    );

    let badge = Rect::from_min_size(
        pos2(rect.right() - 88.0, rect.bottom() - 48.0),
        vec2(64.0, 24.0),
    );
    painter.rect_filled(
        badge,
        3.0,
        Color32::from_rgba_unmultiplied(
            accent.r(),
            accent.g(),
            accent.b(),
            (26.0 + active_t * 40.0) as u8,
        ),
    );
    painter.rect_stroke(
        badge,
        3.0,
        Stroke::new(
            1.0,
            mix_color(Color32::from_rgb(69, 79, 78), accent, active_t),
        ),
    );
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        if selected { "ACTIVE" } else { "TARGET" },
        FontId::proportional(10.0),
        mix_color(
            Color32::from_rgb(154, 166, 164),
            Color32::from_rgb(248, 244, 229),
            active_t,
        ),
    );

    response.clicked() && enabled
}

fn gpu_mux_mode_description(
    mode: GpuMuxMode,
    language: crate::preferences::UiLanguage,
) -> &'static str {
    match mode {
        GpuMuxMode::DGpu => language.pick(
            "dGPU 直接连接显示输出",
            "dGPU connects directly to the display output",
        ),
        GpuMuxMode::MSHybrid => language.pick(
            "Intel 核显输出，独显按需介入",
            "Intel graphics drives the display; dGPU engages on demand",
        ),
    }
}

fn gpu_mux_panel_texture(painter: &egui::Painter, rect: Rect, accent: Color32, active_t: f32) {
    for index in 0..7 {
        let y = rect.top() + 24.0 + index as f32 * 22.0;
        painter.line_segment(
            [pos2(rect.left() + 16.0, y), pos2(rect.right() - 16.0, y)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 10)),
        );
    }
    for index in 0..6 {
        let x = rect.left() + 24.0 + index as f32 * 34.0;
        painter.line_segment(
            [
                pos2(x, rect.top() + 18.0),
                pos2(x + 18.0, rect.top() + 18.0),
            ],
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(
                    accent.r(),
                    accent.g(),
                    accent.b(),
                    (28.0 + active_t * 74.0) as u8,
                ),
            ),
        );
    }
}

fn gpu_mux_corner_brackets(painter: &egui::Painter, rect: Rect, accent: Color32, active_t: f32) {
    let color = Color32::from_rgba_unmultiplied(
        accent.r(),
        accent.g(),
        accent.b(),
        (96.0 + active_t * 120.0) as u8,
    );
    let stroke = Stroke::new(1.5 + active_t, color);
    let len = 24.0;
    for (x, y, sx, sy) in [
        (rect.left() + 12.0, rect.top() + 12.0, 1.0, 1.0),
        (rect.right() - 12.0, rect.top() + 12.0, -1.0, 1.0),
        (rect.left() + 12.0, rect.bottom() - 12.0, 1.0, -1.0),
        (rect.right() - 12.0, rect.bottom() - 12.0, -1.0, -1.0),
    ] {
        painter.line_segment([pos2(x, y), pos2(x + len * sx, y)], stroke);
        painter.line_segment([pos2(x, y), pos2(x, y + len * sy)], stroke);
    }
}

fn draw_discrete_gpu_icon(painter: &egui::Painter, rect: Rect, active_t: f32, enabled: bool) {
    let edge = if enabled {
        Color32::from_rgb(98, 102, 103)
    } else {
        Color32::from_rgb(66, 70, 71)
    };
    let edge_hi = if enabled {
        Color32::from_rgb(151, 154, 154)
    } else {
        Color32::from_rgb(94, 98, 98)
    };
    let body = Rect::from_center_size(
        rect.center() - vec2(0.0, 4.0),
        vec2((rect.width() - 8.0).min(244.0), 96.0),
    );

    painter.rect_filled(body, 11.0, Color32::from_rgb(24, 26, 27));
    painter.rect_stroke(body, 11.0, Stroke::new(1.0 + active_t * 0.8, edge_hi));
    painter.rect_stroke(body.shrink(4.0), 8.0, Stroke::new(1.0, edge));

    let seam = if enabled {
        Color32::from_rgb(72, 76, 77)
    } else {
        Color32::from_rgb(54, 58, 59)
    };
    let center = body.center();
    let inset = 6.0;
    let waist = 8.0;
    let panel_left = body.left() + body.width() * 0.31;
    let panel_right = body.right() - body.width() * 0.31;
    painter.add(Shape::convex_polygon(
        vec![
            pos2(panel_left, body.top() + inset),
            pos2(panel_right, body.top() + inset),
            pos2(center.x + waist, center.y - 5.0),
            pos2(center.x - waist, center.y - 5.0),
        ],
        Color32::from_rgb(47, 49, 50),
        Stroke::new(1.0, seam),
    ));
    painter.add(Shape::convex_polygon(
        vec![
            pos2(center.x - waist, center.y + 5.0),
            pos2(center.x + waist, center.y + 5.0),
            pos2(panel_right, body.bottom() - inset),
            pos2(panel_left, body.bottom() - inset),
        ],
        Color32::from_rgb(42, 44, 45),
        Stroke::new(1.0, seam),
    ));
    painter.add(Shape::convex_polygon(
        vec![
            pos2(body.left() + inset, body.top() + inset),
            pos2(panel_left, body.top() + inset),
            pos2(center.x - waist, center.y - 5.0),
            pos2(center.x - waist, center.y + 5.0),
            pos2(panel_left, body.bottom() - inset),
            pos2(body.left() + inset, body.bottom() - inset),
        ],
        Color32::from_rgb(31, 33, 34),
        Stroke::new(1.0, seam),
    ));
    painter.add(Shape::convex_polygon(
        vec![
            pos2(panel_right, body.top() + inset),
            pos2(body.right() - inset, body.top() + inset),
            pos2(body.right() - inset, body.bottom() - inset),
            pos2(panel_right, body.bottom() - inset),
            pos2(center.x + waist, center.y + 5.0),
            pos2(center.x + waist, center.y - 5.0),
        ],
        Color32::from_rgb(31, 33, 34),
        Stroke::new(1.0, seam),
    ));

    let fan_radius = 39.0;
    let fan_centers = [
        pos2(body.left() + 46.0, center.y),
        pos2(body.right() - 46.0, center.y),
    ];
    for fan_center in fan_centers {
        painter.circle_filled(fan_center, fan_radius + 3.0, Color32::from_rgb(12, 14, 15));
        painter.circle_stroke(fan_center, fan_radius + 3.0, Stroke::new(1.0, edge));
        painter.circle_filled(fan_center, fan_radius, Color32::from_rgb(5, 7, 8));

        for step in 0..7 {
            let angle = step as f32 * std::f32::consts::TAU / 7.0 + 0.18 + active_t * 0.20;
            let sweep = angle + 0.58;
            let inner = fan_radius * 0.27;
            let outer = fan_radius * 0.92;
            painter.add(Shape::convex_polygon(
                vec![
                    pos2(
                        fan_center.x + angle.cos() * inner,
                        fan_center.y + angle.sin() * inner,
                    ),
                    pos2(
                        fan_center.x + (angle + 0.16).cos() * outer,
                        fan_center.y + (angle + 0.16).sin() * outer,
                    ),
                    pos2(
                        fan_center.x + sweep.cos() * outer,
                        fan_center.y + sweep.sin() * outer,
                    ),
                    pos2(
                        fan_center.x + (sweep - 0.20).cos() * inner,
                        fan_center.y + (sweep - 0.20).sin() * inner,
                    ),
                ],
                Color32::from_rgb(48, 51, 52),
                Stroke::new(0.5, Color32::from_rgb(70, 73, 74)),
            ));
        }
        painter.circle_stroke(fan_center, fan_radius, Stroke::new(1.0, edge_hi));
        painter.circle_filled(fan_center, 12.5, Color32::from_rgb(28, 30, 31));
        painter.circle_stroke(fan_center, 12.5, Stroke::new(1.0, edge_hi));
        painter.circle_filled(
            fan_center - vec2(3.0, 3.0),
            4.0,
            Color32::from_rgba_unmultiplied(112, 116, 116, 45),
        );
    }

    let bracket = Rect::from_min_size(
        pos2(body.left() - 4.0, body.top() + 3.0),
        vec2(4.0, body.height() + 9.0),
    );
    painter.rect_filled(bracket, 1.0, Color32::from_rgb(105, 108, 107));
    painter.line_segment(
        [
            pos2(bracket.left(), bracket.top() + 18.0),
            pos2(bracket.left() - 8.0, bracket.top() + 18.0),
        ],
        Stroke::new(1.5, edge_hi),
    );

    let connector = Rect::from_min_size(
        pos2(body.left() + 43.0, body.bottom() + 1.0),
        vec2(72.0, 6.0),
    );
    painter.rect_filled(connector, 1.0, Color32::from_rgb(148, 129, 72));
    for index in 1..12 {
        let x = connector.left() + index as f32 * 6.0;
        painter.line_segment(
            [pos2(x, connector.top()), pos2(x, connector.bottom())],
            Stroke::new(0.5, Color32::from_rgb(91, 78, 48)),
        );
    }
}

fn draw_intel_gpu_icon(painter: &egui::Painter, rect: Rect, active_t: f32, enabled: bool) {
    let accent = if enabled {
        Color32::from_rgb(68, 210, 206)
    } else {
        Color32::from_rgb(86, 94, 94)
    };
    let muted = if enabled {
        Color32::from_rgb(93, 128, 130)
    } else {
        Color32::from_rgb(70, 78, 78)
    };
    let dgpu = if enabled {
        Color32::from_rgb(218, 145, 72)
    } else {
        Color32::from_rgb(102, 89, 75)
    };
    let canvas = Rect::from_center_size(
        rect.center() - vec2(0.0, 4.0),
        vec2((rect.width() - 8.0).min(244.0), 96.0),
    );

    let igpu_chip = Rect::from_min_size(
        pos2(canvas.left() + 8.0, canvas.top() + 7.0),
        vec2(68.0, 42.0),
    );
    let dgpu_chip = Rect::from_min_size(
        pos2(canvas.left() + 18.0, canvas.bottom() - 34.0),
        vec2(58.0, 27.0),
    );
    let mux_center = pos2(canvas.left() + 123.0, canvas.center().y);
    let display = Rect::from_center_size(
        pos2(canvas.right() - 35.0, canvas.center().y),
        vec2(54.0, 39.0),
    );

    painter.line_segment(
        [
            pos2(igpu_chip.right(), igpu_chip.center().y),
            pos2(mux_center.x - 10.0, mux_center.y - 4.0),
        ],
        Stroke::new(2.0, accent),
    );
    painter.line_segment(
        [
            pos2(dgpu_chip.right(), dgpu_chip.center().y),
            pos2(mux_center.x - 10.0, mux_center.y + 4.0),
        ],
        Stroke::new(1.5, dgpu),
    );
    painter.line_segment(
        [
            pos2(mux_center.x + 10.0, mux_center.y),
            pos2(display.left(), display.center().y),
        ],
        Stroke::new(2.0, accent),
    );

    for index in 0..7 {
        let x = igpu_chip.left() + 8.0 + index as f32 * 8.5;
        painter.line_segment(
            [pos2(x, igpu_chip.top() - 5.0), pos2(x, igpu_chip.top())],
            Stroke::new(1.0, accent),
        );
    }
    for index in 0..4 {
        let y = igpu_chip.top() + 7.0 + index as f32 * 9.0;
        painter.line_segment(
            [pos2(igpu_chip.left() - 5.0, y), pos2(igpu_chip.left(), y)],
            Stroke::new(1.0, accent),
        );
    }
    let igpu_fill = if enabled {
        mix_color(
            Color32::from_rgb(27, 59, 67),
            Color32::from_rgb(34, 77, 86),
            active_t,
        )
    } else {
        Color32::from_rgb(43, 48, 48)
    };
    painter.rect_filled(igpu_chip, 6.0, igpu_fill);
    painter.rect_stroke(igpu_chip, 6.0, Stroke::new(1.0 + active_t, accent));
    painter.text(
        igpu_chip.center() - vec2(0.0, 6.0),
        Align2::CENTER_CENTER,
        "Intel",
        FontId::proportional(14.0),
        Color32::from_rgb(230, 236, 232),
    );
    painter.text(
        igpu_chip.center() + vec2(0.0, 8.0),
        Align2::CENTER_CENTER,
        "iGPU",
        FontId::proportional(8.5),
        Color32::from_rgb(143, 190, 192),
    );

    painter.rect_filled(dgpu_chip, 4.0, Color32::from_rgb(34, 32, 29));
    painter.rect_stroke(dgpu_chip, 4.0, Stroke::new(1.0, dgpu));
    painter.text(
        dgpu_chip.center(),
        Align2::CENTER_CENTER,
        "dGPU",
        FontId::proportional(9.5),
        Color32::from_rgb(207, 177, 142),
    );

    painter.circle_filled(
        mux_center,
        15.0 + active_t * 2.0,
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 16),
    );
    painter.add(Shape::convex_polygon(
        vec![
            pos2(mux_center.x, mux_center.y - 10.0),
            pos2(mux_center.x + 10.0, mux_center.y),
            pos2(mux_center.x, mux_center.y + 10.0),
            pos2(mux_center.x - 10.0, mux_center.y),
        ],
        Color32::from_rgb(24, 39, 41),
        Stroke::new(1.0 + active_t * 0.5, accent),
    ));
    painter.text(
        mux_center,
        Align2::CENTER_CENTER,
        "MUX",
        FontId::proportional(7.5),
        Color32::from_rgb(201, 226, 221),
    );

    painter.rect_filled(display, 5.0, Color32::from_rgb(18, 28, 30));
    painter.rect_stroke(display, 5.0, Stroke::new(1.0 + active_t * 0.5, accent));
    painter.rect_filled(
        display.shrink2(vec2(7.0, 6.0)),
        2.0,
        Color32::from_rgb(25, 44, 47),
    );
    painter.line_segment(
        [
            pos2(display.center().x, display.bottom()),
            pos2(display.center().x, display.bottom() + 7.0),
        ],
        Stroke::new(1.5, accent),
    );
    painter.line_segment(
        [
            pos2(display.center().x - 10.0, display.bottom() + 7.0),
            pos2(display.center().x + 10.0, display.bottom() + 7.0),
        ],
        Stroke::new(1.5, accent),
    );

    for (offset, color) in [(0.35, accent), (0.68, muted)] {
        let x = mux_center.x + 10.0 + (display.left() - mux_center.x - 10.0) * offset;
        painter.circle_filled(pos2(x, mux_center.y), 2.0 + active_t, color);
    }
}

fn mix_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
        mix(from.a(), to.a()),
    )
}

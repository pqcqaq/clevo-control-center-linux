use std::time::Duration;

use eframe::egui::{
    pos2, vec2, Align, Align2, Button, CentralPanel, Color32, Context, FontId, Frame, Layout, Rect,
    RichText, Sense, Shape, Stroke, Ui, ViewportCommand,
};

use super::ClevoLedApp;
use crate::module_loader::ModuleState;
use crate::ui::layout;

const BODY_HORIZONTAL_MARGIN: f32 = 12.0;

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        crate::ui::theme::apply(ctx, self.theme_color);
        if self.first_run_pending {
            CentralPanel::default()
                .frame(Frame::none().fill(Color32::from_rgb(20, 20, 18)))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        custom_title_bar(ui, ctx);
                        first_run_backdrop(ui);
                    });
                });
            first_run_disclaimer_dialog(ctx, self);
            ctx.request_repaint_after(Duration::from_millis(80));
            return;
        }

        self.sync_external_settings();
        self.sync_hardware_snapshot();
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(20, 20, 18)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    custom_title_bar(ui, ctx);
                    ui.add_space(8.0);
                    let modal_open = self.module_prompt.is_some()
                        || self.color_picker_open
                        || self.pending_gpu_mux_mode.is_some();
                    body_frame(ui, |ui| {
                        ui.add_enabled_ui(!modal_open, |ui| layout::control_center(ui, self));
                    });
                });
            });
        if self.module_prompt.is_some() {
            module_dialog(ctx, self);
        } else if self.color_picker_open {
            crate::ui::color_picker::color_picker_dialog(ctx, self);
        } else {
            gpu_mux_confirm_dialog(ctx, self);
        }

        self.persist_settings_if_due(false);
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.first_run_pending {
            self.persist_settings_if_due(true);
        }
    }
}

fn first_run_backdrop(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.text(
        pos2(rect.left() + 34.0, rect.bottom() - 38.0),
        Align2::LEFT_BOTTOM,
        "CLEVO CONTROL CENTER  /  LINUX EDITION",
        FontId::proportional(12.0),
        Color32::from_rgb(64, 61, 54),
    );
    painter.line_segment(
        [
            pos2(rect.left() + 34.0, rect.bottom() - 28.0),
            pos2(rect.right() - 34.0, rect.bottom() - 28.0),
        ],
        Stroke::new(1.0, Color32::from_rgb(38, 37, 33)),
    );
}

fn first_run_disclaimer_dialog(ctx: &Context, app: &mut ClevoLedApp) {
    let language = app.language;
    eframe::egui::Window::new("first_run_disclaimer")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 4.0))
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(29, 28, 25))
                .stroke(Stroke::new(1.0, Color32::from_rgb(157, 106, 48)))
                .rounding(8.0)
                .inner_margin(egui::Margin::same(22.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(540.0);

            ui.horizontal(|ui| {
                warning_mark(ui);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(language.pick(
                            "首次启动 · 使用前确认",
                            "FIRST LAUNCH · CONFIRM BEFORE USE",
                        ))
                            .size(11.0)
                            .color(Color32::from_rgb(205, 151, 83)),
                    );
                    ui.label(
                        RichText::new(language.pick(
                            "硬件控制免责声明",
                            "Hardware Control Disclaimer",
                        ))
                            .size(22.0)
                            .strong()
                            .color(Color32::from_rgb(244, 236, 221)),
                    );
                });
            });

            ui.add_space(16.0);
            disclaimer_section(
                ui,
                language.pick("非官方项目", "Unofficial project"),
                language.pick(
                    "本软件由社区独立开发，与 Clevo、蓝天电脑及其品牌商不存在隶属、授权或担保关系。",
                    "This community-developed software is not affiliated with, authorized by, or warranted by Clevo or any Clevo system vendor.",
                ),
            );
            disclaimer_section(
                ui,
                language.pick("固件级操作", "Firmware-level access"),
                language.pick(
                    "程序会通过内核模块访问 DCHU、EC 与固件接口，并可能改变灯光、风扇、功耗和显卡切换等硬件状态。错误操作可能导致系统不稳定、无法启动、数据损坏或不可逆的硬件影响。",
                    "The kernel module accesses DCHU, EC, and firmware interfaces to change lighting, fans, power, and graphics state. Incorrect operation can cause instability, boot failure, data loss, or irreversible hardware impact.",
                ),
            );
            disclaimer_section(
                ui,
                language.pick("兼容性边界", "Compatibility boundary"),
                language.pick(
                    "仅在确认设备属于兼容的蓝天/Clevo 系机型，并了解 BIOS/EC 恢复方法后使用。非蓝天系机器、未经验证的 BIOS/EC 或虚拟机环境请勿继续。",
                    "Continue only on a compatible Clevo-family system and only if you understand BIOS/EC recovery. Do not continue on non-Clevo hardware, an unverified BIOS/EC, or a virtual machine.",
                ),
            );

            ui.add_space(4.0);
            ui.label(
                RichText::new(language.pick(
                    "继续使用即表示你理解上述风险，并自行承担由硬件控制操作产生的后果。",
                    "By continuing, you acknowledge these risks and accept responsibility for hardware-control operations.",
                ))
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(221, 183, 130)),
            );

            if let Some(error) = &app.first_run_error {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(error)
                        .size(12.0)
                        .color(Color32::from_rgb(221, 116, 94)),
                );
            }

            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        vec2(200.0, 38.0),
                        Button::new(
                            RichText::new(language.pick(
                                "我已了解，继续使用",
                                "I understand and continue",
                            ))
                                .size(13.0)
                                .strong()
                                .color(Color32::from_rgb(255, 240, 214)),
                        )
                        .fill(Color32::from_rgb(111, 73, 31))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(225, 164, 88))),
                    )
                    .clicked()
                {
                    app.accept_first_run_disclaimer();
                }
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        vec2(104.0, 38.0),
                        Button::new(language.pick("退出程序", "Exit")),
                    )
                    .clicked()
                {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            });
        });
}

fn warning_mark(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    let points = [
        pos2(rect.center().x, rect.top() + 2.0),
        pos2(rect.right() - 2.0, rect.bottom() - 3.0),
        pos2(rect.left() + 2.0, rect.bottom() - 3.0),
    ];
    painter.add(Shape::convex_polygon(
        points.to_vec(),
        Color32::from_rgb(73, 49, 24),
        Stroke::new(1.5, Color32::from_rgb(225, 164, 88)),
    ));
    painter.text(
        pos2(rect.center().x, rect.center().y + 6.0),
        Align2::CENTER_CENTER,
        "!",
        FontId::proportional(22.0),
        Color32::from_rgb(255, 224, 177),
    );
}

fn disclaimer_section(ui: &mut Ui, title: &str, body: &str) {
    ui.horizontal(|ui| {
        let (rail, _) = ui.allocate_exact_size(vec2(3.0, 50.0), Sense::hover());
        ui.painter()
            .rect_filled(rail, 1.5, Color32::from_rgb(137, 91, 40));
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(title)
                    .size(13.0)
                    .strong()
                    .color(Color32::from_rgb(232, 224, 210)),
            );
            ui.add_space(2.0);
            ui.add(
                eframe::egui::Label::new(
                    RichText::new(body)
                        .size(12.0)
                        .color(Color32::from_rgb(170, 165, 155)),
                )
                .wrap(),
            );
        });
    });
    ui.add_space(10.0);
}

fn body_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .inner_margin(body_margin())
        .show(ui, add_contents);
}

fn body_margin() -> egui::Margin {
    egui::Margin::symmetric(BODY_HORIZONTAL_MARGIN, 0.0)
}

fn module_dialog(ctx: &Context, app: &mut ClevoLedApp) {
    let Some(state) = app.module_prompt else {
        return;
    };

    let language = app.language;
    let palette = crate::ui::theme::palette(app.theme_color);
    let (status, detail, action) = match state {
        ModuleState::Ready => return,
        ModuleState::Missing => (
            language.pick("模块未加载", "Module not loaded").to_owned(),
            language
                .pick(
                    "控制中心需要内核模块才能读取硬件并执行灯光、风扇和性能控制。继续后会请求系统管理员认证。",
                    "The kernel module is required for hardware monitoring and lighting, fan, and performance controls. Continuing requests administrator authentication.",
                )
                .to_owned(),
            language.pick("认证并加载", "Authenticate and load"),
        ),
        ModuleState::Outdated(Some(version)) => (
            match language {
                crate::preferences::UiLanguage::SimplifiedChinese => {
                    format!("模块 API {version} 已过期")
                }
                crate::preferences::UiLanguage::English => {
                    format!("Module API {version} is outdated")
                }
            },
            match language {
                crate::preferences::UiLanguage::SimplifiedChinese => format!(
                    "当前程序需要 API {}。继续后会请求系统管理员认证并加载随程序提供的新模块。",
                    crate::module_loader::required_module_api_version()
                ),
                crate::preferences::UiLanguage::English => format!(
                    "This build requires API {}. Continuing requests administrator authentication and loads the bundled module.",
                    crate::module_loader::required_module_api_version()
                ),
            },
            language.pick("认证并更新", "Authenticate and update"),
        ),
        ModuleState::Outdated(None) => (
            language
                .pick("无法确认模块版本", "Module version unavailable")
                .to_owned(),
            match language {
                crate::preferences::UiLanguage::SimplifiedChinese => format!(
                    "版本节点缺失或不可读，当前程序需要 API {}。继续后会请求系统管理员认证并重新加载模块。",
                    crate::module_loader::required_module_api_version()
                ),
                crate::preferences::UiLanguage::English => format!(
                    "The version node is missing or unreadable; API {} is required. Continuing requests administrator authentication and reloads the module.",
                    crate::module_loader::required_module_api_version()
                ),
            },
            language.pick("认证并修复", "Authenticate and repair"),
        ),
    };

    eframe::egui::Window::new("module_requirement")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 4.0))
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(29, 28, 25))
                .stroke(Stroke::new(1.0, palette.border))
                .rounding(8.0)
                .inner_margin(egui::Margin::same(22.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(500.0);
            ui.horizontal(|ui| {
                warning_mark(ui);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(language.pick(
                            "启动检查 · 内核接口",
                            "STARTUP CHECK · KERNEL INTERFACE",
                        ))
                        .size(11.0)
                        .color(palette.accent),
                    );
                    ui.label(
                        RichText::new(status)
                            .size(22.0)
                            .strong()
                            .color(Color32::from_rgb(244, 236, 221)),
                    );
                });
            });

            ui.add_space(16.0);
            Frame::none()
                .fill(Color32::from_rgb(23, 22, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(57, 52, 45)))
                .rounding(6.0)
                .inner_margin(egui::Margin::same(14.0))
                .show(ui, |ui| {
                    ui.add(
                        eframe::egui::Label::new(
                            RichText::new(detail)
                                .size(13.0)
                                .color(Color32::from_rgb(194, 186, 173)),
                        )
                        .wrap(),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(language.pick(
                            "认证窗口由系统安全服务提供；密码不会传递给控制中心。",
                            "Authentication is provided by the system security service; the password is never passed to Control Center.",
                        ))
                        .size(11.0)
                        .color(Color32::from_rgb(147, 141, 131)),
                    );
                });

            if let Some(error) = &app.module_error {
                ui.add_space(12.0);
                ui.add(
                    eframe::egui::Label::new(
                        RichText::new(error)
                            .size(12.0)
                            .color(Color32::from_rgb(225, 116, 94)),
                    )
                    .wrap(),
                );
            }

            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        vec2(160.0, 38.0),
                        Button::new(RichText::new(action).strong().color(palette.text))
                            .fill(palette.selected_surface)
                            .stroke(Stroke::new(1.0, palette.border)),
                    )
                    .clicked()
                {
                    app.process_module_request();
                }
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        vec2(96.0, 38.0),
                        Button::new(language.pick("退出程序", "Exit")),
                    )
                    .clicked()
                {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            });
        });
}

fn gpu_mux_confirm_dialog(ctx: &Context, app: &mut ClevoLedApp) {
    let Some(mode) = app.pending_gpu_mux_mode else {
        return;
    };

    let language = app.language;
    eframe::egui::Window::new(language.pick("确认重启", "Confirm restart"))
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
                RichText::new(match language {
                    crate::preferences::UiLanguage::SimplifiedChinese => {
                        format!("切换到{}", mode.localized_label(language))
                    }
                    crate::preferences::UiLanguage::English => {
                        format!("Switch to {}", mode.localized_label(language))
                    }
                })
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(244, 235, 219)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(language.pick(
                    "该设置会写入固件，必须重启后生效。确认后会立即写入并重启。",
                    "This setting is written to firmware and requires a restart. Confirming writes the setting and restarts now.",
                ))
                    .size(13.0)
                    .color(Color32::from_rgb(194, 185, 171)),
            );
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        vec2(112.0, 34.0),
                        Button::new(language.pick("取消", "Cancel")),
                    )
                    .clicked()
                {
                    app.cancel_gpu_mux_switch();
                }
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        vec2(150.0, 34.0),
                        Button::new(language.pick("写入并重启", "Apply and restart")),
                    )
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

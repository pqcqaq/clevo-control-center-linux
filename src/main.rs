use std::process;

use eframe::egui::{self, pos2};
use eframe::NativeOptions;
use egui::ViewportBuilder;

mod dchu;
mod effects;
mod fan_curve;
mod hardware;
mod model;
mod module_loader;
mod preferences;
mod service;
mod settings;
mod ui;

fn main() -> eframe::Result {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("dchu") {
        let hardware = hardware::native_backend();
        if let Err(err) = dchu::run_dchu_cli(&args[2..], hardware.as_ref()) {
            eprintln!("{err}");
            dchu::print_dchu_usage();
            process::exit(2);
        }
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--service") {
        service::service_loop(settings::settings_path());
    }

    let (settings_path, first_run) = settings::settings_path_and_first_run();
    let settings = settings::load_settings(&settings_path);
    let hardware_backend = hardware::native_backend();

    let mut viewport = ViewportBuilder::default()
        .with_inner_size([960.0, 600.0])
        .with_min_inner_size([860.0, 540.0])
        .with_decorations(false)
        .with_resizable(true);

    if let Some([x, y]) = settings.window_pos {
        viewport = viewport.with_position(pos2(x, y));
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Clevo Control Center",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            ui::install_cjk_font(&cc.egui_ctx);
            ui::apply_theme(&cc.egui_ctx, settings.theme_color);
            Ok(Box::new(ui::ClevoLedApp::new(
                settings_path,
                settings,
                hardware_backend,
                first_run,
            )))
        }),
    )
}

use std::process;

use eframe::egui::{self, pos2};
use eframe::NativeOptions;
use egui::ViewportBuilder;

mod dchu;
mod effects;
mod led;
mod model;
mod service;
mod settings;
mod ui;

fn main() -> eframe::Result {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("dchu") {
        if let Err(err) = dchu::run_dchu_cli(&args[2..]) {
            eprintln!("{err}");
            dchu::print_dchu_usage();
            process::exit(2);
        }
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--service") {
        service::service_loop(settings::settings_path());
    }

    let settings_path = settings::settings_path();
    let settings = settings::load_settings(&settings_path);
    service::ensure_service_running();

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
        "",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            ui::install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(ui::ClevoLedApp::new(settings_path, settings)))
        }),
    )
}

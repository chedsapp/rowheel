mod app;
mod calibration;
mod config;
mod force_feedback;
mod input;
mod virtual_controller;

use app::RoWheelApp;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 500.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("RoWheel"),
        ..Default::default()
    };

    eframe::run_native(
        "RoWheel",
        native_options,
        Box::new(|cc| Ok(Box::new(RoWheelApp::new(cc)))),
    )
}

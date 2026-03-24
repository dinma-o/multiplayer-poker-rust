mod app;
mod network;
pub(crate) mod screens;
pub(crate) mod widgets;

use app::PokerApp;

pub fn run() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "PokerDUD",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(PokerApp::default()))
        }),
    )
    .expect("failed to start eframe window");
}

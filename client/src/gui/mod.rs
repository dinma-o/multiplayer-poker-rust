mod app;
#[cfg(debug_assertions)]
mod dev_preview;
mod network;
pub(crate) mod screens;
pub(crate) mod theme;
pub(crate) mod widgets;

use app::PokerApp;

pub fn run() {
    run_app("PokerDUD", PokerApp::default());
}

#[cfg(debug_assertions)]
pub fn run_preview(name: &str) -> Result<(), String> {
    let scene = dev_preview::PreviewScene::parse(name)?;
    let title = format!("PokerDUD Preview - {}", scene.label());
    run_app(&title, dev_preview::build_preview_app(scene));
    Ok(())
}

#[cfg(not(debug_assertions))]
pub fn run_preview(_name: &str) -> Result<(), String> {
    Err("GUI preview mode is only available in debug builds.".to_string())
}

fn run_app(title: &str, app: PokerApp) {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        title,
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .expect("failed to start eframe window");
}

use super::super::app::PokerApp;

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Connect to Server");
        ui.add_space(8.0);

        if !app.status_msg.is_empty() {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(80, 30, 30))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.colored_label(egui::Color32::from_rgb(255, 180, 180), &app.status_msg);
                });
            ui.add_space(8.0);
        }

        ui.horizontal(|ui| {
            ui.label("Server address:");
            ui.text_edit_singleline(&mut app.server_address);
        });

        ui.add_space(8.0);

        if ui.button("Connect").clicked() {
            app.status_msg.clear();
            app.connect(ctx);
        }
    });
}

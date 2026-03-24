use super::super::app::PokerApp;

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Connect to Server");
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.label("Server address:");
            ui.text_edit_singleline(&mut app.server_address);
        });

        ui.add_space(8.0);

        if ui.button("Connect").clicked() {
            app.status_msg.clear();
            app.connect(ctx);
        }

        if !app.status_msg.is_empty() {
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::RED, &app.status_msg);
        }
    });
}

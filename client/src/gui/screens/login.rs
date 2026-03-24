use common::wire::RequestOp;

use super::super::app::{LoginTab, PendingKind, PokerApp};

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("PokerDUD");
        ui.add_space(16.0);

        // Tab bar
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.login_tab, LoginTab::Login, "Login");
            ui.selectable_value(
                &mut app.login_tab,
                LoginTab::CreateAccount,
                "Create Account",
            );
        });
        ui.separator();
        ui.add_space(8.0);

        let is_pending = app.is_request_pending();

        match app.login_tab.clone() {
            LoginTab::Login => {
                ui.label("Name:");
                ui.text_edit_singleline(&mut app.input_name);
                ui.add_space(4.0);
                ui.label("PIN:");
                ui.add(egui::TextEdit::singleline(&mut app.input_pin).password(true));
                ui.add_space(8.0);

                if ui
                    .add_enabled(!is_pending, egui::Button::new("Login"))
                    .clicked()
                {
                    let op = RequestOp::Login {
                        name: app.input_name.trim().to_string(),
                        pin: app.input_pin.clone(),
                    };
                    app.send_op(PendingKind::Login, op);
                }
            }
            LoginTab::CreateAccount => {
                ui.label("Name:");
                ui.text_edit_singleline(&mut app.input_name);
                ui.add_space(4.0);
                ui.label("PIN:");
                ui.add(egui::TextEdit::singleline(&mut app.input_pin).password(true));
                ui.add_space(4.0);
                ui.label("Starting balance:");
                ui.text_edit_singleline(&mut app.input_amount);
                ui.add_space(8.0);

                if ui
                    .add_enabled(!is_pending, egui::Button::new("Create Account"))
                    .clicked()
                {
                    let amount: i64 = app.input_amount.trim().parse().unwrap_or(0);
                    let op = RequestOp::CreateAccount {
                        name: app.input_name.trim().to_string(),
                        pin: app.input_pin.clone(),
                        amount,
                    };
                    app.send_op(PendingKind::CreateAccount, op);
                }
            }
        }

        if let Some(err) = &app.last_error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, err);
        }
    });
}

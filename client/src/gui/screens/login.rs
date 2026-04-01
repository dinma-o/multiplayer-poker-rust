use common::wire::RequestOp;

use super::super::{
    app::{LoginTab, PendingKind, PokerApp},
    theme,
};

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        theme::paint_screen_background(ui);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(28.0);

                if ui.available_width() > 920.0 {
                    ui.columns(2, |columns| {
                        show_login_overview(&mut columns[0]);
                        show_login_panel(app, &mut columns[1]);
                    });
                } else {
                    show_login_overview(ui);
                    ui.add_space(18.0);
                    show_login_panel(app, ui);
                }

                ui.add_space(28.0);
            });
    });
}

fn show_login_overview(ui: &mut egui::Ui) {
    theme::accent_panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("PokerDUD"));
        ui.label(theme::hero("Welcome back to the room"));
        ui.label(theme::muted(
            "Sign in to jump into the lobby, manage your bankroll, and move between tables without losing the visual context of the client.",
        ));

        ui.add_space(12.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.label(theme::section_title("Available variants"));
            ui.horizontal_wrapped(|ui| {
                theme::show_badge(ui, "Five Card Draw", theme::PANEL_SOFT, theme::BORDER, theme::TEXT);
                theme::show_badge(ui, "Seven Card Stud", theme::PANEL_SOFT, theme::BORDER, theme::TEXT);
                theme::show_badge(ui, "Texas Hold'em", theme::PANEL_SOFT, theme::BORDER, theme::TEXT);
            });
        });

        ui.columns(2, |columns| {
            theme::show_stat_card(&mut columns[0], "Profiles", "Secure PIN");
            theme::show_stat_card(&mut columns[1], "Lobby", "Live tables");
        });
    });
}

fn show_login_panel(app: &mut PokerApp, ui: &mut egui::Ui) {
    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Account"));
        ui.label(theme::title("Enter the lobby"));
        ui.label(theme::muted(
            "Use an existing account or create a new bankroll before joining a table.",
        ));

        if !app.status_msg.is_empty() {
            ui.add_space(8.0);
            theme::error_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Session notice")
                        .color(theme::DANGER)
                        .strong(),
                );
                ui.label(theme::muted(&app.status_msg));
            });
        }

        if let Some(err) = &app.last_error {
            ui.add_space(8.0);
            theme::error_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Server response")
                        .color(theme::DANGER)
                        .strong(),
                );
                ui.label(theme::muted(err));
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let login_selected = app.login_tab == LoginTab::Login;
            let create_selected = app.login_tab == LoginTab::CreateAccount;

            if ui
                .add_sized(
                    [ui.available_width() * 0.48, 40.0],
                    tab_button("Login", login_selected),
                )
                .clicked()
            {
                app.login_tab = LoginTab::Login;
            }

            if ui
                .add_sized(
                    [ui.available_width(), 40.0],
                    tab_button("Create Account", create_selected),
                )
                .clicked()
            {
                app.login_tab = LoginTab::CreateAccount;
            }
        });

        ui.add_space(8.0);
        show_field(ui, "Player name", &mut app.input_name, false, "your name");
        show_field(ui, "PIN", &mut app.input_pin, true, "****");

        if app.login_tab == LoginTab::CreateAccount {
            show_field(ui, "Starting balance", &mut app.input_amount, false, "1000");
        }

        let is_pending = app.is_request_pending();
        let button_label = match app.login_tab {
            LoginTab::Login => "Login",
            LoginTab::CreateAccount => "Create account",
        };

        let primary = egui::Button::new(
            egui::RichText::new(button_label)
                .color(theme::BACKGROUND)
                .strong(),
        )
        .fill(theme::ACCENT_STRONG)
        .stroke(egui::Stroke::new(1.0, theme::ACCENT))
        .corner_radius(18.0);

        if ui
            .add_enabled_ui(!is_pending, |ui| {
                ui.add_sized([ui.available_width(), 48.0], primary)
            })
            .inner
            .clicked()
        {
            match app.login_tab.clone() {
                LoginTab::Login => {
                    let op = RequestOp::Login {
                        name: app.input_name.trim().to_string(),
                        pin: app.input_pin.clone(),
                    };
                    app.send_op(PendingKind::Login, op);
                }
                LoginTab::CreateAccount => {
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
    });
}

fn tab_button(label: &str, selected: bool) -> egui::Button<'static> {
    let (fill, stroke, text) = if selected {
        (theme::ACCENT_STRONG, theme::ACCENT, theme::BACKGROUND)
    } else {
        (theme::PANEL_ALT, theme::BORDER, theme::TEXT)
    };

    egui::Button::new(egui::RichText::new(label.to_owned()).color(text).strong())
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(16.0)
}

fn show_field(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool, hint: &str) {
    theme::soft_panel_frame().show(ui, |ui| {
        ui.label(theme::small_muted(label));
        ui.add_sized(
            [ui.available_width(), 40.0],
            egui::TextEdit::singleline(value)
                .password(password)
                .hint_text(hint),
        );
    });
}

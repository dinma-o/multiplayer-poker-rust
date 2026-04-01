use super::super::{app::PokerApp, theme};

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        theme::paint_screen_background(ui);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(28.0);

                if ui.available_width() > 880.0 {
                    ui.columns(2, |columns| {
                        show_intro_panel(&mut columns[0]);
                        show_connect_panel(app, ctx, &mut columns[1]);
                    });
                } else {
                    show_intro_panel(ui);
                    ui.add_space(18.0);
                    show_connect_panel(app, ctx, ui);
                }

                ui.add_space(28.0);
            });
    });
}

fn show_intro_panel(ui: &mut egui::Ui) {
    theme::accent_panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Casino"));
        ui.label(theme::hero("ECE Poker House"));
        ui.label(theme::muted(
            "Single-table poker client for Five-Card Draw, Seven-Card Stud, and Texas Hold'em with a live hand log and real-time table updates.",
        ));

        ui.add_space(14.0);
        ui.columns(2, |columns| {
            theme::show_stat_card(&mut columns[0], "Variants", "3");
            theme::show_stat_card(&mut columns[1], "Mode", "Live sync");
        });

        ui.add_space(8.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.label(theme::section_title("What this client is built for"));
            ui.label(theme::muted("Clean table state visibility during demos."));
            ui.label(theme::muted("Fast seat join and table switching."));
            ui.label(theme::muted("One visual theme across lobby and gameplay."));
        });
    });
}

fn show_connect_panel(app: &mut PokerApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Connect"));
        ui.label(theme::title("Join the card room"));
        ui.label(theme::muted(
            "Point the GUI at a running server and open the redesigned poker room.",
        ));

        if !app.status_msg.is_empty() {
            ui.add_space(8.0);
            theme::error_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Connection issue")
                        .color(theme::DANGER)
                        .strong(),
                );
                ui.label(theme::muted(&app.status_msg));
            });
        }

        ui.add_space(6.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.label(theme::small_muted("Server address"));
            ui.add_sized(
                [ui.available_width(), 40.0],
                egui::TextEdit::singleline(&mut app.server_address).hint_text("127.0.0.1:8080"),
            );
        });

        let connect_button = egui::Button::new(
            egui::RichText::new("Connect to server")
                .color(theme::BACKGROUND)
                .strong(),
        )
        .fill(theme::ACCENT_STRONG)
        .stroke(egui::Stroke::new(1.0, theme::ACCENT))
        .corner_radius(18.0);

        if ui
            .add_sized([ui.available_width(), 48.0], connect_button)
            .clicked()
        {
            app.status_msg.clear();
            app.connect(ctx);
        }

        ui.label(theme::small_muted("Local default: 127.0.0.1:8080"));
    });
}

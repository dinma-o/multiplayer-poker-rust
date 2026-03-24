use common::protocol::PokerVariant;
use common::wire::RequestOp;

use super::super::app::{PendingKind, PokerApp};

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    // ── Header bar ───────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("lobby_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if let Some(account) = &app.account {
                ui.label(format!(
                    "{}   |   Balance: {}",
                    account.player_name, account.balance
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Logout").clicked() {
                    app.send_op(PendingKind::Logout, RequestOp::Logout);
                }
            });
        });
    });

    // ── Main panel ───────────────────────────────────────────────────────────
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Lobby");
        ui.add_space(8.0);

        // ── Table list ───────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.strong("Tables");
            if ui.button("Refresh").clicked() {
                app.send_op(PendingKind::ListTables, RequestOp::ListTables);
            }
        });
        ui.separator();

        if app.tables.is_empty() {
            ui.label("No tables available.");
        } else {
            let is_pending = app.is_request_pending();
            let mut join_id: Option<u64> = None;

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for table in &app.tables {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Table {}  |  {}  |  {}/{}",
                                table.table_id,
                                variant_label(table.variant),
                                table.seated_count,
                                table.max_players,
                            ));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(!is_pending, egui::Button::new("Join"))
                                        .clicked()
                                    {
                                        join_id = Some(table.table_id);
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                });

            if let Some(table_id) = join_id {
                app.send_op(PendingKind::JoinTable, RequestOp::JoinTable { table_id });
            }
        }

        ui.add_space(16.0);

        // ── Create table ─────────────────────────────────────────────────────
        ui.strong("Create Table");
        ui.separator();
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Variant")
                .selected_text(variant_label(app.selected_variant))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.selected_variant,
                        PokerVariant::FiveCardDraw,
                        variant_label(PokerVariant::FiveCardDraw),
                    );
                    ui.selectable_value(
                        &mut app.selected_variant,
                        PokerVariant::SevenCardStud,
                        variant_label(PokerVariant::SevenCardStud),
                    );
                    ui.selectable_value(
                        &mut app.selected_variant,
                        PokerVariant::TexasHoldEm,
                        variant_label(PokerVariant::TexasHoldEm),
                    );
                });

            let is_pending = app.is_request_pending();
            if ui
                .add_enabled(!is_pending, egui::Button::new("Create"))
                .clicked()
            {
                app.send_op(
                    PendingKind::CreateTable,
                    RequestOp::CreateTable {
                        variant: app.selected_variant,
                    },
                );
            }
        });

        ui.add_space(16.0);

        // ── Deposit / Withdraw ────────────────────────────────────────────────
        ui.strong("Balance");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Amount:");
            ui.add(
                egui::TextEdit::singleline(&mut app.input_deposit)
                    .desired_width(80.0)
                    .hint_text("0"),
            );

            let amount_valid = app
                .input_deposit
                .parse::<i64>()
                .map(|v| v > 0)
                .unwrap_or(false);
            let is_pending = app.is_request_pending();
            let enabled = amount_valid && !is_pending;

            if ui
                .add_enabled(enabled, egui::Button::new("Deposit"))
                .clicked()
            {
                let amount = app.input_deposit.parse::<i64>().unwrap_or(0);
                app.send_op(PendingKind::Deposit, RequestOp::Deposit { amount });
            }

            if ui
                .add_enabled(enabled, egui::Button::new("Withdraw"))
                .clicked()
            {
                let amount = app.input_deposit.parse::<i64>().unwrap_or(0);
                app.send_op(PendingKind::Withdraw, RequestOp::Withdraw { amount });
            }
        });

        // ── Error display ────────────────────────────────────────────────────
        if let Some(err) = &app.last_error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, err);
        }
    });
}

fn variant_label(v: PokerVariant) -> &'static str {
    match v {
        PokerVariant::FiveCardDraw => "Five Card Draw",
        PokerVariant::SevenCardStud => "Seven Card Stud",
        PokerVariant::TexasHoldEm => "Texas Hold'em",
    }
}

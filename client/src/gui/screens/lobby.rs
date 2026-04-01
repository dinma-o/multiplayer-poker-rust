use common::protocol::{HandStep, PokerVariant};
use common::wire::{RequestOp, TableSummary};

use super::super::{
    app::{PendingKind, PokerApp},
    theme,
};

pub fn show(app: &mut PokerApp, ctx: &egui::Context) {
    let header_account = app.account.clone();

    egui::TopBottomPanel::top("lobby_header").show(ctx, |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(theme::eyebrow("Casino"));
                ui.label(theme::title("ECE Poker House"));
            });

            if let Some(account) = &header_account {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(logout_button()).clicked() {
                        app.send_op(PendingKind::Logout, RequestOp::Logout);
                    }

                    theme::show_badge(
                        ui,
                        &format!("{} chips", account.balance),
                        theme::PANEL_ALT,
                        theme::BORDER,
                        theme::TEXT,
                    );
                    ui.label(theme::muted(format!(
                        "Signed in as {}",
                        account.player_name
                    )));
                });
            }
        });
        ui.add_space(8.0);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        theme::paint_screen_background(ui);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);

                if ui.available_width() > 1100.0 {
                    ui.columns(3, |columns| {
                        show_overview_column(&mut columns[0], app);
                        show_tables_column(&mut columns[1], app);
                        show_account_column(&mut columns[2], app);
                    });
                } else if ui.available_width() > 760.0 {
                    ui.columns(2, |columns| {
                        show_overview_column(&mut columns[0], app);
                        show_tables_column(&mut columns[1], app);
                    });
                    ui.add_space(18.0);
                    show_account_column(ui, app);
                } else {
                    show_overview_column(ui, app);
                    ui.add_space(18.0);
                    show_tables_column(ui, app);
                    ui.add_space(18.0);
                    show_account_column(ui, app);
                }

                ui.add_space(18.0);
            });
    });
}

fn show_overview_column(ui: &mut egui::Ui, app: &PokerApp) {
    let total_tables = app.tables.len();
    let seated_players: usize = app.tables.iter().map(|table| table.seated_count).sum();
    let open_seats: usize = app
        .tables
        .iter()
        .map(|table| table.max_players.saturating_sub(table.seated_count))
        .sum();
    let balance = app
        .account
        .as_ref()
        .map(|account| account.balance)
        .unwrap_or(0);

    theme::accent_panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Lobby"));
        ui.label(theme::hero("Choose your next table"));
        ui.label(theme::muted(
            "The lobby now mirrors the darker poker-room reference: stronger hierarchy, clearer table cards, and quick bankroll access.",
        ));

        ui.add_space(12.0);
        ui.columns(2, |columns| {
            theme::show_stat_card(&mut columns[0], "Open tables", total_tables);
            theme::show_stat_card(&mut columns[1], "Players seated", seated_players);
        });
        ui.columns(2, |columns| {
            theme::show_stat_card(&mut columns[0], "Available seats", open_seats);
            theme::show_stat_card(&mut columns[1], "Bankroll", balance);
        });
    });

    ui.add_space(16.0);

    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::section_title("Game menu"));
        ui.label(theme::muted(
            "Each variant keeps the same room aesthetic, so swapping between screens still feels like one connected client.",
        ));
        ui.add_space(4.0);

        for variant in [
            PokerVariant::FiveCardDraw,
            PokerVariant::SevenCardStud,
            PokerVariant::TexasHoldEm,
        ] {
            theme::soft_panel_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new(variant_label(variant))
                        .color(theme::TEXT)
                        .strong(),
                );
                ui.label(theme::muted(variant_blurb(variant)));
            });
            ui.add_space(6.0);
        }
    });
}

fn show_tables_column(ui: &mut egui::Ui, app: &mut PokerApp) {
    let is_pending = app.is_request_pending();
    let mut join_id: Option<u64> = None;

    theme::panel_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(theme::eyebrow("Directory"));
                ui.label(theme::section_title("Active tables"));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!is_pending, secondary_button("Refresh"))
                    .clicked()
                {
                    app.send_op(PendingKind::ListTables, RequestOp::ListTables);
                }
            });
        });

        ui.label(theme::muted(
            "Table cards surface the game, hand state, and current occupancy before you sit down.",
        ));
        ui.add_space(6.0);

        if app.tables.is_empty() {
            theme::info_frame().show(ui, |ui| {
                ui.label(theme::section_title("No tables live right now"));
                ui.label(theme::muted(
                    "Create a table to start a fresh room, or refresh once another player opens one.",
                ));
            });
        } else {
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for table in &app.tables {
                        if show_table_card(ui, table, !is_pending) {
                            join_id = Some(table.table_id);
                        }
                        ui.add_space(10.0);
                    }
                });
        }
    });

    if let Some(table_id) = join_id {
        app.send_op(PendingKind::JoinTable, RequestOp::JoinTable { table_id });
    }

    ui.add_space(16.0);

    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Create"));
        ui.label(theme::section_title("Open a new table"));
        ui.label(theme::muted(
            "Choose a variant below and spin up a new room directly from the lobby.",
        ));

        ui.add_space(8.0);
        for variant in [
            PokerVariant::FiveCardDraw,
            PokerVariant::SevenCardStud,
            PokerVariant::TexasHoldEm,
        ] {
            show_variant_option(ui, &mut app.selected_variant, variant);
            ui.add_space(8.0);
        }

        if ui
            .add_enabled(!is_pending, primary_button("Create table"))
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
}

fn show_account_column(ui: &mut egui::Ui, app: &mut PokerApp) {
    let account = app.account.clone();

    theme::panel_frame().show(ui, |ui| {
        let balance = account.as_ref().map(|profile| profile.balance).unwrap_or(0);
        ui.label(theme::eyebrow("Wallet"));
        ui.label(theme::title(format!("{} chips", balance)));
        ui.label(theme::muted(
            "Deposit or withdraw without leaving the poker room theme.",
        ));

        ui.add_space(6.0);
        theme::soft_panel_frame().show(ui, |ui| {
            ui.label(theme::small_muted("Amount"));
            ui.add_sized(
                [ui.available_width(), 40.0],
                egui::TextEdit::singleline(&mut app.input_deposit).hint_text("0"),
            );
        });

        let amount_valid = app
            .input_deposit
            .parse::<i64>()
            .map(|value| value > 0)
            .unwrap_or(false);
        let enabled = amount_valid && !app.is_request_pending();

        ui.columns(2, |columns| {
            if columns[0]
                .add_enabled(enabled, primary_button("Deposit"))
                .clicked()
            {
                let amount = app.input_deposit.parse::<i64>().unwrap_or(0);
                app.send_op(PendingKind::Deposit, RequestOp::Deposit { amount });
            }

            if columns[1]
                .add_enabled(enabled, secondary_button("Withdraw"))
                .clicked()
            {
                let amount = app.input_deposit.parse::<i64>().unwrap_or(0);
                app.send_op(PendingKind::Withdraw, RequestOp::Withdraw { amount });
            }
        });
    });

    if let Some(account) = account {
        ui.add_space(16.0);
        theme::panel_frame().show(ui, |ui| {
            ui.label(theme::eyebrow("Stats"));
            ui.label(theme::section_title("Player profile"));
            theme::show_stat_row(ui, "Rounds played", account.games_played);
            theme::show_stat_row(ui, "Pots won", account.pots_won);
            theme::show_stat_row(ui, "Folds", account.folds);
            theme::show_stat_row(ui, "Balance", account.balance);
        });
    }

    if let Some(err) = &app.last_error {
        ui.add_space(16.0);
        theme::error_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("Lobby notice")
                    .color(theme::DANGER)
                    .strong(),
            );
            ui.label(theme::muted(err));
        });
    }
}

fn show_table_card(ui: &mut egui::Ui, table: &TableSummary, enabled: bool) -> bool {
    let mut join_clicked = false;

    theme::soft_panel_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(theme::eyebrow(variant_label(table.variant)));
                ui.label(theme::section_title(format!("Table {}", table.table_id)));
                ui.label(theme::muted(step_label(table.step)));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.add_enabled(enabled, secondary_button("Join")).clicked() {
                    join_clicked = true;
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            theme::show_badge(
                ui,
                &format!("{}/{} seated", table.seated_count, table.max_players),
                theme::PANEL_SOFT,
                theme::BORDER,
                theme::TEXT,
            );
            theme::show_badge(
                ui,
                occupancy_label(table),
                theme::PANEL_SOFT,
                theme::BORDER_SOFT,
                theme::TEXT_MUTED,
            );
        });
    });

    join_clicked
}

fn show_variant_option(
    ui: &mut egui::Ui,
    selected_variant: &mut PokerVariant,
    variant: PokerVariant,
) {
    let selected = *selected_variant == variant;
    let fill = if selected {
        theme::info_frame()
    } else {
        theme::soft_panel_frame()
    };

    let response = fill.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(variant_label(variant))
                        .color(theme::TEXT)
                        .strong(),
                );
                ui.label(theme::muted(variant_blurb(variant)));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if selected {
                    theme::show_badge(
                        ui,
                        "Selected",
                        theme::ACCENT_GLOW,
                        theme::ACCENT,
                        theme::ACCENT,
                    );
                }
            });
        });
    });

    if response.response.interact(egui::Sense::click()).clicked() {
        *selected_variant = variant;
    }
}

fn primary_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label.to_owned())
            .color(theme::BACKGROUND)
            .strong(),
    )
    .fill(theme::ACCENT_STRONG)
    .stroke(egui::Stroke::new(1.0, theme::ACCENT))
    .corner_radius(18.0)
}

fn secondary_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label.to_owned())
            .color(theme::TEXT)
            .strong(),
    )
    .fill(theme::PANEL_ALT)
    .stroke(egui::Stroke::new(1.0, theme::BORDER))
    .corner_radius(18.0)
}

fn logout_button() -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new("Logout").color(theme::TEXT).strong())
        .fill(theme::PANEL_ALT)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(18.0)
}

fn occupancy_label(table: &TableSummary) -> &'static str {
    if table.seated_count >= table.max_players {
        "Waitlist likely"
    } else if table.seated_count == 0 {
        "Open room"
    } else {
        "Seats available"
    }
}

fn variant_label(v: PokerVariant) -> &'static str {
    match v {
        PokerVariant::FiveCardDraw => "Five Card Draw",
        PokerVariant::SevenCardStud => "Seven Card Stud",
        PokerVariant::TexasHoldEm => "Texas Hold'em",
    }
}

fn variant_blurb(v: PokerVariant) -> &'static str {
    match v {
        PokerVariant::FiveCardDraw => {
            "Classic draw round with discard decisions and a fast action cycle."
        }
        PokerVariant::SevenCardStud => {
            "Face-up pressure with staged dealing and strong seat reads."
        }
        PokerVariant::TexasHoldEm => "Community-card play with shared board tension and big pots.",
    }
}

fn step_label(step: HandStep) -> &'static str {
    match step {
        HandStep::WaitingForPlayers => "Waiting for enough players",
        HandStep::CollectForcedBets => "Collecting blinds or antes",
        HandStep::DealPrivate => "Dealing private cards",
        HandStep::DealFaceUp => "Dealing face-up cards",
        HandStep::DealCommunity => "Dealing community cards",
        HandStep::BettingRound1 => "Betting round 1",
        HandStep::DrawPhase => "Draw phase",
        HandStep::BettingRound2 => "Betting round 2",
        HandStep::BettingRound3 => "Betting round 3",
        HandStep::BettingRound4 => "Betting round 4",
        HandStep::BettingRound5 => "Betting round 5",
        HandStep::Showdown => "Showdown",
        HandStep::Payout => "Payout",
    }
}

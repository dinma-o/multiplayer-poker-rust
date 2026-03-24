use common::protocol::{HandStep, PokerVariant};
use common::wire::{RequestOp, TableId};

use super::super::app::{PendingKind, PokerApp};
use super::super::widgets::action_bar::show_action_bar;
use super::super::widgets::card::show_card;
use super::super::widgets::seat::{show_empty_seat, show_seat};

pub fn show(app: &mut PokerApp, ctx: &egui::Context, table_id: TableId) {
    let my_player_id = app.account.as_ref().map(|a| a.player_id);
    let player_view = app.player_view.clone();

    // ── Top bar ──────────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("table_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("← Lobby").clicked() {
                app.enter_lobby();
                return;
            }

            if let Some(state) = &app.table_state {
                ui.separator();
                ui.label(format!("Table {}", state.table_id));
                ui.separator();
                ui.label(variant_label(state.variant));
                ui.separator();
                ui.label(step_label(state.step));
                ui.separator();
                ui.label(format!("Pot: {}", state.pot));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_pending = app.is_request_pending();
                if ui
                    .add_enabled(!is_pending, egui::Button::new("Leave Table"))
                    .clicked()
                {
                    app.send_op(
                        PendingKind::LeaveTable,
                        RequestOp::LeaveTable { table_id },
                    );
                }
            });
        });
    });

    // ── Central panel ────────────────────────────────────────────────────────
    egui::CentralPanel::default().show(ctx, |ui| {
        let state = match &app.table_state {
            Some(s) if s.table_id == table_id => s.clone(),
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for table data…");
                });
                return;
            }
        };

        let acting_id = state.acting_player_id;
        let dealer_seat = state.dealer_seat;

        // Helper: render a seat slot by seat index
        let render_seat = |ui: &mut egui::Ui,
                           ctx: &egui::Context,
                           cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
                           seat_index: usize| {
            if let Some(seat) = state.seats.iter().find(|s| s.seat == seat_index) {
                let is_active = acting_id.map_or(false, |id| id == seat.player_id);
                let is_dealer = seat.seat == dealer_seat;
                let is_me = my_player_id.map_or(false, |id| id == seat.player_id);
                show_seat(ui, ctx, cache, seat, is_active, is_dealer, is_me);
            } else {
                show_empty_seat(ui);
            }
        };

        ui.add_space(12.0);

        // ── Top row: seats 0, 1, 2 ───────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            for seat_index in 0..3 {
                render_seat(ui, ctx, &mut app.texture_cache, seat_index);
                ui.add_space(8.0);
            }
        });

        ui.add_space(16.0);

        // ── Community cards (Hold'em only) ───────────────────────────────────
        if state.variant == PokerVariant::TexasHoldEm {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label("Community:");
                ui.add_space(4.0);
                let board = player_view
                    .as_ref()
                    .map(|v| v.board_cards.clone())
                    .unwrap_or_else(|| state.board_cards.clone());
                if board.is_empty() {
                    ui.label(
                        egui::RichText::new("— none yet —")
                            .color(egui::Color32::from_gray(140)),
                    );
                } else {
                    for card in &board {
                        show_card(ui, ctx, &mut app.texture_cache, *card);
                        ui.add_space(2.0);
                    }
                }
            });
            ui.add_space(16.0);
        }

        // ── Bottom row: seats 3, 4 ───────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            for seat_index in 3..5 {
                render_seat(ui, ctx, &mut app.texture_cache, seat_index);
                ui.add_space(8.0);
            }
        });

        // ── Your hand ────────────────────────────────────────────────────────
        if let Some(view) = &player_view {
            let has_cards =
                !view.private_cards.is_empty() || !view.up_cards.is_empty();
            if has_cards {
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Your Hand").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for card in &view.private_cards {
                        show_card(ui, ctx, &mut app.texture_cache, *card);
                        ui.add_space(2.0);
                    }
                    for card in &view.up_cards {
                        show_card(ui, ctx, &mut app.texture_cache, *card);
                        ui.add_space(2.0);
                    }
                });
            }
        }

        // ── Action bar ───────────────────────────────────────────────────────
        // Show only when there is a prompt, it is not a draw phase, and no
        // request is already in-flight.
        let prompt = player_view
            .as_ref()
            .and_then(|v| v.action_prompt.as_ref())
            .filter(|p| p.max_discards.is_none())
            .cloned();

        if let Some(prompt) = prompt {
            let balance = app.account.as_ref().map(|a| a.balance).unwrap_or(0);
            let is_pending = app.is_request_pending();

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Your Turn").strong());
            ui.add_space(4.0);

            ui.add_enabled_ui(!is_pending, |ui| {
                if let Some(action) =
                    show_action_bar(ui, &prompt, balance, &mut app.input_bet)
                {
                    app.send_op(
                        PendingKind::SubmitAction,
                        RequestOp::SubmitAction { table_id, action },
                    );
                }
            });
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

fn step_label(s: HandStep) -> &'static str {
    match s {
        HandStep::WaitingForPlayers => "Waiting for players",
        HandStep::CollectForcedBets => "Collecting antes",
        HandStep::DealPrivate => "Dealing cards",
        HandStep::DealFaceUp => "Dealing face-up",
        HandStep::DealCommunity => "Dealing community",
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

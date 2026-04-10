use common::protocol::{AnimationFace, AnimationTarget, HandStep, PokerVariant};
use common::wire::{AnimationCue, PlayerView, PublicTableState, RequestOp, TableId};
use image::load_from_memory;

use super::super::{
    app::{PendingKind, PokerApp, TableViewMode},
    theme,
    widgets::{
        action_bar::show_action_bar,
        card::{
            CARD_SIZE, COMPACT_CARD_SIZE, show_card, show_card_back, show_card_back_compact,
            show_card_compact, show_card_slot, show_card_slot_compact,
        },
        discard_picker::show_discard_picker,
        event_log::show_event_log,
        hand_rankings_window::show_hand_rankings_window,
        results_panel::show_results,
        seat::{show_empty_seat, show_seat},
    },
};

pub fn show(app: &mut PokerApp, ctx: &egui::Context, table_id: TableId) {
    let my_player_id = app.account.as_ref().map(|account| account.player_id);
    let player_view = app.player_view.clone();
    let table_view_mode = app.table_view_mode.unwrap_or(TableViewMode::Seated);
    let is_spectating = table_view_mode == TableViewMode::Spectating;
    let state = app
        .table_state
        .clone()
        .filter(|state| state.table_id == table_id);
    let viewport_size = ctx.screen_rect().size();
    let left_panel_width = if viewport_size.x < 1240.0 {
        220.0
    } else {
        250.0
    };
    let right_panel_width = if viewport_size.x < 1240.0 {
        250.0
    } else {
        280.0
    };

    let mut leave_clicked = false;
    egui::TopBottomPanel::top("table_top_bar").show(ctx, |ui| {
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let is_pending = app.is_request_pending();
            if ui
                .add_enabled(!is_pending, secondary_button("Lobby"))
                .clicked()
            {
                leave_clicked = true;
            }

            if ui
                .add_enabled(!is_pending, secondary_button("Hand Rankings"))
                .clicked()
            {
                app.show_hand_rankings = true;
            }

            if let Some(state) = &state {
                theme::show_badge(
                    ui,
                    &format!("Table {}", state.table_id),
                    theme::PANEL_ALT,
                    theme::BORDER,
                    theme::TEXT,
                );
                theme::show_badge(
                    ui,
                    variant_label(state.variant),
                    theme::PANEL_ALT,
                    theme::BORDER,
                    theme::TEXT,
                );
                theme::show_badge(
                    ui,
                    step_label(state.step),
                    theme::PANEL_ALT,
                    theme::BORDER_SOFT,
                    theme::TEXT_MUTED,
                );
                if is_spectating {
                    theme::show_badge(
                        ui,
                        "Spectating",
                        theme::PANEL_SOFT,
                        theme::INFO,
                        theme::TEXT,
                    );
                }
                theme::show_badge(
                    ui,
                    &format!("Pot {}", state.pot),
                    theme::ACCENT_STRONG,
                    theme::ACCENT,
                    theme::BACKGROUND,
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !is_pending,
                        secondary_button(if is_spectating {
                            "Stop watching"
                        } else {
                            "Leave table"
                        }),
                    )
                    .clicked()
                {
                    leave_clicked = true;
                }
            });
        });
        ui.add_space(8.0);
    });

    if leave_clicked {
        if is_spectating {
            app.leave_spectated_table(table_id);
        } else {
            app.send_op(PendingKind::LeaveTable, RequestOp::LeaveTable { table_id });
        }
        return;
    }

    egui::SidePanel::left("table_left_panel")
        .resizable(false)
        .default_width(left_panel_width)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(state) = &state {
                        show_left_sidebar(ui, app, state);
                    } else {
                        show_waiting_panel(
                            ui,
                            "Waiting for table state",
                            "The client is subscribing to the table and fetching the latest hand state.",
                        );
                    }
                });
        });

    egui::SidePanel::right("table_right_panel")
        .resizable(false)
        .default_width(right_panel_width)
        .show(ctx, |ui| {
            ui.set_width(right_panel_width);
            ui.set_min_width(right_panel_width);
            ui.set_max_width(right_panel_width);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(state) = &state {
                        show_right_sidebar(
                            ui,
                            ctx,
                            app,
                            state,
                            table_id,
                            player_view.as_ref(),
                            table_view_mode,
                        );
                    } else {
                        show_waiting_panel(
                            ui,
                            "Syncing seat data",
                            if is_spectating {
                                "Public seat and pot information will appear here once the first table update arrives."
                            } else {
                                "Your hand and actions will appear here once the table payload arrives."
                            },
                        );
                    }
                });
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        theme::paint_screen_background(ui);

        egui::ScrollArea::both()
            .id_salt("table_surface_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(state) = &state {
                    show_table_surface(ui, ctx, app, state, my_player_id, player_view.as_ref());
                } else {
                    ui.centered_and_justified(|ui| {
                        theme::panel_frame().show(ui, |ui| {
                            ui.label(theme::section_title("Waiting for table data"));
                            ui.label(theme::muted(
                                "Once the first state arrives, the table layout, community cards, and active seats will render here.",
                            ));
                        });
                    });
                }
            });
    });

    if app.show_hand_rankings {
        show_hand_rankings_window(ctx, &mut app.show_hand_rankings);
    }
}

fn show_left_sidebar(ui: &mut egui::Ui, app: &PokerApp, state: &PublicTableState) {
    let dealer_name = state
        .seats
        .iter()
        .find(|seat| seat.seat == state.dealer_seat)
        .map(|seat| seat.player_name.as_str())
        .unwrap_or("Waiting");

    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Table"));
        ui.label(theme::title(variant_label(state.variant)));
        ui.label(theme::muted(format!("Table {}", state.table_id)));
        theme::show_stat_row(ui, "Stage", step_label(state.step));
        theme::show_stat_row(ui, "Main pot", state.pot);
        theme::show_stat_row(ui, "Current bet", state.current_bet);
        theme::show_stat_row(ui, "Dealer", dealer_name);
        theme::show_stat_row(ui, "Players", format!("{}/5", state.seats.len()));
    });

    ui.add_space(12.0);

    theme::panel_frame().show(ui, |ui| {
        show_results(ui, app.last_showdown.as_ref(), app.last_payout.as_ref());
        show_event_log(ui, &app.event_log);
    });
}

fn show_right_sidebar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut PokerApp,
    state: &PublicTableState,
    table_id: TableId,
    player_view: Option<&PlayerView>,
    table_view_mode: TableViewMode,
) {
    let is_spectating = table_view_mode == TableViewMode::Spectating;
    let action_locked = app.is_action_locked();

    if !is_spectating && let Some(view) = player_view {
        let has_cards = !view.private_cards.is_empty() || !view.up_cards.is_empty();
        if has_cards {
            theme::panel_frame().show(ui, |ui| {
                ui.label(theme::eyebrow("Player"));
                ui.label(theme::section_title("Your hand"));
                if let Some(seat) = view.seat {
                    theme::show_badge(
                        ui,
                        &format!("Seat {}", seat + 1),
                        theme::PANEL_SOFT,
                        theme::BORDER,
                        theme::TEXT_MUTED,
                    );
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for card in &view.private_cards {
                        show_card(ui, ctx, &mut app.texture_cache, *card);
                    }
                    if state.variant != PokerVariant::SevenCardStud {
                        for card in &view.up_cards {
                            show_card(ui, ctx, &mut app.texture_cache, *card);
                        }
                    }
                });
            });

            ui.add_space(12.0);
        }
    }

    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("Action"));

        if is_spectating {
            ui.label(theme::section_title("Spectator view"));
            ui.label(theme::muted(
                "You are subscribed to public table updates only. Seats, pot, board cards, and history will keep updating here while you watch.",
            ));
        } else if state.step == HandStep::WaitingForPlayers {
            ui.label(theme::section_title("Ready to deal"));
            ui.label(theme::muted(
                "The room is idle. Start the next hand when enough players are seated.",
            ));

            let is_dealer = player_view
                .map(|view| view.player_id)
                .and_then(|pid| state.seats.iter().find(|s| s.player_id == pid))
                .map(|seat| seat.seat == state.dealer_seat)
                .unwrap_or(false);

            ui.add_space(6.0);
            ui.label(theme::eyebrow("Variant"));
            ui.add_enabled_ui(is_dealer && !action_locked, |ui| {
                for (variant, label) in [
                    (PokerVariant::FiveCardDraw, "Five Card Draw"),
                    (PokerVariant::TexasHoldEm, "Texas Hold'em"),
                    (PokerVariant::SevenCardStud, "Seven Card Stud"),
                ] {
                    if ui.radio(state.variant == variant, label).clicked()
                        && state.variant != variant
                    {
                        app.send_op(
                            PendingKind::ChangeVariant,
                            RequestOp::ChangeVariant { table_id, variant },
                        );
                    }
                }
            });

            ui.add_space(6.0);
            if ui
                .add_enabled(is_dealer && !action_locked, primary_button("Start hand"))
                .clicked()
            {
                app.send_op(PendingKind::StartHand, RequestOp::StartHand { table_id });
            }

            ui.add_space(4.0);
            let sit_out = app.sit_out_pending;
            let sit_out_label = if sit_out { "Sit Back In" } else { "Sit Out Next Hand" };
            if ui
                .add_enabled(!action_locked, secondary_button(sit_out_label))
                .clicked()
            {
                if sit_out {
                    app.sit_out_pending = false;
                    app.send_op(PendingKind::SitIn, RequestOp::SitIn { table_id });
                } else {
                    app.sit_out_pending = true;
                    app.send_op(PendingKind::SitOut, RequestOp::SitOut { table_id });
                }
            }
        } else if let Some(prompt) = player_view
            .and_then(|view| view.action_prompt.as_ref())
            .cloned()
        {
            ui.label(theme::section_title("Your turn"));
            ui.label(theme::muted(
                "Primary actions stay visible here so betting rounds move quickly during demos.",
            ));
            ui.add_space(6.0);

            ui.add_enabled_ui(!action_locked, |ui| {
                if let Some(max_discards) = prompt.max_discards {
                    let private_cards = player_view
                        .map(|view| view.private_cards.clone())
                        .unwrap_or_default();

                    if let Some(discards) = show_discard_picker(
                        ui,
                        ctx,
                        &mut app.texture_cache,
                        &private_cards,
                        max_discards,
                        &mut app.selected_discards,
                    ) {
                        app.selected_discards.clear();
                        app.send_op(
                            PendingKind::SubmitAction,
                            RequestOp::SubmitAction {
                                table_id,
                                action: common::wire::PlayerAction::Draw { discard: discards },
                            },
                        );
                    }
                } else {
                    if let Some(action) = show_action_bar(ui, &prompt, &mut app.input_bet) {
                        app.send_op(
                            PendingKind::SubmitAction,
                            RequestOp::SubmitAction { table_id, action },
                        );
                    }
                }
            });
        } else {
            ui.label(theme::section_title("Watching the hand"));
            ui.label(theme::muted(
                "Your action panel will wake up here when the server prompts you to act.",
            ));
        }
    });

    if let Some(account) = &app.account {
        ui.add_space(12.0);
        theme::panel_frame().show(ui, |ui| {
            ui.label(theme::eyebrow("Stats"));
            ui.label(theme::section_title("Player profile"));
            theme::show_stat_row(ui, "Rounds played", account.games_played);
            theme::show_stat_row(ui, "Hands won", account.pots_won);
            theme::show_stat_row(ui, "Folds", account.folds);
            theme::show_stat_row(ui, "Bankroll", account.balance);
        });
    }

    if let Some(err) = &app.last_error {
        ui.add_space(12.0);
        theme::error_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("Table notice")
                    .color(theme::DANGER)
                    .strong(),
            );
            ui.label(theme::muted(err));
        });
    }
}

fn show_table_surface(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut PokerApp,
    state: &PublicTableState,
    my_player_id: Option<u64>,
    player_view: Option<&PlayerView>,
) {
    let acting_id = state.acting_player_id;
    let dealer_seat = state.dealer_seat;
    let available_width = ui.available_width().max(420.0);
    let available_height = ui.available_height();
    let narrow = available_width < 700.0;
    let laptop_height = available_height < 820.0;
    let compact = narrow || laptop_height;
    let very_compact = available_width < 600.0 || available_height < 740.0;
    let table_width_cap = if very_compact {
        520.0
    } else if compact {
        560.0
    } else {
        600.0
    };
    let table_width = available_width.min(table_width_cap);
    let outer_margin = if very_compact {
        6
    } else if compact {
        7
    } else {
        9
    };
    let felt_margin = if very_compact {
        6
    } else if compact {
        7
    } else {
        9
    };
    let table_height = if state.variant == PokerVariant::TexasHoldEm {
        if very_compact {
            214.0
        } else if compact {
            236.0
        } else {
            284.0
        }
    } else if very_compact {
        202.0
    } else if compact {
        220.0
    } else {
        258.0
    };
    let top_row_gap = if very_compact {
        2.0
    } else if compact {
        3.0
    } else {
        5.0
    };
    let middle_gap = if very_compact {
        4.0
    } else if compact {
        6.0
    } else {
        10.0
    };
    let bottom_row_gap = if very_compact {
        10.0
    } else if compact {
        14.0
    } else {
        20.0
    };
    let board_gap = if very_compact {
        2.0
    } else if compact {
        4.0
    } else {
        8.0
    };
    let pot_size = if very_compact {
        18.0
    } else if compact {
        21.0
    } else {
        25.0
    };
    let info_frame_max_width = if very_compact {
        136.0
    } else if compact {
        148.0
    } else {
        172.0
    };
    let chip_image_size = if very_compact {
        egui::vec2(24.0, 24.0)
    } else if compact {
        egui::vec2(28.0, 28.0)
    } else {
        egui::vec2(34.0, 34.0)
    };

    let render_seat = |ui: &mut egui::Ui,
                       ctx: &egui::Context,
                       cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
                       seat_index: usize| {
        if let Some(seat) = state.seats.iter().find(|seat| seat.seat == seat_index) {
            let is_active = acting_id == Some(seat.player_id);
            let is_dealer = seat.seat == dealer_seat;
            let is_me = my_player_id == Some(seat.player_id);
            show_seat(ui, ctx, cache, seat, is_active, is_dealer, is_me);
        } else {
            show_empty_seat(ui);
        }
    };

    ui.vertical_centered(|ui| {
        ui.set_max_width(table_width);
        ui.set_width(table_width);

        let shell = theme::table_shell_frame().show(ui, |ui| {
            egui::Frame::new()
                .fill(theme::TABLE_WOOD)
                .corner_radius(255.0)
                .inner_margin(egui::Margin::same(outer_margin))
                .show(ui, |ui| {
                    egui::Frame::new()
                        .fill(theme::TABLE_FELT)
                        .stroke(egui::Stroke::new(1.0, theme::TABLE_EDGE))
                        .corner_radius(255.0)
                        .inner_margin(egui::Margin::symmetric(felt_margin, outer_margin))
                        .show(ui, |ui| {
                            let felt_width = ui.available_width();
                            ui.allocate_ui_with_layout(
                                egui::vec2(felt_width, table_height),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.set_width(felt_width);
                                    ui.set_max_width(felt_width);

                                    ui.add_space(if very_compact { 2.0 } else { 4.0 });
                                    ui.horizontal_centered(|ui| {
                                        for seat_index in 0..3 {
                                            render_seat(
                                                ui,
                                                ctx,
                                                &mut app.texture_cache,
                                                seat_index,
                                            );
                                            ui.add_space(top_row_gap);
                                        }
                                    });

                                    ui.add_space(middle_gap);
                                    ui.vertical_centered(|ui| {
                                        ui.set_max_width(info_frame_max_width);
                                        theme::info_frame().show(ui, |ui| {
                                            ui.set_max_width(info_frame_max_width);
                                            ui.vertical_centered(|ui| {
                                                show_main_pot_image(
                                                    ui,
                                                    ctx,
                                                    &mut app.texture_cache,
                                                    chip_image_size,
                                                );
                                                ui.add_space(2.0);
                                                ui.label(theme::eyebrow("Main pot"));
                                                ui.label(
                                                    egui::RichText::new(state.pot.to_string())
                                                        .size(pot_size)
                                                        .color(theme::TEXT)
                                                        .strong(),
                                                );
                                                ui.label(theme::small_muted(step_label(
                                                    state.step,
                                                )));
                                            });
                                        });
                                    });

                                    if state.variant == PokerVariant::TexasHoldEm {
                                        ui.add_space(board_gap);
                                        ui.horizontal_centered(|ui| {
                                            let board = player_view
                                                .map(|view| view.board_cards.clone())
                                                .unwrap_or_else(|| state.board_cards.clone());
                                            let placeholders =
                                                ["Flop", "Flop", "Flop", "Turn", "River"];

                                            for index in 0..5 {
                                                if let Some(card) = board.get(index) {
                                                    if compact {
                                                        show_card_compact(
                                                            ui,
                                                            ctx,
                                                            &mut app.texture_cache,
                                                            *card,
                                                        );
                                                    } else {
                                                        show_card(
                                                            ui,
                                                            ctx,
                                                            &mut app.texture_cache,
                                                            *card,
                                                        );
                                                    }
                                                } else {
                                                    if compact {
                                                        show_card_slot_compact(
                                                            ui,
                                                            placeholders[index],
                                                        );
                                                    } else {
                                                        show_card_slot(ui, placeholders[index]);
                                                    }
                                                }
                                            }
                                        });
                                    } else {
                                        ui.add_space(board_gap);
                                        ui.vertical_centered(|ui| {
                                            ui.set_max_width(info_frame_max_width);
                                            theme::soft_panel_frame().show(ui, |ui| {
                                                ui.set_max_width(info_frame_max_width);
                                                ui.vertical_centered(|ui| {
                                                    ui.label(theme::eyebrow("Current bet"));
                                                    ui.label(theme::stat_value(
                                                        state.current_bet.to_string(),
                                                    ));
                                                });
                                            });
                                        });
                                    }

                                    ui.add_space(middle_gap);
                                    ui.horizontal_centered(|ui| {
                                        render_seat(ui, ctx, &mut app.texture_cache, 3);
                                        ui.add_space(bottom_row_gap);
                                        render_seat(ui, ctx, &mut app.texture_cache, 4);
                                    });
                                    ui.add_space(if very_compact { 2.0 } else { 4.0 });
                                },
                            );
                        });
                });
        });

        if let Some((cue, progress)) = app.current_animation_overlay(ctx.input(|input| input.time))
        {
            paint_deal_animation_overlay(
                ctx,
                &mut app.texture_cache,
                &cue,
                progress,
                shell.response.rect,
                compact,
                very_compact,
            );
        }
    });
}

fn show_waiting_panel(ui: &mut egui::Ui, title: &str, body: &str) {
    theme::panel_frame().show(ui, |ui| {
        ui.label(theme::section_title(title));
        ui.label(theme::muted(body));
    });
}

fn paint_deal_animation_overlay(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
    cue: &AnimationCue,
    progress: f32,
    table_rect: egui::Rect,
    compact: bool,
    very_compact: bool,
) {
    let size = if compact {
        COMPACT_CARD_SIZE
    } else {
        CARD_SIZE
    };
    let start = deck_origin(table_rect, compact, very_compact);
    let end = animation_target_center(cue, table_rect, compact, very_compact);
    let eased = 1.0 - (1.0 - progress).powi(3);
    let center = egui::pos2(
        start.x + (end.x - start.x) * eased,
        start.y + (end.y - start.y) * eased,
    );
    let top_left = egui::pos2(center.x - size.x * 0.5, center.y - size.y * 0.5);

    egui::Area::new(egui::Id::new("deal_animation_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(top_left)
        .interactable(false)
        .show(ctx, |ui| {
            if matches!(cue.face, AnimationFace::Down) || cue.card.is_none() {
                if compact {
                    show_card_back_compact(ui, ctx, cache);
                } else {
                    show_card_back(ui, ctx, cache);
                }
            } else if let Some(card) = cue.card {
                if compact {
                    show_card_compact(ui, ctx, cache, card);
                } else {
                    show_card(ui, ctx, cache, card);
                }
            }
        });
}

fn deck_origin(table_rect: egui::Rect, compact: bool, very_compact: bool) -> egui::Pos2 {
    let x_offset = if very_compact {
        54.0
    } else if compact {
        60.0
    } else {
        72.0
    };
    let y_offset = if very_compact {
        18.0
    } else if compact {
        22.0
    } else {
        28.0
    };

    egui::pos2(
        table_rect.center().x + x_offset,
        table_rect.center().y - y_offset,
    )
}

fn animation_target_center(
    cue: &AnimationCue,
    table_rect: egui::Rect,
    compact: bool,
    very_compact: bool,
) -> egui::Pos2 {
    match cue.target {
        AnimationTarget::Board => board_target_center(cue, table_rect, compact, very_compact),
        AnimationTarget::Seat => seat_target_center(cue, table_rect, compact, very_compact),
    }
}

fn board_target_center(
    cue: &AnimationCue,
    table_rect: egui::Rect,
    compact: bool,
    very_compact: bool,
) -> egui::Pos2 {
    let size = if compact {
        COMPACT_CARD_SIZE
    } else {
        CARD_SIZE
    };
    let gap = if very_compact {
        2.0
    } else if compact {
        4.0
    } else {
        8.0
    };
    let total_width = size.x * 5.0 + gap * 4.0;
    let start_x = table_rect.center().x - total_width * 0.5 + size.x * 0.5;
    let y = table_rect.center().y
        + if very_compact {
            48.0
        } else if compact {
            56.0
        } else {
            68.0
        };
    let index = cue.target_index.unwrap_or(0).min(4) as f32;

    egui::pos2(start_x + index * (size.x + gap), y)
}

fn seat_target_center(
    cue: &AnimationCue,
    table_rect: egui::Rect,
    compact: bool,
    very_compact: bool,
) -> egui::Pos2 {
    const SEAT_WIDTH: f32 = 108.0;
    let top_gap = if very_compact {
        2.0
    } else if compact {
        3.0
    } else {
        5.0
    };
    let bottom_gap = if very_compact {
        10.0
    } else if compact {
        14.0
    } else {
        20.0
    };
    let top_total = SEAT_WIDTH * 3.0 + top_gap * 2.0;
    let bottom_total = SEAT_WIDTH * 2.0 + bottom_gap;
    let top_start_x = table_rect.center().x - top_total * 0.5;
    let bottom_start_x = table_rect.center().x - bottom_total * 0.5;
    let top_y = table_rect.top()
        + if very_compact {
            58.0
        } else if compact {
            62.0
        } else {
            70.0
        };
    let bottom_y = table_rect.bottom()
        - if very_compact {
            48.0
        } else if compact {
            52.0
        } else {
            58.0
        };

    let seat = cue.seat.unwrap_or(0);
    let base_x = match seat {
        0..=2 => top_start_x + seat as f32 * (SEAT_WIDTH + top_gap) + SEAT_WIDTH * 0.5,
        3 => bottom_start_x + SEAT_WIDTH * 0.5,
        4 => bottom_start_x + SEAT_WIDTH + bottom_gap + SEAT_WIDTH * 0.5,
        _ => table_rect.center().x,
    };
    let y = if seat <= 2 { top_y } else { bottom_y };
    let raw_index = cue.target_index.unwrap_or(0).min(4) as f32;
    let fan_offset = match cue.face {
        AnimationFace::Down => (raw_index.min(2.0) - 1.0) * 10.0,
        AnimationFace::Up => (raw_index.min(3.0) - 1.5) * 12.0,
        AnimationFace::Board => 0.0,
    };

    egui::pos2(base_x + fan_offset, y)
}

fn show_main_pot_image(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
    size: egui::Vec2,
) {
    const CHIPS_TEXTURE_KEY: &str = "ui::stack_of_chips";
    const CHIPS_TEXTURE_BYTES: &[u8] = include_bytes!("../../../assets/ui/stack_of_chips.png");

    if !cache.contains_key(CHIPS_TEXTURE_KEY)
        && let Ok(image) = load_from_memory(CHIPS_TEXTURE_BYTES)
    {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            rgba.as_raw(),
        );
        let texture = ctx.load_texture(
            CHIPS_TEXTURE_KEY,
            color_image,
            egui::TextureOptions::default(),
        );
        cache.insert(CHIPS_TEXTURE_KEY.to_string(), texture);
    }

    if let Some(handle) = cache.get(CHIPS_TEXTURE_KEY) {
        ui.add(egui::Image::new(handle).fit_to_exact_size(size));
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

fn variant_label(v: PokerVariant) -> &'static str {
    match v {
        PokerVariant::FiveCardDraw => "Five Card Draw",
        PokerVariant::SevenCardStud => "Seven Card Stud",
        PokerVariant::TexasHoldEm => "Texas Hold'em",
    }
}

fn step_label(step: HandStep) -> &'static str {
    match step {
        HandStep::WaitingForPlayers => "Waiting for players",
        HandStep::CollectForcedBets => "Collecting antes",
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

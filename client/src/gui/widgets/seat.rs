use std::collections::HashMap;

use common::wire::PublicSeatState;

use crate::gui::theme;

use super::card::{show_card, show_card_back};

/// Render one seat box including cards.
///
/// - `is_active` — this player is currently acting (gold frame)
/// - `is_dealer` — show [D] badge
/// - `is_me`     — our own seat (blue frame)
pub fn show_seat(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    seat: &PublicSeatState,
    is_active: bool,
    is_dealer: bool,
    is_me: bool,
) {
    let frame_color = if is_active {
        theme::ACCENT
    } else if is_me {
        theme::INFO
    } else if !seat.is_connected {
        theme::DANGER
    } else {
        theme::BORDER
    };

    let text_color = if seat.has_folded || !seat.is_connected {
        theme::TEXT_MUTED
    } else {
        theme::TEXT
    };
    let fill = if is_active {
        egui::Color32::from_rgb(19, 31, 27)
    } else {
        theme::PANEL
    };
    let status = seat_status(seat, is_active);

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(2.0, frame_color))
        .corner_radius(16.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(132.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(&seat.player_name)
                                .color(text_color)
                                .strong(),
                        );
                        if is_me {
                            theme::show_badge(
                                ui,
                                "You",
                                theme::PANEL_SOFT,
                                theme::INFO,
                                theme::INFO,
                            );
                        }
                        if is_dealer {
                            theme::show_badge(
                                ui,
                                "Dealer",
                                theme::GOLD_SOFT,
                                theme::GOLD,
                                theme::GOLD,
                            );
                        }
                    });
                    ui.label(theme::small_muted(status));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    theme::show_badge(
                        ui,
                        &format!("{} chips", seat.balance),
                        theme::PANEL_ALT,
                        theme::BORDER,
                        text_color,
                    );
                });
            });

            if seat.has_folded {
                return;
            }

            if seat.hidden_card_count > 0 || !seat.up_cards.is_empty() {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for _ in 0..seat.hidden_card_count {
                        show_card_back(ui);
                    }
                    for card in &seat.up_cards {
                        show_card(ui, ctx, cache, *card);
                    }
                });
            }
        });
}

/// Render an empty seat placeholder.
pub fn show_empty_seat(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(theme::PANEL_SOFT)
        .stroke(egui::Stroke::new(1.0, theme::BORDER_SOFT))
        .corner_radius(16.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(132.0);
            ui.label(theme::small_muted("Empty seat"));
        });
}

fn seat_status(seat: &PublicSeatState, is_active: bool) -> &'static str {
    if !seat.is_connected {
        "Disconnected"
    } else if seat.has_folded {
        "Folded"
    } else if is_active {
        "Acting now"
    } else if seat.is_in_hand {
        "In hand"
    } else {
        "Waiting"
    }
}

use std::collections::HashMap;

use common::wire::PublicSeatState;

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
        egui::Color32::from_rgb(200, 160, 0)
    } else if is_me {
        egui::Color32::from_rgb(50, 100, 200)
    } else {
        egui::Color32::from_gray(80)
    };

    let text_color = if seat.has_folded {
        egui::Color32::from_gray(120)
    } else {
        egui::Color32::WHITE
    };

    egui::Frame::new()
        .fill(egui::Color32::from_gray(40))
        .stroke(egui::Stroke::new(2.0, frame_color))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(110.0);

            // Name row
            ui.horizontal(|ui| {
                if is_dealer {
                    ui.label(
                        egui::RichText::new("[D]")
                            .color(egui::Color32::YELLOW)
                            .small(),
                    );
                }
                ui.label(egui::RichText::new(&seat.player_name).color(text_color).strong());
            });

            // Balance
            ui.label(
                egui::RichText::new(format!("${}", seat.balance))
                    .color(text_color)
                    .small(),
            );

            // Fold label
            if seat.has_folded {
                ui.label(
                    egui::RichText::new("Folded")
                        .color(egui::Color32::from_gray(140))
                        .small(),
                );
                return;
            }

            // Cards row: N hidden backs then any face-up cards (Stud)
            if seat.hidden_card_count > 0 || !seat.up_cards.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for _ in 0..seat.hidden_card_count {
                        show_card_back(ui);
                        ui.add_space(2.0);
                    }
                    for card in &seat.up_cards {
                        show_card(ui, ctx, cache, *card);
                        ui.add_space(2.0);
                    }
                });
            }
        });
}

/// Render an empty seat placeholder.
pub fn show_empty_seat(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(egui::Color32::from_gray(30))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(110.0);
            ui.label(
                egui::RichText::new("Empty").color(egui::Color32::from_gray(90)).small(),
            );
        });
}

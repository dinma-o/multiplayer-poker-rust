use std::collections::HashMap;

use common::card_deck::Card;

use super::card::{show_card, CARD_SIZE};

/// Render the discard picker for the Five Card Draw draw phase.
///
/// `cards`         — the player's private hand (indices 0..N)
/// `max_discards`  — maximum cards allowed to discard
/// `selected`      — mutable set of currently toggled indices
///
/// Returns `Some(Vec<usize>)` of discard indices when the player clicks Confirm,
/// `None` otherwise. An empty vec means stand pat (draw 0 cards).
pub fn show_discard_picker(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    cards: &[Card],
    max_discards: usize,
    selected: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    ui.label(
        egui::RichText::new(format!(
            "Select cards to discard (max {max_discards}). Click to toggle. Leave all unselected to stand pat."
        ))
        .small()
        .color(egui::Color32::from_gray(180)),
    );

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        for (i, &card) in cards.iter().enumerate() {
            let is_selected = selected.contains(&i);

            // Stroke colour: red border = will be discarded, subtle = kept
            let stroke = if is_selected {
                egui::Stroke::new(3.0, egui::Color32::from_rgb(210, 50, 50))
            } else {
                egui::Stroke::new(1.5, egui::Color32::from_gray(80))
            };

            let inner = egui::Frame::new()
                .stroke(stroke)
                .inner_margin(egui::Margin::same(3))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    show_card(ui, ctx, cache, card);
                });

            // Make the whole card area clickable
            let response = inner.response.interact(egui::Sense::click());

            if response.clicked() {
                if is_selected {
                    selected.retain(|&x| x != i);
                } else if selected.len() < max_discards {
                    selected.push(i);
                }
            }

            // Red "DISCARD" label underneath selected cards
            if is_selected {
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(
                        inner.response.rect.left_bottom() + egui::vec2(0.0, 2.0),
                        egui::vec2(CARD_SIZE.x + 6.0, 14.0),
                    ),
                    |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                egui::RichText::new("DISCARD")
                                    .small()
                                    .color(egui::Color32::from_rgb(210, 50, 50)),
                            );
                        });
                    },
                );
            }

            ui.add_space(4.0);
        }
    });

    ui.add_space(8.0);

    let over_limit = selected.len() > max_discards;
    let confirm_label = if selected.is_empty() {
        "Stand Pat (draw 0)".to_string()
    } else {
        format!("Discard {} card{}", selected.len(), if selected.len() == 1 { "" } else { "s" })
    };

    let mut result = None;

    ui.horizontal(|ui| {
        if ui
            .add_enabled(!over_limit, egui::Button::new(confirm_label))
            .clicked()
        {
            let mut discards = selected.clone();
            discards.sort_unstable();
            result = Some(discards);
        }

        if !selected.is_empty() {
            if ui.button("Clear").clicked() {
                selected.clear();
            }
        }
    });

    result
}

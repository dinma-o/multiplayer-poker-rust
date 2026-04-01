use std::collections::HashMap;

use common::card_deck::Card;

use crate::gui::theme;

use super::card::show_card;

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
    theme::soft_panel_frame().show(ui, |ui| {
        ui.label(theme::small_muted(format!(
            "Select up to {max_discards} card(s). Leave everything unselected to stand pat."
        )));
    });

    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        for (i, &card) in cards.iter().enumerate() {
            let is_selected = selected.contains(&i);

            let stroke = if is_selected {
                egui::Stroke::new(2.0, theme::DANGER)
            } else {
                egui::Stroke::new(1.5, theme::BORDER)
            };

            let inner = egui::Frame::new()
                .fill(if is_selected {
                    theme::DANGER_SOFT
                } else {
                    theme::PANEL_SOFT
                })
                .stroke(stroke)
                .inner_margin(egui::Margin::same(6))
                .corner_radius(18.0)
                .show(ui, |ui| {
                    show_card(ui, ctx, cache, card);
                    if is_selected {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                egui::RichText::new("Marked to discard")
                                    .small()
                                    .color(theme::DANGER)
                                    .strong(),
                            );
                        });
                    }
                });

            let response = inner.response.interact(egui::Sense::click());

            if response.clicked() {
                if is_selected {
                    selected.retain(|&x| x != i);
                } else if selected.len() < max_discards {
                    selected.push(i);
                }
            }

            ui.add_space(4.0);
        }
    });

    ui.add_space(8.0);

    let over_limit = selected.len() > max_discards;
    let confirm_label = if selected.is_empty() {
        "Stand Pat (draw 0)".to_string()
    } else {
        format!(
            "Discard {} card{}",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" }
        )
    };

    let mut result = None;

    ui.horizontal(|ui| {
        if ui
            .add_enabled(!over_limit, primary_button(&confirm_label))
            .clicked()
        {
            let mut discards = selected.clone();
            discards.sort_unstable();
            result = Some(discards);
        }

        if !selected.is_empty() {
            if ui.add(secondary_button("Clear")).clicked() {
                selected.clear();
            }
        }
    });

    result
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

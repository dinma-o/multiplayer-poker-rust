use common::protocol::{AllowedAction, Chips};
use common::wire::{ActionPrompt, PlayerAction};

use crate::gui::theme;

/// Render the action bar for the active player's turn.
///
/// Returns `Some(PlayerAction)` when the player clicks a button, `None` otherwise.
/// Only shown when `action_prompt.max_discards.is_none()` (draw phase uses a
/// separate discard picker).
pub fn show_action_bar(
    ui: &mut egui::Ui,
    prompt: &ActionPrompt,
    bet_input: &mut String,
) -> Option<PlayerAction> {
    let mut chosen: Option<PlayerAction> = None;

    let has_bet = prompt.allowed_actions.contains(&AllowedAction::Bet);
    let has_raise = prompt.allowed_actions.contains(&AllowedAction::Raise);
    let needs_amount = has_bet || has_raise;
    let min = prompt.min_raise.unwrap_or(1);
    let max = prompt.max_amount.unwrap_or(min).max(min);
    let amount_label = if has_bet {
        "Bet amount"
    } else {
        "Raise amount"
    };

    if needs_amount {
        if bet_input.is_empty() {
            *bet_input = min.to_string();
        }

        theme::soft_panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::small_muted(amount_label));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(bet_input.clone())
                            .color(theme::TEXT)
                            .strong(),
                    );
                });
            });

            let mut amount_val = bet_input.parse::<Chips>().unwrap_or(min).clamp(min, max);
            if ui
                .add(
                    egui::Slider::new(&mut amount_val, min..=max)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                )
                .changed()
            {
                *bet_input = amount_val.to_string();
            }

            ui.horizontal(|ui| {
                ui.label(theme::small_muted(format!("Min {}", min)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(theme::small_muted(format!("Max {}", max)));
                });
            });

            let response = ui.add(
                egui::TextEdit::singleline(bet_input)
                    .desired_width(ui.available_width())
                    .hint_text(min.to_string()),
            );

            if response.changed() {
                if let Ok(value) = bet_input.parse::<Chips>() {
                    if value > max {
                        *bet_input = max.to_string();
                    } else if value < min {
                        *bet_input = min.to_string();
                    }
                }
            }
        });
    }

    ui.horizontal_wrapped(|ui| {
        for action in &prompt.allowed_actions {
            let response = match action {
                AllowedAction::Check => {
                    show_action_card(ui, "Check", "Stay in without adding chips", false)
                }
                AllowedAction::Call => {
                    let label = match prompt.call_amount {
                        Some(amount) => format!("Call {}", amount),
                        None => "Call".to_string(),
                    };
                    show_action_card(ui, &label, "Match the current bet", true)
                }
                AllowedAction::Fold => show_action_card(ui, "Fold", "Release the hand", false),
                AllowedAction::Bet => show_action_card(ui, "Bet", "Open the action", true),
                AllowedAction::Raise => {
                    show_action_card(ui, "Raise", "Increase the pressure", true)
                }
                AllowedAction::Draw => continue,
            };

            if response.clicked() {
                chosen = match action {
                    AllowedAction::Check => Some(PlayerAction::Check),
                    AllowedAction::Call => Some(PlayerAction::Call),
                    AllowedAction::Fold => Some(PlayerAction::Fold),
                    AllowedAction::Bet => {
                        let amount = bet_input.parse::<Chips>().unwrap_or(min).clamp(min, max);
                        Some(PlayerAction::Bet { amount })
                    }
                    AllowedAction::Raise => {
                        let amount = bet_input.parse::<Chips>().unwrap_or(min).clamp(min, max);
                        Some(PlayerAction::Raise { amount })
                    }
                    AllowedAction::Draw => None,
                };
            }
        }
    });

    chosen
}

fn show_action_card(
    ui: &mut egui::Ui,
    label: &str,
    subtitle: &str,
    primary: bool,
) -> egui::Response {
    let (fill, stroke, title_color, body_color) = if primary {
        (
            theme::ACCENT_STRONG,
            theme::ACCENT,
            theme::BACKGROUND,
            egui::Color32::from_rgb(7, 32, 24),
        )
    } else {
        (
            theme::PANEL_ALT,
            theme::BORDER,
            theme::TEXT,
            theme::TEXT_MUTED,
        )
    };

    let inner = egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(18.0)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(150.0, 72.0));
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).color(title_color).strong());
                ui.label(egui::RichText::new(subtitle).small().color(body_color));
            });
        });

    inner.response.interact(egui::Sense::click())
}

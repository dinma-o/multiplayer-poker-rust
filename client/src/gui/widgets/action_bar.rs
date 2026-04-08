use common::wire::{ActionPrompt, PlayerAction};
use common::{
    constants::five_card_draw,
    protocol::{AllowedAction, Chips},
};

use crate::gui::theme;

const ACTION_AMOUNT_INCREMENT: Chips = five_card_draw::BET_INCREMENT as Chips;

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
    let (slider_min, slider_max) = valid_amount_bounds(min, max).unwrap_or((min, max));
    let default_amount = normalize_action_amount(None, min, max);
    let amount_label = if has_bet {
        "Bet amount"
    } else {
        "Raise amount"
    };

    if needs_amount {
        if bet_input.is_empty() {
            *bet_input = default_amount.to_string();
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

            let mut amount_val = normalize_action_amount(bet_input.parse::<Chips>().ok(), min, max);
            if ui
                .add(
                    egui::Slider::new(&mut amount_val, slider_min..=slider_max)
                        .show_value(false)
                        .step_by(ACTION_AMOUNT_INCREMENT as f64)
                        .clamping(egui::SliderClamping::Always),
                )
                .changed()
            {
                *bet_input = normalize_action_amount(Some(amount_val), min, max).to_string();
            }

            ui.horizontal(|ui| {
                ui.label(theme::small_muted(format!(
                    "Min {} • step {}",
                    slider_min, ACTION_AMOUNT_INCREMENT
                )));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(theme::small_muted(format!("Max {}", slider_max)));
                });
            });

            let response = ui.add(
                egui::TextEdit::singleline(bet_input)
                    .desired_width(ui.available_width())
                    .hint_text(min.to_string()),
            );

            if response.changed() {
                bet_input.retain(|ch| ch.is_ascii_digit());

                if let Ok(value) = bet_input.parse::<Chips>() {
                    *bet_input = normalize_action_amount(Some(value), min, max).to_string();
                } else if bet_input.is_empty() {
                    *bet_input = default_amount.to_string();
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
                        let amount =
                            normalize_action_amount(bet_input.parse::<Chips>().ok(), min, max);
                        *bet_input = amount.to_string();
                        Some(PlayerAction::Bet { amount })
                    }
                    AllowedAction::Raise => {
                        let amount =
                            normalize_action_amount(bet_input.parse::<Chips>().ok(), min, max);
                        *bet_input = amount.to_string();
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
            ui.set_max_size(egui::vec2(100.0, 48.0));
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).color(title_color).strong());
                ui.label(egui::RichText::new(subtitle).small().color(body_color));
            });
        });

    inner.response.interact(egui::Sense::click())
}

fn normalize_action_amount(amount: Option<Chips>, min: Chips, max: Chips) -> Chips {
    let (min, max) = ordered_bounds(min, max);
    let fallback = valid_amount_bounds(min, max)
        .map(|(first_valid, _)| first_valid)
        .unwrap_or(min);
    let amount = amount.unwrap_or(fallback);

    snap_amount_to_increment(amount, min, max)
}

fn snap_amount_to_increment(amount: Chips, min: Chips, max: Chips) -> Chips {
    let (min, max) = ordered_bounds(min, max);
    let Some((first_valid, last_valid)) = valid_amount_bounds(min, max) else {
        return amount.clamp(min, max);
    };

    let clamped = amount.clamp(first_valid, last_valid);
    let lower = round_down_to_increment(clamped, ACTION_AMOUNT_INCREMENT).max(first_valid);
    let upper = round_up_to_increment(clamped, ACTION_AMOUNT_INCREMENT).min(last_valid);

    if clamped - lower <= upper - clamped {
        lower
    } else {
        upper
    }
}

fn ordered_bounds(min: Chips, max: Chips) -> (Chips, Chips) {
    if min <= max { (min, max) } else { (max, min) }
}

fn valid_amount_bounds(min: Chips, max: Chips) -> Option<(Chips, Chips)> {
    let (min, max) = ordered_bounds(min, max);
    let first_valid = round_up_to_increment(min, ACTION_AMOUNT_INCREMENT);
    let last_valid = round_down_to_increment(max, ACTION_AMOUNT_INCREMENT);

    (first_valid <= last_valid).then_some((first_valid, last_valid))
}

fn round_down_to_increment(amount: Chips, increment: Chips) -> Chips {
    amount - amount.rem_euclid(increment)
}

fn round_up_to_increment(amount: Chips, increment: Chips) -> Chips {
    let remainder = amount.rem_euclid(increment);
    if remainder == 0 {
        amount
    } else {
        amount + (increment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_action_amount;

    #[test]
    fn normalizes_amounts_to_chip_steps() {
        assert_eq!(normalize_action_amount(Some(7), 5, 25), 5);
        assert_eq!(normalize_action_amount(Some(8), 5, 25), 10);
        assert_eq!(normalize_action_amount(Some(23), 5, 25), 25);
    }

    #[test]
    fn clamps_invalid_or_missing_values_into_range() {
        assert_eq!(normalize_action_amount(None, 5, 25), 5);
        assert_eq!(normalize_action_amount(Some(1), 5, 25), 5);
        assert_eq!(normalize_action_amount(Some(27), 5, 25), 25);
    }

    #[test]
    fn finds_valid_increment_even_when_bounds_are_not_aligned() {
        assert_eq!(normalize_action_amount(Some(12), 12, 27), 15);
        assert_eq!(normalize_action_amount(Some(26), 12, 27), 25);
    }
}

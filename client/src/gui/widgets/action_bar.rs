use common::protocol::{AllowedAction, Chips};
use common::wire::{ActionPrompt, PlayerAction};

/// Render the action bar for the active player's turn.
///
/// Returns `Some(PlayerAction)` when the player clicks a button, `None` otherwise.
/// Only shown when `action_prompt.max_discards.is_none()` (draw phase uses a
/// separate discard picker).
pub fn show_action_bar(
    ui: &mut egui::Ui,
    prompt: &ActionPrompt,
    balance: Chips,
    bet_input: &mut String,
) -> Option<PlayerAction> {
    let mut chosen: Option<PlayerAction> = None;

    let has_bet = prompt.allowed_actions.contains(&AllowedAction::Bet);
    let has_raise = prompt.allowed_actions.contains(&AllowedAction::Raise);
    let needs_amount = has_bet || has_raise;
    let min = prompt.min_raise.unwrap_or(1);

    // ── Amount input (only when bet/raise is possible) ────────────────────
    if needs_amount {

        // Seed with min only when empty (start of a new turn).
        // Do not snap mid-type — the player is free to type any value;
        // clamping is enforced when they click Bet / Raise.
        if bet_input.is_empty() {
            *bet_input = min.to_string();
        }

        ui.horizontal(|ui| {
            ui.label("Amount:");

            // Slider
            let mut amount_val = bet_input.parse::<Chips>().unwrap_or(min).clamp(min, balance);
            if ui
                .add(egui::Slider::new(&mut amount_val, min..=balance).clamp_to_range(true))
                .changed()
            {
                *bet_input = amount_val.to_string();
            }

            // Text field override
            let resp = ui.add(
                egui::TextEdit::singleline(bet_input)
                    .desired_width(60.0)
                    .hint_text(min.to_string()),
            );
            if resp.changed() {
                // Clamp on focus loss or if value is out of range
                if let Ok(v) = bet_input.parse::<Chips>() {
                    if v > balance {
                        *bet_input = balance.to_string();
                    }
                }
            }
        });

        ui.add_space(4.0);
    }

    // ── Action buttons ────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        for action in &prompt.allowed_actions {
            match action {
                AllowedAction::Check => {
                    if ui.button("Check").clicked() {
                        chosen = Some(PlayerAction::Check);
                    }
                }
                AllowedAction::Call => {
                    let label = match prompt.call_amount {
                        Some(amt) => format!("Call ${amt}"),
                        None => "Call".to_string(),
                    };
                    if ui.button(label).clicked() {
                        chosen = Some(PlayerAction::Call);
                    }
                }
                AllowedAction::Fold => {
                    if ui.button("Fold").clicked() {
                        chosen = Some(PlayerAction::Fold);
                    }
                }
                AllowedAction::Bet => {
                    if ui.button("Bet").clicked() {
                        let amount = bet_input
                            .parse::<Chips>()
                            .unwrap_or(min)
                            .clamp(min, balance);
                        chosen = Some(PlayerAction::Bet { amount });
                    }
                }
                AllowedAction::Raise => {
                    if ui.button("Raise").clicked() {
                        let amount = bet_input
                            .parse::<Chips>()
                            .unwrap_or(min)
                            .clamp(min, balance);
                        chosen = Some(PlayerAction::Raise { amount });
                    }
                }
                AllowedAction::Draw => {
                    // Draw is handled by the discard picker, not here
                }
            }
        }
    });

    chosen
}

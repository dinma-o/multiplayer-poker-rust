use common::{
    card_deck::Deck,
    constants::five_card_draw::{self, ANTE_AMOUNT},
    hand::Hand,
    hand_rank::{self, rank_five_card_hand},
    protocol::{
        AccountCmd, ActionCmd, AllowedAction, Command, Event, HandStep, LobbyCmd, PlayerId,
        PlayerSummary, ShowdownLine,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::{account_store::AccountStore, game_states::BettingRoundState, table::FiveCardTable};

mod action_ops;
mod auto;
mod betting;
mod draw;
mod events;
mod guards;
mod lobby_ops;
mod payout;
mod showdown;
#[cfg(test)]
mod tests;

pub struct ServerEngine {
    pub accounts: AccountStore,
    pub table: FiveCardTable,
    pub deck: Deck,
    rng_seed: Option<u64>,
    sit_out_next: HashSet<PlayerId>,
}

impl ServerEngine {
    pub fn new(accounts: AccountStore, table: FiveCardTable) -> Self {
        Self {
            accounts,
            table,
            deck: Deck::new_shuffled(),
            rng_seed: None,
            sit_out_next: HashSet::new(),
        }
    }

    pub fn lobby_state_event(&self) -> Event {
        let accounts = self.accounts.list_accounts();
        let mut account_by_id = HashMap::with_capacity(accounts.len());
        for acct in accounts {
            account_by_id.insert(acct.id, acct);
        }

        let mut seated = Vec::new();
        for (seat, player_id) in self.table.seated_players() {
            if let Some(acct) = account_by_id.get(&player_id) {
                seated.push(PlayerSummary {
                    player_id,
                    player_name: acct.name.clone(),
                    seat,
                    balance: acct.balance,
                    games_played: acct.stats.games_played,
                    pots_won: acct.stats.pots_won,
                    folds: acct.stats.folds,
                });
            }
        }

        Event::LobbyState { seated }
    }

    pub fn hand_state_summary_event(&self) -> Event {
        Event::Info {
            message: self.hand_state_summary(),
        }
    }

    fn hand_state_summary(&self) -> String {
        let step = self.table.current_step();
        let mut lines = Vec::new();

        lines.push(format!("Hand state: {:?}", step));
        lines.push(format!("Dealer seat: {}", self.table.dealer_position()));
        lines.push(format!("Pot: {}", self.table.hand_state().pot_size()));
        lines.push(format!(
            "Current bet: {}",
            self.table.hand_state().current_bet()
        ));

        if matches!(step, HandStep::WaitingForPlayers) {
            lines.push("No active hand in progress.".into());
        }

        let seated = self.table.seated_players();
        if seated.is_empty() {
            lines.push("Seated players: none".into());
        } else {
            lines.push("Seated players:".into());
            for (seat, pid) in seated {
                let name = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                let balance = self.accounts.get_by_id(pid).map(|a| a.balance).unwrap_or(0);
                let in_hand = self.table.hand_state().player_hand(&pid).is_some();
                let folded = self.table.hand_state().has_player_folded(&pid);

                let mut tags = Vec::new();
                if in_hand {
                    tags.push("in hand");
                }
                if folded {
                    tags.push("folded");
                }
                if self.sit_out_next.contains(&pid) {
                    tags.push("sitout next");
                }
                if tags.is_empty() {
                    tags.push("seated");
                }

                lines.push(format!(
                    "  seat {}: {} | bal {} | {}",
                    seat,
                    name,
                    balance,
                    tags.join(", ")
                ));
            }
        }

        match step {
            HandStep::BettingRound1 | HandStep::BettingRound2 => {
                if let Some(br) = self.table.hand_state().betting() {
                    let pid = br.current_player();
                    let pname = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                    if let Event::ActionRequested {
                        allowed,
                        call_amount,
                        min_raise,
                        ..
                    } = self.action_requested_event_for(pid, &pname)
                    {
                        let allow_call = allowed.iter().any(|a| matches!(a, AllowedAction::Call));
                        let allow_raise = allowed.iter().any(|a| matches!(a, AllowedAction::Raise));
                        let call_text = if allow_call {
                            Self::format_opt_chips(call_amount)
                        } else {
                            "n/a".into()
                        };
                        let min_raise_text = if allow_raise {
                            Self::format_opt_chips(min_raise)
                        } else {
                            "n/a".into()
                        };

                        lines.push(format!(
                            "To act: {} | allowed: {} | call: {} | min_raise: {}",
                            pname,
                            Self::format_allowed_actions(&allowed),
                            call_text,
                            min_raise_text
                        ));
                    } else {
                        lines.push(format!("To act: {}", pname));
                    }
                } else {
                    lines.push("Betting round not initialized.".into());
                }
            }
            HandStep::DrawPhase => {
                if let Some(dr) = self.table.hand_state().draw() {
                    let pid = dr.current_player();
                    let pname = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                    lines.push(format!(
                        "To draw: {} | max_discards: {}",
                        pname,
                        five_card_draw::MAX_DISCARDS
                    ));
                } else {
                    lines.push("Draw round not initialized.".into());
                }
            }
            _ => {}
        }

        lines.join("\n")
    }

    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::Account(ac) => self.apply_account_cmd(ac),
            Command::Lobby(lc) => self.apply_lobby_cmd(lc),
            Command::Action(ac) => self.apply_action_cmd(ac),
        }
    }

    pub fn apply_account_cmd(&mut self, cmd: AccountCmd) -> Vec<Event> {
        match cmd {
            AccountCmd::Create { name, pin, amount } => {
                match self.accounts.create_account(&name, &pin, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account creation failed: {:?}", e),
                    }],
                }
            }

            AccountCmd::Delete { player_name, pin } => {
                match self.accounts.delete_account(&player_name, &pin) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account deletion failed: {:?}", e),
                    }],
                }
            }

            AccountCmd::Deposit {
                player_name,
                amount,
            } => match self.accounts.deposit(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Deposit failed: {:?}", e),
                }],
            },

            AccountCmd::Withdraw {
                player_name,
                amount,
            } => match self.accounts.withdraw(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Withdraw failed: {:?}", e),
                }],
            },

            AccountCmd::Get { player_name } => match self.accounts.get_by_name(&player_name) {
                Ok(acct) => {
                    let summary = acct.summary();
                    vec![Event::Info {
                        message: format!("Account info:\n{summary}"),
                    }]
                }
                Err(e) => vec![Event::Error {
                    message: format!("Get account failed: {:?}", e),
                }],
            },
        }
    }

    pub fn has_enough_for_ante(&self, player_id: PlayerId) -> bool {
        match self.accounts.get_by_id(player_id) {
            Ok(acct) => acct.balance >= five_card_draw::ANTE_AMOUNT.try_into().unwrap_or(5),
            Err(_) => false,
        }
    }

    fn format_allowed_actions(allowed: &[AllowedAction]) -> String {
        let items: Vec<&'static str> = allowed
            .iter()
            .map(|a| match a {
                AllowedAction::Check => "check",
                AllowedAction::Bet => "bet",
                AllowedAction::Call => "call",
                AllowedAction::Raise => "raise",
                AllowedAction::Fold => "fold",
                AllowedAction::Draw => "draw",
            })
            .collect();
        if items.is_empty() {
            "none".into()
        } else {
            items.join(", ")
        }
    }

    fn format_opt_chips(v: Option<i64>) -> String {
        match v {
            Some(amount) => amount.to_string(),
            None => "n/a".into(),
        }
    }
}


use common::{
    card_deck::Deck,
    constants::{five_card_draw, seven_card_stud},
    hand_rank::{self},
    protocol::{
        AccountCmd, ActionCmd, AllowedAction, Command, Event, HandStep, LobbyCmd, PlayerId,
        PlayerSummary, ShowdownLine,
    },
    wire::ActionPrompt,
};
use rand::{SeedableRng, rngs::StdRng};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{
    account_store::{AccountError, AccountStore},
    accounts::Account,
    game_states::BettingRoundState,
    table::PokerTable,
    variants,
};

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

pub type SharedAccountStore = Arc<Mutex<AccountStore>>;

pub struct GameEngine {
    pub accounts: SharedAccountStore,
    pub table: PokerTable,
    pub deck: Deck,
    rng_seed: Option<u64>,
    sit_out_next: HashSet<PlayerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StudStreet {
    Third,
    Fourth,
    Fifth,
    Sixth,
    Seventh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StudVisibleRank {
    category: u8,
    tiebreakers: [u8; 4],
}

impl GameEngine {
    pub fn new(accounts: SharedAccountStore, table: PokerTable) -> Self {
        Self {
            accounts,
            table,
            deck: Deck::new_shuffled(),
            rng_seed: None,
            sit_out_next: HashSet::new(),
        }
    }

    fn lock_accounts(&self) -> MutexGuard<'_, AccountStore> {
        self.accounts.lock().expect("account store lock poisoned")
    }

    fn with_accounts<R>(&self, f: impl FnOnce(&AccountStore) -> R) -> R {
        let accounts = self.lock_accounts();
        f(&accounts)
    }

    fn with_accounts_mut<R>(&self, f: impl FnOnce(&mut AccountStore) -> R) -> R {
        let mut accounts = self.lock_accounts();
        f(&mut accounts)
    }

    fn list_accounts(&self) -> Vec<Account> {
        self.with_accounts(|accounts| accounts.list_accounts().to_vec())
    }

    fn account_by_name(&self, player_name: &str) -> Result<Account, AccountError> {
        self.with_accounts(|accounts| accounts.get_by_name(player_name).cloned())
    }

    fn account_by_id(&self, player_id: PlayerId) -> Result<Account, AccountError> {
        self.with_accounts(|accounts| accounts.get_by_id(player_id).cloned())
    }

    fn account_name(&self, player_id: PlayerId) -> String {
        self.with_accounts(|accounts| accounts.name_from_id(player_id))
            .unwrap_or("Unknown".into())
    }

    pub fn action_prompt_for_player(&self, player_id: PlayerId) -> Option<ActionPrompt> {
        match self.table.current_step() {
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => {
                let current_player = self.table.hand_state().betting()?.current_player();
                if current_player != player_id {
                    return None;
                }

                let player_name = self.account_name(player_id);
                match self.action_requested_event_for(player_id, &player_name) {
                    Event::ActionRequested {
                        allowed,
                        call_amount,
                        min_raise,
                        ..
                    } => Some(ActionPrompt {
                        allowed_actions: allowed,
                        call_amount,
                        min_raise,
                        max_discards: None,
                    }),
                    _ => None,
                }
            }
            HandStep::DrawPhase => {
                let current_player = self.table.hand_state().draw()?.current_player();
                if current_player != player_id {
                    return None;
                }

                Some(ActionPrompt {
                    allowed_actions: vec![AllowedAction::Draw],
                    call_amount: None,
                    min_raise: None,
                    max_discards: Some(five_card_draw::MAX_DISCARDS),
                })
            }
            _ => None,
        }
    }

    fn is_holdem_variant(&self) -> bool {
        self.table.variant_spec() == crate::variants::texas_holdem::SPEC
    }

    fn is_stud_variant(&self) -> bool {
        self.table.variant_spec() == crate::variants::seven_card_stud::SPEC
    }

    fn variant_min_players(&self) -> usize {
        match self.table.variant_spec() {
            spec if spec == crate::variants::seven_card_stud::SPEC => {
                seven_card_stud::MIN_PLAYERS as usize
            }
            _ => five_card_draw::MIN_PLAYERS as usize,
        }
    }

    fn variant_max_players(&self) -> usize {
        match self.table.variant_spec() {
            spec if spec == crate::variants::seven_card_stud::SPEC => {
                seven_card_stud::MAX_PLAYERS as usize
            }
            _ => self.table.variant_spec().max_players,
        }
    }

    fn variant_ante_amount(&self) -> Option<u32> {
        match self.table.variant_spec() {
            spec if spec == crate::variants::texas_holdem::SPEC => None,
            spec if spec == crate::variants::seven_card_stud::SPEC => {
                Some(seven_card_stud::ANTE_AMOUNT)
            }
            _ => Some(five_card_draw::ANTE_AMOUNT),
        }
    }

    fn current_stud_street(&self) -> Option<StudStreet> {
        if !self.is_stud_variant() {
            return None;
        }

        match self.table.current_step() {
            HandStep::BettingRound1 => Some(StudStreet::Third),
            HandStep::BettingRound2 => Some(StudStreet::Fourth),
            HandStep::BettingRound3 => Some(StudStreet::Fifth),
            HandStep::BettingRound4 => Some(StudStreet::Sixth),
            HandStep::BettingRound5 => Some(StudStreet::Seventh),
            _ => None,
        }
    }

    fn stud_opening_bet_size(&self) -> Option<u32> {
        match self.current_stud_street()? {
            StudStreet::Third | StudStreet::Fourth => Some(seven_card_stud::SMALL_BET),
            StudStreet::Fifth | StudStreet::Sixth | StudStreet::Seventh => {
                Some(seven_card_stud::BIG_BET)
            }
        }
    }

    fn stud_raise_by_amount(&self, current_bet: u32) -> Option<u32> {
        let opening_bet = self.stud_opening_bet_size()?;
        if self.current_stud_street() == Some(StudStreet::Third)
            && current_bet == seven_card_stud::BRING_IN_AMOUNT
        {
            return Some(opening_bet.saturating_sub(seven_card_stud::BRING_IN_AMOUNT));
        }

        Some(opening_bet)
    }

    fn max_raises_for_current_variant(&self) -> u32 {
        if self.is_stud_variant() {
            seven_card_stud::MAX_RAISES_PER_ROUND
        } else {
            five_card_draw::MAX_RAISES_PER_ROUND
        }
    }

    fn active_players_for_current_betting_round(&self) -> Vec<PlayerId> {
        let Some(br) = self.table.hand_state().betting() else {
            return self.active_players_in_hand();
        };

        br.order
            .iter()
            .copied()
            .filter(|pid| self.table.is_active_player(*pid))
            .collect()
    }

    fn stud_visible_rank(cards: &[common::card_deck::Card]) -> StudVisibleRank {
        use std::collections::HashMap;

        let mut counts: HashMap<u8, u8> = HashMap::new();
        for card in cards {
            *counts.entry(card.rank() as u8).or_insert(0) += 1;
        }

        let mut groups: Vec<(u8, u8)> =
            counts.iter().map(|(&rank, &count)| (count, rank)).collect();
        groups.sort_unstable_by(|a, b| b.cmp(a));

        let singles_desc = |skip_ranks: &[u8]| -> Vec<u8> {
            let mut singles: Vec<u8> = cards
                .iter()
                .map(|c| c.rank() as u8)
                .filter(|rank| !skip_ranks.contains(rank))
                .collect();
            singles.sort_unstable_by(|a, b| b.cmp(a));
            singles
        };

        if let Some(&(4, quad_rank)) = groups.first() {
            return StudVisibleRank {
                category: 4,
                tiebreakers: [quad_rank, 0, 0, 0],
            };
        }

        if let Some(&(3, trip_rank)) = groups.first() {
            let kickers = singles_desc(&[trip_rank]);
            return StudVisibleRank {
                category: 3,
                tiebreakers: [
                    trip_rank,
                    *kickers.get(0).unwrap_or(&0),
                    *kickers.get(1).unwrap_or(&0),
                    0,
                ],
            };
        }

        if groups.len() >= 2 && groups[0].0 == 2 && groups[1].0 == 2 {
            let high_pair = groups[0].1.max(groups[1].1);
            let low_pair = groups[0].1.min(groups[1].1);
            let kicker = singles_desc(&[high_pair, low_pair])
                .into_iter()
                .next()
                .unwrap_or(0);
            return StudVisibleRank {
                category: 2,
                tiebreakers: [high_pair, low_pair, kicker, 0],
            };
        }

        if let Some(&(2, pair_rank)) = groups.first() {
            let kickers = singles_desc(&[pair_rank]);
            return StudVisibleRank {
                category: 1,
                tiebreakers: [
                    pair_rank,
                    *kickers.get(0).unwrap_or(&0),
                    *kickers.get(1).unwrap_or(&0),
                    *kickers.get(2).unwrap_or(&0),
                ],
            };
        }

        let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank() as u8).collect();
        ranks.sort_unstable_by(|a, b| b.cmp(a));
        StudVisibleRank {
            category: 0,
            tiebreakers: [
                *ranks.get(0).unwrap_or(&0),
                *ranks.get(1).unwrap_or(&0),
                *ranks.get(2).unwrap_or(&0),
                *ranks.get(3).unwrap_or(&0),
            ],
        }
    }

    fn stud_visible_category_name(cards: &[common::card_deck::Card]) -> &'static str {
        match Self::stud_visible_rank(cards).category {
            4 => "four of a kind",
            3 => "three of a kind",
            2 => "two pair",
            1 => "one pair",
            _ => "high card",
        }
    }

    pub fn lobby_state_event(&self) -> Event {
        let accounts = self.list_accounts();
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
                let name = self.account_name(pid);
                let balance = self.account_by_id(pid).map(|a| a.balance).unwrap_or(0);
                let in_hand = self.table.hand_state().player_hole_cards(&pid).is_some();
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

        if self.is_holdem_variant() && !self.table.hand_state().board_cards().is_empty() {
            let board = self
                .table
                .hand_state()
                .board_cards()
                .iter()
                .map(|card| card.to_string())
                .collect::<Vec<_>>();
            lines.push(format!("Board: {:?}", board));
        }

        if self.is_stud_variant() {
            let mut stud_lines = Vec::new();
            for (_seat, pid) in self.table.seated_players() {
                if let Some(up_cards) = self.table.hand_state().player_up_cards(&pid) {
                    if !up_cards.is_empty() {
                        let name = self.account_name(pid);
                        let cards = up_cards
                            .iter()
                            .map(|card| card.to_string())
                            .collect::<Vec<_>>();
                        let desc = Self::stud_visible_category_name(up_cards);
                        stud_lines.push(format!("  {}: {:?} | visible {}", name, cards, desc));
                    }
                }
            }

            if !stud_lines.is_empty() {
                lines.push("Visible up cards:".into());
                lines.extend(stud_lines);
            }
        }

        match step {
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => {
                if let Some(br) = self.table.hand_state().betting() {
                    let pid = br.current_player();
                    let pname = self.account_name(pid);
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
                    let pname = self.account_name(pid);
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
            AccountCmd::Create { name, pin, amount } => match self
                .with_accounts_mut(|accounts| accounts.create_account(&name, &pin, amount))
            {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Account creation failed: {:?}", e),
                }],
            },

            AccountCmd::Delete { player_name, pin } => match self
                .with_accounts_mut(|accounts| accounts.delete_account(&player_name, &pin))
            {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Account deletion failed: {:?}", e),
                }],
            },

            AccountCmd::Deposit {
                player_name,
                amount,
            } => match self.with_accounts_mut(|accounts| accounts.deposit(&player_name, amount)) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Deposit failed: {:?}", e),
                }],
            },

            AccountCmd::Withdraw {
                player_name,
                amount,
            } => match self.with_accounts_mut(|accounts| accounts.withdraw(&player_name, amount)) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Withdraw failed: {:?}", e),
                }],
            },

            AccountCmd::Get { player_name } => match self.account_by_name(&player_name) {
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

    pub fn has_enough_to_start_hand(&self, player_id: PlayerId) -> bool {
        match self.account_by_id(player_id) {
            Ok(acct) => match self.table.variant_spec() {
                spec if spec == variants::seven_card_stud::SPEC => {
                    acct.balance
                        >= (seven_card_stud::ANTE_AMOUNT + seven_card_stud::BRING_IN_AMOUNT) as i64
                }
                _ => match self.variant_ante_amount() {
                    Some(amount) => acct.balance >= amount as i64,
                    None => true,
                },
            },
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

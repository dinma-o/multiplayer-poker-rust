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

    pub fn apply_lobby_cmd(&mut self, cmd: LobbyCmd) -> Vec<Event> {
        match cmd {
            LobbyCmd::SetVariant { variant } => {
                // Nothing fancy yet
                vec![Event::Info {
                    message: format!("Set poker variant to {:?}", variant),
                }]
            }

            LobbyCmd::GetHandState => vec![self.hand_state_summary_event()],

            LobbyCmd::ViewHand { player_name } => {
                let pid = match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => pa.id,
                    Err(e) => {
                        return vec![Event::Error {
                            message: format!("Get account failed: {:?}", e),
                        }];
                    }
                };

                let hand = match self.table.hand_state().player_hand(&pid) {
                    Some(h) => h.clone(),
                    None => {
                        return vec![Event::Error {
                            message: format!("No active hand for {}", player_name),
                        }];
                    }
                };

                vec![Event::PrivateHand {
                    player_name,
                    cards: hand.cards().to_vec(),
                }]
            }

            LobbyCmd::SetRngSeed { seed } => {
                self.rng_seed = Some(seed);
                vec![Event::Info {
                    message: format!("RNG seed set to {}", seed),
                }]
            }

            LobbyCmd::ClearRngSeed => {
                self.rng_seed = None;
                vec![Event::Info {
                    message: "RNG seed cleared (random shuffle)".into(),
                }]
            }

            LobbyCmd::SitOutNext { player_name } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error {
                        message: "Cannot sit out during a hand".into(),
                    }];
                }

                let pa = match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => pa,
                    Err(e) => {
                        return vec![Event::Error {
                            message: format!("Get account failed: {:?}", e),
                        }];
                    }
                };

                let seated = self.table.seated_players();
                if !seated.iter().any(|(_, pid)| *pid == pa.id) {
                    return vec![Event::Error {
                        message: format!("Player {} is not seated", pa.name),
                    }];
                }

                self.sit_out_next.insert(pa.id);
                vec![Event::Info {
                    message: format!("{} will sit out next hand", pa.name),
                }]
            }

            LobbyCmd::SitIn { player_name } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error {
                        message: "Cannot change sit-out during a hand".into(),
                    }];
                }

                let pa = match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => pa,
                    Err(e) => {
                        return vec![Event::Error {
                            message: format!("Get account failed: {:?}", e),
                        }];
                    }
                };

                self.sit_out_next.remove(&pa.id);
                vec![Event::Info {
                    message: format!("{} will play the next hand", pa.name),
                }]
            }

            LobbyCmd::GetLobbyState => {
                vec![self.lobby_state_event()]
            }

            LobbyCmd::Join { player_name, pin } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error {
                        message: "Cannot join table during hand".into(),
                    }];
                }

                match self.accounts.login(&player_name, &pin) {
                    Ok(pa) => match self.table.join_table(pa.id) {
                        Ok(seat) => vec![
                            Event::PlayerJoined {
                                player_id: pa.id,
                                player_name: pa.name.clone(),
                                seat: seat,
                            },
                            self.lobby_state_event(),
                        ],
                        Err(e) => vec![Event::Error {
                            message: format!("Seating failed: {:?}", e),
                        }],
                    },
                    Err(e) => {
                        vec![Event::Error {
                            message: format!("Login failed: {:?}", e),
                        }]
                    }
                }
            }

            LobbyCmd::Leave { player_name } => {
                let (pid, pname) = match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => (pa.id, pa.name.clone()),
                    Err(e) => {
                        return vec![Event::Error {
                            message: format!("Get account failed: {:?}", e),
                        }];
                    }
                };

                if self.table.current_step() == HandStep::WaitingForPlayers {
                    self.sit_out_next.remove(&pid);
                    match self.table.leave_table(pid) {
                        Ok(_) => vec![
                            Event::PlayerLeft {
                                player_id: pid,
                                player_name: pname.clone(),
                            },
                            self.lobby_state_event(),
                        ],
                        Err(e) => vec![Event::Error {
                            message: format!("Leave failed: {:?}", e),
                        }],
                    }
                } else {
                    self.handle_midhand_leave(pid, &pname)
                }
            }

            LobbyCmd::StartHand => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error {
                        message: "Hand already in progress".into(),
                    }];
                }

                let seated = self.table.seated_players();
                let eligible: Vec<PlayerId> = seated
                    .iter()
                    .map(|(_, pid)| *pid)
                    .filter(|pid| !self.sit_out_next.contains(pid))
                    .collect();

                if eligible.len() < (five_card_draw::MIN_PLAYERS as usize)
                    || eligible.len() > (five_card_draw::MAX_PLAYERS as usize)
                {
                    return vec![Event::Error {
                        message: "need 2–5 players to start".into(),
                    }];
                }

                // Confirm that each player has enough balance to cover the ante
                for player_id in &eligible {
                    if !self.has_enough_for_ante(*player_id) {
                        return vec![Event::Error {
                            message: format!(
                                "Player {} does not have enough balance for ante",
                                player_id
                            ),
                        }];
                    }
                }

                // Ensure dealer is on a valid seat before action order is used
                self.table.ensure_dealer_in_seats();

                self.deck = Deck::new();
                if let Some(seed) = self.rng_seed {
                    let mut rng = StdRng::seed_from_u64(seed);
                    self.deck.shuffle_with_rng(&mut rng);
                } else {
                    self.deck.shuffle();
                }

                self.table.advance_table_state();
                let mut events = vec![Event::StepChanged {
                    step: self.table.current_step(),
                }];

                events.extend(self.run_automatic_steps());

                events
            }

            _ => vec![Event::Error {
                message: "Lobby command not implemented yet".into(),
            }],
        }
    }

    pub fn apply_action_cmd(&mut self, cmd: ActionCmd) -> Vec<Event> {
        // 1) Must be in a decision step that accepts actions
        match self.table.current_step() {
            HandStep::BettingRound1 | HandStep::BettingRound2 => self.apply_betting_action(cmd),
            HandStep::DrawPhase => self.apply_draw_action(cmd), // later
            _ => vec![Event::Error {
                message: "Actions not allowed in current step".into(),
            }],
        }
    }

    fn apply_betting_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
        let mut events = Vec::new();

        // Resolve name -> pid
        let (pid, pname) = match self.resolve_player_from_action(&cmd) {
            Ok(x) => x,
            Err(e) => {
                return vec![e];
            }
        };

        // Must have betting state
        let current_player = match self.table.hand_state().betting() {
            Some(br) => br.current_player(),
            None => {
                return vec![Event::Error {
                    message: "Betting round not initialized".into(),
                }];
            }
        };

        if pid != current_player {
            return vec![Event::Error {
                message: format!(
                    "Not your turn. Expected {}",
                    self.accounts
                        .name_from_id(current_player)
                        .unwrap_or("Unknown".into())
                ),
            }];
        }

        // Active players in order (seated + not folded)
        let active = self.active_players_in_order();

        // Pull current bet + contrib + call amount
        let current_bet = self.table.hand_state().current_bet();
        let contrib = self
            .table
            .hand_state()
            .betting()
            .unwrap()
            .contrib
            .get(&pid)
            .copied()
            .unwrap_or(0);
        let call_amount = current_bet.saturating_sub(contrib);

        // Apply action
        match cmd {
            ActionCmd::Check { .. } => {
                if call_amount != 0 {
                    return vec![Event::Error {
                        message: "Cannot check when facing a bet".into(),
                    }];
                }
                self.table
                    .hand_state_mut()
                    .betting_mut()
                    .unwrap()
                    .pending
                    .remove(&pid);
            }

            ActionCmd::Fold { .. } => {
                // mark folded
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(pid, true);
                self.table
                    .hand_state_mut()
                    .betting_mut()
                    .unwrap()
                    .pending
                    .remove(&pid);

                // EARLY WIN: if only 1 active player remains, end round + fast-forward
                let active = self.active_players_in_hand();
                if active.len() == 1 {
                    self.table.hand_state_mut().end_betting_round();

                    // Skip straight to Showdown/Payout
                    while self.table.current_step() != HandStep::Showdown
                        && self.table.current_step() != HandStep::Payout
                    {
                        self.table.advance_table_state();
                    }
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });

                    events.extend(self.run_automatic_steps());
                    return events;
                }
            }

            ActionCmd::Bet { amount, .. } => {
                if current_bet != 0 {
                    return vec![Event::Error {
                        message: "Cannot bet after betting has started; use raise".into(),
                    }];
                }
                // validate amount min/max here
                if amount <= 0 {
                    return vec![Event::Error {
                        message: "Bet amount must be positive".into(),
                    }];
                }
                if amount < (five_card_draw::MIN_BET_PER_ROUND as i64)
                    || amount > (five_card_draw::MAX_BET_PER_ROUND as i64)
                {
                    return vec![Event::Error {
                        message: format!(
                            "Bet amount must be between {} and {}",
                            five_card_draw::MIN_BET_PER_ROUND,
                            five_card_draw::MAX_BET_PER_ROUND
                        ),
                    }];
                }
                if (amount as u32) % five_card_draw::BET_INCREMENT != 0 {
                    return vec![Event::Error {
                        message: format!(
                            "Bet amount must be a multiple of {}",
                            five_card_draw::BET_INCREMENT
                        ),
                    }];
                }

                // withdraw from accounts
                if let Err(e) = self.accounts.withdraw(&pname, amount as i64) {
                    return vec![Event::Error {
                        message: format!("Withdraw failed: {:?}", e),
                    }];
                }

                // update pot + current bet + contrib
                self.table.hand_state_mut().add_to_pot(amount as u32);
                self.table.hand_state_mut().set_current_bet(amount as u32);

                let br = self.table.hand_state_mut().betting_mut().unwrap();
                br.contrib.insert(pid, amount as u32);
                br.pending = active.iter().cloned().collect();
                br.pending.remove(&pid);
            }

            ActionCmd::Call { .. } => {
                if call_amount == 0 {
                    return vec![Event::Error {
                        message: "Nothing to call".into(),
                    }];
                }

                if let Err(e) = self.accounts.withdraw(&pname, call_amount as i64) {
                    return vec![Event::Error {
                        message: format!("Withdraw failed: {:?}", e),
                    }];
                }

                self.table.hand_state_mut().add_to_pot(call_amount);
                let br = self.table.hand_state_mut().betting_mut().unwrap();
                let new_contrib = br.contrib.get(&pid).copied().unwrap_or(0) + call_amount;
                br.contrib.insert(pid, new_contrib);
                br.pending.remove(&pid);
            }

            ActionCmd::Raise { amount, .. } => {
                if current_bet == 0 {
                    return vec![Event::Error {
                        message: "Cannot raise when no bet exists; use bet".into(),
                    }];
                }
                if amount <= 0 {
                    return vec![Event::Error {
                        message: "Raise amount must be positive".into(),
                    }];
                }

                // raise cap
                {
                    let br = self.table.hand_state().betting().unwrap();
                    if br.raises_used >= five_card_draw::MAX_RAISES_PER_ROUND {
                        return vec![Event::Error {
                            message: "Raise cap reached".into(),
                        }];
                    }
                }

                // interpret as raise-by amount
                let raise_by = amount as u32;
                if raise_by < five_card_draw::MIN_RAISE_AMOUNT {
                    return vec![Event::Error {
                        message: format!(
                            "Raise amount cannot be less than {}",
                            five_card_draw::MIN_RAISE_AMOUNT
                        ),
                    }];
                }
                if raise_by % five_card_draw::RAISE_INCREMENT != 0 {
                    return vec![Event::Error {
                        message: format!(
                            "Raise amount must be a multiple of {}",
                            five_card_draw::RAISE_INCREMENT
                        ),
                    }];
                }
                let new_bet = current_bet + raise_by;
                if new_bet > five_card_draw::MAX_RAISE_TO_AMOUNT {
                    return vec![Event::Error {
                        message: format!(
                            "Resulting bet cannot exceed {}",
                            five_card_draw::MAX_RAISE_TO_AMOUNT
                        ),
                    }];
                }

                let br_contrib = self
                    .table
                    .hand_state()
                    .betting()
                    .unwrap()
                    .contrib
                    .get(&pid)
                    .copied()
                    .unwrap_or(0);
                let delta = new_bet.saturating_sub(br_contrib);

                if let Err(e) = self.accounts.withdraw(&pname, delta as i64) {
                    return vec![Event::Error {
                        message: format!("Withdraw failed: {:?}", e),
                    }];
                }

                self.table.hand_state_mut().add_to_pot(delta);
                self.table.hand_state_mut().set_current_bet(new_bet);

                let br = self.table.hand_state_mut().betting_mut().unwrap();
                br.contrib.insert(pid, new_bet);
                br.raises_used += 1;
                br.pending = active.iter().cloned().collect();
                br.pending.remove(&pid);
            }

            _ => {
                return vec![Event::Error {
                    message: "Unsupported action in betting round".into(),
                }];
            }
        }

        // Emit pot update after money actions (optional)
        events.push(Event::PotUpdated {
            pot: self.table.hand_state().pot_size() as i64,
        });

        // Round complete?
        let pending_empty = self
            .table
            .hand_state()
            .betting()
            .unwrap()
            .pending
            .is_empty();
        if pending_empty {
            self.table.hand_state_mut().end_betting_round();
            self.table.advance_table_state();
            events.push(Event::StepChanged {
                step: self.table.current_step(),
            });

            // Let automatic runner take it to the next prompt (draw phase / betting2)
            events.extend(self.run_automatic_steps());
            return events;
        }

        let seated_set: HashSet<PlayerId> = self
            .table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();

        let folded_set: HashSet<PlayerId> = seated_set
            .iter()
            .copied()
            .filter(|pid| self.table.hand_state().has_player_folded(pid))
            .collect();

        let is_active = |pid: PlayerId| seated_set.contains(&pid) && !folded_set.contains(&pid);

        // Move to next actor after a successful action
        {
            let br = self.table.hand_state_mut().betting_mut().unwrap();
            Self::advance_to_next_actor_local(br, &is_active);
        }

        let br = self.table.hand_state_mut().betting_mut().unwrap();
        while !br.pending.is_empty() && !is_active(br.current_player()) {
            Self::advance_to_next_actor_local(br, &is_active);
        }

        // Prompt next player
        let next_pid = self.table.hand_state().betting().unwrap().current_player();
        let next_name = self
            .accounts
            .name_from_id(next_pid)
            .unwrap_or("Unknown".into());
        events.push(self.action_requested_event_for(next_pid, &next_name));

        events
    }

    pub fn apply_draw_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
        let mut events = Vec::new();

        // Check that we are in a draw phase/step
        if self.table.current_step() != HandStep::DrawPhase {
            return vec![Event::Error {
                message: "Not in draw phase".into(),
            }];
        }

        // Resolve name -> pid
        let (player_name, discard) = match cmd {
            ActionCmd::Draw {
                player_name,
                discard,
            } => (player_name, discard),
            _ => {
                return vec![Event::Error {
                    message: "Only Draw is allowed in DrawPhase".into(),
                }];
            }
        };

        let pid = match self.accounts.get_by_name(&player_name) {
            Ok(acct) => acct.id,
            Err(e) => {
                return vec![Event::Error {
                    message: format!("Get account failed: {:?}", e),
                }];
            }
        };

        let expected = match self.table.hand_state().draw() {
            Some(dr) => dr.current_player(),
            None => {
                return vec![Event::Error {
                    message: "Draw round not initialized".into(),
                }];
            }
        };

        if pid != expected {
            let expected_name = self
                .accounts
                .name_from_id(expected)
                .unwrap_or("Unknown".into());
            return vec![Event::Error {
                message: format!("Not your draw turn. Expected {expected_name}"),
            }];
        }

        // Validate discard list
        if discard.len() > five_card_draw::MAX_DISCARDS {
            return vec![Event::Error {
                message: "Too many discards".into(),
            }];
        }

        let mut seen = std::collections::HashSet::new();
        for &i in &discard {
            if i >= 5 {
                return vec![Event::Error {
                    message: "Discard index out of range (0..4)".into(),
                }];
            }
            if !seen.insert(i) {
                return vec![Event::Error {
                    message: "Duplicate discard index".into(),
                }];
            }
        }

        let hand = match self.table.hand_state().player_hand(&pid) {
            Some(h) => h.clone(),
            None => {
                return vec![Event::Error {
                    message: "Player hand not found".into(),
                }];
            }
        };

        let mut new_hand = hand.clone();
        new_hand.replace_from_deck(&discard, &mut self.deck);

        // Store updated hand
        self.table
            .hand_state_mut()
            .set_player_hand(pid, new_hand.clone());

        // Emit private hand event
        events.push(Event::PrivateHand {
            player_name: player_name.clone(),
            cards: new_hand.cards().to_vec(),
        });

        // Advance draw turn
        {
            let dr = self.table.hand_state_mut().draw_mut().unwrap();
            dr.advance();
            if dr.is_done() {
                self.table.hand_state_mut().end_draw_round();

                // Move to BettingRound2
                self.table.advance_table_state();
                events.push(Event::StepChanged {
                    step: self.table.current_step(),
                });

                // Initialize betting round 2 and prompt
                events.extend(self.enter_betting_round());
                return events;
            }
        }

        // Prompt next drawer
        let next_pid = self.table.hand_state().draw().unwrap().current_player();
        let next_name = self
            .accounts
            .name_from_id(next_pid)
            .unwrap_or("Unknown".into());

        events.push(Event::DrawRequested {
            player_name: next_name,
            max_discards: five_card_draw::MAX_DISCARDS,
        });

        events
    }

    fn handle_midhand_leave(&mut self, pid: PlayerId, pname: &str) -> Vec<Event> {
        let mut events = Vec::new();

        let is_seated = self
            .table
            .seated_players()
            .iter()
            .any(|(_, spid)| *spid == pid);
        if !is_seated {
            return vec![Event::Error {
                message: format!("Player {} is not seated", pname),
            }];
        }

        // Mark folded and queue leave for end of hand
        self.table
            .hand_state_mut()
            .set_player_fold_status(pid, true);
        if let Some(br) = self.table.hand_state_mut().betting_mut() {
            br.pending.remove(&pid);
        }

        let mut draw_removed_current = false;
        let mut draw_done = false;
        let mut next_draw_pid: Option<PlayerId> = None;
        {
            if let Some(dr) = self.table.hand_state_mut().draw_mut() {
                draw_removed_current = dr.remove_player(pid);
                draw_done = dr.is_done();
                if draw_removed_current && !draw_done {
                    next_draw_pid = Some(dr.current_player());
                }
            }
        }

        let _ = self.table.leave_table(pid); // queues leave if round in progress
        self.sit_out_next.remove(&pid);

        events.push(Event::Info {
            message: format!("{pname} disconnected: folded and will leave after the hand"),
        });

        // EARLY WIN: if only 1 active player remains, end hand
        let active = self.active_players_in_hand();
        if active.len() == 1 {
            self.table.hand_state_mut().end_betting_round();

            while self.table.current_step() != HandStep::Showdown
                && self.table.current_step() != HandStep::Payout
            {
                self.table.advance_table_state();
            }
            events.push(Event::StepChanged {
                step: self.table.current_step(),
            });
            events.extend(self.run_automatic_steps());
            return events;
        }

        match self.table.current_step() {
            HandStep::BettingRound1 | HandStep::BettingRound2 => {
                if let Some(br) = self.table.hand_state().betting() {
                    if br.pending.is_empty() {
                        self.table.hand_state_mut().end_betting_round();
                        self.table.advance_table_state();
                        events.push(Event::StepChanged {
                            step: self.table.current_step(),
                        });
                        events.extend(self.run_automatic_steps());
                        return events;
                    }

                    if br.current_player() == pid {
                        let seated_set: HashSet<PlayerId> = self
                            .table
                            .seated_players()
                            .iter()
                            .map(|(_, pid)| *pid)
                            .collect();

                        let folded_set: HashSet<PlayerId> = seated_set
                            .iter()
                            .copied()
                            .filter(|pid| self.table.hand_state().has_player_folded(pid))
                            .collect();

                        let is_active =
                            |pid: PlayerId| seated_set.contains(&pid) && !folded_set.contains(&pid);

                        let br = self.table.hand_state_mut().betting_mut().unwrap();
                        Self::advance_to_next_actor_local(br, &is_active);
                        while !br.pending.is_empty() && !is_active(br.current_player()) {
                            Self::advance_to_next_actor_local(br, &is_active);
                        }

                        let next_pid = br.current_player();
                        let next_name = self
                            .accounts
                            .name_from_id(next_pid)
                            .unwrap_or("Unknown".into());
                        events.push(self.action_requested_event_for(next_pid, &next_name));
                    }
                }
            }
            HandStep::DrawPhase => {
                if draw_done {
                    self.table.hand_state_mut().end_draw_round();
                    self.table.advance_table_state();
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });
                    events.extend(self.enter_betting_round());
                    return events;
                }

                if draw_removed_current {
                    if let Some(next_pid) = next_draw_pid {
                        let next_name = self
                            .accounts
                            .name_from_id(next_pid)
                            .unwrap_or("Unknown".into());
                        events.push(Event::DrawRequested {
                            player_name: next_name,
                            max_discards: five_card_draw::MAX_DISCARDS,
                        });
                    }
                }
            }
            _ => {}
        }

        events
    }

    pub fn run_automatic_steps(&mut self) -> Vec<Event> {
        // Go through the steps in the state machine that don't require player input starting at the current step.
        let mut events = Vec::new();
        loop {
            match self.table.current_step() {
                HandStep::CollectForcedBets => {
                    let ante = five_card_draw::ANTE_AMOUNT;

                    let seated_ids = self
                        .table
                        .seated_players()
                        .into_iter()
                        .map(|(_seat, player_id)| player_id)
                        .collect::<Vec<PlayerId>>();

                    let eligible_ids = seated_ids
                        .into_iter()
                        .filter(|pid| !self.sit_out_next.contains(pid))
                        .collect::<Vec<PlayerId>>();

                    let mut kicked = Vec::new();
                    let mut payers = Vec::new();

                    for pid in &eligible_ids {
                        let bal = self
                            .accounts
                            .get_by_id(*pid)
                            .map(|a| a.balance)
                            .unwrap_or(0);
                        if bal < ante.try_into().unwrap_or(5) {
                            kicked.push(*pid);
                        } else {
                            payers.push(*pid);
                        }
                    }

                    if payers.len() < (five_card_draw::MIN_PLAYERS as usize) {
                        for pid in &kicked {
                            let _ = self.table.leave_table(*pid);
                            events.push(Event::PlayerLeft {
                                player_id: *pid,
                                player_name: self
                                    .accounts
                                    .name_from_id(*pid)
                                    .unwrap_or("Unknown".into()),
                            });
                            events.push(Event::Info {
                                message: format!("Player {} kicked for insufficient funds", pid),
                            });
                        }
                        // Reset table state so lobby can recover
                        self.table.reset_to_waiting();
                        self.table.apply_pending_requests();

                        events.push(Event::Error {
                            message: "Not enough players can pay ante to start hand".into(),
                        });
                        events.push(Event::PotUpdated { pot: 0 });
                        events.push(Event::StepChanged {
                            step: HandStep::WaitingForPlayers,
                        });
                        events.push(self.lobby_state_event());
                        break;
                    }

                    for pid in &payers {
                        let _ = self
                            .accounts
                            .withdraw_by_id(*pid, ante.try_into().unwrap_or(5));
                    }

                    for pid in &kicked {
                        let name = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());
                        let _ = self.table.leave_table(*pid);
                        events.push(Event::Info {
                            message: format!("{} removed: cannot pay ante", name),
                        });
                        events.push(Event::PlayerLeft {
                            player_id: *pid,
                            player_name: name,
                        });
                    }

                    self.table
                        .hand_state_mut()
                        .add_to_pot((payers.len() as u32) * ante);
                    events.push(Event::PotUpdated {
                        pot: self.table.hand_state().pot_size() as i64,
                    });

                    // advance state at end of step
                    self.table.advance_table_state();
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });
                }

                HandStep::DealPrivate => {
                    self.table.hand_state_mut().reset_for_new_deal();

                    // If dealer seat was vacated (e.g., insufficient ante), move to next occupied seat
                    self.table.ensure_dealer_in_seats();

                    // Deal in action order (left of dealer first)
                    let seated: Vec<PlayerId> = self.table.action_order();
                    let sit_out = self.sit_out_next.clone();

                    for pid in seated {
                        if sit_out.contains(&pid) {
                            self.table
                                .hand_state_mut()
                                .set_player_fold_status(pid, true);
                            continue;
                        }
                        let name = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                        let hand = Hand::<5>::draw_from_deck(&mut self.deck).unwrap();

                        self.table
                            .hand_state_mut()
                            .set_player_hand(pid, hand.clone());
                        self.table
                            .hand_state_mut()
                            .set_player_fold_status(pid, false);

                        events.push(Event::PrivateHand {
                            player_name: name,
                            cards: hand.cards().to_vec(), // or hand.cards().to_vec() depending on your API
                        });
                    }

                    // Sit-out applies to this hand only
                    self.sit_out_next.clear();

                    // advance state at end of step
                    self.table.advance_table_state();
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });

                    if self.is_decision_step(self.table.current_step()) {
                        events.extend(self.on_enter_decision_step(self.table.current_step()));
                        break;
                    }
                }

                HandStep::DrawPhase | HandStep::BettingRound1 | HandStep::BettingRound2 => {
                    events.extend(self.on_enter_decision_step(self.table.current_step()));
                    break;
                }

                HandStep::Showdown => {
                    events.extend(self.do_showdown());
                    self.table.advance_table_state();
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });
                }

                HandStep::Payout => {
                    events.extend(self.do_payout());
                    self.table.advance_table_state(); // should go back to WaitingForPlayers
                    events.push(Event::StepChanged {
                        step: self.table.current_step(),
                    });
                    // Now that we're back to WaitingForPlayers, process queued joins/leaves.
                    self.table.apply_pending_requests();
                    break; // stop auto-run after finishing a hand
                }

                _ => {
                    break;
                }
            }
        }

        events
    }

    pub fn has_enough_for_ante(&self, player_id: PlayerId) -> bool {
        match self.accounts.get_by_id(player_id) {
            Ok(acct) => acct.balance >= five_card_draw::ANTE_AMOUNT.try_into().unwrap_or(5),
            Err(_) => false,
        }
    }

    pub fn is_decision_step(&self, step: HandStep) -> bool {
        matches!(
            step,
            HandStep::BettingRound1 | HandStep::BettingRound2 | HandStep::DrawPhase
        )
    }

    fn on_enter_decision_step(&mut self, step: HandStep) -> Vec<Event> {
        match step {
            HandStep::BettingRound1 | HandStep::BettingRound2 => self.enter_betting_round(),
            HandStep::DrawPhase => self.enter_draw_phase(),
            _ => vec![],
        }
    }

    fn enter_draw_phase(&mut self) -> Vec<Event> {
        let order = self.table.action_order();

        let active_in_order: Vec<PlayerId> = order
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid)) // make helper if you want
            .collect();

        if active_in_order.is_empty() {
            return vec![Event::Error {
                message: "No active players for draw".into(),
            }];
        }

        self.table
            .hand_state_mut()
            .start_draw_round(active_in_order.clone());

        // store draw order + index somewhere (HandState) OR just request first for now
        let first_player = active_in_order[0];
        let pname = self
            .accounts
            .name_from_id(first_player)
            .unwrap_or("Unknown".into());

        vec![Event::DrawRequested {
            player_name: pname,
            max_discards: five_card_draw::MAX_DISCARDS,
        }]
    }

    fn enter_betting_round(&mut self) -> Vec<Event> {
        let order = self.table.action_order();

        // Filter out to have only active players (not folded and seated)
        let seated_set: HashSet<PlayerId> = self
            .table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();
        let active_in_order = order
            .into_iter()
            .filter(|pid| {
                seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            })
            .collect();

        // Initialize betting round state
        self.table
            .hand_state_mut()
            .start_betting_round(active_in_order);

        let seated_set: HashSet<PlayerId> = self
            .table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();

        let folded_set: HashSet<PlayerId> = seated_set
            .iter()
            .copied()
            .filter(|pid| self.table.hand_state().has_player_folded(pid))
            .collect();

        let is_active = |pid: PlayerId| seated_set.contains(&pid) && !folded_set.contains(&pid);

        let br = self.table.hand_state_mut().betting_mut().unwrap();
        while !br.pending.is_empty() && !is_active(br.current_player()) {
            Self::advance_to_next_actor_local(br, &is_active);
        }

        // Emit ActionRequested for the first player to act
        let first_player = self.table.hand_state().betting().unwrap().current_player();
        let pname = self
            .accounts
            .name_from_id(first_player)
            .unwrap_or("Unknown".into());

        vec![self.action_requested_event_for(first_player, &pname)]
    }

    fn resolve_pid(&self, player_name: &str) -> Result<PlayerId, Event> {
        let pid = match self.accounts.get_by_name(player_name) {
            Ok(acct) => acct.id,
            Err(e) => {
                return Err(Event::Error {
                    message: format!("Player lookup failed for {}: {:?}", player_name, e),
                });
            }
        };

        if !self.table.is_active_player(pid) {
            return Err(Event::Error {
                message: format!("Player {} is not active at the table", player_name),
            });
        }

        Ok(pid)
    }

    fn is_in_hand_and_active(&self, pid: PlayerId) -> bool {
        // seated AND not folded
        self.table
            .seated_players()
            .iter()
            .any(|(_, spid)| *spid == pid)
            && !self.table.hand_state().has_player_folded(&pid)
    }

    fn active_players_in_order(&self) -> Vec<PlayerId> {
        let seated_set: HashSet<PlayerId> = self
            .table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();

        let order = self.table.action_order();

        order
            .into_iter()
            .filter(|pid| {
                seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            })
            .collect()
    }

    fn active_players_in_hand(&self) -> Vec<PlayerId> {
        self.table
            .seated_players()
            .into_iter()
            .map(|(_, pid)| pid)
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect()
    }

    fn advance_to_next_actor_local(
        br: &mut BettingRoundState,
        is_active: &impl Fn(PlayerId) -> bool,
    ) {
        let n = br.order.len();
        if n == 0 {
            return;
        }

        for _ in 0..n {
            br.to_act_idx = (br.to_act_idx + 1) % n;
            if is_active(br.order[br.to_act_idx]) {
                return;
            }
        }
        br.pending.clear();
    }

    fn resolve_player_from_action(&self, cmd: &ActionCmd) -> Result<(PlayerId, String), Event> {
        match cmd {
            ActionCmd::Check { player_name }
            | ActionCmd::Bet { player_name, .. }
            | ActionCmd::Call { player_name }
            | ActionCmd::Raise { player_name, .. }
            | ActionCmd::Fold { player_name }
            | ActionCmd::Draw { player_name, .. } => {
                let pid = self.resolve_pid(player_name)?;
                Ok((pid, player_name.clone()))
            }
        }
    }

    fn action_requested_event_for(&self, pid: PlayerId, pname: &str) -> Event {
        let current_bet = self.table.hand_state().current_bet();
        let contrib = self
            .table
            .hand_state()
            .betting()
            .unwrap()
            .contrib
            .get(&pid)
            .copied()
            .unwrap_or(0);
        let call_amount = current_bet.saturating_sub(contrib);

        let allowed_actions: Vec<AllowedAction> = if current_bet == 0 {
            vec![
                AllowedAction::Check,
                AllowedAction::Bet,
                AllowedAction::Fold,
            ]
        } else {
            // Check if we have passed raise cap
            if self.table.hand_state().betting().unwrap().raises_used
                >= five_card_draw::MAX_RAISES_PER_ROUND
            {
                return Event::ActionRequested {
                    player_name: pname.to_string(),
                    allowed: vec![AllowedAction::Call, AllowedAction::Fold],
                    call_amount: Some(call_amount as i64),
                    min_raise: None,
                };
            }

            vec![
                AllowedAction::Call,
                AllowedAction::Raise,
                AllowedAction::Fold,
            ]
        };

        Event::ActionRequested {
            player_name: pname.to_string(),
            allowed: allowed_actions,
            call_amount: Some(call_amount as i64),
            min_raise: Some(five_card_draw::RAISE_INCREMENT as i64),
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

    fn do_showdown(&mut self) -> Vec<Event> {
        use common::hand_rank::rank_five_card_hand;

        // Active players = players who still have hands and did not fold
        let active: Vec<PlayerId> = self
            .table
            .hand_state()
            .players_with_hands()
            .into_iter()
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect();

        if active.is_empty() {
            return vec![Event::Error {
                message: "Showdown: no active players".into(),
            }];
        }

        // Rank all active hands + build showdown lines
        let mut ranked: Vec<(PlayerId, common::hand_rank::HandRank)> = Vec::new();
        let mut lines: Vec<common::protocol::ShowdownLine> = Vec::new();

        for pid in &active {
            let hand = match self.table.hand_state().player_hand(pid) {
                Some(h) => h,
                None => {
                    continue;
                }
            };

            let r = rank_five_card_hand(hand); // Hand<5> alias matches
            ranked.push((*pid, r));

            // Minimal readable description for CLI
            let desc = format!("{} => {:?} {:?}", hand, r.category, r.tiebreakers);
            let pname = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());
            lines.push(ShowdownLine {
                player_id: *pid,
                player_name: pname,
                hand_desc: desc,
            });
        }

        // If somehow ranked is empty
        if ranked.is_empty() {
            return vec![Event::Error {
                message: "Showdown: no hands to rank".into(),
            }];
        }

        vec![Event::Showdown { results: lines }]
    }

    fn do_payout(&mut self) -> Vec<Event> {
        use common::hand_rank::rank_five_card_hand;

        let mut events = Vec::new();

        // Players who participated in the hand = those we dealt to
        let participants = self.table.hand_state().players_with_hands();
        if participants.len() < 2 {
            // hand can't really exist, but don't crash
            self.table.hand_state_mut().reset_for_new_deal();
            return vec![Event::Error {
                message: "Payout: not enough participants".into(),
            }];
        }

        // Determine active (not folded)
        let active: Vec<PlayerId> = participants
            .iter()
            .copied()
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect();

        if active.is_empty() {
            self.table.hand_state_mut().reset_for_new_deal();
            return vec![Event::Error {
                message: "Payout: no active players".into(),
            }];
        }

        // Rank active hands and find max (TA clarification on limited tie-breaking)
        let mut best_rank = None;
        let mut winners: Vec<PlayerId> = Vec::new();

        for pid in &active {
            let hand = match self.table.hand_state().player_hand(pid) {
                Some(h) => h,
                None => {
                    continue;
                }
            };
            let r = rank_five_card_hand(hand);

            match best_rank {
                None => {
                    best_rank = Some(r);
                    winners = vec![*pid];
                }
                Some(br) => match Self::compare_for_payout(r, br) {
                    Ordering::Greater => {
                        best_rank = Some(r);
                        winners = vec![*pid];
                    }
                    Ordering::Equal => {
                        winners.push(*pid);
                    }
                    Ordering::Less => {}
                },
            }
        }

        if winners.is_empty() {
            self.table.hand_state_mut().reset_for_new_deal();
            return vec![Event::Error {
                message: "Payout: could not determine winner".into(),
            }];
        }

        // Split pot
        let pot = self.table.hand_state().pot_size() as i64;
        let n = winners.len() as i64;

        let share = pot / n;
        let remainder = pot % n;

        let mut payout_pairs: Vec<(PlayerId, i64)> = Vec::new();

        for pid in winners.iter() {
            payout_pairs.push((*pid, share));
        }

        if remainder > 0 {
            let order = self.table.action_order(); // left of dealer first
            let remainder_pid = order
                .into_iter()
                .find(|pid| winners.contains(pid))
                .unwrap_or(winners[0]);

            if let Some(pair) = payout_pairs
                .iter_mut()
                .find(|(pid, _)| *pid == remainder_pid)
            {
                pair.1 += remainder;
            }
        }

        // Stats updates
        // games_played for participants
        for pid in &participants {
            let pname = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());

            if let Some(name) = pname.into() {
                let _ = self.accounts.record_game_played(&name);
            }
        }

        // pot_won for winners + deposit money
        for (pid, amt) in &payout_pairs {
            let pname = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());

            if let Some(name) = pname.into() {
                if *amt > 0 {
                    let _ = self.accounts.deposit(&name, *amt);
                }
                let _ = self.accounts.record_pot_won(&name);
            }
        }

        // folds for those who folded
        for pid in &participants {
            if self.table.hand_state().has_player_folded(pid) {
                let pname = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());

                if let Some(name) = pname.into() {
                    let _ = self.accounts.record_fold(&name);
                }
            }
        }

        // Emit payout event
        events.push(Event::Payout {
            winners: payout_pairs
                .into_iter()
                .map(|(pid, amt)| {
                    let pname = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                    (pname, amt)
                })
                .collect(),
        });

        // Reset for next hand
        self.table.hand_state_mut().reset_after_hand();
        self.table.hand_state_mut().end_betting_round(); // safe no-op if None

        // Rotate dealer for next hand
        self.table.advance_dealer();

        // Also: pot is now effectively 0; you can emit pot updated if you want
        events.push(Event::PotUpdated { pot: 0 });

        events
    }

    fn compare_for_payout(a: hand_rank::HandRank, b: hand_rank::HandRank) -> Ordering {
        use hand_rank::HandCategory;

        match a.category.cmp(&b.category) {
            Ordering::Equal => match a.category {
                // TA clarification: only partial tie-breaking for these categories
                HandCategory::HighCard | HandCategory::OnePair => {
                    a.tiebreakers[0].cmp(&b.tiebreakers[0])
                }
                HandCategory::TwoPair => {
                    // Compare high pair, then low pair, ignore kicker
                    match a.tiebreakers[0].cmp(&b.tiebreakers[0]) {
                        Ordering::Equal => a.tiebreakers[1].cmp(&b.tiebreakers[1]),
                        ord => ord,
                    }
                }
                // Other categories use full ordering
                _ => a.cmp(&b),
            },
            ord => ord,
        }
    }
}

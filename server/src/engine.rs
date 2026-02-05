use common::{
    card_deck::Deck,
    constants::five_card_draw::{ self, ANTE_AMOUNT },
    hand::Hand,
    protocol::{
        AccountCmd,
        ActionCmd,
        AllowedAction,
        Command,
        Event,
        HandStep,
        LobbyCmd,
        PlayerId,
        PlayerSummary,
    },
};
use std::collections::{ HashMap, HashSet };

use crate::{
    account_store::{ AccountError, AccountStore },
    game_states::BettingRoundState,
    table::FiveCardTable,
};

pub struct ServerEngine {
    pub accounts: AccountStore,
    pub table: FiveCardTable,
    pub deck: Deck,
}

impl ServerEngine {
    pub fn new(accounts: AccountStore, table: FiveCardTable) -> Self {
        Self {
            accounts,
            table,
            deck: Deck::new_shuffled(),
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
                    Err(e) =>
                        vec![Event::Error { message: format!("Account creation failed: {:?}", e) }],
                }
            }

            AccountCmd::Delete { player_name, pin } => {
                match self.accounts.delete_account(&player_name, &pin) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) =>
                        vec![Event::Error { message: format!("Account deletion failed: {:?}", e) }],
                }
            }

            AccountCmd::Deposit { player_name, amount } => {
                match self.accounts.deposit(&player_name, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error { message: format!("Deposit failed: {:?}", e) }],
                }
            }

            AccountCmd::Withdraw { player_name, amount } => {
                match self.accounts.withdraw(&player_name, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error { message: format!("Withdraw failed: {:?}", e) }],
                }
            }

            AccountCmd::Get { player_name } => {
                match self.accounts.get_by_name(&player_name) {
                    Ok(acct) => vec![Event::Info { message: format!("Account info: {:?}", acct) }],
                    Err(e) =>
                        vec![Event::Error { message: format!("Get account failed: {:?}", e) }],
                }
            }
        }
    }

    pub fn apply_lobby_cmd(&mut self, cmd: LobbyCmd) -> Vec<Event> {
        match cmd {
            LobbyCmd::SetVariant { variant } => {
                // Nothing fancy yet
                vec![Event::Info { message: format!("Set poker variant to {:?}", variant) }]
            }

            LobbyCmd::GetLobbyState => { vec![self.lobby_state_event()] }

            LobbyCmd::Join { player_name, pin } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Cannot join table during hand".into() }];
                }

                match self.accounts.login(&player_name, &pin) {
                    Ok(pa) => {
                        match self.table.join_table(pa.id) {
                            Ok(seat) =>
                                vec![
                                    Event::PlayerJoined {
                                        player_id: pa.id,
                                        player_name: pa.name.clone(),
                                        seat: seat,
                                    },
                                    self.lobby_state_event()
                                ],
                            Err(e) =>
                                vec![Event::Error { message: format!("Seating failed: {:?}", e) }],
                        }
                    }
                    Err(e) => { vec![Event::Error { message: format!("Login failed: {:?}", e) }] }
                }
            }

            LobbyCmd::Leave { player_name } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Cannot leave table during hand".into() }];
                }

                match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => {
                        match self.table.leave_table(pa.id) {
                            Ok(_) =>
                                vec![
                                    Event::PlayerLeft {
                                        player_id: pa.id,
                                        player_name: pa.name.clone(),
                                    },
                                    self.lobby_state_event()
                                ],
                            Err(e) =>
                                vec![Event::Error { message: format!("Leave failed: {:?}", e) }],
                        }
                    }
                    Err(e) => {
                        vec![Event::Error { message: format!("Get account failed: {:?}", e) }]
                    }
                }
            }

            LobbyCmd::StartHand => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Hand already in progress".into() }];
                }

                let seated = self.table.seated_players();

                if
                    seated.len() < (five_card_draw::MIN_PLAYERS as usize) ||
                    seated.len() > (five_card_draw::MAX_PLAYERS as usize)
                {
                    return vec![Event::Error { message: "need 2–5 players to start".into() }];
                }

                // Confirm that each player has enough balance to cover the ante
                for (_seat, player_id) in seated {
                    if !self.has_enough_for_ante(player_id) {
                        return vec![Event::Error {
                            message: format!("Player {} does not have enough balance for ante", player_id),
                        }];
                    }
                }

                self.deck = Deck::new_shuffled();

                self.table.advance_table_state();
                let mut events = vec![Event::StepChanged { step: self.table.current_step() }];

                events.extend(self.run_automatic_steps());

                events
            }

            _ => vec![Event::Error { message: "Lobby command not implemented yet".into() }],
        }
    }

    pub fn apply_action_cmd(&mut self, cmd: ActionCmd) -> Vec<Event> {
        // 1) Must be in a decision step that accepts actions
        match self.table.current_step() {
            HandStep::BettingRound1 | HandStep::BettingRound2 => self.apply_betting_action(cmd),
            HandStep::DrawPhase => self.apply_draw_action(cmd), // later
            _ => vec![Event::Error { message: "Actions not allowed in current step".into() }],
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
                return vec![Event::Error { message: "Betting round not initialized".into() }];
            }
        };

        if pid != current_player {
            return vec![Event::Error {
                message: format!(
                    "Not your turn. Expected {}",
                    self.accounts.name_from_id(current_player).unwrap_or("Unknown".into())
                ),
            }];
        }

        // Active players in order (seated + not folded)
        let active = self.active_players_in_order();

        // Pull current bet + contrib + call amount
        let current_bet = self.table.hand_state().current_bet();
        let contrib = self.table
            .hand_state()
            .betting()
            .unwrap()
            .contrib.get(&pid)
            .copied()
            .unwrap_or(0);
        let call_amount = current_bet.saturating_sub(contrib);

        // Apply action
        match cmd {
            ActionCmd::Check { .. } => {
                if call_amount != 0 {
                    return vec![Event::Error { message: "Cannot check when facing a bet".into() }];
                }
                self.table.hand_state_mut().betting_mut().unwrap().pending.remove(&pid);
            }

            ActionCmd::Fold { .. } => {
                self.table.hand_state_mut().set_player_fold_status(pid, true);
                self.table.hand_state_mut().betting_mut().unwrap().pending.remove(&pid);
            }

            ActionCmd::Bet { amount, .. } => {
                if current_bet != 0 {
                    return vec![Event::Error {
                        message: "Cannot bet after betting has started; use raise".into(),
                    }];
                }
                // validate amount min/max here
                if
                    amount < (five_card_draw::MIN_BET_PER_ROUND as i64) ||
                    amount > (five_card_draw::MAX_BET_PER_ROUND as i64)
                {
                    return vec![Event::Error {
                        message: format!(
                            "Bet amount must be between {} and {}",
                            five_card_draw::MIN_BET_PER_ROUND,
                            five_card_draw::MAX_BET_PER_ROUND
                        ),
                    }];
                }

                // withdraw from accounts
                if let Err(e) = self.accounts.withdraw(&pname, amount as i64) {
                    return vec![Event::Error { message: format!("Withdraw failed: {:?}", e) }];
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
                    return vec![Event::Error { message: "Nothing to call".into() }];
                }

                if let Err(e) = self.accounts.withdraw(&pname, call_amount as i64) {
                    return vec![Event::Error { message: format!("Withdraw failed: {:?}", e) }];
                }

                self.table.hand_state_mut().add_to_pot(call_amount);
                let br = self.table.hand_state_mut().betting_mut().unwrap();
                let new_contrib = br.contrib.get(&pid).copied().unwrap_or(0) + call_amount;
                br.contrib.insert(pid, new_contrib);
                br.pending.remove(&pid);
            }

            ActionCmd::Raise { amount, .. } => {
                // interpret as raise-to
                let new_bet = amount as u32;

                if current_bet == 0 {
                    return vec![Event::Error {
                        message: "Cannot raise when no bet exists; use bet".into(),
                    }];
                }
                if new_bet <= current_bet {
                    return vec![Event::Error {
                        message: "Raise amount must exceed current bet".into(),
                    }];
                }

                // raise cap
                {
                    let br = self.table.hand_state().betting().unwrap();
                    if br.raises_used >= five_card_draw::MAX_RAISES_PER_ROUND {
                        return vec![Event::Error { message: "Raise cap reached".into() }];
                    }
                }

                let br_contrib = self.table
                    .hand_state()
                    .betting()
                    .unwrap()
                    .contrib.get(&pid)
                    .copied()
                    .unwrap_or(0);
                let delta = new_bet.saturating_sub(br_contrib);

                if let Err(e) = self.accounts.withdraw(&pname, delta as i64) {
                    return vec![Event::Error { message: format!("Withdraw failed: {:?}", e) }];
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
                return vec![Event::Error { message: "Unsupported action in betting round".into() }];
            }
        }

        // Emit pot update after money actions (optional)
        events.push(Event::PotUpdated { pot: self.table.hand_state().pot_size() as i64 });

        // Round complete?
        let pending_empty = self.table.hand_state().betting().unwrap().pending.is_empty();
        if pending_empty {
            self.table.hand_state_mut().end_betting_round();
            self.table.advance_table_state();
            events.push(Event::StepChanged { step: self.table.current_step() });

            // Let automatic runner take it to the next prompt (draw phase / betting2)
            events.extend(self.run_automatic_steps());
            return events;
        }

        // Otherwise, advance to next actor (skip folded)
        {
            let br = self.table.hand_state_mut().betting_mut().unwrap();
            let to_act_idx = br.to_act_idx;
            self.advance_to_next_actor_impl(to_act_idx, &active);
        }

        // Prompt next player
        let next_pid = self.table.hand_state().betting().unwrap().current_player();
        let next_name = self.accounts.name_from_id(next_pid).unwrap_or("Unknown".into());
        events.push(self.action_requested_event_for(next_pid, &next_name));

        events
    }

    pub fn apply_draw_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
        // To be implemented: process draw actions
        vec![Event::Error { message: "Draw actions not implemented yet".into() }]
    }

    pub fn run_automatic_steps(&mut self) -> Vec<Event> {
        // Go through the steps in the state machine that don't require player input starting at the current step.
        let mut events = Vec::new();
        loop {
            match self.table.current_step() {
                HandStep::CollectForcedBets => {
                    let ante = five_card_draw::ANTE_AMOUNT;

                    let seated_ids = self.table
                        .seated_players()
                        .into_iter()
                        .map(|(_seat, player_id)| player_id)
                        .collect::<Vec<PlayerId>>();

                    let mut kicked = Vec::new();
                    let mut payers = Vec::new();

                    for pid in &seated_ids {
                        let bal = self.accounts
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
                                player_name: self.accounts
                                    .name_from_id(*pid)
                                    .unwrap_or("Unknown".into()),
                            });
                            events.push(Event::Info {
                                message: format!("Player {} kicked for insufficient funds", pid),
                            });
                        }
                        events.push(Event::Error {
                            message: "Not enough players can pay ante to start hand".into(),
                        });
                        events.push(self.lobby_state_event());
                        break;
                    }

                    for pid in &payers {
                        let _ = self.accounts.withdraw_by_id(*pid, ante.try_into().unwrap_or(5));
                    }

                    for pid in &kicked {
                        let name = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());
                        let _ = self.table.leave_table(*pid);
                        events.push(Event::Info {
                            message: format!("{} removed: cannot pay ante", name),
                        });
                        events.push(Event::PlayerLeft { player_id: *pid, player_name: name });
                    }

                    self.table.hand_state_mut().add_to_pot((payers.len() as u32) * ante);
                    events.push(Event::PotUpdated {
                        pot: self.table.hand_state().pot_size() as i64,
                    });

                    // advance state at end of step
                    self.table.advance_table_state();
                    events.push(Event::StepChanged { step: self.table.current_step() });
                }

                HandStep::DealPrivate => {
                    self.table.hand_state_mut().reset_for_new_deal();

                    let seated: Vec<PlayerId> = self.table
                        .seated_players()
                        .into_iter()
                        .map(|(_seat, pid)| pid)
                        .collect();

                    for pid in seated {
                        let name = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
                        let hand = Hand::<5>::draw_from_deck(&mut self.deck).unwrap();

                        self.table.hand_state_mut().set_player_hand(pid, hand.clone());
                        self.table.hand_state_mut().set_player_fold_status(pid, false);

                        events.push(Event::PrivateHand {
                            player_name: name,
                            cards: hand.cards().to_vec(), // or hand.cards().to_vec() depending on your API
                        });
                    }

                    // advance state at end of step
                    self.table.advance_table_state();
                    events.push(Event::StepChanged { step: self.table.current_step() });

                    if self.is_decision_step(self.table.current_step()) {
                        events.extend(self.on_enter_decision_step(self.table.current_step()));
                        break;
                    }
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
        matches!(step, HandStep::BettingRound1 | HandStep::BettingRound2 | HandStep::DrawPhase)
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
            return vec![Event::Error { message: "No active players for draw".into() }];
        }

        // store draw order + index somewhere (HandState) OR just request first for now
        let first_player = active_in_order[0];
        let pname = self.accounts.name_from_id(first_player).unwrap_or("Unknown".into());

        vec![Event::DrawRequested {
            player_name: pname,
            max_discards: five_card_draw::MAX_DISCARDS,
        }]
    }

    fn enter_betting_round(&mut self) -> Vec<Event> {
        let order = self.table.action_order();

        // Filter out to have only active players (not folded and seated)
        let seated_set: HashSet<PlayerId> = self.table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();
        let active_in_order = order
            .into_iter()
            .filter(
                |pid| seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            )
            .collect();

        // Initialize betting round state
        self.table.hand_state_mut().start_betting_round(active_in_order);

        // Emit ActionRequested for the first player to act
        let first_player = self.table.hand_state().betting().unwrap().current_player();
        let pname = self.accounts.name_from_id(first_player).unwrap_or("Unknown".into());

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

    fn active_players_in_order(&self) -> Vec<PlayerId> {
        let seated_set: HashSet<PlayerId> = self.table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();

        let order = self.table.action_order();

        order
            .into_iter()
            .filter(
                |pid| seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            )
            .collect()
    }

    fn advance_to_next_actor_impl(&mut self, mut to_act_idx: usize, active: &[PlayerId]) {
        let num_active = active.len();
        for _ in 0..num_active {
            to_act_idx = (to_act_idx + 1) % num_active;
            let next_pid = active[to_act_idx];
            if !self.table.hand_state().has_player_folded(&next_pid) {
                break;
            }
        }
        self.table.hand_state_mut().betting_mut().unwrap().to_act_idx = to_act_idx;
    }

    fn resolve_player_from_action(&self, cmd: &ActionCmd) -> Result<(PlayerId, String), Event> {
        match cmd {
            | ActionCmd::Check { player_name }
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
        let contrib = self.table
            .hand_state()
            .betting()
            .unwrap()
            .contrib.get(&pid)
            .copied()
            .unwrap_or(0);
        let call_amount = current_bet.saturating_sub(contrib);

        let allowed_actions: Vec<AllowedAction> = if current_bet == 0 {
            vec![AllowedAction::Check, AllowedAction::Bet, AllowedAction::Fold]
        } else {
            // Check if we have passed raise cap
            if
                self.table.hand_state().betting().unwrap().raises_used >=
                five_card_draw::MAX_RAISES_PER_ROUND
            {
                return Event::ActionRequested {
                    player_name: pname.to_string(),
                    allowed: vec![AllowedAction::Call, AllowedAction::Fold],
                    call_amount: Some(call_amount as i64),
                    min_raise: None,
                };
            }

            vec![AllowedAction::Call, AllowedAction::Raise, AllowedAction::Fold]
        };

        Event::ActionRequested {
            player_name: pname.to_string(),
            allowed: allowed_actions,
            call_amount: Some(call_amount as i64),
            min_raise: Some(five_card_draw::RAISE_INCREMENT as i64),
        }
    }
}

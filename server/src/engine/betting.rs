use super::*;

enum ValidatedBet {
    Check { pid: PlayerId },
    Fold { pid: PlayerId },
    Bet {
        pid: PlayerId,
        player_name: String,
        amount: i64,
        active_players: Vec<PlayerId>,
    },
    Call {
        pid: PlayerId,
        player_name: String,
        call_amount: u32,
    },
    Raise {
        pid: PlayerId,
        player_name: String,
        new_bet: u32,
        delta: u32,
        active_players: Vec<PlayerId>,
    },
}

impl ServerEngine {
    pub(super) fn apply_betting_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
        // Resolve name -> pid
        let (pid, _pname) = match self.resolve_player_from_action(&cmd) {
            Ok(x) => x,
            Err(e) => {
                return vec![e];
            }
        };

        let validated = match self.validate_betting_action(pid, &cmd) {
            Ok(v) => v,
            Err(events) => return events,
        };

        let apply_events = self.apply_validated_bet(validated);
        if !apply_events.is_empty() {
            return apply_events;
        }

        let mut events = vec![self.emit_current_pot_updated()];
        events.extend(self.maybe_finalize_betting_round());
        events
    }

    fn validate_betting_action(
        &self,
        pid: PlayerId,
        cmd: &ActionCmd,
    ) -> Result<ValidatedBet, Vec<Event>> {
        if let Err(events) = self.require_betting_turn(pid) {
            return Err(events);
        }

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

        if self.is_stud_variant() {
            return self.validate_stud_betting_action(pid, cmd, current_bet, call_amount);
        }

        match cmd {
            ActionCmd::Check { .. } => {
                if call_amount != 0 {
                    return Err(vec![self.emit_error("Cannot check when facing a bet")]);
                }
                Ok(ValidatedBet::Check { pid })
            }

            ActionCmd::Fold { .. } => Ok(ValidatedBet::Fold { pid }),

            ActionCmd::Bet { player_name, amount } => {
                if current_bet != 0 {
                    return Err(vec![
                        self.emit_error("Cannot bet after betting has started; use raise"),
                    ]);
                }
                if *amount <= 0 {
                    return Err(vec![self.emit_error("Bet amount must be positive")]);
                }
                if *amount < (five_card_draw::MIN_BET_PER_ROUND as i64)
                    || *amount > (five_card_draw::MAX_BET_PER_ROUND as i64)
                {
                    return Err(vec![self.emit_error(format!(
                        "Bet amount must be between {} and {}",
                        five_card_draw::MIN_BET_PER_ROUND,
                        five_card_draw::MAX_BET_PER_ROUND
                    ))]);
                }
                if (*amount as u32) % five_card_draw::BET_INCREMENT != 0 {
                    return Err(vec![self.emit_error(format!(
                        "Bet amount must be a multiple of {}",
                        five_card_draw::BET_INCREMENT
                    ))]);
                }

                Ok(ValidatedBet::Bet {
                    pid,
                    player_name: player_name.clone(),
                    amount: *amount,
                    active_players: self.active_players_for_current_betting_round(),
                })
            }

            ActionCmd::Call { player_name } => {
                if call_amount == 0 {
                    return Err(vec![self.emit_error("Nothing to call")]);
                }

                Ok(ValidatedBet::Call {
                    pid,
                    player_name: player_name.clone(),
                    call_amount,
                })
            }

            ActionCmd::Raise { player_name, amount } => {
                if current_bet == 0 {
                    return Err(vec![
                        self.emit_error("Cannot raise when no bet exists; use bet"),
                    ]);
                }
                if *amount <= 0 {
                    return Err(vec![self.emit_error("Raise amount must be positive")]);
                }

                {
                    let br = self.table.hand_state().betting().unwrap();
                    if br.raises_used >= self.max_raises_for_current_variant() {
                        return Err(vec![self.emit_error("Raise cap reached")]);
                    }
                }

                let raise_by = *amount as u32;
                if raise_by < five_card_draw::MIN_RAISE_AMOUNT {
                    return Err(vec![self.emit_error(format!(
                        "Raise amount cannot be less than {}",
                        five_card_draw::MIN_RAISE_AMOUNT
                    ))]);
                }
                if raise_by % five_card_draw::RAISE_INCREMENT != 0 {
                    return Err(vec![self.emit_error(format!(
                        "Raise amount must be a multiple of {}",
                        five_card_draw::RAISE_INCREMENT
                    ))]);
                }

                let new_bet = current_bet + raise_by;
                if new_bet > five_card_draw::MAX_RAISE_TO_AMOUNT {
                    return Err(vec![self.emit_error(format!(
                        "Resulting bet cannot exceed {}",
                        five_card_draw::MAX_RAISE_TO_AMOUNT
                    ))]);
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

                Ok(ValidatedBet::Raise {
                    pid,
                    player_name: player_name.clone(),
                    new_bet,
                    delta,
                    active_players: self.active_players_for_current_betting_round(),
                })
            }

            _ => Err(vec![self.emit_error("Unsupported action in betting round")]),
        }
    }

    fn validate_stud_betting_action(
        &self,
        pid: PlayerId,
        cmd: &ActionCmd,
        current_bet: u32,
        call_amount: u32,
    ) -> Result<ValidatedBet, Vec<Event>> {

        let opening_bet = self.stud_opening_bet_size().ok_or_else(|| vec![self.emit_error("Invalid hand step for Stud betting")])?;

        let raise_by = self.stud_raise_by_amount(current_bet).ok_or_else(|| vec![self.emit_error("Invalid raise amount for Stud betting")])?;

        let raises_used = self.table.hand_state().betting().map(|br| br.raises_used).unwrap_or(0);

        match cmd {
            ActionCmd::Check { .. } => {
                if call_amount != 0 {
                    return Err(vec![self.emit_error("Cannot check when facing a bet")]);
                }
                Ok(ValidatedBet::Check { pid })
            }
            ActionCmd::Fold { .. } => Ok(ValidatedBet::Fold { pid }),
            ActionCmd::Bet { player_name, amount} => {
                if current_bet != 0 {
                    return Err(vec![
                        self.emit_error("Cannot bet after betting has started; use raise"),
                    ]);
                }

                if *amount != opening_bet as i64 {
                    return Err(vec![self.emit_error(format!(
                        "Bet amount must be exactly {} in Stud",
                        opening_bet
                    ))]);
                }

                Ok(ValidatedBet::Bet {
                    pid,
                    player_name: player_name.clone(),
                    amount: *amount,
                    active_players: self.active_players_in_order(),
                })
            }

            ActionCmd::Call { player_name } => {
                if call_amount == 0 {
                    return Err(vec![self.emit_error("Nothing to call")]);
                }

                Ok(ValidatedBet::Call {
                    pid,
                    player_name: player_name.clone(),
                    call_amount,
                })
            }

            ActionCmd::Raise { player_name, amount } => {
                if current_bet == 0 {
                    return Err(vec![
                        self.emit_error("Cannot raise when no bet exists; use bet"),
                    ]);
                }

                if raises_used >= seven_card_stud::MAX_RAISES_PER_ROUND {
                    return Err(vec![self.emit_error("Raise cap reached")]);
                }

                if *amount != raise_by as i64 {
                    return Err(vec![self.emit_error(format!(
                        "Raise amount must be exactly {} in Stud",
                        raise_by
                    ))]);
                }

                let contrib = self
                    .table
                    .hand_state()
                    .betting()
                    .unwrap()
                    .contrib
                    .get(&pid)
                    .copied()
                    .unwrap_or(0);

                let new_bet = current_bet + raise_by;
                let delta = new_bet.saturating_sub(contrib);

                Ok(ValidatedBet::Raise {
                    pid,
                    player_name: player_name.clone(),
                    new_bet,
                    delta,
                    active_players: self.active_players_in_order(),
                })
            }

            _ => Err(vec![self.emit_error("Unsupported action in betting round")]),
        }
    }

    fn apply_validated_bet(&mut self, validated: ValidatedBet) -> Vec<Event> {
        match validated {
            ValidatedBet::Check { pid } => {
                self.table
                    .hand_state_mut()
                    .betting_mut()
                    .unwrap()
                    .pending
                    .remove(&pid);
                Vec::new()
            }

            ValidatedBet::Fold { pid } => {
                self.table.hand_state_mut().set_player_fold_status(pid, true);
                self.table
                    .hand_state_mut()
                    .betting_mut()
                    .unwrap()
                    .pending
                    .remove(&pid);

                let active = self.active_players_in_hand();
                if active.len() == 1 {
                    let mut events = Vec::new();
                    self.table.hand_state_mut().end_betting_round();

                    while self.table.current_step() != HandStep::Showdown
                        && self.table.current_step() != HandStep::Payout
                    {
                        self.table.advance_table_state();
                    }
                    events.push(self.emit_step_changed());
                    events.extend(self.run_automatic_steps());
                    return events;
                }

                Vec::new()
            }

            ValidatedBet::Bet {
                pid,
                player_name,
                amount,
                active_players,
            } => {
                if let Err(e) = self.accounts.withdraw(&player_name, amount as i64) {
                    return vec![self.emit_error(format!("Withdraw failed: {:?}", e))];
                }

                self.table.hand_state_mut().add_to_pot(amount as u32);
                self.table.hand_state_mut().set_current_bet(amount as u32);

                let br = self.table.hand_state_mut().betting_mut().unwrap();
                br.contrib.insert(pid, amount as u32);
                br.pending = active_players.iter().cloned().collect();
                br.pending.remove(&pid);
                Vec::new()
            }

            ValidatedBet::Call {
                pid,
                player_name,
                call_amount,
            } => {
                if let Err(e) = self.accounts.withdraw(&player_name, call_amount as i64) {
                    return vec![self.emit_error(format!("Withdraw failed: {:?}", e))];
                }

                self.table.hand_state_mut().add_to_pot(call_amount);
                let br = self.table.hand_state_mut().betting_mut().unwrap();
                let new_contrib = br.contrib.get(&pid).copied().unwrap_or(0) + call_amount;
                br.contrib.insert(pid, new_contrib);
                br.pending.remove(&pid);
                Vec::new()
            }

            ValidatedBet::Raise {
                pid,
                player_name,
                new_bet,
                delta,
                active_players,
            } => {
                if let Err(e) = self.accounts.withdraw(&player_name, delta as i64) {
                    return vec![self.emit_error(format!("Withdraw failed: {:?}", e))];
                }

                self.table.hand_state_mut().add_to_pot(delta);
                self.table.hand_state_mut().set_current_bet(new_bet);

                let br = self.table.hand_state_mut().betting_mut().unwrap();
                br.contrib.insert(pid, new_bet);
                br.raises_used += 1;
                br.pending = active_players.iter().cloned().collect();
                br.pending.remove(&pid);
                Vec::new()
            }
        }
    }

    fn maybe_finalize_betting_round(&mut self) -> Vec<Event> {
        let mut events = Vec::new();

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
            events.push(self.emit_step_changed());

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

        {
            let br = self.table.hand_state_mut().betting_mut().unwrap();
            Self::advance_to_next_actor_local(br, &is_active);
        }

        let br = self.table.hand_state_mut().betting_mut().unwrap();
        while !br.pending.is_empty() && !is_active(br.current_player()) {
            Self::advance_to_next_actor_local(br, &is_active);
        }

        let next_pid = self.table.hand_state().betting().unwrap().current_player();
        events.push(self.emit_action_requested(next_pid));

        events
    }

    fn resolve_pid(&self, player_name: &str) -> Result<PlayerId, Event> {
        let pid = match self.accounts.get_by_name(player_name) {
            Ok(acct) => acct.id,
            Err(e) => {
                return Err(
                    self.emit_error(format!("Player lookup failed for {}: {:?}", player_name, e))
                );
            }
        };

        if !self.table.is_active_player(pid) {
            return Err(
                self.emit_error(format!("Player {} is not active at the table", player_name))
            );
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
            .filter(|pid| {
                seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            })
            .collect()
    }

    pub(super) fn active_players_in_hand(&self) -> Vec<PlayerId> {
        self.table
            .seated_players()
            .into_iter()
            .map(|(_, pid)| pid)
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect()
    }

    pub(super) fn advance_to_next_actor_local(
        br: &mut BettingRoundState,
        is_active: &impl Fn(PlayerId) -> bool
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

    pub(super) fn action_requested_event_for(&self, pid: PlayerId, pname: &str) -> Event {
        let current_bet = self.table.hand_state().current_bet();
        let contrib = self.table
            .hand_state()
            .betting()
            .unwrap()
            .contrib.get(&pid)
            .copied()
            .unwrap_or(0);
        let call_amount = current_bet.saturating_sub(contrib);

        let raises_capped =
            self.table.hand_state().betting().unwrap().raises_used >=
            self.max_raises_for_current_variant();

        let min_raise = if self.is_stud_variant() {
            if current_bet == 0 {
                self.stud_opening_bet_size().map(|amount| amount as i64)
            } else {
                self.stud_raise_by_amount(current_bet).map(|am| am as i64)
            }
        } else {
            Some(five_card_draw::MIN_RAISE_AMOUNT as i64)
        };

        let allowed_actions: Vec<AllowedAction> = if current_bet == 0 {
            vec![AllowedAction::Check, AllowedAction::Bet, AllowedAction::Fold]
        } else if call_amount == 0 {
            if raises_capped {
                vec![AllowedAction::Check, AllowedAction::Fold]
            } else {
                vec![AllowedAction::Check, AllowedAction::Raise, AllowedAction::Fold]
            }
        } else {
            if raises_capped {
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
            min_raise,
        }
    }
}

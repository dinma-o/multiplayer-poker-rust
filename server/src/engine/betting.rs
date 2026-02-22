use super::*;

impl ServerEngine {
    pub(super) fn apply_betting_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
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

    pub(super) fn action_requested_event_for(&self, pid: PlayerId, pname: &str) -> Event {
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
}


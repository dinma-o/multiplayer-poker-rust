use super::*;

impl ServerEngine {
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
                            events.push(
                                self.emit_info(format!("Player {} kicked for insufficient funds", pid)),
                            );
                        }
                        // Reset table state so lobby can recover
                        self.table.reset_to_waiting();
                        self.table.apply_pending_requests();

                        events.push(self.emit_error("Not enough players can pay ante to start hand"));
                        events.push(self.emit_pot_updated(0));
                        events.push(self.emit_step_changed_to(HandStep::WaitingForPlayers));
                        events.push(self.emit_lobby_state());
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
                        events.push(self.emit_info(format!("{} removed: cannot pay ante", name)));
                        events.push(Event::PlayerLeft {
                            player_id: *pid,
                            player_name: name,
                        });
                    }

                    self.table
                        .hand_state_mut()
                        .add_to_pot((payers.len() as u32) * ante);
                    events.push(self.emit_current_pot_updated());

                    // advance state at end of step
                    self.table.advance_table_state();
                    events.push(self.emit_step_changed());
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
                    events.push(self.emit_step_changed());

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
                    events.push(self.emit_step_changed());
                }

                HandStep::Payout => {
                    events.extend(self.do_payout());
                    self.table.advance_table_state(); // should go back to WaitingForPlayers
                    events.push(self.emit_step_changed());
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

    pub fn is_decision_step(&self, step: HandStep) -> bool {
        matches!(
            step,
            HandStep::BettingRound1 | HandStep::BettingRound2 | HandStep::DrawPhase
        )
    }

    pub(super) fn on_enter_decision_step(&mut self, step: HandStep) -> Vec<Event> {
        match step {
            HandStep::BettingRound1 | HandStep::BettingRound2 => self.enter_betting_round(),
            HandStep::DrawPhase => self.enter_draw_phase(),
            _ => vec![],
        }
    }

    pub(super) fn enter_draw_phase(&mut self) -> Vec<Event> {
        let order = self.table.action_order();

        let active_in_order: Vec<PlayerId> = order
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid)) // make helper if you want
            .collect();

        if active_in_order.is_empty() {
            return vec![self.emit_error("No active players for draw")];
        }

        self.table
            .hand_state_mut()
            .start_draw_round(active_in_order.clone());

        // store draw order + index somewhere (HandState) OR just request first for now
        let first_player = active_in_order[0];
        vec![self.emit_draw_requested(first_player)]
    }

    pub(super) fn enter_betting_round(&mut self) -> Vec<Event> {
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
        vec![self.emit_action_requested(first_player)]
    }
}


use super::*;

struct MidhandRemovalState {
    draw_removed_current: bool,
    draw_done: bool,
    next_draw_pid: Option<PlayerId>,
}

impl ServerEngine {
    pub fn apply_lobby_cmd(&mut self, cmd: LobbyCmd) -> Vec<Event> {
        match cmd {
            LobbyCmd::SetVariant { variant } => self.lobby_set_variant(variant),
            LobbyCmd::GetHandState => self.lobby_view_state(),
            LobbyCmd::ViewHand { player_name } => self.lobby_view_hand(player_name),
            LobbyCmd::SetRngSeed { seed } => self.lobby_set_seed(seed),
            LobbyCmd::ClearRngSeed => self.lobby_clear_seed(),
            LobbyCmd::SitOutNext { player_name } => self.lobby_sit_out_next(player_name),
            LobbyCmd::SitIn { player_name } => self.lobby_sit_in(player_name),
            LobbyCmd::GetLobbyState => self.lobby_view_lobby_state(),
            LobbyCmd::Join { player_name, pin } => self.lobby_join(player_name, pin),
            LobbyCmd::Leave { player_name } => self.lobby_leave(player_name),
            LobbyCmd::StartHand => self.lobby_start_hand(),
            _ => self.lobby_unimplemented(),
        }
    }

    fn lobby_set_variant(&mut self, variant: common::protocol::PokerVariant) -> Vec<Event> {
        if let Err(events) = self.require_step_with_message(
            HandStep::WaitingForPlayers,
            "Cannot change variant during a hand",
        ) {
            return events;
        }

        let spec = crate::variants::spec_for(variant);
        self.table.set_variant_spec(spec);

        vec![self.emit_info(format!(
            "Set poker variant to {:?} (active: {})",
            variant,
            self.table.variant_spec().name
        ))]
    }

    fn lobby_view_state(&mut self) -> Vec<Event> {
        vec![self.emit_hand_summary()]
    }

    fn lobby_view_hand(&mut self, player_name: String) -> Vec<Event> {
        let pid = match self.require_pid_by_name_get_account(&player_name) {
            Ok(pid) => pid,
            Err(events) => return events,
        };

        let cards = match self.table.hand_state().player_hole_cards(&pid) {
            Some(cards) => cards.clone(),
            None => {
                return vec![self.emit_error(format!("No active hand for {}", player_name))];
            }
        };

        vec![Event::PrivateHand {
            player_name,
            cards,
        }]
    }

    fn lobby_set_seed(&mut self, seed: u64) -> Vec<Event> {
        self.rng_seed = Some(seed);
        vec![self.emit_info(format!("RNG seed set to {}", seed))]
    }

    fn lobby_clear_seed(&mut self) -> Vec<Event> {
        self.rng_seed = None;
        vec![self.emit_info("RNG seed cleared (random shuffle)")]
    }

    fn lobby_sit_out_next(&mut self, player_name: String) -> Vec<Event> {
        if let Err(events) =
            self.require_step_with_message(HandStep::WaitingForPlayers, "Cannot sit out during a hand")
        {
            return events;
        }

        let (pid, pname) = match self.require_pid_and_name_by_name_get_account(&player_name) {
            Ok(x) => x,
            Err(events) => return events,
        };

        if let Err(events) = self.require_seated(pid, &pname) {
            return events;
        }

        self.sit_out_next.insert(pid);
        vec![self.emit_info(format!("{} will sit out next hand", pname))]
    }

    fn lobby_sit_in(&mut self, player_name: String) -> Vec<Event> {
        if let Err(events) = self.require_step_with_message(
            HandStep::WaitingForPlayers,
            "Cannot change sit-out during a hand",
        ) {
            return events;
        }

        let (pid, pname) = match self.require_pid_and_name_by_name_get_account(&player_name) {
            Ok(x) => x,
            Err(events) => return events,
        };

        self.sit_out_next.remove(&pid);
        vec![self.emit_info(format!("{} will play the next hand", pname))]
    }

    fn lobby_view_lobby_state(&mut self) -> Vec<Event> {
        vec![self.emit_lobby_state()]
    }

    fn lobby_join(&mut self, player_name: String, pin: String) -> Vec<Event> {
        if let Err(events) = self.require_step_with_message(
            HandStep::WaitingForPlayers,
            "Cannot join table during hand",
        ) {
            return events;
        }

        match self.accounts.login(&player_name, &pin) {
            Ok(pa) => match self.table.join_table(pa.id) {
                Ok(seat) => vec![
                    Event::PlayerJoined {
                        player_id: pa.id,
                        player_name: pa.name.clone(),
                        seat: seat,
                    },
                    self.emit_lobby_state(),
                ],
                Err(e) => vec![self.emit_error(format!("Seating failed: {:?}", e))],
            },
            Err(e) => vec![self.emit_error(format!("Login failed: {:?}", e))],
        }
    }

    fn lobby_leave(&mut self, player_name: String) -> Vec<Event> {
        let (pid, pname) = match self.require_pid_and_name_by_name_get_account(&player_name) {
            Ok(x) => x,
            Err(events) => return events,
        };

        if self.table.current_step() == HandStep::WaitingForPlayers {
            self.sit_out_next.remove(&pid);
            match self.table.leave_table(pid) {
                Ok(_) => vec![
                    Event::PlayerLeft {
                        player_id: pid,
                        player_name: pname.clone(),
                    },
                    self.emit_lobby_state(),
                ],
                Err(e) => vec![self.emit_error(format!("Leave failed: {:?}", e))],
            }
        } else {
            self.handle_midhand_leave(pid, &pname)
        }
    }

    fn lobby_start_hand(&mut self) -> Vec<Event> {
        if let Err(events) = self.require_step_with_message(
            HandStep::WaitingForPlayers,
            "Hand already in progress",
        ) {
            return events;
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
            return vec![self.emit_error("need 2–5 players to start")];
        }

        // Confirm that each player has enough balance to cover the initial forced bet (ante or bring-in depending on variant)
        for player_id in &eligible {
            if !self.has_enough_to_start_hand(*player_id) {
                return vec![self.emit_error(format!(
                    "Player {} does not have enough balance for required forced beta",
                    player_id
                ))];
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
        let mut events = vec![self.emit_step_changed()];

        events.extend(self.run_automatic_steps());

        events
    }

    fn lobby_unimplemented(&mut self) -> Vec<Event> {
        vec![self.emit_error("Lobby command not implemented yet")]
    }

    fn handle_midhand_leave(&mut self, pid: PlayerId, pname: &str) -> Vec<Event> {
        self.handle_player_removed_midhand(pid, pname)
    }

    // Single entry point for "player removed mid-hand" behavior.
    fn handle_player_removed_midhand(&mut self, pid: PlayerId, pname: &str) -> Vec<Event> {
        let mut events = Vec::new();

        if let Err(events) = self.require_seated(pid, pname) {
            return events;
        }

        let removal = self.apply_midhand_fold_equivalent(pid);
        self.update_midhand_leave_flags(pid);

        events.push(self.emit_info(format!(
            "{pname} disconnected: folded and will leave after the hand"
        )));

        if let Some(fast_forward_events) = self.fast_forward_if_single_active_player_remains() {
            events.extend(fast_forward_events);
            return events;
        }

        events.extend(self.continue_after_midhand_player_removed(pid, removal));
        events
    }

    fn apply_midhand_fold_equivalent(&mut self, pid: PlayerId) -> MidhandRemovalState {
        // Treat a disconnecting leave like a fold for the current hand.
        self.table.hand_state_mut().set_player_fold_status(pid, true);
        if let Some(br) = self.table.hand_state_mut().betting_mut() {
            br.pending.remove(&pid);
        }

        let mut draw_removed_current = false;
        let mut draw_done = false;
        let mut next_draw_pid: Option<PlayerId> = None;
        if let Some(dr) = self.table.hand_state_mut().draw_mut() {
            draw_removed_current = dr.remove_player(pid);
            draw_done = dr.is_done();
            if draw_removed_current && !draw_done {
                next_draw_pid = Some(dr.current_player());
            }
        }

        MidhandRemovalState {
            draw_removed_current,
            draw_done,
            next_draw_pid,
        }
    }

    fn update_midhand_leave_flags(&mut self, pid: PlayerId) {
        let _ = self.table.leave_table(pid); // queues leave if round in progress
        self.sit_out_next.remove(&pid);
    }

    fn fast_forward_if_single_active_player_remains(&mut self) -> Option<Vec<Event>> {
        let active = self.active_players_in_hand();
        if active.len() != 1 {
            return None;
        }

        let mut events = Vec::new();
        self.table.hand_state_mut().end_betting_round();

        while self.table.current_step() != HandStep::Showdown
            && self.table.current_step() != HandStep::Payout
        {
            self.table.advance_table_state();
        }
        events.push(self.emit_step_changed());
        events.extend(self.run_automatic_steps());

        Some(events)
    }

    fn continue_after_midhand_player_removed(
        &mut self,
        pid: PlayerId,
        removal: MidhandRemovalState,
    ) -> Vec<Event> {
        let mut events = Vec::new();

        match self.table.current_step() {
            HandStep::BettingRound1 | HandStep::BettingRound2 => {
                if let Some(br) = self.table.hand_state().betting() {
                    if br.pending.is_empty() {
                        self.table.hand_state_mut().end_betting_round();
                        self.table.advance_table_state();
                        events.push(self.emit_step_changed());
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
                        events.push(self.emit_action_requested(next_pid));
                    }
                }
            }
            HandStep::DrawPhase => {
                if removal.draw_done {
                    self.table.hand_state_mut().end_draw_round();
                    self.table.advance_table_state();
                    events.push(self.emit_step_changed());
                    events.extend(self.enter_betting_round());
                    return events;
                }

                if removal.draw_removed_current {
                    if let Some(next_pid) = removal.next_draw_pid {
                        events.push(self.emit_draw_requested(next_pid));
                    }
                }
            }
            _ => {}
        }

        events
    }
}


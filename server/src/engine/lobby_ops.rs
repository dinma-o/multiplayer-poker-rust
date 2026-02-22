use super::*;

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
        // Nothing fancy yet
        vec![Event::Info {
            message: format!("Set poker variant to {:?}", variant),
        }]
    }

    fn lobby_view_state(&mut self) -> Vec<Event> {
        vec![self.hand_state_summary_event()]
    }

    fn lobby_view_hand(&mut self, player_name: String) -> Vec<Event> {
        let pid = match self.require_pid_by_name_get_account(&player_name) {
            Ok(pid) => pid,
            Err(events) => return events,
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

    fn lobby_set_seed(&mut self, seed: u64) -> Vec<Event> {
        self.rng_seed = Some(seed);
        vec![Event::Info {
            message: format!("RNG seed set to {}", seed),
        }]
    }

    fn lobby_clear_seed(&mut self) -> Vec<Event> {
        self.rng_seed = None;
        vec![Event::Info {
            message: "RNG seed cleared (random shuffle)".into(),
        }]
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
        vec![Event::Info {
            message: format!("{} will sit out next hand", pname),
        }]
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
        vec![Event::Info {
            message: format!("{} will play the next hand", pname),
        }]
    }

    fn lobby_view_lobby_state(&mut self) -> Vec<Event> {
        vec![self.lobby_state_event()]
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
            return vec![Event::Error {
                message: "need 2–5 players to start".into(),
            }];
        }

        // Confirm that each player has enough balance to cover the ante
        for player_id in &eligible {
            if !self.has_enough_for_ante(*player_id) {
                return vec![Event::Error {
                    message: format!("Player {} does not have enough balance for ante", player_id),
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

    fn lobby_unimplemented(&mut self) -> Vec<Event> {
        vec![Event::Error {
            message: "Lobby command not implemented yet".into(),
        }]
    }

    fn handle_midhand_leave(&mut self, pid: PlayerId, pname: &str) -> Vec<Event> {
        let mut events = Vec::new();

        if let Err(events) = self.require_seated(pid, pname) {
            return events;
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
}


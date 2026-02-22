use super::*;

impl ServerEngine {
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

}


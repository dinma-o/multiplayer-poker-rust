use super::*;

impl ServerEngine {
    pub fn apply_draw_action(&mut self, cmd: ActionCmd) -> Vec<Event> {
        let mut events = Vec::new();

        // Check that we are in a draw phase/step
        if let Err(events) = self.require_step_with_message(HandStep::DrawPhase, "Not in draw phase")
        {
            return events;
        }

        // Resolve name -> pid
        let (player_name, discard) = match cmd {
            ActionCmd::Draw {
                player_name,
                discard,
            } => (player_name, discard),
            _ => {
                return vec![self.emit_error("Only Draw is allowed in DrawPhase")];
            }
        };

        let pid = match self.require_pid_by_name_get_account(&player_name) {
            Ok(pid) => pid,
            Err(events) => return events,
        };

        if let Err(events) = self.require_draw_turn(pid) {
            return events;
        }

        // Validate discard list
        if discard.len() > five_card_draw::MAX_DISCARDS {
            return vec![self.emit_error("Too many discards")];
        }

        let mut seen = std::collections::HashSet::new();
        for &i in &discard {
            if i >= 5 {
                return vec![self.emit_error("Discard index out of range (0..4)")];
            }
            if !seen.insert(i) {
                return vec![self.emit_error("Duplicate discard index")];
            }
        }

        let hand = match self.table.hand_state().player_hand(&pid) {
            Some(h) => h.clone(),
            None => {
                return vec![self.emit_error("Player hand not found")];
            }
        };

        let mut new_hand = hand.clone();
        new_hand.replace_from_deck(&discard, &mut self.deck);

        // Store updated hand
        self.table
            .hand_state_mut()
            .set_player_hand(pid, new_hand.clone());

        // Emit private hand event
        let cards = self
            .table
            .hand_state()
            .player_hole_cards(&pid)
            .cloned()
            .unwrap_or_else(|| new_hand.cards().to_vec());
        events.push(Event::PrivateHand {
            player_name: player_name.clone(),
            cards,
        });

        // Advance draw turn
        {
            let dr = self.table.hand_state_mut().draw_mut().unwrap();
            dr.advance();
            if dr.is_done() {
                self.table.hand_state_mut().end_draw_round();

                // Move to BettingRound2
                self.table.advance_table_state();
                events.push(self.emit_step_changed());

                // Initialize betting round 2 and prompt
                events.extend(self.enter_betting_round());
                return events;
            }
        }

        // Prompt next drawer
        let next_pid = self.table.hand_state().draw().unwrap().current_player();
        events.push(self.emit_draw_requested(next_pid));

        events
    }

}


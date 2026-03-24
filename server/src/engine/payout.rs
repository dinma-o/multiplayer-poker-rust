use super::*;

impl GameEngine {
    pub(super) fn do_payout(&mut self) -> Vec<Event> {
        let mut events = Vec::new();

        // Players who participated in the hand = those we dealt to
        let participants = self.table.hand_state().players_with_hands();
        if participants.len() < 2 {
            // hand can't really exist, but don't crash
            self.table.hand_state_mut().reset_for_new_deal();
            return vec![self.emit_error("Payout: not enough participants")];
        }

        // Determine active (not folded)
        let active: Vec<PlayerId> = participants
            .iter()
            .copied()
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect();

        if active.is_empty() {
            self.table.hand_state_mut().reset_for_new_deal();
            return vec![self.emit_error("Payout: no active players")];
        }

        // Rank active hands and find max (TA clarification on limited tie-breaking)
        let mut best_rank = None;
        let mut winners: Vec<PlayerId> = Vec::new();

        for pid in &active {
            let r = match self.rank_player_for_current_variant(*pid) {
                Some(rank) => rank,
                None => {
                    continue;
                }
            };

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
            return vec![self.emit_error("Payout: could not determine winner")];
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
            let pname = self.account_name(*pid);

            if let Some(name) = pname.into() {
                let _ = self.with_accounts_mut(|accounts| accounts.record_game_played(&name));
            }
        }

        // pot_won for winners + deposit money
        for (pid, amt) in &payout_pairs {
            let pname = self.account_name(*pid);

            if let Some(name) = pname.into() {
                if *amt > 0 {
                    let _ = self.with_accounts_mut(|accounts| accounts.deposit(&name, *amt));
                }
                let _ = self.with_accounts_mut(|accounts| accounts.record_pot_won(&name));
            }
        }

        // folds for those who folded
        for pid in &participants {
            if self.table.hand_state().has_player_folded(pid) {
                let pname = self.account_name(*pid);

                if let Some(name) = pname.into() {
                    let _ = self.with_accounts_mut(|accounts| accounts.record_fold(&name));
                }
            }
        }

        // Emit payout event
        events.push(Event::Payout {
            winners: payout_pairs
                .into_iter()
                .map(|(pid, amt)| common::protocol::PayoutLine {
                    player_name: self.account_name(pid),
                    player_id: pid,
                    amount: amt,
                })
                .collect(),
        });

        // Reset for next hand
        self.table.hand_state_mut().reset_after_hand();
        self.table.hand_state_mut().end_betting_round(); // safe no-op if None

        // Rotate dealer for next hand
        self.table.advance_dealer();

        // Also: pot is now effectively 0; you can emit pot updated if you want
        events.push(self.emit_pot_updated(0));

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

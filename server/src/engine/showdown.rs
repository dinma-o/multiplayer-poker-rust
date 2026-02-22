use super::*;

impl ServerEngine {
    pub(super) fn do_showdown(&mut self) -> Vec<Event> {
        // Active players = players who still have hands and did not fold
        let active: Vec<PlayerId> = self
            .table
            .hand_state()
            .players_with_hands()
            .into_iter()
            .filter(|pid| !self.table.hand_state().has_player_folded(pid))
            .collect();

        if active.is_empty() {
            return vec![self.emit_error("Showdown: no active players")];
        }

        // Rank all active hands + build showdown lines
        let mut ranked: Vec<(PlayerId, common::hand_rank::HandRank)> = Vec::new();
        let mut lines: Vec<common::protocol::ShowdownLine> = Vec::new();

        for pid in &active {
            let r = match self.rank_player_for_current_variant(*pid) {
                Some(rank) => rank,
                None => continue,
            };
            ranked.push((*pid, r));

            let desc = match self.table.variant_spec() == crate::variants::texas_holdem::SPEC {
                true => self
                    .holdem_showdown_desc(*pid, r)
                    .unwrap_or_else(|| format!("best={:?}", r.category)),
                false => match self.table.hand_state().player_hand(pid) {
                    Some(hand) => format!("{} => {:?} {:?}", hand, r.category, r.tiebreakers),
                    None => format!("{:?} {:?}", r.category, r.tiebreakers),
                },
            };
            let pname = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());
            lines.push(ShowdownLine {
                player_id: *pid,
                player_name: pname,
                hand_desc: desc,
            });
        }

        // If somehow ranked is empty
        if ranked.is_empty() {
            return vec![self.emit_error("Showdown: no hands to rank")];
        }

        vec![Event::Showdown { results: lines }]
    }

    pub(super) fn rank_player_for_current_variant(&self, pid: PlayerId) -> Option<hand_rank::HandRank> {
        if self.table.variant_spec() == crate::variants::texas_holdem::SPEC {
            let seven = self.holdem_seven_cards_for_player(pid)?;
            return Some(hand_rank::best_hand_from_seven_cards(&seven));
        }

        let hand = self.table.hand_state().player_hand(&pid)?;
        Some(hand_rank::rank_five_card_hand(&hand))
    }

    fn holdem_seven_cards_for_player(&self, pid: PlayerId) -> Option<[common::card_deck::Card; 7]> {
        let hole = self.table.hand_state().player_hole_cards(&pid)?.clone();
        let board = self.table.hand_state().board_cards();

        if hole.len() != 2 || board.len() != 5 {
            return None;
        }

        let mut cards = Vec::with_capacity(7);
        cards.extend(hole.iter().copied());
        cards.extend(board.iter().copied());
        cards.try_into().ok()
    }

    fn holdem_showdown_desc(&self, pid: PlayerId, rank: hand_rank::HandRank) -> Option<String> {
        let hole = self.table.hand_state().player_hole_cards(&pid)?;
        let board = self.table.hand_state().board_cards();
        if hole.len() != 2 || board.len() != 5 {
            return None;
        }

        let hole_text = hole.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        let board_text = board.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        Some(format!(
            "board={:?} hole={:?} => {:?}",
            board_text, hole_text, rank.category
        ))
    }
}


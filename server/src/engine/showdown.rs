use super::*;

impl ServerEngine {
    pub(super) fn do_showdown(&mut self) -> Vec<Event> {
        use common::hand_rank::rank_five_card_hand;

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
            let hand = match self.table.hand_state().player_hand(pid) {
                Some(h) => h,
                None => {
                    continue;
                }
            };

            let r = rank_five_card_hand(&hand); // Hand<5> alias matches
            ranked.push((*pid, r));

            // Minimal readable description for CLI
            let desc = format!("{} => {:?} {:?}", hand, r.category, r.tiebreakers);
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
}


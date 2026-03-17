use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepKind {
    Waiting,
    ForcedBets,
    Deal,
    Betting,
    DrawDiscard,
    Showdown,
    Payout,
}

impl ServerEngine {
    pub fn run_automatic_steps(&mut self) -> Vec<Event> {
        // Go through the steps in the state machine that don't require player input starting at the current step.
        let mut events = Vec::new();
        loop {
            let step = self.table.current_step();
            if self.is_decision_step(step) {
                events.extend(self.on_enter_decision_step(step));
                break;
            }

            let mut made_progress = false;

            let (progressed, step_events) = self.auto_collect_antes();
            made_progress |= progressed;
            events.extend(step_events);

            let step = self.table.current_step();
            if self.is_decision_step(step) {
                events.extend(self.on_enter_decision_step(step));
                break;
            }

            let (progressed, step_events) = self.auto_deal_private();
            made_progress |= progressed;
            events.extend(step_events);

            let step = self.table.current_step();
            if self.is_decision_step(step) {
                events.extend(self.on_enter_decision_step(step));
                break;
            }

            let (progressed, step_events) = self.auto_deal_community();
            made_progress |= progressed;
            events.extend(step_events);

            let step = self.table.current_step();
            if self.is_decision_step(step) {
                events.extend(self.on_enter_decision_step(step));
                break;
            }

            let (progressed, step_events) = self.auto_showdown_if_ready();
            made_progress |= progressed;
            events.extend(step_events);

            let (payout_progressed, step_events) = self.auto_payout_if_ready();
            made_progress |= payout_progressed;
            events.extend(step_events);

            if payout_progressed {
                let (_progressed, step_events) = self.auto_apply_pending_joins_leaves_if_hand_over();
                events.extend(step_events);
                break; // stop auto-run after finishing a hand
            }

            if !made_progress {
                break;
            }
        }

        events
    }

    fn auto_collect_antes(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::CollectForcedBets {
            return (false, vec![]);
        }

        if self.table.variant_spec() == crate::variants::texas_holdem::SPEC {
            return self.auto_collect_holdem_blinds();
        }

        let mut events = Vec::new();
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
            let bal = self.accounts.get_by_id(*pid).map(|a| a.balance).unwrap_or(0);
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
                    player_name: self.accounts.name_from_id(*pid).unwrap_or("Unknown".into()),
                });
                events.push(self.emit_info(format!(
                    "Player {} kicked for insufficient funds",
                    pid
                )));
            }
            // Reset table state so lobby can recover
            self.table.reset_to_waiting();
            self.table.apply_pending_requests();

            events.push(self.emit_error("Not enough players can pay ante to start hand"));
            events.push(self.emit_pot_updated(0));
            events.push(self.emit_step_changed_to(HandStep::WaitingForPlayers));
            events.push(self.emit_lobby_state());
            return (true, events);
        }

        for pid in &payers {
            let _ = self.accounts.withdraw_by_id(*pid, ante.try_into().unwrap_or(5));
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

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_collect_holdem_blinds(&mut self) -> (bool, Vec<Event>) {
        let mut events = Vec::new();
        let sb_amount = crate::variants::texas_holdem::SMALL_BLIND;
        let bb_amount = crate::variants::texas_holdem::BIG_BLIND;

        // Ensure the dealer button points to an occupied seat before deriving blind order.
        self.table.ensure_dealer_in_seats();

        let eligible_in_order: Vec<PlayerId> = self
            .table
            .action_order()
            .into_iter()
            .filter(|pid| !self.sit_out_next.contains(pid))
            .collect();

        if eligible_in_order.len() < (five_card_draw::MIN_PLAYERS as usize) {
            self.table.reset_to_waiting();
            self.table.apply_pending_requests();

            events.push(self.emit_error("Not enough players to post blinds and start hand"));
            events.push(self.emit_pot_updated(0));
            events.push(self.emit_step_changed_to(HandStep::WaitingForPlayers));
            events.push(self.emit_lobby_state());
            return (true, events);
        }

        let (sb_pid, bb_pid) = Self::holdem_blind_payers_in_dealer_order(&eligible_in_order)
            .expect("eligible_in_order length checked");

        let sb_balance = self.accounts.get_by_id(sb_pid).map(|a| a.balance).unwrap_or(0);
        if sb_balance < sb_amount as i64 {
            self.table.reset_to_waiting();
            self.table.apply_pending_requests();
            return (
                true,
                vec![
                    self.emit_error("Small blind post failed: insufficient funds"),
                    self.emit_pot_updated(0),
                    self.emit_step_changed_to(HandStep::WaitingForPlayers),
                    self.emit_lobby_state(),
                ],
            );
        }

        let bb_balance = self.accounts.get_by_id(bb_pid).map(|a| a.balance).unwrap_or(0);
        if bb_balance < bb_amount as i64 {
            self.table.reset_to_waiting();
            self.table.apply_pending_requests();
            return (
                true,
                vec![
                    self.emit_error("Big blind post failed: insufficient funds"),
                    self.emit_pot_updated(0),
                    self.emit_step_changed_to(HandStep::WaitingForPlayers),
                    self.emit_lobby_state(),
                ],
            );
        }

        if let Err(e) = self.accounts.withdraw_by_id(sb_pid, sb_amount as i64) {
            return (true, vec![self.emit_error(format!("Small blind post failed: {:?}", e))]);
        }

        if let Err(e) = self.accounts.withdraw_by_id(bb_pid, bb_amount as i64) {
            return (true, vec![self.emit_error(format!("Big blind post failed: {:?}", e))]);
        }

        self.table.hand_state_mut().add_to_pot(sb_amount + bb_amount);
        events.push(self.emit_current_pot_updated());

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn holdem_blind_payers_in_dealer_order(order: &[PlayerId]) -> Option<(PlayerId, PlayerId)> {
        match order.len() {
            0 | 1 => None,
            2 => Some((order[1], order[0])), // heads-up: dealer posts SB, other posts BB
            _ => Some((order[0], order[1])),
        }
    }

    fn auto_deal_private(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::DealPrivate {
            return (false, vec![]);
        }

        if self.table.variant_spec() == crate::variants::texas_holdem::SPEC {
            return self.auto_deal_private_texas_holdem();
        }

        else if self.table.variant_spec() == crate::variants::seven_card_stud::SPEC {
            // Stud is more complex and not fully implemented, so we'll skip auto-deal for now.
            return self.auto_deal_private_seven_card_stud();
        }

        self.auto_deal_private_five_card_draw()
    }

    fn auto_deal_private_five_card_draw(&mut self) -> (bool, Vec<Event>) {
        let mut events = Vec::new();
        self.table.hand_state_mut().reset_for_new_deal();

        // If dealer seat was vacated (e.g., insufficient ante), move to next occupied seat
        self.table.ensure_dealer_in_seats();

        // Deal in action order (left of dealer first)
        let seated: Vec<PlayerId> = self.table.action_order();
        let sit_out = self.sit_out_next.clone();

        for pid in seated {
            if sit_out.contains(&pid) {
                self.table.hand_state_mut().set_player_fold_status(pid, true);
                continue;
            }
            let name = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
            let hand = Hand::<5>::draw_from_deck(&mut self.deck).unwrap();

            self.table.hand_state_mut().set_player_hand(pid, hand.clone());
            self.table.hand_state_mut().set_player_fold_status(pid, false);

            let cards = self
                .table
                .hand_state()
                .player_hole_cards(&pid)
                .cloned()
                .unwrap_or_else(|| hand.cards().to_vec());
            events.push(Event::PrivateHand {
                player_name: name,
                cards, // or hand.cards().to_vec() depending on your API
            });
        }

        // Sit-out applies to this hand only
        self.sit_out_next.clear();

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_private_texas_holdem(&mut self) -> (bool, Vec<Event>) {
        let mut events = Vec::new();
        self.table.hand_state_mut().reset_for_new_deal();

        self.table.ensure_dealer_in_seats();

        let seated: Vec<PlayerId> = self.table.action_order();
        let sit_out = self.sit_out_next.clone();
        let active: Vec<PlayerId> = seated
            .iter()
            .copied()
            .filter(|pid| !sit_out.contains(pid))
            .collect();

        for pid in &seated {
            if sit_out.contains(pid) {
                self.table.hand_state_mut().set_player_fold_status(*pid, true);
            } else {
                self.table.hand_state_mut().set_player_fold_status(*pid, false);
                self.table.hand_state_mut().set_player_hole_cards(*pid, Vec::new());
            }
        }

        // Deal Hold'em hole cards round-robin (one card to each player, twice).
        for _ in 0..2 {
            for pid in &active {
                if let Some(card) = self.deck.deal_card() {
                    let mut cards = self
                        .table
                        .hand_state()
                        .player_hole_cards(pid)
                        .cloned()
                        .unwrap_or_default();
                    cards.push(card);
                    self.table.hand_state_mut().set_player_hole_cards(*pid, cards);
                }
            }
        }
        // Hold'em hole cards are private: only shown via ViewHand.

        self.sit_out_next.clear();

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_private_seven_card_stud(&mut self) -> (bool, Vec<Event>) {
        // Placeholder for now since Stud implementation is incomplete. Just advance the state to allow testing of the flow.
        let in_third_street = self.table.hand_state_mut().players_with_hands().is_empty();
        if in_third_street {
            return self.auto_deal_private_seven_card_stud_third_street();
        }
        else{
            return self.auto_deal_private_seven_card_stud_seventh_street();
        }
    }

    fn auto_deal_private_seven_card_stud_third_street(&mut self) -> (bool, Vec<Event>) {

        let mut events: Vec<Event> = Vec::new();
        self.table.hand_state_mut().reset_for_new_deal();

        self.table.ensure_dealer_in_seats();

        let seated: Vec<PlayerId> = self.table.action_order();
        let sit_out = self.sit_out_next.clone();

        let active: Vec<PlayerId> = seated
            .iter()
            .copied()
            .filter(|pid| !sit_out.contains(pid))
            .collect();

        for pid in &seated {
            if sit_out.contains(pid) {
                self.table.hand_state_mut().set_player_fold_status(*pid, true);
            } else {
                self.table.hand_state_mut().set_player_fold_status(*pid, false);
                self.table.hand_state_mut().set_player_hole_cards(*pid, Vec::new());
                self.table.hand_state_mut().set_player_up_cards(*pid, Vec::new());
            }
        }

        // Deal Stud third street cards: 2 down (private) + 1 up (public) to each active player.
        for _ in 0..2 {
            for pid in &active {
                if let Some(card) = self.deck.deal_card() {
                    let mut hole_cards = self
                        .table
                        .hand_state()
                        .player_hole_cards(pid)
                        .cloned()
                        .unwrap_or_default();
                    hole_cards.push(card);
                    self.table.hand_state_mut().set_player_hole_cards(*pid, hole_cards);
                }
            }
        }
        for pid in &active {
            if let Some(card) = self.deck.deal_card() {
                let mut up_cards = self
                    .table
                    .hand_state()
                    .player_up_cards(pid)
                    .cloned()
                    .unwrap_or_default();
                up_cards.push(card);
                self.table.hand_state_mut().set_player_up_cards(*pid, up_cards);
            }
        }

        for pid in &active{
            let name = self.accounts.name_from_id(*pid).unwrap_or("Unknown".into());
            let hole_cards = self
                .table
                .hand_state()
                .player_hole_cards(pid)
                .cloned()
                .unwrap_or_default();
            events.push(Event::PrivateHand {
                player_name: name,
                cards: hole_cards,
            });
        }

        events.push(self.emit_info(format!(
            "Stud third street: {}",
            self.stud_visible_boards_summary()
        )));

        self.sit_out_next.clear();
        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_private_seven_card_stud_seventh_street(&mut self) -> (bool, Vec<Event>){
        let mut events: Vec<Event> = Vec::new();
        self.table.ensure_dealer_in_seats();

        let seated: Vec<PlayerId> = self.table.action_order();
        let sit_out = self.sit_out_next.clone();

        let active: Vec<PlayerId> = seated
            .iter()
            .copied()
            .filter(|pid| !sit_out.contains(pid))
            .collect();

        for pid in &seated {
            if sit_out.contains(pid) {
                self.table.hand_state_mut().set_player_fold_status(*pid, true);
            } else {
                self.table.hand_state_mut().set_player_fold_status(*pid, false);
            }
        }

        for pid in &active {
            if let Some(card) = self.deck.deal_card() {
                let mut hole_cards = self
                    .table
                    .hand_state()
                    .player_hole_cards(pid)
                    .cloned()
                    .unwrap_or_default();
                hole_cards.push(card);
                self.table.hand_state_mut().set_player_hole_cards(*pid, hole_cards);
            }
        }

        events.push(self.emit_info(format!(
            "Stud seventh street: {}",
            self.stud_visible_boards_summary()
        )));

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_community(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::DealCommunity {
            return (false, vec![]);
        }

        if self.table.variant_spec() != crate::variants::texas_holdem::SPEC {
            return (false, vec![]);
        }

        let mut events = Vec::new();
        let board_len = self.table.hand_state().board_cards().len();

        let (street_name, cards_to_deal) = match board_len {
            0 => ("flop", 3usize),
            3 => ("turn", 1usize),
            4 => ("river", 1usize),
            _ => {
                return (
                    false,
                    vec![self.emit_error(format!(
                        "Hold'em board progression invalid: {} board cards",
                        board_len
                    ))],
                );
            }
        };

        if let Some(burn) = self.deck.deal_card() {
            self.deck.discard(burn);
        }

        for _ in 0..cards_to_deal {
            if let Some(card) = self.deck.deal_card() {
                self.table.hand_state_mut().push_board_card(card);
            }
        }

        let board_text = self
            .table
            .hand_state()
            .board_cards()
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        events.push(self.emit_info(format!(
            "Board ({street_name}): {:?}",
            board_text
        )));

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_showdown_if_ready(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::Showdown {
            return (false, vec![]);
        }

        let mut events = Vec::new();
        events.extend(self.do_showdown());
        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_payout_if_ready(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::Payout {
            return (false, vec![]);
        }

        let mut events = Vec::new();
        events.extend(self.do_payout());
        self.table.advance_table_state(); // should go back to WaitingForPlayers
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_apply_pending_joins_leaves_if_hand_over(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::WaitingForPlayers {
            return (false, vec![]);
        }

        // Now that we're back to WaitingForPlayers, process queued joins/leaves.
        self.table.apply_pending_requests();
        (true, vec![])
    }

    fn stud_visible_boards_summary(&self) -> String {
        let mut summary = String::new();
        for pid in self.table.hand_state().players_with_hands() {
            let name = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
            let up_cards = self
                .table
                .hand_state()
                .player_up_cards(&pid)
                .cloned()
                .unwrap_or_default();
            let up_text = up_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
            summary.push_str(&format!("{}: [{}] ", name, up_text));
        }
        summary
    }

    pub fn is_decision_step(&self, step: HandStep) -> bool {
        matches!(
            Self::step_kind(step),
            StepKind::Betting | StepKind::DrawDiscard
        )
    }

    fn step_kind(step: HandStep) -> StepKind {
        match step {
            HandStep::WaitingForPlayers => StepKind::Waiting,
            HandStep::CollectForcedBets => StepKind::ForcedBets,
            HandStep::DealPrivate | HandStep::DealCommunity | HandStep::DealFaceUp => StepKind::Deal,
            HandStep::BettingRound1 | HandStep::BettingRound2 | HandStep::BettingRound3 | HandStep::BettingRound4 | HandStep::BettingRound5 => StepKind::Betting,
            HandStep::DrawPhase => StepKind::DrawDiscard,
            HandStep::Showdown => StepKind::Showdown,
            HandStep::Payout => StepKind::Payout,
        }
    }

    pub(super) fn on_enter_decision_step(&mut self, step: HandStep) -> Vec<Event> {
        match step {
            HandStep::BettingRound1 | HandStep::BettingRound2 | HandStep::BettingRound3 | HandStep::BettingRound4 | HandStep::BettingRound5 => self.enter_betting_round(),
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
        let street = match self.table.current_step() {
            HandStep::BettingRound1 => Street::FirstBetting,
            HandStep::BettingRound2 => Street::SecondBetting,
            HandStep::BettingRound3 => Street::ThirdBetting,
            HandStep::BettingRound4 => Street::FourthBetting,
            HandStep::BettingRound5 => Street::FifthBetting,
            _ => {
                // Preserve behavior for unexpected calls by falling back to default order.
                let order = self.table.action_order();
                return self.enter_betting_round_with_order(order);
            }
        };
        let order = self.table.betting_order(street);
        self.enter_betting_round_with_order(order)
    }

    fn enter_betting_round_with_order(&mut self, order: Vec<PlayerId>) -> Vec<Event> {

        // Filter out to have only active players (not folded and seated)
        let seated_set: HashSet<PlayerId> = self
            .table
            .seated_players()
            .iter()
            .map(|(_, pid)| *pid)
            .collect();
        let mut active_in_order: Vec<PlayerId> = order
            .into_iter()
            .filter(|pid| {
                seated_set.contains(pid) && !self.table.hand_state().has_player_folded(pid)
            })
            .collect();

        let is_holdem_preflop =
            self.table.variant_spec() == crate::variants::texas_holdem::SPEC
            && self.table.current_step() == HandStep::BettingRound1;

        let mut preflop_blinds = None;
        if is_holdem_preflop {
            if let Some((sb_pid, bb_pid)) = Self::holdem_blind_payers_in_dealer_order(&active_in_order)
            {
                preflop_blinds = Some((sb_pid, bb_pid));
                if let Some(bb_idx) = active_in_order.iter().position(|pid| *pid == bb_pid) {
                    let first_to_act_idx = (bb_idx + 1) % active_in_order.len();
                    active_in_order.rotate_left(first_to_act_idx);
                }
            }
        }

        // Initialize betting round state
        self.table
            .hand_state_mut()
            .start_betting_round(active_in_order);

        if let Some((sb_pid, bb_pid)) = preflop_blinds {
            self.table
                .hand_state_mut()
                .set_current_bet(crate::variants::texas_holdem::BIG_BLIND);
            if let Some(br) = self.table.hand_state_mut().betting_mut() {
                br.contrib.insert(sb_pid, crate::variants::texas_holdem::SMALL_BLIND);
                br.contrib.insert(bb_pid, crate::variants::texas_holdem::BIG_BLIND);
            }
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

        let br = self.table.hand_state_mut().betting_mut().unwrap();
        while !br.pending.is_empty() && !is_active(br.current_player()) {
            Self::advance_to_next_actor_local(br, &is_active);
        }

        // Emit ActionRequested for the first player to act
        let first_player = self.table.hand_state().betting().unwrap().current_player();
        vec![self.emit_action_requested(first_player)]
    }
}


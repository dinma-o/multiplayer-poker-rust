use crate::variants;

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

impl GameEngine {
    /// Advances the state machine through every automatic phase until player input is required or
    /// a hand fully ends.
    pub fn run_automatic_steps(&mut self) -> Vec<Event> {
        // Each helper is allowed to advance the table step, so we re-check after every call
        // instead of assuming a single fixed flow across all variants.
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

            let (progressed, step_events) = self.auto_deal_face_up();
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
                let (_progressed, step_events) =
                    self.auto_apply_pending_joins_leaves_if_hand_over();
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
        let ante = self
            .variant_ante_amount()
            .unwrap_or(five_card_draw::ANTE_AMOUNT);

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
            let bal = self.account_by_id(*pid).map(|a| a.balance).unwrap_or(0);
            if bal < ante.try_into().unwrap_or(5) {
                kicked.push(*pid);
            } else {
                payers.push(*pid);
            }
        }

        if payers.len() < self.variant_min_players() {
            for pid in &kicked {
                let _ = self.table.leave_table(*pid);
                events.push(Event::PlayerLeft {
                    player_id: *pid,
                    player_name: self.account_name(*pid),
                });
                events
                    .push(self.emit_info(format!("Player {} kicked for insufficient funds", pid)));
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
            let _ = self.with_accounts_mut(|accounts| {
                accounts.withdraw_by_id(*pid, ante.try_into().unwrap_or(5))
            });
        }

        for pid in &kicked {
            let name = self.account_name(*pid);
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

        if eligible_in_order.len() < self.variant_min_players() {
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

        let sb_balance = self.account_by_id(sb_pid).map(|a| a.balance).unwrap_or(0);
        if sb_balance < (sb_amount as i64) {
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

        let bb_balance = self.account_by_id(bb_pid).map(|a| a.balance).unwrap_or(0);
        if bb_balance < (bb_amount as i64) {
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

        if let Err(e) =
            self.with_accounts_mut(|accounts| accounts.withdraw_by_id(sb_pid, sb_amount as i64))
        {
            return (
                true,
                vec![self.emit_error(format!("Small blind post failed: {:?}", e))],
            );
        }

        if let Err(e) =
            self.with_accounts_mut(|accounts| accounts.withdraw_by_id(bb_pid, bb_amount as i64))
        {
            return (
                true,
                vec![self.emit_error(format!("Big blind post failed: {:?}", e))],
            );
        }

        self.table
            .hand_state_mut()
            .add_to_pot(sb_amount + bb_amount);
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

        if self.is_stud_variant() {
            return self.auto_deal_private_seven_card_stud();
        }

        if self.is_holdem_variant() {
            return self.auto_deal_private_texas_holdem();
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

        let active: Vec<PlayerId> = seated
            .iter()
            .copied()
            .filter(|pid| !sit_out.contains(pid))
            .collect();

        for pid in &seated {
            if sit_out.contains(pid) {
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, true);
            } else {
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, false);
                self.table
                    .hand_state_mut()
                    .set_player_hole_cards(*pid, Vec::new());
            }
        }

        // Deal 5 hole cards round-robin (one card to each player, five times).
        for _ in 0..5 {
            for pid in &active {
                let _ = self.deal_down_card_to_player(
                    &mut events,
                    AnimationStreet::FiveCardDrawInitial,
                    *pid,
                );
            }
        }

        // Emit PrivateHand events with hole cards for each player. For draw, hole cards are private until showdown, so only include them in the event for the respective player.
        for pid in &active {
            let name = self.account_name(*pid);
            let cards = self
                .table
                .hand_state()
                .player_hole_cards(pid)
                .cloned()
                .unwrap_or_else(Vec::new);
            events.push(Event::PrivateHand {
                player_name: name,
                cards,
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
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, true);
            } else {
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, false);
                self.table
                    .hand_state_mut()
                    .set_player_hole_cards(*pid, Vec::new());
            }
        }

        // Deal Hold'em hole cards round-robin (one card to each player, twice).
        for _ in 0..2 {
            for pid in &active {
                let _ = self.deal_down_card_to_player(
                    &mut events,
                    AnimationStreet::HoldemPreflop,
                    *pid,
                );
            }
        }
        // Hold'em hole cards are private: only shown via ViewHand.

        self.sit_out_next.clear();

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_private_seven_card_stud(&mut self) -> (bool, Vec<Event>) {
        let is_third_street = self.table.hand_state().players_with_hands().is_empty();
        if is_third_street {
            return self.auto_deal_private_seven_card_stud_third_street();
        }

        self.auto_deal_private_seven_card_stud_seventh_street()
    }

    fn auto_deal_private_seven_card_stud_third_street(&mut self) -> (bool, Vec<Event>) {
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
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, true);
            } else {
                self.table
                    .hand_state_mut()
                    .set_player_fold_status(*pid, false);
                self.table
                    .hand_state_mut()
                    .set_player_hole_cards(*pid, Vec::new());
                self.table
                    .hand_state_mut()
                    .set_player_up_cards(*pid, Vec::new());
            }
        }

        for _ in 0..2 {
            for pid in &active {
                let _ = self.deal_down_card_to_player(
                    &mut events,
                    AnimationStreet::StudThirdDown,
                    *pid,
                );
            }
        }

        for pid in &active {
            let _ = self.deal_up_card_to_player(&mut events, AnimationStreet::StudThirdUp, *pid);
        }

        for pid in &active {
            let name = self.account_name(*pid);
            let cards = self
                .table
                .hand_state()
                .player_hole_cards(pid)
                .cloned()
                .unwrap_or_default();
            events.push(Event::PrivateHand {
                player_name: name,
                cards,
            });
        }

        events.push(self.emit_info(format!(
            "Stud third visible street: {}",
            self.stud_visible_boards_summary()
        )));

        self.sit_out_next.clear();

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_private_seven_card_stud_seventh_street(&mut self) -> (bool, Vec<Event>) {
        let mut events = Vec::new();
        self.burn_one_card();

        let active: Vec<PlayerId> = self
            .table
            .action_order()
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid))
            .collect();

        for pid in &active {
            let _ = self.deal_down_card_to_player(&mut events, AnimationStreet::StudSeventh, *pid);
        }

        for pid in &active {
            let name = self.account_name(*pid);
            let cards = self
                .table
                .hand_state()
                .player_hole_cards(pid)
                .cloned()
                .unwrap_or_default();
            events.push(Event::PrivateHand {
                player_name: name,
                cards,
            });
        }

        events.push(self.emit_info("Stud seventh street dealt"));

        self.table.advance_table_state();
        events.push(self.emit_step_changed());

        (true, events)
    }

    fn auto_deal_face_up(&mut self) -> (bool, Vec<Event>) {
        if self.table.current_step() != HandStep::DealFaceUp {
            return (false, vec![]);
        }

        if !self.is_stud_variant() {
            return (false, vec![]);
        }

        let mut events = Vec::new();
        self.burn_one_card();

        let active: Vec<PlayerId> = self
            .table
            .action_order()
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid))
            .collect();
        let street = match active
            .first()
            .and_then(|pid| self.table.hand_state().player_up_cards(pid))
            .map(|cards| cards.len())
        {
            Some(1) => AnimationStreet::StudFourth,
            Some(2) => AnimationStreet::StudFifth,
            Some(3) => AnimationStreet::StudSixth,
            _ => AnimationStreet::StudFourth,
        };

        for pid in &active {
            let _ = self.deal_up_card_to_player(&mut events, street, *pid);
        }

        let street_name = match active
            .first()
            .and_then(|pid| self.table.hand_state().player_up_cards(pid))
            .map(|cards| cards.len())
        {
            Some(2) => "visible fourth street",
            Some(3) => "visible fifth street",
            Some(4) => "visible sixth street",
            _ => "visible next street",
        };

        events.push(self.emit_info(format!(
            "Stud {street_name}: {}",
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
            let _ = self.deal_board_card(
                &mut events,
                match board_len {
                    0 => AnimationStreet::HoldemFlop,
                    3 => AnimationStreet::HoldemTurn,
                    4 => AnimationStreet::HoldemRiver,
                    _ => unreachable!("validated board progression"),
                },
            );
        }

        let board_text = self
            .table
            .hand_state()
            .board_cards()
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        events.push(self.emit_info(format!("Board ({street_name}): {:?}", board_text)));

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
            HandStep::DealPrivate | HandStep::DealFaceUp | HandStep::DealCommunity => {
                StepKind::Deal
            }
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => StepKind::Betting,
            HandStep::DrawPhase => StepKind::DrawDiscard,
            HandStep::Showdown => StepKind::Showdown,
            HandStep::Payout => StepKind::Payout,
        }
    }

    pub(super) fn on_enter_decision_step(&mut self, step: HandStep) -> Vec<Event> {
        match step {
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => self.enter_betting_round(),
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
        if self.is_stud_variant() {
            return self.enter_stud_betting_round();
        }

        let active_in_order: Vec<PlayerId> = self
            .table
            .action_order()
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid))
            .collect();

        let mut order = active_in_order;
        let mut seeded_current_bet = 0;
        let mut seeded_contribs = Vec::new();

        if self.is_holdem_variant() && self.table.current_step() == HandStep::BettingRound1 {
            if let Some((sb_pid, bb_pid)) = Self::holdem_blind_payers_in_dealer_order(&order) {
                seeded_current_bet = variants::texas_holdem::BIG_BLIND;
                seeded_contribs.push((sb_pid, variants::texas_holdem::SMALL_BLIND));
                seeded_contribs.push((bb_pid, variants::texas_holdem::BIG_BLIND));

                if let Some(bb_idx) = order.iter().position(|pid| *pid == bb_pid) {
                    let first_to_act_idx = (bb_idx + 1) % order.len();
                    order.rotate_left(first_to_act_idx);
                }
            }
        }

        self.enter_betting_round_with_seeded_state(order, seeded_current_bet, seeded_contribs)
    }

    fn enter_stud_betting_round(&mut self) -> Vec<Event> {
        let active_in_order: Vec<PlayerId> = self
            .table
            .action_order()
            .into_iter()
            .filter(|pid| self.table.is_active_player(*pid))
            .collect();

        if active_in_order.is_empty() {
            return vec![self.emit_error("No active players for stud betting round")];
        }

        match self.current_stud_street() {
            Some(StudStreet::Third) => {
                let Some(bring_in_pid) = self.stud_bring_in_player(&active_in_order) else {
                    return vec![self.emit_error("Could not determine stud bring-in player")];
                };

                if let Err(e) = self.with_accounts_mut(|accounts| {
                    accounts.withdraw_by_id(bring_in_pid, seven_card_stud::BRING_IN_AMOUNT as i64)
                }) {
                    return vec![self.emit_error(format!("Bring-in payment failed: {:?}", e))];
                }

                self.table
                    .hand_state_mut()
                    .add_to_pot(seven_card_stud::BRING_IN_AMOUNT);

                let mut order = active_in_order;
                if let Some(bring_in_idx) = order.iter().position(|pid| *pid == bring_in_pid) {
                    let first_to_act_idx = (bring_in_idx + 1) % order.len();
                    order.rotate_left(first_to_act_idx);
                }

                let mut events = vec![self.emit_current_pot_updated()];

                events.extend(self.enter_betting_round_with_seeded_state(
                    order,
                    seven_card_stud::BRING_IN_AMOUNT,
                    vec![(bring_in_pid, seven_card_stud::BRING_IN_AMOUNT)],
                ));

                events
            }
            Some(StudStreet::Fourth)
            | Some(StudStreet::Fifth)
            | Some(StudStreet::Sixth)
            | Some(StudStreet::Seventh) => {
                let Some(opener_pid) = self.stud_visible_opener(&active_in_order) else {
                    return vec![self.emit_error("Could not determine stud opener")];
                };

                let mut order = active_in_order;
                if let Some(opener_idx) = order.iter().position(|pid| *pid == opener_pid) {
                    order.rotate_left(opener_idx);
                }

                self.enter_betting_round_with_seeded_state(order, 0, Vec::new())
            }
            None => vec![self.emit_error("Invalid stud betting street")],
        }
    }

    fn enter_betting_round_with_seeded_state(
        &mut self,
        order: Vec<PlayerId>,
        seeded_current_bet: u32,
        seeded_contribs: Vec<(PlayerId, u32)>,
    ) -> Vec<Event> {
        if order.is_empty() {
            return vec![self.emit_error("No active players for betting round")];
        }

        // The generic betting state machine does not know where blinds or the stud bring-in came
        // from, so callers seed those forced contributions before normal action starts.
        self.table.hand_state_mut().start_betting_round(order);
        self.table
            .hand_state_mut()
            .set_current_bet(seeded_current_bet);

        if let Some(br) = self.table.hand_state_mut().betting_mut() {
            for (pid, contrib) in seeded_contribs {
                br.contrib.insert(pid, contrib);
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
        // Skip anyone who is seated but already inactive before we emit the first action prompt.
        while !br.pending.is_empty() && !is_active(br.current_player()) {
            Self::advance_to_next_actor_local(br, &is_active);
        }

        // Emit ActionRequested for the first player to act
        let first_player = self.table.hand_state().betting().unwrap().current_player();
        vec![self.emit_action_requested(first_player)]
    }

    fn push_card_to_player_hand(&mut self, player_id: PlayerId, card: common::card_deck::Card) {
        let mut cards = self
            .table
            .hand_state()
            .player_hole_cards(&player_id)
            .cloned()
            .unwrap_or_default();
        cards.push(card);
        self.table
            .hand_state_mut()
            .set_player_hole_cards(player_id, cards);
    }

    fn record_deal_event(
        &self,
        events: &mut Vec<Event>,
        street: AnimationStreet,
        target: AnimationTarget,
        player_id: Option<PlayerId>,
        face: AnimationFace,
        card: common::card_deck::Card,
        target_index: usize,
    ) {
        events.push(Event::Deal {
            deal: DealEvent {
                street,
                target,
                player_id,
                face,
                card,
                target_index,
            },
        });
    }

    fn deal_down_card_to_player(
        &mut self,
        events: &mut Vec<Event>,
        street: AnimationStreet,
        player_id: PlayerId,
    ) -> Option<common::card_deck::Card> {
        let card = self.deck.deal_card()?;
        let target_index = self
            .table
            .hand_state()
            .player_hole_cards(&player_id)
            .map(|cards| cards.len())
            .unwrap_or(0);
        self.push_card_to_player_hand(player_id, card);
        self.record_deal_event(
            events,
            street,
            AnimationTarget::Seat,
            Some(player_id),
            AnimationFace::Down,
            card,
            target_index,
        );
        Some(card)
    }

    fn deal_up_card_to_player(
        &mut self,
        events: &mut Vec<Event>,
        street: AnimationStreet,
        player_id: PlayerId,
    ) -> Option<common::card_deck::Card> {
        let card = self.deck.deal_card()?;
        let target_index = self
            .table
            .hand_state()
            .player_up_cards(&player_id)
            .map(|cards| cards.len())
            .unwrap_or(0);
        self.push_card_to_player_hand(player_id, card);
        self.table
            .hand_state_mut()
            .push_player_up_card(player_id, card);
        self.record_deal_event(
            events,
            street,
            AnimationTarget::Seat,
            Some(player_id),
            AnimationFace::Up,
            card,
            target_index,
        );
        Some(card)
    }

    fn deal_board_card(
        &mut self,
        events: &mut Vec<Event>,
        street: AnimationStreet,
    ) -> Option<common::card_deck::Card> {
        let card = self.deck.deal_card()?;
        let target_index = self.table.hand_state().board_cards().len();
        self.table.hand_state_mut().push_board_card(card);
        self.record_deal_event(
            events,
            street,
            AnimationTarget::Board,
            None,
            AnimationFace::Board,
            card,
            target_index,
        );
        Some(card)
    }

    fn burn_one_card(&mut self) {
        if let Some(card) = self.deck.deal_card() {
            self.deck.discard(card);
        }
    }

    fn stud_visible_boards_summary(&self) -> String {
        let mut parts = Vec::new();
        for (_seat, pid) in self.table.seated_players() {
            if let Some(up_cards) = self.table.hand_state().player_up_cards(&pid) {
                if !up_cards.is_empty() {
                    let name = self.account_name(pid);
                    let cards = up_cards
                        .iter()
                        .map(|card| card.to_string())
                        .collect::<Vec<_>>();
                    parts.push(format!("{name} {:?}", cards));
                }
            }
        }

        if parts.is_empty() {
            "no up cards".into()
        } else {
            parts.join(" | ")
        }
    }

    fn stud_bring_in_player(&self, active_in_order: &[PlayerId]) -> Option<PlayerId> {
        active_in_order
            .iter()
            .copied()
            .filter_map(|pid| {
                let up_card = self
                    .table
                    .hand_state()
                    .player_up_cards(&pid)?
                    .first()
                    .copied()?;
                Some((
                    pid,
                    up_card.rank() as u8,
                    Self::stud_suit_order(up_card.suit()),
                ))
            })
            .min_by(|a, b| (a.1, a.2).cmp(&(b.1, b.2)))
            .map(|(pid, _, _)| pid)
    }

    fn stud_visible_opener(&self, active_in_order: &[PlayerId]) -> Option<PlayerId> {
        let mut best_pid = None;
        let mut best_rank = None;

        for pid in active_in_order {
            let Some(up_cards) = self.table.hand_state().player_up_cards(pid) else {
                continue;
            };
            let rank = Self::stud_visible_rank(up_cards);
            match best_rank {
                None => {
                    best_rank = Some(rank);
                    best_pid = Some(*pid);
                }
                Some(current_best) if rank > current_best => {
                    best_rank = Some(rank);
                    best_pid = Some(*pid);
                }
                _ => {}
            }
        }

        best_pid
    }

    fn stud_suit_order(suit: common::card_deck::Suit) -> u8 {
        match suit {
            common::card_deck::Suit::Clubs => 0,
            common::card_deck::Suit::Diamonds => 1,
            common::card_deck::Suit::Hearts => 2,
            common::card_deck::Suit::Spades => 3,
        }
    }
}

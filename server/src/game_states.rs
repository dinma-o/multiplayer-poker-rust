use common::{
    hand::Hand,
    protocol::{HandStep, PlayerId},
};
use std::collections::{HashMap, HashSet};
// Define the various states of a Five Card Draw Poker game.

#[derive(Debug, Clone)]
pub struct PokerStateMachine {
    plan: Vec<HandStep>,
    current_index: usize,
}

#[derive(Debug, Clone)]
pub struct BettingRoundState {
    pub order: Vec<PlayerId>,            // fixed action order
    pub to_act_idx: usize,               // index into order
    pub contrib: HashMap<PlayerId, u32>, // chips contributed this round
    pub pending: HashSet<PlayerId>,      // players yet to act this round
    pub raises_used: u32,                // number of raises used this round
}

impl BettingRoundState {
    pub fn new(order: Vec<PlayerId>) -> Self {
        let mut contrib = HashMap::new();
        for &pid in &order {
            contrib.insert(pid, 0);
        }
        let pending = order.iter().cloned().collect();
        Self {
            order,
            to_act_idx: 0,
            contrib,
            pending,
            raises_used: 0,
        }
    }

    pub fn current_player(&self) -> PlayerId {
        self.order[self.to_act_idx]
    }

    pub fn contrib_of(&self, pid: PlayerId) -> u32 {
        *self.contrib.get(&pid).unwrap_or(&0)
    }

    pub fn set_contrib(&mut self, pid: PlayerId, amount: u32) {
        self.contrib.insert(pid, amount);
    }

    pub fn mark_done(&mut self, pid: PlayerId) {
        self.pending.remove(&pid);
    }

    pub fn reset_pending_except(&mut self, active: &[PlayerId], actor: PlayerId) {
        self.pending = active.iter().cloned().collect();
        self.pending.remove(&actor);
    }
}

#[derive(Debug, Clone)]
pub struct DrawRoundState {
    pub order: Vec<PlayerId>,
    pub to_act_idx: usize,
}

impl DrawRoundState {
    pub fn new(order: Vec<PlayerId>) -> Self {
        Self {
            order,
            to_act_idx: 0,
        }
    }

    pub fn current_player(&self) -> PlayerId {
        self.order[self.to_act_idx]
    }

    pub fn advance(&mut self) {
        self.to_act_idx += 1;
    }

    pub fn is_done(&self) -> bool {
        self.to_act_idx >= self.order.len()
    }

    pub fn remove_player(&mut self, pid: PlayerId) -> bool {
        if let Some(pos) = self.order.iter().position(|p| *p == pid) {
            let current_pos = self.to_act_idx;
            self.order.remove(pos);
            if pos < self.to_act_idx {
                self.to_act_idx = self.to_act_idx.saturating_sub(1);
            }
            return pos == current_pos;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct HandState<const HAND_SIZE: usize> {
    player_hands: HashMap<PlayerId, Hand<HAND_SIZE>>, // Map of player ID to their hand of cards
    player_fold_status: HashMap<PlayerId, bool>,      // Map of player ID to their fold status
    pot_size: u32,
    current_bet: u32,
    players_in_round: u32,
    ante_amount: u32,
    betting: Option<BettingRoundState>,
    draw: Option<DrawRoundState>,
}

impl PokerStateMachine {
    pub fn new(new_plan: Vec<HandStep>) -> Self {
        PokerStateMachine {
            plan: new_plan,
            current_index: 0,
        }
    }
    pub fn reset(&mut self) {
        self.current_index = 0;
    }
    pub fn get_current_state(&self) -> &HandStep {
        &self.plan[self.current_index]
    }
    pub fn advance_state(&mut self) {
        self.current_index = (self.current_index + 1) % self.plan.len();
    }
}

impl<const HandSize: usize> HandState<HandSize> {
    pub fn new(ante_amount: u32) -> Self {
        HandState {
            player_hands: HashMap::new(),
            player_fold_status: HashMap::new(),
            pot_size: 0,
            current_bet: 0,
            players_in_round: 0,
            ante_amount,
            betting: None,
            draw: None,
        }
    }

    pub fn reset_for_new_deal(&mut self) {
        self.player_hands.clear();
        self.player_fold_status.clear();
        self.current_bet = 0;
        self.players_in_round = 0;
    }

    pub fn reset_after_hand(&mut self) {
        self.reset_for_new_deal();
        self.pot_size = 0;
    }

    pub fn add_to_pot(&mut self, amount: u32) {
        self.pot_size += amount;
    }

    pub fn set_current_bet(&mut self, amount: u32) {
        self.current_bet = amount;
    }

    pub fn current_bet(&self) -> u32 {
        self.current_bet
    }

    pub fn pot_size(&self) -> u32 {
        self.pot_size
    }

    pub fn set_player_hand(&mut self, player_id: PlayerId, hand: Hand<HandSize>) {
        self.player_hands.insert(player_id, hand);
    }

    pub fn player_hand(&self, player_id: &PlayerId) -> Option<&Hand<HandSize>> {
        self.player_hands.get(player_id)
    }

    pub fn players_with_hands(&self) -> Vec<PlayerId> {
        self.player_hands.keys().copied().collect()
    }

    pub fn set_player_fold_status(&mut self, player_id: PlayerId, has_folded: bool) {
        self.player_fold_status.insert(player_id, has_folded);
    }

    pub fn has_player_folded(&self, player_id: &PlayerId) -> bool {
        *self.player_fold_status.get(player_id).unwrap_or(&false)
    }

    pub fn start_betting_round(&mut self, order: Vec<PlayerId>) {
        self.current_bet = 0;
        self.betting = Some(BettingRoundState::new(order.clone()));
        self.players_in_round = order.len() as u32;
    }

    pub fn betting(&self) -> Option<&BettingRoundState> {
        self.betting.as_ref()
    }
    pub fn betting_mut(&mut self) -> Option<&mut BettingRoundState> {
        self.betting.as_mut()
    }

    pub fn end_betting_round(&mut self) {
        self.betting = None;
    }

    pub fn start_draw_round(&mut self, order: Vec<PlayerId>) {
        self.draw = Some(DrawRoundState::new(order));
    }

    pub fn draw(&self) -> Option<&DrawRoundState> {
        self.draw.as_ref()
    }
    pub fn draw_mut(&mut self) -> Option<&mut DrawRoundState> {
        self.draw.as_mut()
    }

    pub fn end_draw_round(&mut self) {
        self.draw = None;
    }
}

// Define the various states of Seven Card Stud Poker game.

// Define the various states of a Texas Hold'em Poker game.

// Tests for the state machine and hand state can be added here.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poker_state_machine() {
        let plan = vec![
            HandStep::WaitingForPlayers,
            HandStep::CollectForcedBets,
            HandStep::DealPrivate,
            HandStep::BettingRound1,
            HandStep::DrawPhase,
            HandStep::BettingRound2,
            HandStep::Showdown,
            HandStep::Payout,
        ];
        let mut fsm = PokerStateMachine::new(plan);

        assert_eq!(*fsm.get_current_state(), HandStep::WaitingForPlayers);
        fsm.advance_state();
        assert_eq!(*fsm.get_current_state(), HandStep::CollectForcedBets);
        fsm.advance_state();
        assert_eq!(*fsm.get_current_state(), HandStep::DealPrivate);
    }

    #[test]
    fn test_hand_state() {
        let mut hand_state: HandState<5> = HandState::new(10);
        hand_state.add_to_pot(50);
        assert_eq!(hand_state.pot_size, 50);
        hand_state.set_current_bet(20);
        assert_eq!(hand_state.current_bet, 20);
    }

    #[test]
    fn test_hand_state_fold_status() {
        let mut hand_state: HandState<5> = HandState::new(10);
        hand_state.player_fold_status.insert(1, false);
        hand_state.player_fold_status.insert(2, true);

        assert_eq!(*hand_state.player_fold_status.get(&1).unwrap(), false);
        assert_eq!(*hand_state.player_fold_status.get(&2).unwrap(), true);
    }
}

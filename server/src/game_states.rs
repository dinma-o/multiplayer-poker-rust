use std::collections::HashMap;
use common::hand::{self, Hand};
// Define the various states of a Five Card Draw Poker game.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HandStep {
    WaitingForPlayers,
    CollectForcedBets,        // ante or blinds later
    DealPrivate,              // draw5/hole cards/stud down cards
    DealCommunity,            // hold'em later
    BettingRound1,
    DrawPhase,                // draw5 only (no-op for others)
    BettingRound2,
    Showdown,
    Payout,
}

#[derive(Debug, Clone)]
pub struct PokerStateMachine {
    plan: Vec<HandStep>,
    current_index: usize,
}

#[derive(Debug, Clone)]
pub struct HandState<const HAND_SIZE: usize> {
    player_hands: HashMap<usize, Hand<HAND_SIZE>>, // Map of player ID to their hand of cards
    player_fold_status: HashMap<usize, bool>, // Map of player ID to their fold status
    pot_size: u32,
    current_bet: u32,
    players_in_round: u32,
    ante_amount: u32,
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
        }
    }

    pub fn reset(&mut self) {
        self.pot_size = 0;
        self.current_bet = 0;
        self.players_in_round = 0;
    }

    pub fn add_to_pot(&mut self, amount: u32) {
        self.pot_size += amount;
    }

    pub fn set_current_bet(&mut self, amount: u32) {
        self.current_bet = amount;
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
        hand_state.reset();
        assert_eq!(hand_state.pot_size, 0);
        assert_eq!(hand_state.current_bet, 0);
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

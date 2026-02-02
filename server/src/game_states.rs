// Define the various states of a Five Card Draw Poker game.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PokerGameState {
    WaitingForPlayers,
    DealingCards,
    BettingRound1,
    DrawPhase,
    BettingRound2,
    Showdown,
    Payout,
}
#[derive(Debug, Clone)]
pub struct PokerStateMachine {
    poker_states: Vec<PokerGameState>,
    current_state_index: usize,
    pot_size: u32,
    current_bet: u32,
    players_in_round: u32,
    ante_amount: u32,
    hand_size: u8,
}


pub impl PokerStateMachine {
    pub fn new(ante_amount: u32) -> Self {
        PokerStateMachine {
            poker_states: vec![
                PokerGameState::WaitingForPlayers,
                PokerGameState::DealingCards,
                PokerGameState::BettingRound1,
                PokerGameState::DrawPhase,
                PokerGameState::BettingRound2,
                PokerGameState::Showdown,
                PokerGameState::Payout,
            ],
            current_state_index: 0,
            pot_size: 0,
            current_bet: 0,
            players_in_round: 0,
            ante_amount,
            hand_size: 5,
        }
    }
    pub fn get_current_state(&self) -> &PokerGameState {
        &self.poker_states[self.current_state_index]
    }
    pub fn set_current_state(&mut self, state: PokerGameState) {
        self.current_state_index = self.poker_states.iter().position(|&s| s == state).unwrap_or(0);
    }
    pub fn get_pot_size(&self) -> u32 {
        self.pot_size
    }
    pub fn set_pot_size(&mut self, amount: u32) {
        self.pot_size = amount;
    }
    pub fn add_to_pot(&mut self, amount: u32) {
        self.pot_size += amount;
    }
    pub fn get_current_bet(&self) -> u32 {
        self.current_bet
    }
    pub fn set_current_bet(&mut self, amount: u32) {
        self.current_bet = amount;
    }
    pub fn get_players_in_round(&self) -> u32 {
        self.players_in_round
    }
    pub fn set_players_in_round(&mut self, count: u32) {
        self.players_in_round = count;
    }
    pub fn get_ante_amount(&self) -> u32 {
        self.ante_amount
    }
    pub fn set_ante_amount(&mut self, amount: u32) {
        self.ante_amount = amount;
    }
    pub fn get_hand_size(&self) -> u8 {
        self.hand_size
    }
    pub fn set_hand_size(&mut self, size: u8) {
        self.hand_size = size;
    }
    pub fn advance_state(&mut self) {
        if self.current_state_index < self.poker_states.len() - 1 {
            self.current_state_index += 1;
        } else {
            self.current_state_index = 0; // Loop back to the beginning
        }
    }

    pub fn reset_round(&mut self) {
        self.pot_size = 0;
        self.current_bet = 0;
    }
}

// Define the various states of Seven Card Stud Poker game.

// Define the various states of a Texas Hold'em Poker game.

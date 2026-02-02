// Define the various states of a Five Card Draw Poker game.
pub enum FiveCardDrawPokerGameState {
    WaitingForPlayers,
    DealingCards,
    BettingRound1,
    DrawPhase,
    BettingRound2,
    Showdown,
    Payout,
}
pub struct PokerStateMachine {
    current_state: FiveCardDrawPokerGameState,
    pot_size: u32,
    current_bet: u32,
    players_in_round: u32,
    ante_amount: u32,
    hand_size: u8,
}


pub impl FiveCardDrawPokerStateMachine {
    pub fn new(ante_amount: u32) -> Self {
        FiveCardDrawPokerStateMachine {
            current_state: FiveCardDrawPokerGameState::WaitingForPlayers,
            pot_size: 0,
            current_bet: 0,
            players_in_round: 0,
            ante_amount,
            hand_size: 5,
        }
    }
    pub fn get_current_state(&self) -> &FiveCardDrawPokerGameState {
        &self.current_state
    }
    pub fn set_current_state(&mut self, state: FiveCardDrawPokerGameState) {
        self.current_state = state;
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

    pub fn reset_round(&mut self) {
        self.pot_size = 0;
        self.current_bet = 0;
    }
}

// Define the various states of Seven Card Stud Poker game.

// Define the various states of a Texas Hold'em Poker game.

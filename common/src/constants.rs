/* Constant values for the following Poker games
• Five-card Draw: https://en.wikipedia.org/wiki/Five-card_draw
• Seven-card Stud: https://en.wikipedia.org/wiki/Seven-card_stud
• Texas Hold’em: https://en.wikipedia.org/wiki/Texas_hold_%27em
*/

// For Five-card Draw
pub mod five_card_draw {
    // Table start limits
    pub const INITIAL_CHIPS: u32 = 200; // Starting stack for each player
    pub const MIN_PLAYERS: u32 = 2; // Minimum number of players to start the game
    pub const MAX_PLAYERS: u32 = 5; // Maximum number of players allowed at the table
    pub const ANTE_AMOUNT: u32 = 5; // Ante amount each player must post before the hand starts

    // Betting limits
    pub const BET_INCREMENT: u32 = 5; // Standard bet increment
    pub const MAX_BET_PER_ROUND: u32 = 25; // Maximum bet allowed per betting round
    pub const MIN_BET_PER_ROUND: u32 = 5; // Minimum bet required per betting round

    // Raise limits
    pub const RAISE_INCREMENT: u32 = BET_INCREMENT; // Standard raise increment
    pub const MAX_RAISES_PER_ROUND: u32 = 1; // Maximum number of raises allowed per betting round
    pub const MIN_RAISE_AMOUNT: u32 = 5; // Minimum amount for each raise
    pub const MAX_RAISE_TO_AMOUNT: u32 = 40; // Maximum amount to which a player can raise

    // Draw limits
    pub const MAX_DISCARDS: usize = 4; // Maximum number of cards a player can discard during the draw phase
}

// For Seven-card Stud
pub mod seven_card_stud {
    // Table start limits
    pub const INITIAL_CHIPS: u32 = 200; // Starting stack for each player
    pub const MIN_PLAYERS: u32 = 2; // Minimum number of players to start the game
    pub const MAX_PLAYERS: u32 = 5; // Maximum number of players allowed at the table
    pub const ANTE_AMOUNT: u32 = 5; // Ante amount each player must post before the hand starts
    pub const BRING_IN_AMOUNT: u32 = 5; // Bring-in amount for the player with the lowest upcard

    // Fixed betting limits
    pub const SMALL_BET: u32 = 10; // Small bet amount for the first two betting rounds
    pub const BIG_BET: u32 = 20; // Big bet amount for the subsequent betting rounds
    pub const MAX_RAISES_PER_ROUND: u32 = 3; // Maximum number of raises allowed per betting round
}

// For Texas Hold’em
pub mod texas_hold_em {
    // Table start limits
    pub const INITIAL_CHIPS: u32 = 200; // Starting stack for each player
    pub const MIN_PLAYERS: u32 = 2; // Minimum number of players to start the game
    pub const MAX_PLAYERS: u32 = 5; // Maximum number of players allowed at the table

    // Blinds
    pub const SMALL_BLIND: u32 = 5; // Small blind amount posted by the player left of the dealer
    pub const BIG_BLIND: u32 = 10; // Big blind amount posted by the player two left of the dealer

    // Betting limits
    pub const MAX_RAISES_PER_ROUND: u32 = 4; // Maximum number of raises allowed per betting round
}

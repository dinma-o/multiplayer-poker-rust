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
    pub const MAX_RAISE_TO_AMOUNT: u32 = MAX_BET_PER_ROUND; // Maximum amount to which a player can raise


}


// For Seven-card Stud

// For Texas Hold’em

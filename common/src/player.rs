// Player module for PokerDud game
// Represents a player with a name, chip count and hand of cards
use crate::card_deck::Card;

pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub fn new() -> Self {
        Hand { cards: Vec::new() }
    }
}

pub struct Player {
    name: String,
    chips: u32,
    hand: Hand,
}

impl Player {
    pub fn new(name: String, chips: u32) -> Self {
        Player {
            name,
            chips,
            hand: Hand::new(),
        }
    }
}

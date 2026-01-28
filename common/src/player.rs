// Player module for PokerDud game
// Represents a player with a name, chip count and hand of cards
struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    fn new() -> Self {
        Hand { cards: Vec::new() }
    }
}

struct Player {
    name: String,
    chips: u32,
    hand: Hand,
}

impl Player {
    fn new(name: String, chips: u32) -> Self {
        Player {
            name,
            chips,
            hand: Hand::new(),
        }
    }
}
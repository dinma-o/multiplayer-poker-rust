use std::hash::{ Hash };

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub const ALL: [Rank; 13] = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Card {
    rank: Rank,
    suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Card { rank, suit }
    }

    pub fn rank(&self) -> Rank {
        self.rank
    }

    pub fn suit(&self) -> Suit {
        self.suit
    }
}

#[derive(Debug)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(52);

        for suit in Suit::ALL.iter() {
            for rank in Rank::ALL.iter() {
                cards.push(Card::new(*rank, *suit));
            }
        }

        Deck { cards }
    }

    pub fn size(&self) -> usize {
        self.cards.len()
    }

    pub fn deal_card(&mut self) -> Option<Card> {
        self.cards.pop()
    }

    pub fn shuffle(&mut self) {
        use rand::seq::SliceRandom;
        use rand::rng;

        let mut rng = rng();
        self.cards.shuffle(&mut rng);
    }

    pub fn add_back(&mut self, card: Card) {
        self.cards.push(card);
        self.shuffle();
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Card tests
    #[test]
    fn create_card() {
        let card = Card::new(Rank::Ace, Suit::Spades);
        assert_eq!(card.rank(), Rank::Ace);
        assert_eq!(card.suit(), Suit::Spades);
    }

    #[test]
    fn card_equality() {
        let card1 = Card::new(Rank::Ten, Suit::Hearts);
        let card2 = Card::new(Rank::Ten, Suit::Hearts);
        let card3 = Card::new(Rank::Jack, Suit::Hearts);

        assert_eq!(card1, card2);
        assert_ne!(card1, card3);
    }

    // Deck tests
    #[test]
    fn new_deck_has_52_cards() {
        let deck = Deck::new();
        assert_eq!(deck.size(), 52);
    }

    #[test]
    fn new_deck_has_no_duplicates() {
        let deck = Deck::new();

        let mut seen = HashSet::new();
        for c in deck.cards.iter() {
            assert!(seen.insert(*c), "duplicate card found: {:?}", c);
        }
        assert_eq!(seen.len(), 52);
    }

    #[test]
    fn dealing_reduces_size_to_zero() {
        let mut deck = Deck::new();
        for _ in 0..52 {
            assert!(deck.deal_card().is_some());
        }
        assert!(deck.deal_card().is_none());
        assert_eq!(deck.size(), 0);
    }

    #[test]
    fn shuffle_keeps_size() {
        let mut deck = Deck::new();
        deck.shuffle();
        assert_eq!(deck.size(), 52);
    }

    #[test]
    fn add_back_increases_size() {
        let mut deck = Deck::new();
        let card = deck.deal_card().unwrap();
        assert_eq!(deck.size(), 51);
        deck.add_back(card);
        assert_eq!(deck.size(), 52);
    }

    #[test]
    fn add_back_shuffles_deck() {
        let mut deck = Deck::new();
        let original_order: Vec<Card> = deck.cards.clone();
        let card = deck.deal_card().unwrap();
        deck.add_back(card);
        let new_order: Vec<Card> = deck.cards.clone();
        assert_ne!(original_order, new_order, "Deck was not shuffled after adding back a card");
    }
}
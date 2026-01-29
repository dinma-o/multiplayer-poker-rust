use rand::{ seq::SliceRandom, thread_rng };

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
    discarded: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(52);

        for suit in Suit::ALL.iter() {
            for rank in Rank::ALL.iter() {
                cards.push(Card::new(*rank, *suit));
            }
        }

        Deck { cards, discarded: Vec::new() }
    }

    pub fn new_shuffled() -> Self {
        let mut d = Self::new();
        d.shuffle();
        d
    }

    pub fn size(&self) -> usize {
        self.cards.len()
    }

    pub fn deal_card(&mut self) -> Option<Card> {
        self.cards.pop()
    }

    pub fn shuffle(&mut self) {
        use rand::thread_rng;
        let mut rng = thread_rng();
        self.shuffle_with_rng(&mut rng);
    }

    pub fn shuffle_with_rng<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.cards.shuffle(rng);
    }

    pub fn discard(&mut self, card: Card) {
        self.discarded.push(card);
    }

    pub fn discarded_size(&self) -> usize {
        self.discarded.len()
    }

    pub fn reset_from_discarded(&mut self) {
        self.cards.append(&mut self.discarded);
        self.shuffle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use rand::rngs::StdRng;
    use rand::{ SeedableRng };

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
    fn shuffle_preserves_cards() {
        let mut deck = Deck::new();
        let before: HashSet<Card> = deck.cards.iter().copied().collect();

        deck.shuffle();

        let after: HashSet<Card> = deck.cards.iter().copied().collect();
        assert_eq!(before, after);
        assert_eq!(deck.size(), 52);
    }

    #[test]
    fn shuffle_with_rng_is_deterministic() {
        let mut deck1 = Deck::new();
        let mut deck2 = Deck::new();

        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);

        deck1.shuffle_with_rng(&mut rng1);
        deck2.shuffle_with_rng(&mut rng2);

        let cards1: Vec<Card> = deck1.cards.iter().cloned().collect();
        let cards2: Vec<Card> = deck2.cards.iter().cloned().collect();

        assert_eq!(cards1, cards2, "Two decks shuffled with the same RNG seed differ");
    }
}

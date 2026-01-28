enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

enum Rank {
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

struct Card {
    rank: Rank,
    suit: Suit,
}

impl Card {
    fn new(rank: Rank, suit: Suit) -> Self {
        Card { rank, suit }
    }
}

struct Deck {
    cards: Vec<Card>,
    size: usize,
}

impl Deck {
    fn new() -> Self {
        let mut cards = [Card::new(Rank::Two, Suit::Hearts); 52];
        let mut index = 0;

        for &suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            for &rank in &[
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
            ] {
                cards[index] = Card::new(rank, suit);
                index += 1;
            }
        }

        Deck { cards, size: 52 }
    }

    fn size(&self) -> usize {
        self.size
    }
}
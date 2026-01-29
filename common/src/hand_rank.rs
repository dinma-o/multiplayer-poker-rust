#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HandCategory {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush, // RoyalFlush can be considered a special case of StraightFlush
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandRank {
    pub category: HandCategory,
    pub tiebreakers: [u8; 5], // Ranks of cards used for tiebreaking
}

impl HandRank {
    pub fn new(category: HandCategory, tiebreakers: [u8; 5]) -> Self {
        HandRank { category, tiebreakers }
    }

    pub fn straight(category: HandCategory, high_card: u8) -> Self {
        Self { category, tiebreakers: [high_card, 0, 0, 0, 0] }
    }
}

impl Ord for HandRank {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.category.cmp(&other.category) {
            std::cmp::Ordering::Equal => self.tiebreakers.cmp(&other.tiebreakers),
            ord => ord,
        }
    }
}

impl PartialOrd for HandRank {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[test]
fn handrank_ordering_works() {
    let a = HandRank::new(HandCategory::OnePair, [10, 14, 9, 3, 0]); // pair of 10s
    let b = HandRank::new(HandCategory::OnePair, [11, 2, 3, 4, 0]); // pair of Jacks
    assert!(b > a);

    let sf = HandRank::straight(HandCategory::StraightFlush, 14); // royal flush
    let fk = HandRank::new(HandCategory::FourOfAKind, [14, 2, 0, 0, 0]);
    assert!(sf > fk);
}

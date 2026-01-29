use std::collections::HashMap;

use crate::{ card_deck::{ Card, Suit }, hand::FiveCardHand };

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

pub fn rank_five_card_hand(hand: &FiveCardHand) -> HandRank {
    rank_five_cards(hand.cards())
}

fn rank_five_cards(cards: &[Card; 5]) -> HandRank {
    let suits: [Suit; 5] = cards.map(|c| c.suit());
    let mut ranks: Vec<u8> = cards
        .iter()
        .map(|c| c.rank() as u8)
        .collect();
    ranks.sort_unstable(); // ascending

    let is_flush = suits.iter().all(|&s| s == suits[0]);

    // counts
    let mut rank_counts: HashMap<u8, u8> = HashMap::new();
    for &r in &ranks {
        *rank_counts.entry(r).or_insert(0) += 1;
    }

    let unique = rank_counts.len() == 5;
    let is_wheel = ranks == [2, 3, 4, 5, 14];
    let is_straight = unique && (is_wheel || ranks.windows(2).all(|w| w[1] == w[0] + 1));
    let straight_high = if is_straight {
        if is_wheel { 5 } else { *ranks.last().unwrap() }
    } else {
        0
    };

    // helper: descending ranks -> [u8;5]
    fn to_arr5_desc(mut v: Vec<u8>) -> [u8; 5] {
        v.sort_unstable_by(|a, b| b.cmp(a));
        [v[0], v[1], v[2], v[3], v[4]]
    }

    // group counts as (count, rank), sorted desc
    let mut groups: Vec<(u8, u8)> = rank_counts
        .iter()
        .map(|(&r, &c)| (c, r))
        .collect();
    groups.sort_unstable_by(|a, b| b.cmp(a));

    // Straight Flush
    if is_straight && is_flush {
        return HandRank::straight(HandCategory::StraightFlush, straight_high);
    }

    // Four of a Kind / Full House
    if groups[0].0 == 4 {
        return HandRank::new(HandCategory::FourOfAKind, [groups[0].1, groups[1].1, 0, 0, 0]);
    }
    if groups[0].0 == 3 && groups[1].0 == 2 {
        return HandRank::new(HandCategory::FullHouse, [groups[0].1, groups[1].1, 0, 0, 0]);
    }

    // Flush
    if is_flush {
        return HandRank::new(HandCategory::Flush, to_arr5_desc(ranks.clone()));
    }

    // Straight
    if is_straight {
        return HandRank::straight(HandCategory::Straight, straight_high);
    }

    // Three of a kind
    if groups[0].0 == 3 {
        let mut kickers: Vec<u8> = groups
            .iter()
            .filter(|&&(c, _)| c == 1)
            .map(|&(_, r)| r)
            .collect();
        kickers.sort_unstable_by(|a, b| b.cmp(a));
        return HandRank::new(HandCategory::ThreeOfAKind, [
            groups[0].1,
            kickers[0],
            kickers[1],
            0,
            0,
        ]);
    }

    // Two pair
    if groups[0].0 == 2 && groups[1].0 == 2 {
        let high_pair = groups[0].1.max(groups[1].1);
        let low_pair = groups[0].1.min(groups[1].1);
        let kicker = groups
            .iter()
            .find(|&&(c, _)| c == 1)
            .map(|&(_, r)| r)
            .unwrap();
        return HandRank::new(HandCategory::TwoPair, [high_pair, low_pair, kicker, 0, 0]);
    }

    // One pair
    if groups[0].0 == 2 {
        let mut kickers: Vec<u8> = groups
            .iter()
            .filter(|&&(c, _)| c == 1)
            .map(|&(_, r)| r)
            .collect();
        kickers.sort_unstable_by(|a, b| b.cmp(a));
        return HandRank::new(HandCategory::OnePair, [
            groups[0].1,
            kickers[0],
            kickers[1],
            kickers[2],
            0,
        ]);
    }

    // High card
    HandRank::new(HandCategory::HighCard, to_arr5_desc(ranks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ card_deck::{ Card, Rank, Suit }, hand::FiveCardHand };

    fn h(cards: [Card; 5]) -> FiveCardHand {
        FiveCardHand::new(cards)
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

    #[test]
    fn detects_high_card_and_tiebreakers() {
        // A K 9 7 3 (not straight, not flush)
        let hand = h([
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::HighCard);
        assert_eq!(r.tiebreakers, [14, 13, 9, 7, 3]);
    }

    #[test]
    fn detects_one_pair_and_kickers_order() {
        // Pair of 10s with A, 7, 3 kickers
        let hand = h([
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Hearts),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::OnePair);
        assert_eq!(r.tiebreakers, [10, 14, 7, 3, 0]);
    }

    #[test]
    fn detects_two_pair_with_high_low_pair_and_kicker() {
        // Two pair: Aces and Jacks, kicker 9
        let hand = h([
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Diamonds),
            Card::new(Rank::Nine, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::TwoPair);
        assert_eq!(r.tiebreakers, [14, 11, 9, 0, 0]);
    }

    #[test]
    fn detects_three_of_a_kind_and_kickers() {
        // Trips 7s with A and 2 kickers
        let hand = h([
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::ThreeOfAKind);
        assert_eq!(r.tiebreakers, [7, 14, 2, 0, 0]);
    }

    #[test]
    fn detects_straight_normal() {
        // 9-T-J-Q-K straight (high = K=13)
        let hand = h([
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::King, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::Straight);
        assert_eq!(r.tiebreakers, [13, 0, 0, 0, 0]);
    }

    #[test]
    fn detects_wheel_straight_ace_low() {
        // A-2-3-4-5 straight (wheel) should be high=5
        let hand = h([
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::Straight);
        assert_eq!(r.tiebreakers, [5, 0, 0, 0, 0]);
    }

    #[test]
    fn detects_flush_and_uses_high_card_tiebreak() {
        // Flush: A J 9 6 3 all spades
        let hand = h([
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Three, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::Flush);
        assert_eq!(r.tiebreakers, [14, 11, 9, 6, 3]);
    }

    #[test]
    fn detects_full_house() {
        // Full house: 8s full of 2s
        let hand = h([
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Two, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::FullHouse);
        assert_eq!(r.tiebreakers, [8, 2, 0, 0, 0]);
    }

    #[test]
    fn detects_four_of_a_kind() {
        // Quads: Kings, kicker 5
        let hand = h([
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Spades),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::FourOfAKind);
        assert_eq!(r.tiebreakers, [13, 5, 0, 0, 0]);
    }

    #[test]
    fn detects_straight_flush_and_royal_flush_is_high_14() {
        // Royal flush: 10 J Q K A all hearts => straight flush high=14
        let hand = h([
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Hearts),
        ]);
        let r = rank_five_card_hand(&hand);

        assert_eq!(r.category, HandCategory::StraightFlush);
        assert_eq!(r.tiebreakers, [14, 0, 0, 0, 0]);
    }

    #[test]
    fn ordering_works_across_categories_and_tiebreakers() {
        // Pair of 10s
        let p10 = rank_five_card_hand(
            &h([
                Card::new(Rank::Ten, Suit::Spades),
                Card::new(Rank::Ten, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Clubs),
                Card::new(Rank::Seven, Suit::Diamonds),
                Card::new(Rank::Three, Suit::Hearts),
            ])
        );

        // Pair of Jacks (should beat pair of 10s)
        let pj = rank_five_card_hand(
            &h([
                Card::new(Rank::Jack, Suit::Spades),
                Card::new(Rank::Jack, Suit::Hearts),
                Card::new(Rank::Two, Suit::Clubs),
                Card::new(Rank::Three, Suit::Diamonds),
                Card::new(Rank::Four, Suit::Hearts),
            ])
        );

        assert!(pj > p10);

        // Straight flush beats four of a kind
        let sf = rank_five_card_hand(
            &h([
                Card::new(Rank::Nine, Suit::Clubs),
                Card::new(Rank::Ten, Suit::Clubs),
                Card::new(Rank::Jack, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Clubs),
                Card::new(Rank::King, Suit::Clubs),
            ])
        );

        let fk = rank_five_card_hand(
            &h([
                Card::new(Rank::Ace, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Clubs),
                Card::new(Rank::Ace, Suit::Diamonds),
                Card::new(Rank::Two, Suit::Spades),
            ])
        );

        assert!(sf > fk);
    }

    #[test]
    fn two_pair_tiebreaker_compares_kicker() {
        // AA + JJ + 9
        let h1 = rank_five_card_hand(
            &h([
                Card::new(Rank::Ace, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
                Card::new(Rank::Jack, Suit::Clubs),
                Card::new(Rank::Jack, Suit::Diamonds),
                Card::new(Rank::Nine, Suit::Spades),
            ])
        );

        // AA + JJ + 8 (should lose to kicker 9)
        let h2 = rank_five_card_hand(
            &h([
                Card::new(Rank::Ace, Suit::Clubs),
                Card::new(Rank::Ace, Suit::Diamonds),
                Card::new(Rank::Jack, Suit::Hearts),
                Card::new(Rank::Jack, Suit::Spades),
                Card::new(Rank::Eight, Suit::Clubs),
            ])
        );

        assert!(h1 > h2);
    }
}

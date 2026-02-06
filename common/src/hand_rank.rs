use itertools::Itertools;
use std::collections::HashMap;

use crate::{
    card_deck::{Card, Suit},
    hand::FiveCardHand,
};

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
        HandRank {
            category,
            tiebreakers,
        }
    }

    pub fn straight(category: HandCategory, high_card: u8) -> Self {
        Self {
            category,
            tiebreakers: [high_card, 0, 0, 0, 0],
        }
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
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank() as u8).collect();
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
    let mut groups: Vec<(u8, u8)> = rank_counts.iter().map(|(&r, &c)| (c, r)).collect();
    groups.sort_unstable_by(|a, b| b.cmp(a));

    // Straight Flush
    if is_straight && is_flush {
        return HandRank::straight(HandCategory::StraightFlush, straight_high);
    }

    // Four of a Kind / Full House
    if groups[0].0 == 4 {
        return HandRank::new(
            HandCategory::FourOfAKind,
            [groups[0].1, groups[1].1, 0, 0, 0],
        );
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
        return HandRank::new(
            HandCategory::ThreeOfAKind,
            [groups[0].1, kickers[0], kickers[1], 0, 0],
        );
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
        return HandRank::new(
            HandCategory::OnePair,
            [groups[0].1, kickers[0], kickers[1], kickers[2], 0],
        );
    }

    // High card
    HandRank::new(HandCategory::HighCard, to_arr5_desc(ranks))
}

// Given 7 cards, determine the best possible 5-card hand rank
// This is used in games like Texas Hold'em and Seven-Card Stud
pub fn best_hand_from_seven_cards(cards: &[Card; 7]) -> HandRank {
    cards
        .iter()
        .copied()
        .combinations(5)
        .map(|v| {
            // v is Vec<Card> length 5
            let five: [Card; 5] = v.try_into().expect("combo must be length 5");
            rank_five_cards(&five)
        })
        .max()
        .expect("7 cards always produce at least one 5-card combination")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShowDownResult {
    pub winner_indices: Vec<usize>, // indices of winning hands (can be multiple for ties)
    pub winning_rank: HandRank,
}

pub fn determine_showdown_winner(ranked_hands: &[(usize, HandRank)]) -> ShowDownResult {
    let max_rank = ranked_hands
        .iter()
        .max_by(|a, b| a.1.cmp(&b.1))
        .expect("at least one hand must be present")
        .1;

    let winner_indices: Vec<usize> = ranked_hands
        .iter()
        .filter_map(|&(idx, ref rank)| if rank == &max_rank { Some(idx) } else { None })
        .collect();

    ShowDownResult {
        winner_indices,
        winning_rank: max_rank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        card_deck::{Card, Rank, Suit},
        hand::FiveCardHand,
    };

    // Shared helpers for all tests in this file
    fn h(cards: [Card; 5]) -> FiveCardHand {
        FiveCardHand::new(cards)
    }

    fn c(r: Rank, s: Suit) -> Card {
        Card::new(r, s)
    }

    mod handrank_ordering_tests {
        use super::*;

        #[test]
        fn handrank_ordering_works() {
            let a = HandRank::new(HandCategory::OnePair, [10, 14, 9, 3, 0]); // pair of 10s
            let b = HandRank::new(HandCategory::OnePair, [11, 2, 3, 4, 0]); // pair of Jacks
            assert!(b > a);

            let sf = HandRank::straight(HandCategory::StraightFlush, 14); // royal flush
            let fk = HandRank::new(HandCategory::FourOfAKind, [14, 2, 0, 0, 0]);
            assert!(sf > fk);
        }
    }

    mod five_card_category_tests {
        use super::*;

        #[test]
        fn detects_high_card() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::King, Suit::Hearts),
                c(Rank::Nine, Suit::Clubs),
                c(Rank::Seven, Suit::Diamonds),
                c(Rank::Three, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::HighCard);
        }

        #[test]
        fn detects_one_pair() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ten, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Seven, Suit::Diamonds),
                c(Rank::Three, Suit::Hearts),
            ]));
            assert_eq!(r.category, HandCategory::OnePair);
        }

        #[test]
        fn detects_two_pair() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Jack, Suit::Diamonds),
                c(Rank::Nine, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::TwoPair);
        }

        #[test]
        fn detects_three_of_a_kind() {
            let r = rank_five_card_hand(&h([
                c(Rank::Seven, Suit::Spades),
                c(Rank::Seven, Suit::Hearts),
                c(Rank::Seven, Suit::Clubs),
                c(Rank::Ace, Suit::Diamonds),
                c(Rank::Two, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::ThreeOfAKind);
        }

        #[test]
        fn detects_straight() {
            let r = rank_five_card_hand(&h([
                c(Rank::Nine, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Queen, Suit::Diamonds),
                c(Rank::King, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::Straight);
        }

        #[test]
        fn detects_wheel_straight() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::Two, Suit::Hearts),
                c(Rank::Three, Suit::Clubs),
                c(Rank::Four, Suit::Diamonds),
                c(Rank::Five, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::Straight);
            assert_eq!(r.tiebreakers, [5, 0, 0, 0, 0]);
        }

        #[test]
        fn detects_flush() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::Jack, Suit::Spades),
                c(Rank::Nine, Suit::Spades),
                c(Rank::Six, Suit::Spades),
                c(Rank::Three, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::Flush);
        }

        #[test]
        fn detects_full_house() {
            let r = rank_five_card_hand(&h([
                c(Rank::Eight, Suit::Spades),
                c(Rank::Eight, Suit::Hearts),
                c(Rank::Eight, Suit::Clubs),
                c(Rank::Two, Suit::Diamonds),
                c(Rank::Two, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::FullHouse);
        }

        #[test]
        fn detects_four_of_a_kind() {
            let r = rank_five_card_hand(&h([
                c(Rank::King, Suit::Spades),
                c(Rank::King, Suit::Hearts),
                c(Rank::King, Suit::Clubs),
                c(Rank::King, Suit::Diamonds),
                c(Rank::Five, Suit::Spades),
            ]));
            assert_eq!(r.category, HandCategory::FourOfAKind);
        }

        #[test]
        fn detects_straight_flush_and_royal_is_high_14() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Jack, Suit::Hearts),
                c(Rank::Queen, Suit::Hearts),
                c(Rank::King, Suit::Hearts),
                c(Rank::Ace, Suit::Hearts),
            ]));
            assert_eq!(r.category, HandCategory::StraightFlush);
            assert_eq!(r.tiebreakers, [14, 0, 0, 0, 0]);
        }
    }

    mod five_card_tiebreak_tests {
        use super::*;

        #[test]
        fn high_card_tiebreakers_descending() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::King, Suit::Hearts),
                c(Rank::Nine, Suit::Clubs),
                c(Rank::Seven, Suit::Diamonds),
                c(Rank::Three, Suit::Spades),
            ]));
            assert_eq!(r.tiebreakers, [14, 13, 9, 7, 3]);
        }

        #[test]
        fn one_pair_tiebreakers_pair_then_kickers() {
            let r = rank_five_card_hand(&h([
                c(Rank::Ten, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Seven, Suit::Diamonds),
                c(Rank::Three, Suit::Hearts),
            ]));
            assert_eq!(r.tiebreakers, [10, 14, 7, 3, 0]);
        }

        #[test]
        fn two_pair_tiebreaker_compares_kicker() {
            let h1 = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Jack, Suit::Diamonds),
                c(Rank::Nine, Suit::Spades),
            ]));

            let h2 = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Ace, Suit::Diamonds),
                c(Rank::Jack, Suit::Hearts),
                c(Rank::Jack, Suit::Spades),
                c(Rank::Eight, Suit::Clubs),
            ]));

            assert!(h1 > h2);
        }

        #[test]
        fn ordering_works_across_categories_and_tiebreakers() {
            let p10 = rank_five_card_hand(&h([
                c(Rank::Ten, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Seven, Suit::Diamonds),
                c(Rank::Three, Suit::Hearts),
            ]));

            let pj = rank_five_card_hand(&h([
                c(Rank::Jack, Suit::Spades),
                c(Rank::Jack, Suit::Hearts),
                c(Rank::Two, Suit::Clubs),
                c(Rank::Three, Suit::Diamonds),
                c(Rank::Four, Suit::Hearts),
            ]));

            assert!(pj > p10);

            let sf = rank_five_card_hand(&h([
                c(Rank::Nine, Suit::Clubs),
                c(Rank::Ten, Suit::Clubs),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Queen, Suit::Clubs),
                c(Rank::King, Suit::Clubs),
            ]));

            let fk = rank_five_card_hand(&h([
                c(Rank::Ace, Suit::Spades),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Ace, Suit::Diamonds),
                c(Rank::Two, Suit::Spades),
            ]));

            assert!(sf > fk);
        }
    }

    mod best_of_seven_tests {
        use super::*;

        #[test]
        fn best_of_seven_finds_straight_flush_over_other_hands() {
            let seven: [Card; 7] = [
                c(Rank::Nine, Suit::Clubs),
                c(Rank::Ten, Suit::Clubs),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Queen, Suit::Clubs),
                c(Rank::King, Suit::Clubs),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::Two, Suit::Spades),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::StraightFlush);
            assert_eq!(best.tiebreakers, [13, 0, 0, 0, 0]);
        }

        #[test]
        fn best_of_seven_prefers_full_house_over_three_of_a_kind() {
            let seven: [Card; 7] = [
                c(Rank::Eight, Suit::Spades),
                c(Rank::Eight, Suit::Hearts),
                c(Rank::Eight, Suit::Clubs),
                c(Rank::Two, Suit::Diamonds),
                c(Rank::Two, Suit::Spades),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::King, Suit::Clubs),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::FullHouse);
            assert_eq!(best.tiebreakers, [8, 2, 0, 0, 0]);
        }

        #[test]
        fn best_of_seven_picks_best_two_pair_and_kicker() {
            let seven: [Card; 7] = [
                c(Rank::Ace, Suit::Spades),
                c(Rank::Ace, Suit::Hearts),
                c(Rank::Jack, Suit::Clubs),
                c(Rank::Jack, Suit::Diamonds),
                c(Rank::Ten, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Nine, Suit::Clubs),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::TwoPair);
            assert_eq!(best.tiebreakers, [14, 11, 10, 0, 0]);
        }

        #[test]
        fn best_of_seven_uses_kicker_to_break_one_pair_ties() {
            let seven: [Card; 7] = [
                c(Rank::King, Suit::Spades),
                c(Rank::King, Suit::Hearts),
                c(Rank::Ace, Suit::Clubs),
                c(Rank::Queen, Suit::Diamonds),
                c(Rank::Jack, Suit::Spades),
                c(Rank::Nine, Suit::Hearts),
                c(Rank::Two, Suit::Clubs),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::OnePair);
            assert_eq!(best.tiebreakers, [13, 14, 12, 11, 0]);
        }

        #[test]
        fn best_of_seven_detects_wheel_straight_when_present() {
            let seven: [Card; 7] = [
                c(Rank::Ace, Suit::Spades),
                c(Rank::Two, Suit::Hearts),
                c(Rank::Three, Suit::Clubs),
                c(Rank::Four, Suit::Diamonds),
                c(Rank::Five, Suit::Spades),
                c(Rank::King, Suit::Hearts),
                c(Rank::Queen, Suit::Clubs),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::Straight);
            assert_eq!(best.tiebreakers, [5, 0, 0, 0, 0]);
        }

        #[test]
        fn best_of_seven_prefers_flush_over_straight_when_both_possible() {
            let seven: [Card; 7] = [
                c(Rank::Ace, Suit::Spades),
                c(Rank::Jack, Suit::Spades),
                c(Rank::Nine, Suit::Spades),
                c(Rank::Six, Suit::Spades),
                c(Rank::Three, Suit::Spades),
                c(Rank::Ten, Suit::Hearts),
                c(Rank::Queen, Suit::Clubs),
            ];

            let best = best_hand_from_seven_cards(&seven);
            assert_eq!(best.category, HandCategory::Flush);
            assert_eq!(best.tiebreakers, [14, 11, 9, 6, 3]);
        }
    }

    mod showdown_winner_tests {
        use super::*;

        #[test]
        fn determines_winner_correctly() {
            let hands = vec![
                (
                    0,
                    rank_five_card_hand(&h([
                        c(Rank::Ten, Suit::Spades),
                        c(Rank::Ten, Suit::Hearts),
                        c(Rank::Ace, Suit::Clubs),
                        c(Rank::Seven, Suit::Diamonds),
                        c(Rank::Three, Suit::Hearts),
                    ])),
                ), // One Pair
                (
                    1,
                    rank_five_card_hand(&h([
                        c(Rank::Jack, Suit::Spades),
                        c(Rank::Jack, Suit::Hearts),
                        c(Rank::Two, Suit::Clubs),
                        c(Rank::Three, Suit::Diamonds),
                        c(Rank::Four, Suit::Hearts),
                    ])),
                ), // One Pair
                (
                    2,
                    rank_five_card_hand(&h([
                        c(Rank::Nine, Suit::Clubs),
                        c(Rank::Ten, Suit::Clubs),
                        c(Rank::Jack, Suit::Clubs),
                        c(Rank::Queen, Suit::Clubs),
                        c(Rank::King, Suit::Clubs),
                    ])),
                ), // Straight Flush
            ];

            let result = determine_showdown_winner(&hands);
            assert_eq!(result.winner_indices, vec![2]);
            assert_eq!(result.winning_rank.category, HandCategory::StraightFlush);
        }

        #[test]
        fn determines_tie_correctly() {
            let hands = vec![
                (
                    0,
                    rank_five_card_hand(&h([
                        c(Rank::Ace, Suit::Spades),
                        c(Rank::Ace, Suit::Hearts),
                        c(Rank::Jack, Suit::Clubs),
                        c(Rank::Jack, Suit::Diamonds),
                        c(Rank::Nine, Suit::Spades),
                    ])),
                ), // Two Pair
                (
                    1,
                    rank_five_card_hand(&h([
                        c(Rank::Ace, Suit::Clubs),
                        c(Rank::Ace, Suit::Diamonds),
                        c(Rank::Jack, Suit::Hearts),
                        c(Rank::Jack, Suit::Spades),
                        c(Rank::Nine, Suit::Clubs),
                    ])),
                ), // Two Pair
            ];

            let result = determine_showdown_winner(&hands);
            assert_eq!(result.winner_indices, vec![0, 1]);
            assert_eq!(result.winning_rank.category, HandCategory::TwoPair);
        }
    }
}

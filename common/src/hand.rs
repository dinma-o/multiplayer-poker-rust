use crate::card_deck::{ Card, Deck };
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hand<const N: usize> {
    cards: [Card; N],
}

impl<const N: usize> Hand<N> {
    pub fn new(cards: [Card; N]) -> Self {
        Self { cards }
    }

    pub fn draw_from_deck(deck: &mut Deck) -> Option<Self> {
        // Confirm enough cards in deck
        if deck.size() < N {
            return None;
        }

        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(deck.deal_card()?);
        }
        let cards: [Card; N] = v.try_into().ok()?;
        Some(Self::new(cards))
    }

    pub fn cards(&self) -> &[Card; N] {
        &self.cards
    }

    pub fn get(&self, idx: usize) -> Option<Card> {
        self.cards.get(idx).copied()
    }

    pub fn replace_from_deck(&mut self, indices: &[usize], deck: &mut Deck) -> Option<Vec<Card>> {
        // bounds check + uniqueness check
        {
            use std::collections::HashSet;
            let mut seen = HashSet::with_capacity(indices.len());
            for &idx in indices {
                if idx >= N {
                    panic!("Index out of bounds for hand replacement");
                }
                if !seen.insert(idx) {
                    panic!("Duplicate index in hand replacement");
                }
            }

            // confirm enough cards in deck
            if deck.size() < indices.len() {
                return None;
            }
        }

        // Sort indices
        let mut indices = indices.to_vec();
        indices.sort_unstable();

        // draw replacements first
        let mut new_cards = Vec::with_capacity(indices.len());
        for _ in 0..indices.len() {
            new_cards.push(deck.deal_card()?);
        }

        // now apply replacements
        let mut discarded = Vec::with_capacity(indices.len());
        for (i, &idx) in indices.iter().enumerate() {
            let old = std::mem::replace(&mut self.cards[idx], new_cards[i]);
            discarded.push(old);
        }

        Some(discarded)
    }

    pub fn contains(&self, card: &Card) -> bool {
        self.cards.iter().any(|c| c == card)
    }

    pub fn len(&self) -> usize {
        N
    }
}

impl<const N: usize> fmt::Display for Hand<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hand[")?;
        for (i, c) in self.cards.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", c)?;
        }
        write!(f, "]")
    }
}

// Five-card draw alias
pub type FiveCardHand = Hand<5>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_deck::{ Deck };
    use std::collections::HashSet;

    // FiveCardHand tests
    #[test]
    fn hand_new_len_get_cards() {
        let mut deck = Deck::new();
        let hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        assert_eq!(hand.len(), 5);
        assert_eq!(hand.cards().len(), 5);

        // get in bounds
        assert!(hand.get(0).is_some());
        assert!(hand.get(4).is_some());

        // get out of bounds
        assert!(hand.get(5).is_none());
    }

    #[test]
    fn draw_from_deck_reduces_deck_size() {
        let mut deck = Deck::new();
        let _hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();
        assert_eq!(deck.size(), 52 - 5);
    }

    #[test]
    fn draw_from_deck_returns_none_if_not_enough_cards() {
        type BigHand = Hand<60>;

        let mut deck = Deck::new();
        let before = deck.size();

        let hand = BigHand::draw_from_deck(&mut deck);
        assert!(hand.is_none());
        assert_eq!(deck.size(), before, "Deck size should not change on failed draw");
    }

    #[test]
    fn contains_works() {
        let mut deck = Deck::new();
        let hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        let c0 = hand.get(0).unwrap();
        assert!(hand.contains(&c0));

        // draw a new card from deck; should not be in the current hand (almost certainly)
        // but to be exact, pick a card we know isn't in the hand by checking.
        let mut found_outside = None;
        while let Some(c) = deck.deal_card() {
            if !hand.contains(&c) {
                found_outside = Some(c);
                break;
            }
        }
        let outside = found_outside.expect("expected to find a card not in hand");
        assert!(!hand.contains(&outside));
    }

    #[test]
    fn replace_from_deck_replaces_cards_and_returns_discarded() {
        let mut deck = Deck::new();
        let mut hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        let before_cards: Vec<Card> = hand.cards().iter().copied().collect();
        let before_deck_size = deck.size();

        let indices = [1usize, 3usize];
        let discarded = hand.replace_from_deck(&indices, &mut deck).unwrap();

        // returned correct count
        assert_eq!(discarded.len(), indices.len());

        // deck decreased by number replaced
        assert_eq!(deck.size(), before_deck_size - indices.len());

        // discarded cards are exactly the old cards at those indices (order is sorted internally)
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        assert_eq!(discarded[0], before_cards[sorted[0]]);
        assert_eq!(discarded[1], before_cards[sorted[1]]);

        // hand changed at those indices
        assert_ne!(hand.get(sorted[0]).unwrap(), before_cards[sorted[0]]);
        assert_ne!(hand.get(sorted[1]).unwrap(), before_cards[sorted[1]]);

        // other indices unchanged
        for i in 0..5 {
            if !sorted.contains(&i) {
                assert_eq!(hand.get(i).unwrap(), before_cards[i]);
            }
        }

        // no duplicates inside hand (sanity)
        let set: HashSet<Card> = hand.cards().iter().copied().collect();
        assert_eq!(set.len(), 5);
    }

    #[test]
    fn replace_from_deck_accepts_unsorted_indices() {
        let mut deck = Deck::new();
        let mut hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        let before = hand.cards().to_vec();
        let discarded = hand.replace_from_deck(&[4, 0, 2], &mut deck).unwrap();

        // Internal sorting means discarded corresponds to old cards at [0,2,4]
        assert_eq!(discarded.len(), 3);
        assert_eq!(discarded[0], before[0]);
        assert_eq!(discarded[1], before[2]);
        assert_eq!(discarded[2], before[4]);
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn replace_from_deck_panics_on_out_of_bounds() {
        let mut deck = Deck::new();
        let mut hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        // idx 5 is out of bounds for 5-card hand
        let _ = hand.replace_from_deck(&[5], &mut deck);
    }

    #[test]
    #[should_panic(expected = "Duplicate index")]
    fn replace_from_deck_panics_on_duplicate_indices() {
        let mut deck = Deck::new();
        let mut hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        let _ = hand.replace_from_deck(&[2, 2], &mut deck);
    }

    #[test]
    fn replace_from_deck_returns_none_if_not_enough_cards() {
        type BigHand = Hand<50>;
        let mut deck = Deck::new();
        let mut hand = BigHand::draw_from_deck(&mut deck).unwrap();

        // only 2 cards should be in deck right now, so try to replace 3
        let result = hand.replace_from_deck(&[0, 1, 2], &mut deck);
        assert!(result.is_none());
    }

    #[test]
    fn display_includes_hand_brackets_and_cards() {
        let mut deck = Deck::new();
        let hand = FiveCardHand::draw_from_deck(&mut deck).unwrap();

        let s = format!("{}", hand);
        assert!(s.starts_with("Hand["));
        assert!(s.ends_with("]"));
        // should contain commas between cards for 5-card hand
        assert!(s.contains(", "));
    }
}

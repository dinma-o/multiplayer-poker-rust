# Five-Card Draw Poker — House Rules

This document defines the **house rules** for Five-Card Draw Poker as implemented by the Poker Playing System. These rules are designed to be deterministic, unambiguous, and suitable for both human play and server-side enforcement.

The rules align with standard Five-Card Draw conventions while making explicit choices where poker rules commonly vary. These clarifications are intentional and required for correct gameplay, testing, and grading.

---

## 1. Game Overview

- Variant: **Five-Card Draw Poker**
- Players per table: **2–5 players**
- Deck: **Standard 52-card deck**, no jokers
- Objective: Win chips by forming the highest-ranking five-card poker hand or by causing all other players to fold
- Dealer: **House-controlled dealer**

---

## 2. Player Accounts and Chips

- Each player has a **unique ID** (name) and an associated account
- Accounts track:
  - Chip balance
  - Hands played
  - Pots won
  - Folds
- Chips are finite and deducted immediately when wagered
- A player with insufficient chips to ante **may not participate** in the hand

---

## 3. Table and Dealer Rules

- The dealer button rotates **clockwise** after each completed hand
- Players may **only join or leave** the table between hands
- The dealer is responsible for:
  - Shuffling
  - Dealing cards
  - Managing betting rounds
  - Enforcing house rules
  - Determining the winner

---

## 4. Antes and Betting Limits

### 4.1 Antes

- Each active player must post a **fixed ante** before cards are dealt
- The ante amount is defined by the table configuration
- Antes contribute directly to the pot

### 4.2 Betting Structure

- Betting is **fixed-limit**
- A betting round consists of:
  - Bet / Call / Raise / Fold
- Maximum of **one bet and one raise** per betting round
- All bets and raises must be made in fixed increments

---

## 5. Deal Phase

- Cards are dealt **one at a time**, face down
- Each player receives **exactly five cards**
- Deal order begins with the player **to the left of the dealer** and proceeds clockwise

---

## 6. First Betting Round

- The first betting round begins with the player **to the left of the dealer**
- If all players check/pass, the betting round ends
- A player may:
  - Check (if no bet exists)
  - Bet
  - Call
  - Raise (if allowed)
  - Fold

---

## 7. Draw Phase

- Players may discard **0 to 3 cards**
- Discards are replaced from the deck
- Draw order begins with the player **to the left of the dealer**
- Cards are dealt face down
- A player who stands pat draws **zero cards**

---

## 8. Second Betting Round

- The second betting round begins with the **first active player left of the dealer**
- Betting rules are identical to the first betting round
- If all but one player folds, the remaining player wins the pot immediately

---

## 9. Showdown Rules

- If two or more players remain after the second betting round, a showdown occurs
- Hands are revealed in clockwise order starting left of the dealer
- The dealer determines the winning hand using standard poker rankings
- Only hands that are **called** must be shown

---

## 10. Hand Rankings (Highest to Lowest)

1. Royal Flush
2. Straight Flush
3. Four of a Kind
4. Full House
5. Flush
6. Straight
7. Three of a Kind
8. Two Pair
9. One Pair
10. High Card

---

## 11. Tie-Breaking Rules

- Only **one level of tie-breaking** is applied
  - Example: highest pair, then highest kicker
- If hands remain tied after first-level comparison:
  - The pot is **split evenly**
  - Any remainder chip is discarded (house keeps odd chip)

---

## 12. Winning the Pot

A player wins the pot by:

- Holding the highest-ranking hand at showdown, **or**
- Being the last remaining player after all others fold

Winnings are credited immediately to the player’s account.

---

## 13. Invalid Actions and Enforcement

- Invalid actions (e.g., betting out of turn, exceeding limits) are rejected
- The dealer enforces all rules strictly
- The system assumes **no player collusion**

---

## 14. Determinism and Fair Play

- All shuffles must be statistically random
- Game state transitions are deterministic
- No hidden information is exposed beyond what poker rules allow

---

## 15. References

- Five-Card Draw Poker (Wikipedia)
- Project specifications and marking criteria

---

*End of RULES.md*


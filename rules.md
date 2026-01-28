# Five-Card Draw Poker — House Rules

This document defines the **house rules** for Five-Card Draw Poker as implemented by the Poker Playing System. These rules are designed to be deterministic, unambiguous, and suitable for both human play and server-side enforcement.

Where values are numeric (ante, min/max bet, etc.), the **single source of truth** is:
`common/src/constants.rs` under `five_card_draw`.

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

### 2.1 Starting Stack

- Each player starts with **INITIAL_CHIPS** (default: **200**) at the beginning of a new table/game.

### 2.2 Table Eligibility and Elimination

To keep gameplay simple and avoid side pots / all-in logic:

- **Minimum participation requirement:** A player must have at least the **ANTE_AMOUNT** to be dealt into a hand.
- If a player has **insufficient chips to post the ante**, they **do not participate** in that hand and are **removed from the table** (eliminated).
- Players may only be removed **between hands** (i.e., before antes are collected and cards are dealt).

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

- Each active player must post a **fixed ante** of **ANTE_AMOUNT** (default: **5 chips**) before cards are dealt
- Antes contribute directly to the pot

### 4.2 Betting Structure (Spread-Limit in 5s, No All-In)

Bet sizing is **in increments of BET_INCREMENT** (default: **5 chips**) with a **maximum** per betting round.

- A betting round consists of actions: **Check / Bet / Call / Raise / Fold**
- **All bet/raise amounts must be multiples of BET_INCREMENT**
- **Max raises per betting round:** **MAX_RAISES_PER_ROUND** (default: **1**, table-wide)

#### Bet Amount Rules
- If no bet exists yet in the round:
  - A player may **check**, or
  - **bet** an amount `B` where:
    - `MIN_BET_PER_ROUND ≤ B ≤ MAX_BET_PER_ROUND`
    - `B` is a multiple of `BET_INCREMENT`

#### Call Amount Rules
- If a bet exists:
  - A player may **call** by matching the current bet amount (paying the difference between their contribution this round and the current bet).

#### Raise-To Rules (Raise Sizing)
- If a bet exists and a raise is still available this round:
  - A player may **raise-to** an amount `R` where:
    - `R > current_bet`
    - `R` is a multiple of `RAISE_INCREMENT` (default: same as `BET_INCREMENT`)
    - `R ≤ MAX_RAISE_TO_AMOUNT` (default: same as `MAX_BET_PER_ROUND`)
- Example (with defaults):
  - Current bet = 5 → legal raise-to values include 10, 15, 20, 25 (since max is 25)

#### Insufficient Chips Rule (No All-In)
- **All-in is not allowed.**
- If a player is required to put chips in the pot (e.g., to **call** or **raise-to**) but does **not** have enough chips to pay the **full required amount**, they **must fold**.
- A player may not make a partial call or partial raise.

---

## 5. Deal Phase

- Cards are dealt **one at a time**, face down
- Each player receives **exactly five cards**
- Deal order begins with the player **to the left of the dealer** and proceeds clockwise

---

## 6. First Betting Round

- The first betting round begins with the player **to the left of the dealer**
- If all active players check, the betting round ends
- If at any time all but one player folds, the remaining player wins the pot immediately

---

## 7. Draw Phase

- Players may discard **0 to 3 cards**
- Discards are replaced from the deck
- Draw order begins with the player **to the left of the dealer**
- Replacement cards are dealt face down
- A player who stands draws **zero cards**

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

- Hands are compared using standard poker tie-breaking:
  - Compare the primary made hand (e.g., pair rank, trips rank, etc.)
  - If still tied, compare remaining kickers in descending order as needed
- If hands remain tied after full comparison:
  - The pot is **split evenly**
  - Any remainder chip is discarded (house keeps odd chip) to keep determinism

---

## 12. Winning the Pot

A player wins the pot by:

- Holding the highest-ranking hand at showdown, **or**
- Being the last remaining player after all others fold

Winnings are credited immediately to the player’s account.

---

## 13. Invalid Actions and Enforcement

- Invalid actions (e.g., betting out of turn, exceeding limits, drawing too many cards) are rejected
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

*End of Five-Card Draw House Rules*
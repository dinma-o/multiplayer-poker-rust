# Suggested Issue List — Project 1 (Server) (1-week scope)

Create these issues together as a team (use Task/Bug templates).  
Each issue should have: **Epic + Owner + Size + Type_** and 3–6 acceptance criteria.

---

## A) Project setup / docs (keep light)

### 1. [Chore] Define “house rules” constants
**Epic:** Project Management & Process  
**AC:**
- [ ] Decide ante amount, min bet, max bet (or max raises), starting chips policy
- [ ] Put constants in one module (e.g., `server::rules`)
- [ ] Document rules in README

### 2. [Docs] README: build/run + demo script
**Epic:** Code Quality / Build / README / Tests  
**AC:**
- [ ] `cargo build` / `cargo run -p server` instructions
- [ ] Demo script for 2 hands (step-by-step)
- [ ] Mention persistence file location + reset steps

---

## B) Common types (shared)

### 3. [Feature] Card + Deck primitives
**Epic:** Hand Ranking  
**AC:**
- [ ] `Suit`, `Rank`, `Card` types in `common/`
- [ ] `Deck::new()`, `shuffle()`, `deal(n)`
- [ ] Unit test: 52 unique cards and deal reduces deck

### 4. [Feature] Hand representation + helpers
**Epic:** Hand Ranking  
**AC:**
- [ ] 5-card hand type (e.g., `[Card; 5]` wrapper)
- [ ] Sorting/grouping helpers for ranks/suits
- [ ] Unit test: stable sorting/grouping works

---

## C) Hand ranking (start early)

### 5. [Feature] Hand evaluator (categories)
**Epic:** Hand Ranking  
**AC:**
- [ ] Detect: high card, pair, two pair, trips, straight, flush, full house, quads, straight flush
- [ ] Return `category + tiebreak vector` (or equivalent)
- [ ] No panics on invalid input

### 6. [Test] Hand ranking unit tests (core coverage)
**Epic:** Hand Ranking  
**AC:**
- [ ] Tests per category (at least 1–2 each)
- [ ] A couple edge cases (wheel straight A-2-3-4-5, etc.)
- [ ] Tests run in CI/local with `cargo test`

### 7. [Feature] Tie-breakers (first-level) + split pot support
**Epic:** Hand Ranking  
**AC:**
- [ ] Compare `tiebreak vector` for first-level tie-break
- [ ] If still tied after first-level: mark as tie
- [ ] Support splitting pot among tied winners

---

## D) Accounts + persistence

### 8. [Feature] Account model + stats
**Epic:** Customer Management (Accounts)  
**AC:**
- [ ] Account has ID/name, PIN, balance
- [ ] Stats tracked: games played, pots won, folds
- [ ] Can display account summary in CLI

### 9. [Feature] Create/login account (PIN)
**Epic:** Customer Management (Accounts)  
**AC:**
- [ ] Create account flow
- [ ] Login validates PIN (reject incorrect)
- [ ] Unique identifier enforced (name or id)

### 10. [Feature] Deposit/withdraw + balance validation
**Epic:** Customer Management (Accounts)  
**AC:**
- [ ] Deposit increases balance
- [ ] Withdraw decreases but never negative
- [ ] Clear errors on invalid amounts

### 11. [Feature] Persist accounts to file (JSON)
**Epic:** Customer Management (Accounts)  
**AC:**
- [ ] Load accounts on startup
- [ ] Save accounts on changes / exit
- [ ] Restart preserves balances + stats

---

## E) Table management

### 12. [Feature] Table: seating + join/leave rules
**Epic:** Table Management  
**AC:**
- [ ] 2–5 players at a table
- [ ] Join/leave allowed only between hands
- [ ] Prevent duplicates / enforce capacity

### 13. [Feature] Dealer rotation + action order helpers
**Epic:** Table Management  
**AC:**
- [ ] Dealer rotates each hand
- [ ] Determine “left of dealer” correctly
- [ ] Used consistently in betting and draw order

---

## F) Gameplay engine (one-hand end-to-end)

### 14. [Feature] Hand lifecycle: start hand (ante + pot) + deal 5
**Epic:** Gameplay Hand #1  
**AC:**
- [ ] Collect ante from all active players
- [ ] Initialize pot
- [ ] Deal 5 cards to each player (in order)

### 15. [Feature] Betting round engine (pass/bet/call/raise/fold)
**Epic:** Gameplay Hand #1  
**AC:**
- [ ] Supports actions: pass/check, bet, call, raise, fold
- [ ] Enforces basic constraints (min/max or raise limits)
- [ ] Correctly ends round when bets are matched or everyone folded

### 16. [Feature] Draw phase (discard/draw) in correct order
**Epic:** Gameplay Hand #1  
**AC:**
- [ ] Players choose cards to discard (0–5)
- [ ] Replace from deck
- [ ] Draw order starts left of dealer

### 17. [Feature] Showdown + determine winner + payout
**Epic:** Gameplay Hand #1  
**AC:**
- [ ] Use ranking engine to determine winner(s)
- [ ] Split pot if tie
- [ ] Update balances

### 18. [Feature] Update stats after hand + persist
**Epic:** Customer Management (Accounts)  
**AC:**
- [ ] Increment games played for participants
- [ ] Increment folds for folders
- [ ] Increment pots won for winner(s)
- [ ] Persist updated accounts

---

## G) CLI (required)

### 19. [Feature] CLI: account menu + table menu shells
**Epic:** Code Quality / Build / README / Tests (or Customer Mgmt if you prefer)  
**AC:**
- [ ] Menu: create/login, deposit/withdraw, show stats
- [ ] Menu: join table, leave table (between hands), start hand
- [ ] Inputs validated (no panic on bad input)

### 20. [Feature] CLI: play 1 full hand end-to-end
**Epic:** Gameplay Hand #1  
**AC:**
- [ ] Prompt each player for betting actions
- [ ] Prompt discard/draw choices
- [ ] Display winner + pot + new balances

### 21. [Feature] CLI: two-hand demo script support
**Epic:** Gameplay Hand #2  
**AC:**
- [ ] Supports “all pass first betting round” path
- [ ] Supports “raise after draw then fold” path
- [ ] Supports opt-out between hands
- [ ] Outputs are readable for a TA demo

---

## H) Buffer / bugfix (keep as needed)

### 22. [Chore] Input validation + error handling sweep
**Epic:** Code Quality / Build / README / Tests  
**AC:**
- [ ] No panics on normal invalid input
- [ ] Clear error messages and re-prompt
- [ ] Basic smoke test: fresh run → create accounts → play hand

---

## Optional (only if ahead)
- [Test] Add more evaluator tests / randomized deck sanity checks
- [Chore] Add `cargo fmt` / `clippy` GitHub Action
- [Feature] “Demo mode” that guides prompts (still CLI)
# Suggested Issue List — Project 1 (Server) (11-day pace)

Create these together as a team (Task/Bug templates).  
Each issue must have Epic + Owner + Size + Type_ and 3–6 acceptance criteria.

---

## A) Setup / docs (tiny, high value)

1. [Chore] Define & document house rules constants
- AC: decide ante/min/max/raise policy; constants in one place; README rules section updated

2. [Docs] README: build/run + 2-hand demo script
- AC: exact commands; step-by-step demo; persistence file + reset instructions

---

## B) Common types (shared)

3. [Feature] Card + Deck primitives (common/)
- AC: Suit/Rank/Card; Deck new/shuffle/deal; uniqueness test

4. [Feature] Hand representation + helpers (common/)
- AC: 5-card hand type; sorting/grouping helpers; tests

---

## C) Hand ranking (core marks)

5. [Feature] Hand evaluator — categories (common/)
- AC: detect all categories; returns (category + tiebreak); no panics

6. [Test] Evaluator unit tests — category coverage (common/)
- AC: tests per category; edge cases (A-2-3-4-5 straight); `cargo test` passes

7. [Feature] Tie-break (first level) + tie output (common/)
- AC: compare tiebreak vectors; if still tied → mark tie

---

## D) Accounts + persistence

8. [Feature] Account model + stats (server/)
- AC: id/name+PIN+balance; stats (games played, pots won, folds); display summary

9. [Feature] Create/login account with PIN (server/)
- AC: create + login; reject bad PIN; enforce unique id/name

10. [Feature] Deposit/withdraw with validation (server/)
- AC: deposit/withdraw; cannot go negative; clear errors

11. [Feature] Persistence layer (JSON load/save) (server/)
- AC: load at startup; save on change/exit; restart preserves balances+stats

---

## E) Game skeleton (unblocks everyone)

12. [Feature] Game state machine skeleton (phases + structs) (server/)
- AC: phases exist (waiting, ante/deal, bet1, draw, bet2, showdown, payout); compiles; stubs OK

13. [Feature] Table management: seating + join/leave rules (server/)
- AC: 2–5 players; join/leave only between hands; prevent duplicates

14. [Feature] Dealer rotation + action/draw order helpers (server/)
- AC: dealer rotates each hand; left-of-dealer order correct; reusable helper functions

---

## F) Gameplay vertical slice (1 full hand)

15. [Feature] Start hand: ante + pot + deal 5 (server/)
- AC: ante collected; pot tracked; deal in order; players have hands

16. [Feature] Betting engine v1 (actions + round completion) (server/)
- AC: supports check/pass, bet, call, raise, fold; ends round correctly

17. [Feature] Draw phase (discard/draw) (server/)
- AC: choose discards; draw replacements; order starts left of dealer

18. [Feature] Showdown + winner selection using evaluator (server/)
- AC: rank hands; determine winner(s); tie handled

19. [Feature] Payout + account/stat updates + persist (server/)
- AC: balances updated; stats updated; saved to disk

---

## G) CLI (required)

20. [Feature] CLI: account menu (create/login/deposit/withdraw/stats)
- AC: menu works; input validated; uses persistence

21. [Feature] CLI: table menu (join/leave/start hand)
- AC: join/leave rules enforced; start hand triggers game loop

22. [Feature] CLI: play 1 full hand end-to-end (prompts)
- AC: prompt betting + discard/draw; prints winner + pot + balances

---

## H) Demo correctness + robustness

23. [Feature] Demo paths for marking guide (2 hands)
- AC: supports “all pass first round”; “raise after draw then fold”; opt-out between hands; output readable

24. [Chore] Input validation + “no panic” sweep
- AC: re-prompt on bad input; invalid actions handled; smoke tested

25. [Chore] Fresh-clone rehearsal checklist
- AC: clean clone build/run; demo script followed successfully; README matches reality

---

## Bug template usage
- Use Bug issues for crashes, incorrect ranking, betting logic edge cases, persistence problems, etc.
# PokerDUD — Project 1 (Server) 11-Day Roadmap (Student-friendly)

**Timebox:** 11 days (assume limited weekday time due to quizzes/assignments)
**Goal:** reliably demo **two hands of Five-Card Draw** with **accounts + betting + hand ranking** via CLI.

Repo focus:
- `server/` = Project 1
- `common/` = cards/ranking shared types
- `client/` = ignore for Project 1

---

## Minimal workflow (keep it light)
- Every work item is an Issue (Task/Bug template)
- Fields: Epic + Owner + Size + Type_
- One PR closes one issue (`Closes #123`)
- Max 1–2 In Progress per person
- Blocked items must link what they’re blocked by

Definition of Done:
- Acceptance criteria met
- `cargo test` passes (ranking tests at minimum)
- PR merged, issue closed

---

# The schedule (11 days)

## Day 1 (60–90 min): Align + lock minimal interfaces
Outcome: team can work in parallel without merge chaos.
- Agree house rules: ante, min/max bet, raise policy, starting chips policy
- Agree persistence format: JSON file
- Agree module boundaries + minimal APIs (ranking/accounts/game/cli)
- Create the initial issues together (don’t over-split)

Deliverable:
- A short “Rules” section in README
- Skeleton modules exist (empty ok)

---

## Days 2–3 (1–2 hrs/day): Foundations in parallel
Goal: unblock everything early.
- Ranking + tests (common/)
- Accounts + persistence (server/)
- CLI shell (server/)
- Game state skeleton (server/)

Deliverable by end of Day 3:
- CLI can create/login/deposit/withdraw and persists accounts
- Ranking detects several categories with tests
- Game skeleton compiles with stubs (phases exist even if TODO)

---

## Day 4 (2–3 hrs): Vertical slice — 1 full hand runs (rough UI ok)
Goal: one playable hand end-to-end through CLI.
Order:
- ante + pot + deal 5
- betting round 1
- draw phase
- betting round 2
- showdown + payout + persist

Deliverable:
- One complete hand without crashing.

---

## Day 5 (1–2 hrs): Correctness day — enforce rules & turn order
Goal: make “it runs” become “it’s correct enough to demo”.
- dealer rotation
- action order and draw order (left of dealer)
- min/max bet & raise/call rules
- fold logic and “win by everyone folding”
- join/leave only between hands

Deliverable:
- One-hand gameplay is consistent and rule-correct.

---

## Day 6 (1–2 hrs): Expand ranking coverage + tie/split pot
Goal: protect marks.
- finish evaluator coverage for all categories
- tie-breaker first-level
- split pot logic integrated into payout

Deliverable:
- Evaluator tests cover all categories (or very close).

---

## Days 7–8 (weekend block): Marking-demo mode (two hands reliably)
Goal: reliably reproduce the marking guide scenario.
- “All pass first round” path
- “raise after draw then fold” path
- opt-out between hands
- stats update (games played, pots won, folds)
- output readability (TA-friendly)

Deliverable by end of Day 8:
- Two-hand demo works following a written script.

---

## Day 9 (60–90 min): Stabilize + input validation sweep
Goal: eliminate silly failures.
- no panics on normal invalid input
- re-prompt on bad input
- clean errors for invalid actions (bet out of turn, too large, etc.)

Deliverable:
- Smooth demo even with minor input mistakes.

---

## Day 10 (60–90 min): README + fresh-clone rehearsal
Goal: “TA can run it”.
- README: build/run/demo steps
- mention persistence file location + how to reset
- run on a fresh clone / clean build to ensure no hidden dependencies

Deliverable:
- Fresh clone → run → demo works.

---

## Day 11 (30–60 min): Final rehearsal + bugfix-only freeze
Goal: confidence.
- run demo 2–3 times
- bugfix only
- tag final commit if you want

Deliverable:
- submission-ready.

---

## Guardrails
- If time is tight: simplify CLI UI, NOT core requirements.
- Always protect the vertical slice first.
- Weekend is for “two-hand demo + polish”, not new features.
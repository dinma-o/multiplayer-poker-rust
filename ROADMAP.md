# PokerDUD — Project 1 (Server) 7-Day Roadmap

**Timebox:** 1 week  
**Goal:** reliably demo **two hands of Five-Card Draw** with **accounts + betting + hand ranking** via CLI.

Repo structure:

- `server/` = Project 1 focus
- `common/` = shared types (cards/ranking/etc.)
- `client/` = ignore for Project 1 (Phase 2 later)

---

## Minimal workflow (keep it light)

We use GitHub Projects for visibility, not bureaucracy.

Rules:

1. Every work item is an **Issue** (Task/Bug template).
2. Each Issue has: **Epic + Owner + Size + Type\_**.
3. One PR closes one Issue (`Closes #123`).
4. Keep WIP small: **max 1–2 In Progress per person**.
5. Blocked items must say **Blocked by:** and link the dependency.

Definition of Done (DoD):

- [ ] Acceptance criteria met
- [ ] `cargo test` passes (at least ranking tests)
- [ ] PR merged, issue closed

---

## Critical path (don’t deviate)

If it doesn’t directly support the demo, it’s optional.

**Must-have for marks / demo:**

- CLI (mandatory)
- Accounts (create/login + balance + persistence)
- Betting (pass/bet/call/raise/fold + limits)
- Draw phase (discard/draw)
- Hand ranking + 1st-level tie-break + split pot
- Table rules: 2–5 players, join/leave between hands, dealer rotation
- Stats updates after hands

---

# 7-Day plan (day-by-day)

## Day 1 — Align + lock interfaces + create issues (2–3 hrs total)

**Outcome:** everyone can work in parallel with minimal merge pain.

1. 30-min team sync (requirements)

- Agree house rules: ante, min/max bet, raise policy, starting chip behavior.
- Decide persistence: JSON file.

2. 45-min interface lock (minimal APIs)

- Ranking API signature in `common/`
- Account + storage API in `server/`
- Game state machine skeleton in `server/`

3. 45-min “create issues together”

- Create only the issues needed for the demo (avoid extras).
- Assign Owner + Size.

**End of Day 1 deliverable**

- Skeleton modules created
- CI optional (nice-to-have, don’t block)
- Everyone has 2–4 issues owned

---

## Day 2 — Foundations: ranking + accounts + CLI shell

**Parallel focus (high leverage):**

- Person A: game state skeleton (table/hand phases stubbed)
- Person B: hand ranking + tests (start here)
- Person C: accounts + persistence + CLI menus (start here)

**End of Day 2 deliverable**

- `common::cards` exists (Card/Suit/Rank)
- Ranking passes unit tests for at least 3–4 hand categories
- CLI can create/login/deposit/withdraw and persists data

---

## Day 3 — One full hand loop (first playable vertical slice)

**Goal:** play 1 complete hand end-to-end (even if UI is rough).

Order:

- Deal 5 cards
- Ante + pot
- Betting round 1 (basic actions)
- Draw phase (discard/draw)
- Betting round 2
- Showdown + payout using ranking
- Update balances + persist

**End of Day 3 deliverable**

- You can play 1 full hand through CLI without crashing.

---

## Day 4 — Make betting + rules correct (demo correctness day)

**Goal:** match expected behaviors (not just “it runs”).

Checklist:

- Enforce min/max bet and valid raise/call logic
- Fold logic correct (winner by everyone folding)
- Dealer rotation correct
- Draw order: left of dealer draws first
- Join/leave only between hands

**End of Day 4 deliverable**

- One-hand gameplay is consistent and rule-correct.

---

## Day 5 — Implement the “two-hand marking demo” path

**Goal:** reliably reproduce the two-hand scenario.

Add/verify:

- “All pass first betting round” path
- Raise after draw path
- Fold after raise path
- Opt out between hands
- Stats updated (games played, pots won, folds)

**End of Day 5 deliverable**

- Two-hand demo works when you follow a written script.

---

## Day 6 — Stabilize + polish + README + error handling

**Goal:** eliminate silly mark losses.

Checklist:

- README: build/run/demo steps
- Ranking tests complete for all categories (or most, if time)
- Input validation (no panics on normal bad input)
- `cargo fmt` clean
- Smoke test on fresh clone

**End of Day 6 deliverable**

- “Submission-ready” build that a TA can run quickly.

---

## Day 7 — Final rehearsal + bugfix-only

**Goal:** demo rehearsal and freeze.

Plan:

- Run the full demo 2–3 times
- Fix only critical bugs
- Tag a release / final commit
- Ensure license + repo structure intact

**End of Day 7 deliverable**

- Confident live demo.

---

## Ownership split (recommended)

- **Person A (Game/State Machine):** table + phases + betting engine
- **Person B (Ranking):** evaluation + tie-break + tests
- **Person C (Accounts/CLI):** accounts + persistence + CLI demo path

---

## Guardrails

- If something is taking too long: **simplify UI**, not the core requirements.
- Never block the whole team on “perfect design.”
- Target a working vertical slice by **Day 3**.

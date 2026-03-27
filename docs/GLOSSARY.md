# Glossary — PokerDUD (ECE 421 Poker Project)

This glossary defines key terms used in:
- our roadmap/planning docs,
- the ECE 421 project proposal,
- the Project 1 marking guide.

---

## A

**Account**  
A persistent player record containing identity info (e.g., name/ID), authentication (PIN), chip/token balance, and statistics/history.

**Action (Poker Action)**  
A player decision during betting (e.g., check/pass, bet, call, raise, fold).

**All-in**  
A bet of a player’s entire remaining chips. The proposal allows a *house rule* that does **not** require standard all-in behavior.

**Ante**  
A forced contribution each player pays at the start of a hand (before cards are dealt), added to the pot.

**API (Application Programming Interface)**  
A set of function/type boundaries between modules (e.g., ranking API, accounts API) or between client/server later.

---

## B

**Backlog**  
The list of planned issues/tasks not currently in progress.

**Bank / Token Balance / Chip Balance**  
The amount of in-game currency a player has available for betting. (Terminology varies: “bank”, “tokens”, “chips”.)

**Bet**  
Putting chips into the pot. Often the first “money action” in a betting round.

**Betting**  
The wagering system of poker (bet/call/raise/fold/check), controlled by house rules.

**Betting Round**  
A phase of the hand where players act in turn until all remaining players have matched the current bet (or everyone checks).

**Blocked (Issue Status)**  
A task that cannot proceed due to a dependency (another issue/PR, missing design decision, etc.). Should include “Blocked by: …”.

---

## C

**Call**  
Matching the current bet amount to stay in the hand.

**Cargo**  
Rust’s build tool and package manager. Common commands: `cargo build`, `cargo run`, `cargo test`.

**Cargo Workspace**  
A Rust repo structure where multiple crates (e.g., `client`, `server`, `common`) share one top-level `Cargo.toml`.

**Card**  
A playing card with a rank (2–A) and suit (♣♦♥♠).

**Category (Hand Category)**  
The hand class for ranking (e.g., pair, two pair, straight, flush, full house, etc.).

**Chips**  
Poker currency used for betting (may be represented as tokens/integers).

**CLI (Command Line Interface)**  
Text-based interaction via terminal prompts/menus. Required for Project 1 demo.

**Client (a.k.a. Player)**  
The program representing a user/player. Phase 2 focuses on the client; Phase 1 uses CLI to exercise the server.

**Common Module / `common/`**  
Shared code between `client/` and `server/` such as cards, ranking types, and later protocol structs.

**Crate**  
A Rust package/unit of compilation (e.g., `server` crate, `client` crate, `common` crate).

---

## D

**Deal**  
Distributing cards to players from the deck.

**Dealer**  
The role that determines order of actions (and dealing order). In many variants the dealer position rotates each hand.

**Dealer Rotation**  
The dealer “button” moves to the next player after each hand.

**Deck**  
A standard 52-card set used to deal cards.

**Demo Script**  
A written, step-by-step sequence used during marking to reliably show required features (two-hand scenario).

**Discard**  
In Five-card Draw, selecting cards to throw away before drawing replacements.

**Draw Phase**  
In Five-card Draw, players discard and then draw replacement cards (typically starting left of dealer).

**Draw Poker / Five-card Draw**  
Poker variant where each player gets 5 cards, can exchange some cards once, then showdown.

---

## E

**Epic (Project Tracking)**  
A high-level grouping of issues (e.g., “Hand Ranking”, “Accounts”, “Gameplay”). Used to organize work in GitHub Projects.

**egui**  
A Rust GUI framework mentioned as an acceptable option (primarily Phase 2 / GUI work).

**ExpressJS**  
A Node.js web framework mentioned as a possible GUI/graphical interface basis (primarily Phase 2).

---

## F

**Fold**  
To leave the hand and stop competing for the pot (you lose any chips already committed).

**FR (Functional Requirement)**  
A “what the system must do” requirement (server or client). In the proposal these appear as S-FR-* or C-FR-*.

**Fresh Clone Test**  
A rehearsal where you clone the repo into a clean environment and ensure build/run/demo steps work as written.

---

## G

**Game Engine (Server Game Engine)**  
The logic on the server that manages tables, betting flow, dealing, draw phases, and winner determination.

**Gameplay Loop / Hand Loop**  
The full sequence of a hand: start → ante → deal → betting → draw → betting → showdown → payout → update stats.

**Git**  
Version control system used to track changes. The course expects all code under revision control.

**GitHub Classroom**  
Course-managed GitHub environment where the repo is shared with instructors/TAs.

**GitHub Projects (Projects v2)**  
GitHub’s project-tracking board used to manage a sized backlog and track task closure via PRs.

---

## H

**Hand**  
One round of poker from dealing cards to winner payout.

**Hand Ranking / Ranking Engine**  
Code that evaluates a player’s final hand and determines its strength compared to others.

**History (Account History)**  
Recorded past actions/outcomes for a player (e.g., previous games, results). Marking guide mentions history as part of account record.

**House / Casino (Server / House)**  
The proposal uses “server” and “house” interchangeably. The house runs tables, enforces rules, manages accounts/chips.

**House Rules**  
Local rule choices allowed by the project (e.g., raise limits, min/max bet, how to handle all-in).

**HTTP (Hypertext Transfer Protocol)**  
A protocol mentioned as a possible wrapper for Phase 2 client/server communication (not required for Project 1).

---

## I

**ID (Player Identifier)**  
A unique name or identifier for a player.

**Inter-node Interface**  
Client/server communication interface. Must be IP-based (TCP or UDP) for Phase 2; not necessarily distributed for Phase 1.

**IP (Internet Protocol)**  
Networking layer requirement for Phase 2 communication: inter-node interface must be IP-based.

**Issue**  
A tracked work item (feature/bug/chore/docs/test) in GitHub.

---

## J

**Join/Leave Between Hands**  
Table rule: players can only enter or leave a table at breaks between hands, not mid-hand.

---

## L

**Label**  
A GitHub tag applied to issues/PRs for categorization (e.g., `bug`, `blocked`).

---

## M

**Marker / TA**  
The evaluator of the project demo (teaching assistant / marker).

**Marking Guide / Rubric**  
The structured scoring checklist used to award marks. Project 1 rubric focuses on server/gameplay/ranking/accounts.

---

## P

**Pass / Check**  
A betting action where a player does not bet additional chips when no bet is currently required to stay in.

**Persistent Storage**  
Saving accounts and stats so they remain after restarting the program (e.g., JSON file).

**Phase 1**  
Server-focused stage: implement Five-card Draw, betting, accounts, ranking, CLI demo.

**Phase 2**  
Client-focused stage: support all three variants and include GUI; requires IP-based client/server communication.

**PIN**  
A numeric/secret code used to authenticate access to an account.

**Player (a.k.a. Client)**  
A participant in the poker game (human or program representation). Proposal uses “client/player” interchangeably.

**Pot**  
The total chips wagered in a hand, awarded to the winner(s).

**PR (Pull Request)**  
A GitHub change proposal used for code review and merging. In your workflow, a PR should close an issue (e.g., `Closes #12`).

**PR Closure / Track Task Closure**  
Linking PRs to issues so completion is visible (often required/encouraged by marking guide for project management marks).

**PRs (Performance Requirements)**  
In the proposal, PR/PRS refers to “Performance Requirement” (not pull request). Context matters:
- **PR (Pull Request)**: GitHub change request  
- **PR (Performance Requirement)**: proposal requirement category

---

## R

**Raise**  
Increasing the current bet during a betting round.

**README**  
Project documentation file explaining how to build/run and how to demo.

**Revision Control**  
Tracking changes to code/artifacts over time (Git).

**Round (Betting Round / Hand Round)**  
Often used informally; in this project, “round” may refer to a full hand or a betting round depending on context.

---

## S

**S-FR / C-FR (Server/Client Functional Requirement)**  
Proposal-coded requirement IDs for server/client functional requirements (e.g., S-FR-1).

**S-PR / C-PR (Server/Client Performance Requirement)**  
Proposal-coded performance requirements (e.g., responsiveness, resource setup, etc.).

**Seating**  
The ordering of players at a table, used to compute action order and dealer rotation.

**Server (a.k.a. House)**  
The program that manages tables, accounts, betting flow, and hand ranking. Project 1 focuses here.

**Showdown**  
The phase where remaining players reveal hands and the winner is determined via ranking.

**Sized Task / Size Field**  
A rough estimate of effort (story points) used for planning and to show work decomposition.

**Split Pot**  
When multiple players tie, the pot is divided among them.

**State Machine (Game State Machine)**  
A structured representation of hand phases (waiting → deal → bet1 → draw → bet2 → showdown → payout), enforcing valid transitions.

**Stats (Player Statistics)**  
Recorded player performance: games played, pots won, folds, etc.

**Stud Poker / Seven-card Stud**  
A poker variant required later (Phase 2) where players receive a mix of face-up/face-down cards over multiple betting rounds.

---

## T

**Table**  
A game instance where 2–5 players sit and play hands together under house rules.

**Table Information**  
Details shown to players (dealer, pot, player list, etc.). More emphasized in Phase 2 client requirements.

**Task (Project Tracking Task)**  
A unit of work represented as a GitHub issue (feature/bug/chore/docs/test).

**TCP (Transmission Control Protocol)**  
A reliable IP-based transport option allowed for Phase 2 inter-node interface.

**Texas Hold’em**  
A poker variant required later (Phase 2) where players use community cards + private hole cards.

**Tie-breaker (First-level)**  
A comparison rule to decide who wins when players have the same hand category (e.g., higher pair). If still tied after first level, split pot.

---

## U

**UDP (User Datagram Protocol)**  
A connectionless IP-based transport option allowed for Phase 2 inter-node interface.

**UML (Unified Modeling Language)**  
A diagramming standard mentioned in proposal references; sometimes used for design artifacts.

**Unit Tests**  
Automated tests that verify correctness of small components; rubric emphasizes tests for the ranking engine.

---

## V

**Viewer**  
A non-playing observer that can request results (mentioned in client-side performance requirements).

**Vertical Slice**  
A development approach where you build a thin end-to-end path (e.g., one full hand) early, then improve correctness/polish.

---

## W

**Workspace Members**  
The crates included in the top-level Rust workspace (e.g., `client`, `server`, `common`).

---

## Y

**“Two-hand Demo Scenario”**  
The marking-guide-required demonstration flow: 3 players enter, access accounts/chips, play two hands of Draw Poker with required behaviors (betting, passing, raising, folding, stats/balances updated).

---
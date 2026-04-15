# PokerDUD - ECE 421 Poker Playing System

PokerDUD is a multiplayer poker platform written entirely in Rust. It ships a **game server** that
enforces all poker rules and persists player accounts, plus a **client** that connects to the server
and can run as either a text-mode CLI or a desktop GUI built with [egui](https://github.com/emilk/egui).
Shared types, deck logic, and hand-evaluation live in a third `common` crate that both sides
depend on.

Supported game variants:

| Variant | Deal | Betting |
|---------|------|---------|
| **Five-Card Draw** | 5 private cards + draw phase | 2 rounds |
| **Texas Hold'em** | 2 hole cards + 5 community cards | 4 rounds (pre-flop → river) |
| **Seven-Card Stud** | 3-street private + 4 face-up streets | 5 rounds |

Communication between server and client uses a **newline-delimited JSON** protocol over TCP
(default port **8080**).

---

## Quick Start

> **Prerequisite:** [Rust toolchain](https://rustup.rs) (edition 2024 / Rust ≥ 1.85).

### Option A — play locally in one terminal (no network)

```bash
cargo run -p poker
```

This launches the single-process CLI. Create accounts, join the table, and play:

```text
acct create alice 1234 200
acct create bob   5678 200
variant five
join alice 1234
join bob   5678
start
bet alice 10
call bob
draw alice 0 3
draw bob
check alice
check bob
```

Type `help` at any time to see available commands.

### Option B — networked play with the desktop GUI (two terminals)

**Terminal 1 — start the server:**

```bash
cargo run -p poker -- serve
```

**Terminal 2 — start the GUI client:**

```bash
cargo run -p poker-gui -- --gui
```

The GUI will open a Connect screen. Leave the default `127.0.0.1:8080` and click **Connect**,
then create an account and log in.

### Option C — networked play with the text CLI client (two terminals)

**Terminal 1 — start the server:**

```bash
cargo run -p poker -- serve
```

**Terminal 2 — start the text client:**

```bash
cargo run -p poker-gui
```

Then type commands such as:

```text
create_account alice 1234 200
login alice 1234
list_tables
join_table 1
subscribe_table 1
submit_action 1 check
```

---

## Workspace Crates

| Directory | Cargo Package | Description |
|-----------|---------------|-------------|
| `common` | `common` | Shared cards, deck logic, hand ranking, protocol types, wire messages, and constants |
| `server` | `poker` | Authoritative game engine, account persistence, table management, dealer logic, local CLI, and TCP server |
| `client` | `poker-gui` | Reusable TCP client library, text-mode TCP client, desktop GUI, and UI assets |

## Dependencies

| Crate | Used In | Purpose |
|-------|---------|---------|
| `serde` / `serde_json` | common, server, client | Shared serialization for accounts, protocol data, and wire messages |
| `rand` | common, server | Deck shuffling and deterministic seeded runs |
| `itertools` | common | Combinatorics for hand evaluation |
| `eframe` / `egui` / `egui_extras` | client | Desktop GUI and asset-backed UI rendering |
| `image` | client | PNG card and chip asset loading |
| `tempfile` | server (dev) | Temporary directories for persistence tests |

## Build and Test

From the workspace root:

```bash
cargo build
cargo test
```

## Running the Project

### 1. Local server CLI

Runs the original single-process CLI directly inside the `server` crate:

```bash
cargo run -p poker
# or
cargo run -p poker -- cli
```

### 2. TCP server

Runs the networked poker server. By default it listens on `0.0.0.0:8080`.

```bash
cargo run -p poker -- serve
```

Account data is persisted to `server/data/accounts.json`.
If you run a packaged `poker.exe` outside the repo, it falls back to `data/accounts.json`
next to the executable. You can also override the location with `POKER_ACCOUNTS_PATH`.

### 3. TCP CLI client

Runs the `client` crate in text mode and connects to `127.0.0.1:8080` by default:

```bash
cargo run -p poker-gui
```

You can also pass the server address explicitly:

```bash
cargo run -p poker-gui -- 127.0.0.1:8080
# or
cargo run -p poker-gui -- --host 127.0.0.1 --port 8080
```

### 4. Desktop GUI client

Runs the `egui` desktop client:

```bash
cargo run -p poker-gui -- --gui
```

The GUI defaults to `127.0.0.1:8080` and includes connect, login, lobby, and table screens.

## Sharing Windows Builds (.exe)

Your friends do not need Cargo or Rust installed. Build the release binaries on your machine:

```bash
cargo build --release -p poker -p poker-gui
```

Then share these files from `target/release`:

- `poker.exe` — the server
- `poker-gui.exe` — the client

Typical setup for a friend:

1. Put both executables in the same folder.
2. Start `poker.exe serve`.
3. Start `poker-gui.exe --gui`.

If the client should connect to a server on another machine, launch it with that address:

```bash
poker-gui.exe --gui --host <server-ip> --port 8080
```

### 5. GUI preview screens

Debug builds also support previewing individual GUI screens without connecting to a live server:

```bash
cargo run -p poker-gui -- --gui-preview connect
cargo run -p poker-gui -- --gui-preview login
cargo run -p poker-gui -- --gui-preview lobby
cargo run -p poker-gui -- --gui-preview table holdem
cargo run -p poker-gui -- --gui-preview rankings
```

## Local Server CLI Demo

The original local CLI is still useful for quick gameplay demos. Example flow for Five-Card Draw:

1. Create accounts and join the table:

```text
acct create alice 1234 200
acct create bob 5678 200
acct create carol 0000 200
variant five
join alice 1234
join bob 5678
join carol 0000
```

2. Start a hand:

```text
start
```

3. Follow the first betting round prompts:

```text
bet alice 10
call bob
fold carol
```

4. Run the draw phase:

```text
draw alice 0 3
draw bob
```

5. Finish the hand:

```text
check alice
check bob
```

6. Inspect the updated state:

```text
acct get alice
acct get bob
lobbystate
```

## TCP Client Command Examples

When using the networked client, the command set is request-based instead of the local server CLI:

```text
create_account alice 1234 200
login alice 1234
list_tables
create_table holdem
join_table 1
subscribe_table 1
get_table_state 1
get_player_view 1
submit_action 1 check
```

The TCP client also supports:

```text
logout
deposit <amount>
withdraw <amount>
leave_table <table_id>
get_history <table_id>
unsubscribe_table <table_id>
submit_action <table_id> call
submit_action <table_id> fold
submit_action <table_id> bet <amount>
submit_action <table_id> raise <amount>
submit_action <table_id> draw 0 1 2
quit
```

## Other Local Server CLI Commands

```text
leave <name>
sitin <name>
sitout <name>
variant five|holdem|stud
seed <u64> | seed clear
lobbystate
handstate
view state
view hand <name>
acct get <name>
acct deposit <name> <amt>
acct withdraw <name> <amt>
acct delete <name> <pin>
help
quit
```

## Documentation

- Five-Card Draw house rules: [docs/rules.md](docs/rules.md)
- GUI client implementation notes: [docs/gui_client_phases.md](docs/gui_client_phases.md)

The `docs/rules.md` document is specifically for the Five-Card Draw rule set. Variant-specific gameplay behavior is implemented in the server variant modules under `server/src/variants/`.


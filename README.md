# PokerDUD - ECE 421 Poker Playing System

PokerDUD is a Rust workspace for the ECE 421 poker project. The project now includes both the `server` crate and the `client` crate: the server can run as a local CLI demo or as a TCP game server, and the client can run as either a TCP CLI or an `egui` desktop GUI. Shared cards, protocol, and hand-evaluation code live in `common`.

The codebase currently supports:

- Five-Card Draw
- Texas Hold'em
- Seven-Card Stud

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

Requires the Rust toolchain (edition 2024). From the workspace root:

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

### 5. GUI preview screens

Debug builds also support previewing individual GUI screens without connecting to a live server:

```bash
cargo run -p poker-gui -- --gui-preview connect
cargo run -p poker-gui -- --gui-preview login
cargo run -p poker-gui -- --gui-preview lobby
cargo run -p poker-gui -- --gui-preview table holdem
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

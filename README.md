# PokerDUD — ECE 421 Poker Playing System

A Five-Card Draw poker server implemented in Rust. Phase 1 focuses on the server: managing players, gameplay, hand ranking, and account persistence via a CLI interface.

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `common` | Shared types: cards, deck, hand representation, hand ranking engine, game constants, protocol types |
| `server` | Game engine, accounts, table management, dealer logic, and CLI entry point |
| `client` | Placeholder for Phase 2 (GUI + networking) |

## Dependencies

| Crate | Used In | Purpose |
|-------|---------|---------|
| `serde` / `serde_json` | server | Account serialization and JSON persistence |
| `rand` | common, server | Deck shuffling (deterministic via seed or random) |
| `itertools` | common | Combinatorics for 7-card best-hand evaluation |
| `tempfile` | server (dev) | Temporary directories for persistence tests |

## Build and Run

Requires Rust toolchain (edition 2024). From the workspace root:

```bash
# Build
cargo build

# Run all tests
cargo test

# Run the CLI
cargo run -p poker
```

## How to Play (Demo)

1. **Create accounts and join the table:**
```
acct create alice 1234 200
acct create bob 5678 200
acct create carol 0000 200
join alice 1234
join bob 5678
join carol 0000
```

2. **Start a hand** (ante is collected, cards are dealt automatically):
```
start
```

3. **Betting Round 1** — follow the action prompts:
```
bet alice 10
call bob
fold carol
```

4. **Draw Phase** — discard cards by index (0-4), or pass no indices to keep all:
```
draw alice 0 3
draw bob
```

5. **Betting Round 2:**
```
check alice
check bob
```

6. **Showdown and payout happen automatically.** Check updated stats:
```
acct get alice
acct get bob
lobbystate
```

7. **Play a second hand** (dealer rotates, a player can opt out):
```
sitout carol
start
```

## Other CLI Commands

```
leave <name>              Leave the table (between hands)
sitin <name>              Cancel sit-out
seed <u64> | seed clear   Set/clear deterministic shuffle
lobbystate                Show seated players and balances
acct get <name>           View account info and stats
acct deposit <name> <amt> Deposit chips
acct withdraw <name> <amt> Withdraw chips
help                      List all commands
quit                      Exit
```

## Persistence

Account data (balances, stats) is saved to a JSON file and survives restarts. The data directory is created automatically on first run.

## House Rules

See [rules.md](rules.md) for the full Five-Card Draw house rules. Numeric constants (ante, bet limits, raise caps) are defined in `common/src/constants.rs`.

# Phase 2 Server and Client Implementation Plan

This document is the working plan for completing the Phase 2 distributed server/client system.

It is intentionally contract-first. The first goal is to freeze the shared communication interface and public/private data model so the server work and client/GUI work can happen in parallel without blocking each other.

This plan is based on the current project structure and the deployment guidance in [deployment.md](deployment.md).

For the focused GUI-client execution order after the TCP CLI became functional, see [gui_client_phases.md](gui_client_phases.md).

## Summary

We will build Phase 2 around a LAN TCP + JSON architecture, not HTTP and not a public internet deployment.

The internal game core should remain:

- `GameEngine` as the per-table game state machine
- `TableRuntime` as `engine + history`
- `ServerApp` as the top-level service boundary

The transport layer will call `ServerApp`, not `GameEngine` directly.

The first implementation milestone is a working distributed demo over local LAN with:

- a TCP server bound to `0.0.0.0:<port>`
- a shared request/response JSON protocol
- session-based identity
- table-based gameplay
- public table state and private player views
- local JSON persistence for accounts

## Shared Contract First

Before implementation starts, the following shared decisions must be treated as fixed for both server and client work.

### Transport

- Persistent TCP connection
- UTF-8 newline-delimited JSON
- One request object per line
- One response object per line
- Default server bind: `0.0.0.0:8080`
- Clients connect to `<server-lan-ip>:<port>`

### Session Model

- `login` returns a `session_id`
- All authenticated player operations use `session_id`
- Sessions are in-memory only
- Sessions do not survive server restart
- Sessions are not tied to a single socket in v1

### Table Model

- Every table has a `table_id`
- Clients never target a game action without a `table_id`
- The server starts with one default table
- The API also supports creating more tables

### Public vs Private State

- `get_table_state(table_id)` returns public information only
- `get_player_view(session_id, table_id)` returns private information only for that player
- Spectator access uses public table state only
- No client-facing endpoint may expose another player's hole cards

### Update Model

- v1 uses request/response plus polling
- No server-push subscription protocol in the first milestone
- Clients poll `get_table_state`, `get_player_view`, and `get_history`

### Shared Artifact

The first documentation artifact to write and freeze during implementation is:

- `docs/phase2_wire_contract.md`

That document should contain:

- all request types
- all response types
- public table state shape
- private player view shape
- error shape
- at least one full example flow from login through a completed hand

## Shared Public API and Types

The current `common/src/protocol.rs` is still internal and CLI-oriented. We should not expose it directly over the network.

Instead, we will add a separate wire-facing module in `common`.

### Shared Types to Add

- `SessionId = String`
- `TableId = u64`
- `ClientRequest`
- `ServerResponse`
- `RequestOp`
- `ResponsePayload`
- `ErrorPayload`
- `PublicTableState`
- `PlayerView`
- `TableSummary`
- `HistoryEntry`
- `PlayerAction`

### Initial Wire Operations

- `create_account`
- `login`
- `deposit`
- `withdraw`
- `list_tables`
- `create_table`
- `join_table`
- `leave_table`
- `start_hand`
- `get_table_state`
- `get_player_view`
- `submit_action`
- `get_history`

### Shared Wire Decisions

- `submit_action` uses `session_id + table_id + action`
- Actions are not sent with raw player names
- `create_table` takes a poker variant
- Responses always include `request_id`
- Responses return either `payload` or structured `error`

### Data Reuse

Where useful, the wire layer should reuse shared enums already in `common`, especially:

- poker variant
- hand step
- allowed actions
- cards, suits, and ranks

This means `common` will need serde support for the shared domain types used by the wire API.

## Server Implementation Checklist

### 1. Keep the game core internal

- Keep `GameEngine` as the per-table engine
- Keep `TableRuntime` as the owner of `GameEngine` and in-memory history
- Keep `ServerApp` as the top-level service boundary
- Do not let TCP handlers talk to the engine directly

### 2. Expand `ServerApp` into the real service API

`ServerApp` should own:

- account store
- session store
- table registry

`ServerApp` should expose methods matching the shared wire operations.

### 3. Add session handling

Create a session layer that:

- authenticates with account name and PIN
- creates `session_id`
- resolves session to account/player identity
- rejects invalid or expired sessions

This layer should be used for:

- joining tables
- leaving tables
- requesting private player state
- submitting actions

### 4. Add table-aware state builders

Add builders that convert internal runtime state into:

- public table state
- private player view
- table history

This is where public/private card visibility must be enforced.

### 5. Add TCP transport

Add a TCP server module that:

- listens on a configurable host/port
- parses newline-delimited JSON requests
- calls `ServerApp`
- serializes JSON responses

The server should stay simple:

- standard Rust networking is acceptable
- no external DB
- no system-level dependencies

### 6. Split CLI from transport

The current `server/src/main.rs` is still a CLI adapter.

During implementation:

- keep the CLI path for debugging
- move parsing/printing logic into a CLI-specific module if needed
- add a separate TCP server entrypoint or binary for deployment

### 7. Keep multi-table support incremental

The server API should define multi-table behavior immediately.

Implementation order should be:

- boot with one default table
- support `list_tables`
- support `create_table`
- support `join_table` by `table_id`

This keeps the API future-proof without forcing a rewrite later.

## Client and GUI Parallel Checklist

These are the items the client/GUI team can work on as soon as the wire contract is frozen.

- TCP client wrapper for sending and receiving JSON messages
- login and account management screens/flows
- table browser based on `list_tables`
- join/leave/start-hand flows
- public table rendering from `PublicTableState`
- private player rendering from `PlayerView`
- action controls from `allowed_actions`
- polling loop for refreshing state during active play
- spectator mode from public-only table state

The client should treat:

- `PublicTableState` as the source of all shared table information
- `PlayerView` as the source of private cards and private prompts

## Testing and Demo Checklist

### Contract Tests

- serialization round-trip tests for all wire request/response types
- sample JSON fixtures checked against the frozen wire contract

### Server Tests

- request-to-service mapping tests in `ServerApp`
- session validation tests
- table state visibility tests
- private view authorization tests
- multi-client consistency tests

### Transport Tests

- one TCP client can connect and perform account + table flow
- two TCP clients can join the same table and complete a hand
- spectator client sees public state but no private cards

### Demo Acceptance

The first milestone is complete when:

- the server runs on a LAN-visible port
- two remote clients can log in and connect over LAN IP
- they can join a table and complete a hand remotely
- account persistence still works through local JSON storage
- the same server API supports both CLI and GUI clients

## Assumptions and Defaults

- TCP + JSON is the required transport for v1
- Request/response polling is enough for the first milestone
- No HTTP layer is needed
- Default port is `8080`
- One default table exists at startup
- Sessions are in-memory only
- Account persistence remains JSON-file based

## Important Follow-Up After the Shared Contract

After the contract and shared API/types are ready, the next focused task will be:

- prepare the exact shared contract document
- freeze public API names and JSON field names
- freeze public/private state shapes
- then begin implementation on the server and client sides in parallel

One important refactor that should happen after TCP v1 is stable:

- separate casino bankroll from at-table chips / buy-ins

That is important for full multi-table correctness, but it should not block the first shared-contract milestone.

# GUI Client Phases Plan

This document is the focused implementation plan for the GUI client work now that the TCP CLI client is functional.

It is intentionally staged so we can complete one stable layer at a time:

1. extract and stabilize the reusable transport
2. validate that the CLI still works on top of the refactor
3. build the GUI communication/state foundation
4. build the actual egui screens on top of that foundation

## Current Baseline

Today the project already has:

- shared wire/protocol types in `common/`
- a working TCP server transport in `server/src/transport/tcp.rs`
- a working TCP CLI client in `client/src/main.rs`
- card/chip/UI assets already checked into `client/assets/`

The biggest limitation is that the current client transport, CLI parsing, and CLI rendering all live inside one file. That is fine for the CLI, but not for a GUI.

## Guiding Decisions

- Keep the existing TCP + newline-delimited JSON protocol.
- Reuse the same shared `common` types for both CLI and GUI.
- Design the reusable client transport for GUI needs first.
- Keep the GUI non-blocking; no egui frame should wait on network I/O.
- Keep the CLI working by adapting it onto the reusable transport layer.
- Prefer one selected active table at a time for the first GUI milestone.

## Phase 1: Extract Reusable Transport

### Goal

Move the transport and session logic out of `client/src/main.rs` into a reusable module that both the CLI and future GUI can use.

### Planned Structure

- `client/src/lib.rs`
- `client/src/net/mod.rs`
- `client/src/net/tcp_client.rs`

Optional follow-up split if needed:

- `client/src/cli.rs` for parsing/formatting helpers

### Responsibilities To Move

- TCP connect/disconnect
- request id generation
- JSON line encode/decode
- reader thread setup
- pending response tracking
- session capture on login/logout
- routing of `Response`, `TableUpdate`, and `SessionClosed`

### Public API Target

The reusable transport layer should expose:

- a client handle for connect/send/disconnect
- a non-blocking event stream or polling API
- current session lookup
- request ids so the frontend can correlate responses

The CLI-specific layer should keep:

- command parsing
- help text
- user-friendly rendering
- prompt loop

### Acceptance Criteria

- The CLI still supports the current account/table/game flows.
- No wire format changes are introduced.
- Transport code is no longer embedded directly in the CLI entrypoint.
- Push updates and session closed messages still work after the refactor.

## Phase 2: Client Crate Reshape

### Goal

Separate transport, state, and frontend concerns so the GUI can be added without reworking the transport again.

### Target Layers

- `net`: socket + protocol I/O
- `state`: frontend-facing event/state helpers
- `cli`: current text frontend
- `gui`: future egui frontend

### New Shared Concepts

- `ClientEvent`
  - `Connected`
  - `Disconnected`
  - `Response(ServerResponse)`
  - `TableUpdate(TableUpdateMessage)`
  - `SessionClosed(SessionClosedMessage)`
  - `TransportError(String)`
- `ClientHandle`
  - send requests
  - poll/receive events
  - inspect session state

### Acceptance Criteria

- The CLI uses the same shared client library as the future GUI will use.
- Frontend code no longer depends on `TcpStream` directly.
- The crate layout clearly separates transport from presentation.

## Phase 3: GUI Communication and State Foundation

### Goal

Build the non-blocking egui-side app state and event handling on top of the extracted transport.

### GUI State Shape

- connection status
- current account/session
- table list
- selected table id
- latest public table state
- latest player view
- history/event feed
- pending request markers
- last server/transport error

### Communication Model

- Run the TCP client core on a background worker thread.
- Send `RequestOp` from the GUI to the worker through channels.
- Receive `ClientEvent` from the worker back into the GUI through channels.
- Poll all available events each egui frame without blocking.
- Update app state from events, then render from app state.

### Acceptance Criteria

- The GUI app can connect and stay responsive while network activity happens.
- Incoming table updates can update state without freezing the UI.
- Request/response correlation works for login and actions.

## Phase 4: First GUI Vertical Slice

### Goal

Get one usable end-to-end GUI flow working over the same transport.

### Planned Screens

- connect/login
- lobby
- selected table
- action bar
- event/history panel

### First Supported Flows

- connect to server
- create account / login
- list tables
- create table / join table
- subscribe to table
- display public table state + player view
- submit actions from the action prompt

### Acceptance Criteria

- One player can connect from the GUI and play through a hand.
- The GUI updates from server push messages.
- Showdown and payout information appear in the GUI event feed.

## Working Order

We should execute these phases in this exact order:

1. finish Phase 1 and confirm the CLI still works
2. finish Phase 2 and keep the transport/frontend boundary clean
3. implement Phase 3 before drawing serious GUI screens
4. implement Phase 4 as the first visible GUI milestone

We should not start serious egui screen work before Phase 1 and Phase 2 are stable.

## Validation Checklist

- `cargo test` continues to pass after each phase
- the CLI remains usable after the transport extraction
- no protocol regressions are introduced between client and server
- GUI state is driven by server messages, not duplicated local game logic

## Immediate Next Step

Start Phase 1 by extracting the current transport from `client/src/main.rs` into a reusable library module, then verify the CLI still behaves the same before starting Phase 2.

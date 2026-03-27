# Phase 2 Wire Contract

This document describes the shared server/client contract currently implemented in `common::wire`.

It is the reference the server and client/GUI teams should use before full TCP transport work begins.

## Transport

- TCP connection
- UTF-8 newline-delimited JSON
- One request object per line
- One response object per line
- Default server bind target for Phase 2: `0.0.0.0:8080`

## Envelope Types

### Client Request

```json
{
  "request_id": "req-1",
  "session_id": "session-1-1",
  "op": {
    "type": "get_table_state",
    "table_id": 1
  }
}
```

Fields:

- `request_id`: client-generated request identifier
- `session_id`: optional session token, required for authenticated operations
- `op`: request operation payload

### Server Response

Success:

```json
{
  "request_id": "req-1",
  "ok": true,
  "payload": {
    "type": "table_state",
    "table_id": 1,
    "variant": "five_card_draw",
    "step": "waiting_for_players",
    "dealer_seat": 0,
    "acting_player_id": null,
    "pot": 0,
    "current_bet": 0,
    "board_cards": [],
    "seats": []
  },
  "error": null
}
```

Failure:

```json
{
  "request_id": "req-2",
  "ok": false,
  "payload": null,
  "error": {
    "code": "table_not_found",
    "message": "Table 99 was not found"
  }
}
```

## Request Operations

Currently defined request operation types:

- `create_account`
- `login`
- `logout`
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

## Authentication Rules

Operations that should be treated as authenticated:

- `logout`
- `deposit`
- `withdraw`
- `create_table`
- `join_table`
- `leave_table`
- `start_hand`
- `get_player_view`
- `submit_action`

Operations that may be public:

- `create_account`
- `login`
- `list_tables`
- `get_table_state`
- `get_history`

## Shared Response Payloads

### Account

Used by account creation, deposit, and withdraw.

```json
{
  "type": "account",
  "player_id": 1,
  "player_name": "alice",
  "balance": 500,
  "games_played": 0,
  "pots_won": 0,
  "folds": 0
}
```

### Login

```json
{
  "type": "login",
  "session_id": "session-1-1",
  "account": {
    "player_id": 1,
    "player_name": "alice",
    "balance": 500,
    "games_played": 0,
    "pots_won": 0,
    "folds": 0
  }
}
```

### Logout

```json
{
  "type": "logout",
  "session_id": "session-1-1"
}
```

### Table Summary

Used by `list_tables` and `create_table`.

```json
{
  "type": "table",
  "table_id": 1,
  "variant": "texas_hold_em",
  "step": "waiting_for_players",
  "seated_count": 0,
  "max_players": 10
}
```

### Public Table State

`get_table_state` returns only public information.

Each seat includes:

- `seat`
- `player_id`
- `player_name`
- `balance`
- `is_in_hand`
- `has_folded`
- `hidden_card_count`
- `up_cards`

The top-level table state includes:

- `table_id`
- `variant`
- `step`
- `dealer_seat`
- `acting_player_id`
- `pot`
- `current_bet`
- `board_cards`
- `seats`

### Player View

`get_player_view` returns:

- `table_id`
- `player_id`
- `player_name`
- `seat`
- `private_cards`
- `up_cards`
- `board_cards`
- `action_prompt`

`action_prompt` contains:

- `allowed_actions`
- `call_amount`
- `min_raise`
- `max_discards`

### History

`get_history` returns sanitized public history entries:

- `index`
- `kind`
- `message`

Private hand reveal events are intentionally filtered out of public history.

## Player Actions

Currently defined action payloads:

- `check`
- `bet { amount }`
- `call`
- `raise { amount }`
- `fold`
- `draw { discard }`

Clients should send actions using:

- `session_id`
- `table_id`
- `action`

They should not send raw player names as the acting identity.

## Notes on the Current Server API

The current `ServerApp` API now mirrors this contract and provides methods for:

- account creation and login
- session-based deposit and withdraw
- listing and creating tables
- joining and leaving a table
- starting a hand
- reading public table state
- reading private player view
- submitting an action
- reading public history

This is a bridge layer for the upcoming TCP server. The current CLI still uses the older internal command flow, but the shared contract types are now available for both server and client work.

use std::fmt::{Write as _, format};
use std::io::{self, Write};
use std::sync::Arc;

use crate::net::TcpClient;
use crate::state::{ClientEvent, EventHandler};
use common::card_deck::Card;
use common::protocol::{AllowedAction, HandStep, PayoutLine, PokerVariant};
use common::wire::{
    AccountProfile, ActionPrompt, ErrorCode, HistoryEntry, HistoryKind, JoinTableResult,
    LeaveTableResult, LeaveTableStatus, OperationReceipt, PlayerAction, PlayerView,
    PublicSeatState, PublicTableState, RequestOp, ResponsePayload, SeatAssignmentStatus,
    ServerResponse, SessionCloseReason, SessionClosedMessage, SubscriptionResult, TableHistory,
    TableId, TableUpdateMessage,
};

pub fn run(address: &str) {
    let event_handler: EventHandler = Arc::new(handle_client_event);
    let mut client = match TcpClient::connect(address, Some(event_handler)) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to connect to {address}: {error}");
            return;
        }
    };

    println!("PokerDUD TCP client connected to {address}");
    println!("Type 'help' for commands.");

    loop {
        print_prompt();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("Error reading input.");
            continue;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line.to_ascii_lowercase().as_str(), "quit" | "exit") {
            break;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts[0].eq_ignore_ascii_case("help") {
            print_help();
            continue;
        }
        if parts[0].eq_ignore_ascii_case("session") {
            match client.session_id() {
                Some(session_id) => println!("session: {session_id}"),
                None => println!("session: <none>"),
            }
            continue;
        }

        let op = match parse_request_op(&parts) {
            Ok(op) => op,
            Err(error) => {
                println!("Parse error: {error}");
                continue;
            }
        };

        match client.send(op) {
            Ok(response) => print_response(&response),
            Err(error) => {
                println!("Request failed: {error}");
                break;
            }
        }
    }
}

fn handle_client_event(event: ClientEvent) {
    match event {
        ClientEvent::UnexpectedResponse(response) => {
            println!("\n[push] unexpected response:");
            print_response(&response);
            print_prompt();
        }
        ClientEvent::TableUpdate(update) => {
            print_table_update(&update);
        }
        ClientEvent::SessionClosed(message) => {
            print_session_closed(&message);
        }
        ClientEvent::TransportError(message) => {
            eprintln!("{message}");
        }
        ClientEvent::Disconnected => {
            eprintln!("server closed the connection");
        }
    }
}

fn print_table_update(update: &TableUpdateMessage) {
    println!("\n{}", format_table_update(update));
    print_prompt();
}

fn print_session_closed(message: &SessionClosedMessage) {
    println!("\n{}", format_session_closed(message));
    print_prompt();
}

fn print_prompt() {
    print!("client> ");
    let _ = io::stdout().flush();
}

fn parse_request_op(parts: &[&str]) -> Result<RequestOp, String> {
    match parts[0].to_ascii_lowercase().as_str() {
        "create_account" => Ok(RequestOp::CreateAccount {
            name: required_arg(parts, 1, "name")?.to_string(),
            pin: required_arg(parts, 2, "pin")?.to_string(),
            amount: parse_i64(parts.get(3), "amount")?,
        }),
        "login" => Ok(RequestOp::Login {
            name: required_arg(parts, 1, "name")?.to_string(),
            pin: required_arg(parts, 2, "pin")?.to_string(),
        }),
        "logout" => Ok(RequestOp::Logout),
        "deposit" => Ok(RequestOp::Deposit {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "withdraw" => Ok(RequestOp::Withdraw {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "list_tables" => Ok(RequestOp::ListTables),
        "create_table" => Ok(RequestOp::CreateTable {
            variant: parse_variant(required_arg(parts, 1, "variant")?)?,
        }),
        "join_table" => Ok(RequestOp::JoinTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "leave_table" => Ok(RequestOp::LeaveTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "start_hand" => Ok(RequestOp::StartHand {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_table_state" => Ok(RequestOp::GetTableState {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_player_view" => Ok(RequestOp::GetPlayerView {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_history" => Ok(RequestOp::GetHistory {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "subscribe_table" => Ok(RequestOp::SubscribeTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "unsubscribe_table" => Ok(RequestOp::UnsubscribeTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "submit_action" => {
            let action_parts = parts.get(2..).unwrap_or(&[]);
            Ok(RequestOp::SubmitAction {
                table_id: parse_u64(parts.get(1), "table_id")?,
                action: parse_action(action_parts)?,
            })
        }
        other => Err(format!("unknown command '{other}'")),
    }
}

fn parse_variant(value: &str) -> Result<PokerVariant, String> {
    match value.to_ascii_lowercase().as_str() {
        "five" | "five_card_draw" | "fivecarddraw" => Ok(PokerVariant::FiveCardDraw),
        "holdem" | "hold'em" | "texas_holdem" | "texasholdem" => Ok(PokerVariant::TexasHoldEm),
        "stud" | "seven_card_stud" | "sevencardstud" => Ok(PokerVariant::SevenCardStud),
        _ => Err("unknown variant (five|holdem|stud)".into()),
    }
}

fn parse_action(parts: &[&str]) -> Result<PlayerAction, String> {
    let action = parts
        .first()
        .ok_or("missing action (check|call|fold|bet|raise|draw)")?
        .to_ascii_lowercase();

    match action.as_str() {
        "check" => Ok(PlayerAction::Check),
        "call" => Ok(PlayerAction::Call),
        "fold" => Ok(PlayerAction::Fold),
        "bet" => Ok(PlayerAction::Bet {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "raise" => Ok(PlayerAction::Raise {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "draw" => {
            let discard = parts
                .iter()
                .skip(1)
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid discard index".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PlayerAction::Draw { discard })
        }
        _ => Err("unknown action (check|call|fold|bet|raise|draw)".into()),
    }
}

fn parse_i64(value: Option<&&str>, name: &str) -> Result<i64, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .parse::<i64>()
        .map_err(|_| format!("invalid {name}"))
}

fn parse_u64(value: Option<&&str>, name: &str) -> Result<TableId, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid {name}"))
}

fn required_arg<'a>(parts: &'a [&str], index: usize, name: &str) -> Result<&'a str, String> {
    parts
        .get(index)
        .copied()
        .ok_or_else(|| format!("missing {name}"))
}

fn print_response(response: &ServerResponse) {
    println!("{}", format_response(response));
}

fn format_response(response: &ServerResponse) -> String {
    if let Some(formatted) = format_response_friendly(response) {
        return formatted;
    }

    match serde_json::to_string_pretty(response) {
        Ok(json) => format!("Response\n{json}"),
        Err(error) => format!("failed to render response: {error}"),
    }
}

fn format_response_friendly(response: &ServerResponse) -> Option<String> {
    if response.ok {
        let payload = response.payload.as_ref()?;
        let mut output = String::from("Success");
        match payload {
            ResponsePayload::Account(account) => {
                output.push('\n');
                output.push_str(&format_account_profile(account));
            }
            ResponsePayload::Login(login) => {
                let _ = write!(output, "\nLogged in with session {}", login.session_id);
                output.push('\n');
                output.push_str(&format_account_profile(&login.account));
            }
            ResponsePayload::Logout(logout) => {
                let _ = write!(output, "\nLogged out of session {}", logout.session_id);
            }
            ResponsePayload::Tables(tables) => {
                let _ = write!(output, "\nTables ({})", tables.tables.len());
                if tables.tables.is_empty() {
                    output.push_str("\n  none");
                } else {
                    for table in &tables.tables {
                        let _ = write!(output, "\n  {}", format_table_summary_line(table));
                    }
                }
            }
            ResponsePayload::Table(table) => {
                output.push_str("\nCreated table");
                let _ = write!(output, "\n  {}", format_table_summary_line(table));
            }
            ResponsePayload::JoinTable(result) => {
                output.push('\n');
                output.push_str(&format_join_table_result(result));
            }
            ResponsePayload::LeaveTable(result) => {
                output.push('\n');
                output.push_str(&format_leave_table_result(result));
            }
            ResponsePayload::Receipt(receipt) => {
                output.push('\n');
                output.push_str(&format_receipt(receipt));
            }
            ResponsePayload::TableState(state) => {
                output.push('\n');
                output.push_str(&format_public_table_state(state));
            }
            ResponsePayload::PlayerView(view) => {
                output.push('\n');
                output.push_str(&format_player_view_compact(view));
            }
            ResponsePayload::History(history) => {
                output.push('\n');
                output.push_str(&format_history(history));
            }
            ResponsePayload::Subscription(subscription) => {
                output.push('\n');
                output.push_str(&format_subscription(subscription));
            }
        }
        return Some(output);
    }

    let error = response.error.as_ref()?;
    Some(format!(
        "Error [{}]: {}",
        format_error_code(error.code.clone()),
        error.message
    ))
}

fn format_table_update(update: &TableUpdateMessage) -> String {
    if let Some(formatted) = format_table_update_friendly(update) {
        return formatted;
    }

    match serde_json::to_string_pretty(update) {
        Ok(json) => format!("Table update\n{json}"),
        Err(error) => format!("failed to render table update: {error}"),
    }
}

fn format_table_update_friendly(update: &TableUpdateMessage) -> Option<String> {
    let mut output = String::new();
    let _ = write!(output, "=== Table {} Update ===", update.table_id);

    output.push('\n');
    output.push_str(&format_public_table_state(&update.table_state));

    output.push_str("\n");

    if !update.events.is_empty() {
        output.push_str("\nLast Events:");
        for event in &update.events {
            append_history_entry(&mut output, event, "  ");
        }
    }

    if let Some(player_view) = &update.player_view {
        output.push_str("\n\n");
        output.push_str(&format_player_view_compact(player_view));
    }

    Some(output)
}

fn format_session_closed(message: &SessionClosedMessage) -> String {
    format!(
        "[session closed] {} ({})",
        message.message,
        format_session_close_reason(message.reason.clone())
    )
}

fn format_account_profile(account: &AccountProfile) -> String {
    format!(
        "Account: {} (player #{})\nBalance: {}\nStats: Games: {} Pots Won: {} Folds: {}",
        account.player_name,
        account.player_id,
        account.balance,
        account.games_played,
        account.pots_won,
        account.folds
    )
}

fn format_table_summary_line(table: &common::wire::TableSummary) -> String {
    format!(
        "Table {} | {} | {} | seats {}/{}",
        table.table_id,
        format_variant(table.variant),
        format_step(table.step),
        table.seated_count,
        table.max_players
    )
}

fn format_join_table_result(result: &JoinTableResult) -> String {
    match result.status {
        SeatAssignmentStatus::Seated => match result.seat {
            Some(seat) => format!("Joined table {} at seat {}", result.table_id, seat),
            None => format!("Joined table {}", result.table_id),
        },
        SeatAssignmentStatus::Queued => format!("Queued for table {}", result.table_id),
    }
}

fn format_leave_table_result(result: &LeaveTableResult) -> String {
    match result.status {
        LeaveTableStatus::Left => format!("Left table {}", result.table_id),
        LeaveTableStatus::LeavingAfterHand => {
            format!(
                "Will leave table {} after the current hand",
                result.table_id
            )
        }
    }
}

fn format_receipt(receipt: &OperationReceipt) -> String {
    format!(
        "Table {} updated ({} event{})",
        receipt.table_id,
        receipt.event_count,
        plural_suffix(receipt.event_count)
    )
}

fn format_public_table_state(state: &PublicTableState) -> String {
    let mut output = String::new();
    let mut acter = None;
    for seat in &state.seats {
        if state.acting_player_id == Some(seat.player_id) {
            acter.replace(format!("{}", seat.player_name));
            break;
        }
    }
    let _ = write!(
        output,
        "{} | Pot: {} | Bet: {} | To act: {}\n",
        format_step(state.step),
        state.pot,
        state.current_bet,
        acter.as_ref().map(|s| s.as_str()).unwrap_or("none"),
    );
    if !state.board_cards.is_empty() {
        let _ = write!(output, "\nBoard: {}", format_cards(&state.board_cards));
    } 
    output.push_str("\nSeats:");
    if state.seats.is_empty() {
        // output.push_str("\n  none");
    } else {
        for seat in &state.seats {
            let _ = write!(output, "\n  {}", format_seat_line_compact(seat, state));
        }
    }
    output
}

fn format_player_view_full(view: &PlayerView) -> String {
    let mut output = String::new();
    let _ = write!(
        output,
        "Player view for {} (player #{})",
        view.player_name, view.player_id
    );
    let _ = write!(output, "\nTable: {}", view.table_id);
    let _ = write!(output, "\nSeat: {}", format_optional_seat(view.seat));
    let _ = write!(
        output,
        "\nPrivate cards: {}",
        format_cards(&view.private_cards)
    );
    let _ = write!(output, "\nUp cards: {}", format_cards(&view.up_cards));
    let _ = write!(output, "\nBoard: {}", format_cards(&view.board_cards));
    match &view.action_prompt {
        Some(prompt) => {
            output.push('\n');
            output.push_str(&format_action_prompt(prompt));
        }
        None => output.push_str("\nAction: waiting"),
    }
    output
}

fn format_player_view_compact(view: &PlayerView) -> String {
    let mut output = String::new();
    let _ = write!(
        output,
        "You: {}",
        view.player_name
    );
    let _ = write!(
        output,
        "\nHand: {}",
        format_cards(&view.private_cards)
    );
    match &view.action_prompt {
        Some(prompt) => {
            output.push('\n');
            output.push_str(&format_action_prompt(prompt));
        }
        None => output.push_str("\nWaiting for turn...\n"),
    }
    output
}

fn format_action_prompt(prompt: &ActionPrompt) -> String {
    let mut output = String::new();
    
    if format_allowed_actions(&prompt.allowed_actions) == "n/a"{
        output.push_str("Options: None\n");
    } else{
        output.push_str(&format!("Options: {} \n", format_allowed_actions(&prompt.allowed_actions)));
    }

    if format_optional_chips(prompt.call_amount) != "n/a"{
        output.push_str(&format!("Call: {} \n", format_optional_chips(prompt.call_amount)));
    }

    if format_optional_chips(prompt.min_raise) != "n/a"{
        output.push_str(&format!("Min Raise: {} \n", format_optional_chips(prompt.min_raise)));
    }

    if format_optional_usize(prompt.max_discards) != "n/a"{
        output.push_str(&format!("Max Number of Discards: {} \n", format_optional_usize(prompt.max_discards)));
    }

    output
}

fn format_history(history: &TableHistory) -> String {
    let mut output = String::new();
    let _ = write!(output, "History for table {}", history.table_id);
    if history.entries.is_empty() {
        output.push_str("\n  none");
    } else {
        for entry in &history.entries {
            append_history_entry(&mut output, entry, "  ");
        }
    }
    output
}

fn format_subscription(subscription: &SubscriptionResult) -> String {
    if subscription.subscribed {
        format!("Subscribed to table {}", subscription.table_id)
    } else {
        format!("Unsubscribed from table {}", subscription.table_id)
    }
}

fn format_history_entry(entry: &HistoryEntry) -> String {
    let mut output = format!("- {}", entry.message);
    if entry.kind == HistoryKind::Showdown
        && let Some(results) = &entry.showdown_results
        && !results.is_empty()
    {
        output.push_str("\n  Showdown:");
        for result in results {
            let _ = write!(
                output,
                "\n    {} -> {}",
                result.player_name, result.hand_desc
            );
        }
    }
    if entry.kind == HistoryKind::Payout
        && let Some(results) = &entry.payout_results
        && !results.is_empty()
    {
        output.push_str("\n  Payout:");
        for result in results {
            let _ = write!(output, "\n    {}", format_payout_line(result));
        }
    }
    output
}

fn append_history_entry(output: &mut String, entry: &HistoryEntry, indent: &str) {
    let formatted = format_history_entry(entry);
    for line in formatted.lines() {
        output.push('\n');
        output.push_str(indent);
        output.push_str(line);
    }
}

fn format_payout_line(result: &PayoutLine) -> String {
    format!(
        "{} +{}",
        result.player_name, result.amount
    )
}

fn format_seat_line_full(seat: &PublicSeatState, state: &PublicTableState) -> String {
    let mut markers = Vec::new();
    if seat.seat == state.dealer_seat {
        markers.push("Dealer");
    }
    if state.acting_player_id == Some(seat.player_id) {
        markers.push("Acting");
    }
    if !seat.is_connected {
        markers.push("Disconnected");
    }
    if seat.has_folded {
        markers.push("Folded");
    } else if seat.is_in_hand {
        markers.push("In Hand");
    } else {
        markers.push("Waiting");
    }

    let tags = if markers.is_empty() {
        String::new()
    } else {
        format!(" {}", markers.join(", "))
    };

    format!(
        "Seat {}: {} {}, balance {}",
        seat.seat,
        seat.player_name,
        tags,
        seat.balance,
    )
}

fn format_seat_line_compact(seat: &PublicSeatState, state: &PublicTableState) -> String {
    let mut markers = Vec::new();
    if seat.seat == state.dealer_seat {
        markers.push("Dealer");
    }
    if state.acting_player_id == Some(seat.player_id) {
        markers.push("Acting");
    }
    if !seat.is_connected {
        markers.push("Disconnected");
    }
    if seat.has_folded {
        markers.push("Folded");
    } else if seat.is_in_hand {
        markers.push("In Hand");
    } else {
        markers.push("Waiting");
    }

    let tags = if markers.is_empty() {
        String::new()
    } else {
        format!(" {}", markers.join(", "))
    };

    format!(
        "Seat {}: {},{}, Balance {}",
        seat.seat,
        seat.player_name,
        tags,
        seat.balance,
    )
}

fn format_cards(cards: &[Card]) -> String {
    if cards.is_empty() {
        return "none".to_string();
    }

    cards
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_allowed_actions(actions: &[AllowedAction]) -> String {
    if actions.is_empty() {
        return "none".to_string();
    }

    actions
        .iter()
        .map(|action| format_allowed_action(*action))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_allowed_action(action: AllowedAction) -> &'static str {
    match action {
        AllowedAction::Check => "Check",
        AllowedAction::Bet => "Bet",
        AllowedAction::Call => "Call",
        AllowedAction::Raise => "Raise",
        AllowedAction::Fold => "Fold",
        AllowedAction::Draw => "Draw",
    }
}

fn format_variant(variant: PokerVariant) -> &'static str {
    match variant {
        PokerVariant::FiveCardDraw => "Five Card Draw",
        PokerVariant::TexasHoldEm => "Texas Hold'em",
        PokerVariant::SevenCardStud => "Seven Card Stud",
    }
}

fn format_step(step: HandStep) -> &'static str {
    match step {
        HandStep::WaitingForPlayers => "Waiting for players",
        HandStep::CollectForcedBets => "Collecting forced bets",
        HandStep::DealPrivate => "Dealing private cards",
        HandStep::DealFaceUp => "Dealing face-up cards",
        HandStep::DealCommunity => "Dealing community cards",
        HandStep::BettingRound1 => "Betting round 1",
        HandStep::DrawPhase => "Draw phase",
        HandStep::BettingRound2 => "Betting round 2",
        HandStep::BettingRound3 => "Betting round 3",
        HandStep::BettingRound4 => "Betting round 4",
        HandStep::BettingRound5 => "Betting round 5",
        HandStep::Showdown => "Showdown",
        HandStep::Payout => "Payout",
    }
}

fn format_error_code(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidRequest => "invalid_request",
        ErrorCode::AuthenticationFailed => "authentication_failed",
        ErrorCode::SessionNotFound => "session_not_found",
        ErrorCode::PermissionDenied => "permission_denied",
        ErrorCode::TableNotFound => "table_not_found",
        ErrorCode::AccountError => "account_error",
        ErrorCode::GameError => "game_error",
        ErrorCode::Internal => "internal",
    }
}

fn format_session_close_reason(reason: SessionCloseReason) -> &'static str {
    match reason {
        SessionCloseReason::LoggedOut => "logged_out",
        SessionCloseReason::ReplacedByLogin => "replaced_by_login",
    }
}

fn format_optional_player_id(player_id: Option<u64>) -> String {
    player_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn format_optional_seat(seat: Option<usize>) -> String {
    seat.map(|value| value.to_string())
        .unwrap_or_else(|| "not seated".to_string())
}

fn format_optional_chips(amount: Option<i64>) -> String {
    amount
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_optional_usize(value: Option<usize>) -> String {
    value
        .map(|item| item.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn print_help() {
    println!("Commands:");
    println!("  help");
    println!("  session");
    println!("  create_account <name> <pin> <amount>");
    println!("  login <name> <pin>");
    println!("  logout");
    println!("  deposit <amount>");
    println!("  withdraw <amount>");
    println!("  list_tables");
    println!("  create_table <five|holdem|stud>");
    println!("  join_table <table_id>");
    println!("  leave_table <table_id>");
    println!("  start_hand <table_id>");
    println!("  get_table_state <table_id>");
    println!("  get_player_view <table_id>");
    println!("  get_history <table_id>");
    println!("  subscribe_table <table_id>");
    println!("  unsubscribe_table <table_id>");
    println!("  submit_action <table_id> check");
    println!("  submit_action <table_id> call");
    println!("  submit_action <table_id> fold");
    println!("  submit_action <table_id> bet <amount>");
    println!("  submit_action <table_id> raise <amount>");
    println!("  submit_action <table_id> draw 0 1 2 ...");
    println!("  quit | exit");
    println!("Startup flags:");
    println!("  --host <host> --port <port>");
    println!("  or pass <host>:<port>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::card_deck::{Rank, Suit};
    use common::protocol::ShowdownLine;
    use common::wire::{ErrorPayload, LoginResult, TableList, TableSummary};

    #[test]
    fn formats_login_response_with_account_summary() {
        let response = ServerResponse::ok(
            "req-1",
            ResponsePayload::Login(LoginResult {
                session_id: "session-123".into(),
                account: sample_account(),
            }),
        );

        let output = format_response(&response);

        assert!(output.contains("Logged in with session session-123"));
        assert!(output.contains("Account: alice (player #7)"));
        assert!(output.contains("Balance: 980"));
    }

    #[test]
    fn formats_error_response_concisely() {
        let response = ServerResponse::error(
            "req-2",
            ErrorPayload::new(ErrorCode::TableNotFound, "table 99 not found"),
        );

        let output = format_response(&response);

        assert_eq!(output, "Error [table_not_found]: table 99 not found");
    }

    #[test]
    fn formats_table_list_and_table_summary() {
        let response = ServerResponse::ok(
            "req-3",
            ResponsePayload::Tables(TableList {
                tables: vec![
                    sample_table_summary(),
                    TableSummary {
                        table_id: 2,
                        variant: PokerVariant::FiveCardDraw,
                        step: HandStep::WaitingForPlayers,
                        seated_count: 1,
                        max_players: 5,
                    },
                ],
            }),
        );

        let output = format_response(&response);

        assert!(output.contains("Tables (2)"));
        assert!(output.contains("Table 1 | Texas Hold'em | Betting round 2 | seats 2/6"));
        assert!(output.contains("Table 2 | Five Card Draw | Waiting for players | seats 1/5"));
    }

    #[test]
    fn formats_join_leave_and_subscription_responses() {
        let join = format_response(&ServerResponse::ok(
            "req-4",
            ResponsePayload::JoinTable(JoinTableResult {
                table_id: 4,
                status: SeatAssignmentStatus::Seated,
                seat: Some(2),
            }),
        ));
        let leave = format_response(&ServerResponse::ok(
            "req-5",
            ResponsePayload::LeaveTable(LeaveTableResult {
                table_id: 4,
                status: LeaveTableStatus::LeavingAfterHand,
            }),
        ));
        let subscription = format_response(&ServerResponse::ok(
            "req-6",
            ResponsePayload::Subscription(SubscriptionResult {
                table_id: 4,
                subscribed: true,
            }),
        ));

        assert!(join.contains("Joined table 4 at seat 2"));
        assert!(leave.contains("Will leave table 4 after the current hand"));
        assert!(subscription.contains("Subscribed to table 4"));
    }

    #[test]
    fn formats_table_state_with_board_and_seats() {
        let response = ServerResponse::ok(
            "req-7",
            ResponsePayload::TableState(sample_public_table_state()),
        );

        let output = format_response(&response);

        assert!(output.contains("Table 1 | Texas Hold'em | Betting round 2"));
        assert!(output.contains("Board: A♠ K♥"));
        assert!(output.contains("Seat 0: alice (player #7) [dealer, acting, in hand]"));
        assert!(output.contains("Seat 1: bob (player #8) [folded]"));
    }

    #[test]
    fn formats_player_view_with_action_prompt() {
        let response =
            ServerResponse::ok("req-8", ResponsePayload::PlayerView(sample_player_view()));

        let output = format_response(&response);

        assert!(output.contains("Player view for alice (player #7)"));
        assert!(output.contains("Private cards: Q♣ J♦"));
        assert!(
            output
                .contains("Action: check, call, raise | call=10 | min_raise=20 | max_discards=n/a")
        );
    }

    #[test]
    fn formats_history_entries() {
        let response = ServerResponse::ok(
            "req-9",
            ResponsePayload::History(TableHistory {
                table_id: 1,
                entries: vec![
                    sample_history_entry(3, "alice joined"),
                    sample_showdown_history_entry(),
                    sample_payout_history_entry(),
                ],
            }),
        );

        let output = format_response(&response);

        assert!(output.contains("History for table 1"));
        assert!(output.contains("3. alice joined"));
        assert!(output.contains("4. showdown: 2 result(s)"));
        assert!(output.contains("Showdown:"));
        assert!(output.contains("alice -> board=[A♠ K♥ Q♣] hole=[J♦ 10♦] => Straight"));
        assert!(output.contains("5. payout: 2 winner(s)"));
        assert!(output.contains("Payout:"));
        assert!(output.contains("alice (player #7) +60"));
    }

    #[test]
    fn formats_table_update_with_events_and_player_view() {
        let output = format_table_update(&TableUpdateMessage {
            table_id: 1,
            table_state: sample_public_table_state(),
            player_view: Some(sample_player_view()),
            events: vec![
                sample_history_entry(7, "alice bet 10"),
                sample_showdown_history_entry(),
                sample_payout_history_entry(),
            ],
        });

        assert!(output.contains("[push table 1]"));
        assert!(output.contains("Events:"));
        assert!(output.contains("7. alice bet 10"));
        assert!(output.contains("4. showdown: 2 result(s)"));
        assert!(output.contains("alice -> board=[A♠ K♥ Q♣] hole=[J♦ 10♦] => Straight"));
        assert!(output.contains("5. payout: 2 winner(s)"));
        assert!(output.contains("bob (player #8) +40"));
        assert!(output.contains("Player view for alice (player #7)"));
    }

    #[test]
    fn falls_back_to_summary_when_showdown_results_are_missing() {
        let output = format_history_entry(&HistoryEntry {
            index: 12,
            kind: HistoryKind::Showdown,
            message: "showdown: 0 result(s)".into(),
            showdown_results: None,
            payout_results: None,
        });

        assert_eq!(output, "12. showdown: 0 result(s)");
    }

    #[test]
    fn falls_back_to_summary_when_payout_results_are_missing() {
        let output = format_history_entry(&HistoryEntry {
            index: 13,
            kind: HistoryKind::Payout,
            message: "payout: 0 winner(s)".into(),
            showdown_results: None,
            payout_results: None,
        });

        assert_eq!(output, "13. payout: 0 winner(s)");
    }

    #[test]
    fn falls_back_to_json_when_response_shape_is_missing_payload() {
        let response = ServerResponse {
            request_id: "req-10".into(),
            ok: true,
            payload: None,
            error: None,
        };

        let output = format_response(&response);

        assert!(output.starts_with("Response\n{"));
        assert!(output.contains("\"request_id\": \"req-10\""));
    }

    fn sample_account() -> AccountProfile {
        AccountProfile {
            player_id: 7,
            player_name: "alice".into(),
            balance: 980,
            games_played: 12,
            pots_won: 4,
            folds: 3,
        }
    }

    fn sample_table_summary() -> TableSummary {
        TableSummary {
            table_id: 1,
            variant: PokerVariant::TexasHoldEm,
            step: HandStep::BettingRound2,
            seated_count: 2,
            max_players: 6,
        }
    }

    fn sample_public_table_state() -> PublicTableState {
        PublicTableState {
            table_id: 1,
            variant: PokerVariant::TexasHoldEm,
            step: HandStep::BettingRound2,
            dealer_seat: 0,
            acting_player_id: Some(7),
            pot: 40,
            current_bet: 10,
            board_cards: vec![
                card(Rank::Ace, Suit::Spades),
                card(Rank::King, Suit::Hearts),
            ],
            seats: vec![
                PublicSeatState {
                    seat: 0,
                    player_id: 7,
                    player_name: "alice".into(),
                    balance: 980,
                    is_connected: true,
                    is_in_hand: true,
                    has_folded: false,
                    hidden_card_count: 2,
                    up_cards: vec![],
                },
                PublicSeatState {
                    seat: 1,
                    player_id: 8,
                    player_name: "bob".into(),
                    balance: 950,
                    is_connected: true,
                    is_in_hand: false,
                    has_folded: true,
                    hidden_card_count: 2,
                    up_cards: vec![card(Rank::Ten, Suit::Clubs)],
                },
            ],
        }
    }

    fn sample_player_view() -> PlayerView {
        PlayerView {
            table_id: 1,
            player_id: 7,
            player_name: "alice".into(),
            seat: Some(0),
            private_cards: vec![
                card(Rank::Queen, Suit::Clubs),
                card(Rank::Jack, Suit::Diamonds),
            ],
            up_cards: vec![],
            board_cards: vec![
                card(Rank::Ace, Suit::Spades),
                card(Rank::King, Suit::Hearts),
            ],
            action_prompt: Some(ActionPrompt {
                allowed_actions: vec![
                    AllowedAction::Check,
                    AllowedAction::Call,
                    AllowedAction::Raise,
                ],
                call_amount: Some(10),
                min_raise: Some(20),
                max_discards: None,
            }),
        }
    }

    fn sample_history_entry(index: u64, message: &str) -> HistoryEntry {
        HistoryEntry {
            index,
            kind: common::wire::HistoryKind::Info,
            message: message.into(),
            showdown_results: None,
            payout_results: None,
        }
    }

    fn sample_showdown_history_entry() -> HistoryEntry {
        HistoryEntry {
            index: 4,
            kind: HistoryKind::Showdown,
            message: "showdown: 2 result(s)".into(),
            showdown_results: Some(vec![
                ShowdownLine {
                    player_name: "alice".into(),
                    player_id: 7,
                    hand_desc: "board=[A♠ K♥ Q♣] hole=[J♦ 10♦] => Straight".into(),
                },
                ShowdownLine {
                    player_name: "bob".into(),
                    player_id: 8,
                    hand_desc: "board=[A♠ K♥ Q♣] hole=[K♣ K♦] => Three of a Kind".into(),
                },
            ]),
            payout_results: None,
        }
    }

    fn sample_payout_history_entry() -> HistoryEntry {
        HistoryEntry {
            index: 5,
            kind: HistoryKind::Payout,
            message: "payout: 2 winner(s)".into(),
            showdown_results: None,
            payout_results: Some(vec![
                PayoutLine {
                    player_name: "alice".into(),
                    player_id: 7,
                    amount: 60,
                },
                PayoutLine {
                    player_name: "bob".into(),
                    player_id: 8,
                    amount: 40,
                },
            ]),
        }
    }

    fn card(rank: Rank, suit: Suit) -> Card {
        Card::new(rank, suit)
    }
}

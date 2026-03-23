use serde::{Deserialize, Serialize};

use crate::{
    card_deck::Card,
    protocol::{AllowedAction, Chips, HandStep, PlayerId, PokerVariant},
};

pub type SessionId = String;
pub type TableId = u64;
pub type RequestId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRequest {
    pub request_id: RequestId,
    pub session_id: Option<SessionId>,
    pub op: RequestOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerResponse {
    pub request_id: RequestId,
    pub ok: bool,
    pub payload: Option<ResponsePayload>,
    pub error: Option<ErrorPayload>,
}

impl ServerResponse {
    pub fn ok(request_id: impl Into<RequestId>, payload: ResponsePayload) -> Self {
        Self {
            request_id: request_id.into(),
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn error(request_id: impl Into<RequestId>, error: ErrorPayload) -> Self {
        Self {
            request_id: request_id.into(),
            ok: false,
            payload: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Response { response: ServerResponse },
    TableUpdate(TableUpdateMessage),
    SessionClosed(SessionClosedMessage),
}

impl ServerMessage {
    pub fn response(response: ServerResponse) -> Self {
        Self::Response { response }
    }

    pub fn table_update(update: TableUpdateMessage) -> Self {
        Self::TableUpdate(update)
    }

    pub fn session_closed(
        session_id: impl Into<SessionId>,
        reason: SessionCloseReason,
        message: impl Into<String>,
    ) -> Self {
        Self::SessionClosed(SessionClosedMessage {
            session_id: session_id.into(),
            reason,
            message: message.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestOp {
    CreateAccount {
        name: String,
        pin: String,
        amount: Chips,
    },
    Login {
        name: String,
        pin: String,
    },
    Logout,
    Deposit {
        amount: Chips,
    },
    Withdraw {
        amount: Chips,
    },
    ListTables,
    CreateTable {
        variant: PokerVariant,
    },
    JoinTable {
        table_id: TableId,
    },
    LeaveTable {
        table_id: TableId,
    },
    StartHand {
        table_id: TableId,
    },
    GetTableState {
        table_id: TableId,
    },
    GetPlayerView {
        table_id: TableId,
    },
    SubmitAction {
        table_id: TableId,
        action: PlayerAction,
    },
    GetHistory {
        table_id: TableId,
    },
    SubscribeTable {
        table_id: TableId,
    },
    UnsubscribeTable {
        table_id: TableId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsePayload {
    Account(AccountProfile),
    Login(LoginResult),
    Logout(LogoutResult),
    Tables(TableList),
    Table(TableSummary),
    JoinTable(JoinTableResult),
    LeaveTable(LeaveTableResult),
    Receipt(OperationReceipt),
    TableState(PublicTableState),
    PlayerView(PlayerView),
    History(TableHistory),
    Subscription(SubscriptionResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    AuthenticationFailed,
    SessionNotFound,
    PermissionDenied,
    TableNotFound,
    AccountError,
    GameError,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
}

impl ErrorPayload {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountProfile {
    pub player_id: PlayerId,
    pub player_name: String,
    pub balance: Chips,
    pub games_played: u32,
    pub pots_won: u32,
    pub folds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginResult {
    pub session_id: SessionId,
    pub account: AccountProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogoutResult {
    pub session_id: SessionId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableList {
    pub tables: Vec<TableSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableSummary {
    pub table_id: TableId,
    pub variant: PokerVariant,
    pub step: HandStep,
    pub seated_count: usize,
    pub max_players: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeatAssignmentStatus {
    Seated,
    Queued,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JoinTableResult {
    pub table_id: TableId,
    pub status: SeatAssignmentStatus,
    pub seat: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaveTableStatus {
    Left,
    LeavingAfterHand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaveTableResult {
    pub table_id: TableId,
    pub status: LeaveTableStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationReceipt {
    pub table_id: TableId,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTableState {
    pub table_id: TableId,
    pub variant: PokerVariant,
    pub step: HandStep,
    pub dealer_seat: usize,
    pub acting_player_id: Option<PlayerId>,
    pub pot: Chips,
    pub current_bet: Chips,
    pub board_cards: Vec<Card>,
    pub seats: Vec<PublicSeatState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicSeatState {
    pub seat: usize,
    pub player_id: PlayerId,
    pub player_name: String,
    pub balance: Chips,
    pub is_connected: bool,
    pub is_in_hand: bool,
    pub has_folded: bool,
    pub hidden_card_count: usize,
    pub up_cards: Vec<Card>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerView {
    pub table_id: TableId,
    pub player_id: PlayerId,
    pub player_name: String,
    pub seat: Option<usize>,
    pub private_cards: Vec<Card>,
    pub up_cards: Vec<Card>,
    pub board_cards: Vec<Card>,
    pub action_prompt: Option<ActionPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionPrompt {
    pub allowed_actions: Vec<AllowedAction>,
    pub call_amount: Option<Chips>,
    pub min_raise: Option<Chips>,
    pub max_discards: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerAction {
    Check,
    Bet { amount: Chips },
    Call,
    Raise { amount: Chips },
    Fold,
    Draw { discard: Vec<usize> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableHistory {
    pub table_id: TableId,
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableUpdateMessage {
    pub table_id: TableId,
    pub table_state: PublicTableState,
    pub player_view: Option<PlayerView>,
    pub events: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub index: u64,
    pub kind: HistoryKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Info,
    Error,
    StepChanged,
    PlayerJoined,
    PlayerLeft,
    PotUpdated,
    ActionPrompt,
    DrawPrompt,
    Showdown,
    Payout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscriptionResult {
    pub table_id: TableId,
    pub subscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionClosedMessage {
    pub session_id: SessionId,
    pub reason: SessionCloseReason,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionCloseReason {
    LoggedOut,
    ReplacedByLogin,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_deck::{Card, Rank, Suit};

    #[test]
    fn client_request_round_trip_json() {
        let req = ClientRequest {
            request_id: "req-1".into(),
            session_id: Some("session-1".into()),
            op: RequestOp::SubmitAction {
                table_id: 7,
                action: PlayerAction::Raise { amount: 25 },
            },
        };

        let json = serde_json::to_string(&req).expect("serialize request");
        let decoded: ClientRequest = serde_json::from_str(&json).expect("deserialize request");
        assert_eq!(decoded, req);
    }

    #[test]
    fn server_response_round_trip_json() {
        let state = PublicTableState {
            table_id: 1,
            variant: PokerVariant::TexasHoldEm,
            step: HandStep::BettingRound2,
            dealer_seat: 0,
            acting_player_id: Some(3),
            pot: 40,
            current_bet: 10,
            board_cards: vec![Card::new(Rank::Ace, Suit::Spades)],
            seats: vec![PublicSeatState {
                seat: 0,
                player_id: 3,
                player_name: "alice".into(),
                balance: 980,
                is_connected: true,
                is_in_hand: true,
                has_folded: false,
                hidden_card_count: 2,
                up_cards: vec![],
            }],
        };

        let resp = ServerResponse::ok("req-9", ResponsePayload::TableState(state));
        let json = serde_json::to_string(&resp).expect("serialize response");
        let decoded: ServerResponse = serde_json::from_str(&json).expect("deserialize response");
        assert_eq!(decoded, resp);
    }

    #[test]
    fn server_message_round_trip_json() {
        let update = TableUpdateMessage {
            table_id: 1,
            table_state: PublicTableState {
                table_id: 1,
                variant: PokerVariant::FiveCardDraw,
                step: HandStep::BettingRound1,
                dealer_seat: 0,
                acting_player_id: Some(7),
                pot: 15,
                current_bet: 5,
                board_cards: vec![],
                seats: vec![PublicSeatState {
                    seat: 0,
                    player_id: 7,
                    player_name: "alice".into(),
                    balance: 495,
                    is_connected: true,
                    is_in_hand: true,
                    has_folded: false,
                    hidden_card_count: 5,
                    up_cards: vec![],
                }],
            },
            player_view: Some(PlayerView {
                table_id: 1,
                player_id: 7,
                player_name: "alice".into(),
                seat: Some(0),
                private_cards: vec![Card::new(Rank::King, Suit::Hearts)],
                up_cards: vec![],
                board_cards: vec![],
                action_prompt: None,
            }),
            events: vec![HistoryEntry {
                index: 2,
                kind: HistoryKind::ActionPrompt,
                message: "action requested for alice".into(),
            }],
        };

        let msg = ServerMessage::table_update(update);
        let json = serde_json::to_string(&msg).expect("serialize message");
        let decoded: ServerMessage = serde_json::from_str(&json).expect("deserialize message");
        assert_eq!(decoded, msg);
    }

    #[test]
    fn subscription_result_round_trip_json() {
        let payload = ResponsePayload::Subscription(SubscriptionResult {
            table_id: 3,
            subscribed: true,
        });

        let json = serde_json::to_string(&payload).expect("serialize payload");
        let decoded: ResponsePayload = serde_json::from_str(&json).expect("deserialize payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn session_closed_message_round_trip_json() {
        let msg = ServerMessage::session_closed(
            "session-9",
            SessionCloseReason::ReplacedByLogin,
            "session replaced",
        );

        let json = serde_json::to_string(&msg).expect("serialize message");
        let decoded: ServerMessage = serde_json::from_str(&json).expect("deserialize message");
        assert_eq!(decoded, msg);
    }
}

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

use common::protocol::PokerVariant;
use common::wire::{
    AccountProfile, HistoryEntry, HistoryKind, PlayerView, PublicTableState, RequestOp,
    ResponsePayload, SessionId, TableId, TableSummary,
};

use crate::state::ClientEvent;

use super::network::spawn_network_threads;

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Login,
    Lobby,
    Table { table_id: TableId },
}

// ---------------------------------------------------------------------------
// LoginTab — which tab is active on the login screen
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginTab {
    Login,
    CreateAccount,
}

// ---------------------------------------------------------------------------
// PendingKind — tracks what each in-flight request_id was for
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingKind {
    Login,
    CreateAccount,
    Logout,
    ListTables,
    CreateTable,
    JoinTable,
    LeaveTable,
    Subscribe,
    StartHand,
    SubmitAction,
    GetTableState,
    GetPlayerView,
    Deposit,
    Withdraw,
}

// ---------------------------------------------------------------------------
// GuiCommand — sent from the GUI to the network worker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum GuiCommand {
    SendOp(RequestOp),
}

// ---------------------------------------------------------------------------
// PokerApp
// ---------------------------------------------------------------------------

pub struct PokerApp {
    // Navigation
    pub screen: Screen,

    // Connection
    pub server_address: String,
    pub connected: bool,

    // Channels to/from the network threads (None until connected)
    pub cmd_tx: Option<Sender<GuiCommand>>,
    pub resp_rx: Option<Receiver<common::wire::ServerResponse>>,
    pub push_rx: Option<Receiver<ClientEvent>>,

    // Session / account
    pub session_id: Option<SessionId>,
    pub account: Option<AccountProfile>,

    // Lobby state
    pub tables: Vec<TableSummary>,

    // Active table state
    pub table_state: Option<PublicTableState>,
    pub player_view: Option<PlayerView>,
    pub event_log: Vec<HistoryEntry>,

    // In-flight request tracking: request_id -> what it was for
    pub pending: HashMap<String, PendingKind>,

    // Last server or transport error message shown to the user
    pub last_error: Option<String>,

    // Status message shown on the connect screen (e.g. connection error)
    pub status_msg: String,

    // Login / create-account input buffers
    pub input_name: String,
    pub input_pin: String,
    pub input_amount: String,
    pub login_tab: LoginTab,

    // Bet/raise amount input on the action bar
    pub input_bet: String,

    // Deposit/withdraw amount input in the lobby
    pub input_deposit: String,

    // Indices of cards toggled for discard during the draw phase
    pub selected_discards: Vec<usize>,

    // Most recent showdown and payout results — cleared when next hand starts
    pub last_showdown: Option<HistoryEntry>,
    pub last_payout: Option<HistoryEntry>,

    // Lobby: selected variant for the create-table dropdown
    pub selected_variant: PokerVariant,

    // Cached GPU textures for card images, keyed by "rank_suit" e.g. "ace_hearts"
    pub texture_cache: HashMap<String, egui::TextureHandle>,
}

impl Default for PokerApp {
    fn default() -> Self {
        Self {
            screen: Screen::Connect,
            server_address: "127.0.0.1:8080".to_string(),
            connected: false,
            cmd_tx: None,
            resp_rx: None,
            push_rx: None,
            session_id: None,
            account: None,
            tables: Vec::new(),
            table_state: None,
            player_view: None,
            event_log: Vec::new(),
            pending: HashMap::new(),
            last_error: None,
            status_msg: String::new(),
            input_name: String::new(),
            input_pin: String::new(),
            input_amount: String::new(),
            login_tab: LoginTab::Login,
            selected_variant: PokerVariant::FiveCardDraw,
            texture_cache: HashMap::new(),
            input_bet: String::new(),
            input_deposit: String::new(),
            selected_discards: Vec::new(),
            last_showdown: None,
            last_payout: None,
        }
    }
}

impl PokerApp {
    /// Called by the Connect screen to open the TCP connection and wire up channels.
    pub fn connect(&mut self, ctx: &egui::Context) {
        match spawn_network_threads(&self.server_address, ctx.clone()) {
            Ok((cmd_tx, resp_rx, push_rx)) => {
                self.cmd_tx = Some(cmd_tx);
                self.resp_rx = Some(resp_rx);
                self.push_rx = Some(push_rx);
                self.connected = true;
                self.status_msg.clear();
                self.last_error = None;
                self.screen = Screen::Login;
            }
            Err(e) => {
                self.status_msg = format!("Could not connect: {e}");
            }
        }
    }

    /// Transition to the Lobby screen and immediately refresh the table list.
    pub fn enter_lobby(&mut self) {
        self.screen = Screen::Lobby;
        self.send_op(PendingKind::ListTables, RequestOp::ListTables);
    }

    /// Send a request op to the network thread and record it as pending.
    pub fn send_op(&mut self, kind: PendingKind, op: RequestOp) {
        if let Some(tx) = &self.cmd_tx {
            if tx.send(GuiCommand::SendOp(op)).is_ok() {
                self.pending.insert("in-flight".to_string(), kind);
                self.last_error = None;
            }
        }
    }

    /// Returns true if any request is currently in-flight.
    pub fn is_request_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Drain both incoming channels without blocking.
    /// Must be called at the top of every `update()` call.
    pub fn drain_channels(&mut self) {
        self.drain_responses();
        self.drain_push_events();
    }

    fn drain_responses(&mut self) {
        loop {
            let msg = match self.resp_rx.as_ref() {
                Some(rx) => rx.try_recv(),
                None => return,
            };
            match msg {
                Ok(response) => self.handle_response(response),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.resp_rx = None;
                    break;
                }
            }
        }
    }

    fn drain_push_events(&mut self) {
        loop {
            let msg = match self.push_rx.as_ref() {
                Some(rx) => rx.try_recv(),
                None => return,
            };
            match msg {
                Ok(event) => self.handle_push_event(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.push_rx = None;
                    break;
                }
            }
        }
    }

    fn handle_response(&mut self, response: common::wire::ServerResponse) {
        // TcpClient serializes one request at a time — clear the single pending entry.
        self.pending.clear();

        if !response.ok {
            if let Some(err) = &response.error {
                self.last_error = Some(err.message.clone());
            }
            return;
        }

        match response.payload {
            Some(ResponsePayload::Login(result)) => {
                self.session_id = Some(result.session_id);
                self.account = Some(result.account);
                self.enter_lobby();
            }
            Some(ResponsePayload::Account(profile)) => {
                if self.account.is_some() {
                    // Deposit or withdraw — just update the balance in place.
                    self.account = Some(profile);
                } else {
                    // Account creation — auto-login immediately.
                    let op = RequestOp::Login {
                        name: profile.player_name.clone(),
                        pin: self.input_pin.clone(),
                    };
                    self.send_op(PendingKind::Login, op);
                }
            }
            Some(ResponsePayload::Logout(_)) => {
                self.session_id = None;
                self.account = None;
                self.screen = Screen::Login;
            }
            Some(ResponsePayload::Tables(list)) => {
                self.tables = list.tables;
            }
            Some(ResponsePayload::Table(summary)) => {
                // CreateTable succeeded — immediately join the new table.
                let table_id = summary.table_id;
                self.send_op(PendingKind::JoinTable, RequestOp::JoinTable { table_id });
            }
            Some(ResponsePayload::JoinTable(result)) => {
                let table_id = result.table_id;
                // Subscribe then fetch initial state — these go through the channel
                // in order without blocking the UI.
                if let Some(tx) = &self.cmd_tx {
                    let _ = tx.send(GuiCommand::SendOp(
                        RequestOp::SubscribeTable { table_id },
                    ));
                    let _ = tx.send(GuiCommand::SendOp(
                        RequestOp::GetTableState { table_id },
                    ));
                    let _ = tx.send(GuiCommand::SendOp(
                        RequestOp::GetPlayerView { table_id },
                    ));
                }
                self.screen = Screen::Table { table_id };
            }
            Some(ResponsePayload::LeaveTable(_)) => {
                self.table_state = None;
                self.player_view = None;
                self.enter_lobby();
            }
            Some(ResponsePayload::TableState(state)) => {
                self.table_state = Some(state);
            }
            Some(ResponsePayload::PlayerView(view)) => {
                self.player_view = Some(view);
            }
            _ => {}
        }
    }

    fn handle_push_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::TableUpdate(update) => {
                self.table_state = Some(update.table_state);
                if let Some(view) = update.player_view {
                    self.player_view = Some(view);
                }
                for entry in &update.events {
                    match entry.kind {
                        HistoryKind::Showdown => self.last_showdown = Some(entry.clone()),
                        HistoryKind::Payout => self.last_payout = Some(entry.clone()),
                        HistoryKind::StepChanged => {
                            if entry.message.contains("CollectForcedBets") {
                                self.last_showdown = None;
                                self.last_payout = None;
                            }
                        }
                        _ => {}
                    }
                }
                self.event_log.extend(update.events);
            }
            ClientEvent::SessionClosed(msg) => {
                self.session_id = None;
                self.account = None;
                self.table_state = None;
                self.player_view = None;
                self.event_log.clear();
                self.last_showdown = None;
                self.last_payout = None;
                use common::wire::SessionCloseReason;
                self.status_msg = match msg.reason {
                    SessionCloseReason::ReplacedByLogin => {
                        "Session replaced by a new login".to_string()
                    }
                    SessionCloseReason::LoggedOut => String::new(),
                };
                self.screen = Screen::Login;
            }
            ClientEvent::Disconnected => {
                self.connected = false;
                self.cmd_tx = None;
                self.resp_rx = None;
                self.push_rx = None;
                self.table_state = None;
                self.player_view = None;
                self.event_log.clear();
                self.last_showdown = None;
                self.last_payout = None;
                self.status_msg = "Connection lost — please reconnect".to_string();
                self.screen = Screen::Connect;
            }
            ClientEvent::TransportError(msg) => {
                self.connected = false;
                self.cmd_tx = None;
                self.resp_rx = None;
                self.push_rx = None;
                self.table_state = None;
                self.player_view = None;
                self.event_log.clear();
                self.status_msg = format!("Connection error: {msg}");
                self.screen = Screen::Connect;
            }
            ClientEvent::UnexpectedResponse(_) => {}
        }
    }
}

impl eframe::App for PokerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_channels();

        match self.screen.clone() {
            Screen::Connect => super::screens::connect::show(self, ctx),
            Screen::Login => super::screens::login::show(self, ctx),
            Screen::Lobby => super::screens::lobby::show(self, ctx),
            Screen::Table { table_id } => {
                super::screens::table::show(self, ctx, table_id);
            }
        }
    }
}

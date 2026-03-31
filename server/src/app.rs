use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use common::{
    protocol::{AccountCmd, ActionCmd, Command, Event, HandStep, LobbyCmd, PlayerId, PokerVariant},
    wire::{
        AccountProfile, ClientRequest, ErrorCode, ErrorPayload, HistoryEntry, HistoryKind,
        JoinTableResult, LeaveTableResult, LeaveTableStatus, LoginResult, LogoutResult,
        OperationReceipt, PlayerAction, PlayerView, PublicSeatState, PublicTableState, RequestOp,
        ResponsePayload, SeatAssignmentStatus, ServerResponse, SessionId, TableHistory, TableId,
        TableList, TableSummary, TableUpdateMessage,
    },
};

use crate::{
    account_store::{AccountError, AccountStore},
    accounts::Account,
    engine::SharedAccountStore,
    sessions::{Session, SessionStore},
    table::PokerTable,
    table_runtime::TableRuntime,
    table_worker::{PlayerViewState, TableHandle, TableWorkerError},
    variants,
};

pub type AppResult<T> = Result<T, ErrorPayload>;

const DEFAULT_TABLE_ID: TableId = 1;

#[derive(Debug, Clone)]
pub struct TableChangeNotice {
    pub table_id: TableId,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct RequestOutcome {
    pub response: ServerResponse,
    pub table_changes: Vec<TableChangeNotice>,
    pub invalidated_sessions: Vec<SessionId>,
}

pub struct ServerApp {
    pub accounts: SharedAccountStore,
    sessions: Mutex<SessionStore>,
    tables: RwLock<HashMap<TableId, TableHandle>>,
    default_table_id: TableId,
    next_table_id: AtomicU64,
}

impl ServerApp {
    pub fn new(store: AccountStore) -> Self {
        let accounts = Arc::new(Mutex::new(store));
        let default_table_id = DEFAULT_TABLE_ID;
        let mut tables = HashMap::new();
        tables.insert(default_table_id, TableHandle::new(Arc::clone(&accounts)));

        Self {
            accounts,
            sessions: Mutex::new(SessionStore::new()),
            tables: RwLock::new(tables),
            default_table_id,
            next_table_id: AtomicU64::new(default_table_id + 1),
        }
    }

    pub fn apply(&self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::Account(account_cmd) => self.apply_account_cmd(account_cmd),
            other => self.apply_to_table(self.default_table_id, other),
        }
    }

    pub fn apply_to_table(&self, table_id: TableId, cmd: Command) -> Vec<Event> {
        let handle = match self.table_handle(table_id) {
            Ok(handle) => handle,
            Err(_) => {
                return vec![Event::Error {
                    message: format!("Table {table_id} not found"),
                }];
            }
        };

        match handle.apply(cmd) {
            Ok(events) => events,
            Err(error) => vec![Event::Error {
                message: format!("Table {table_id} unavailable: {error}"),
            }],
        }
    }

    pub fn create_account(
        &self,
        name: String,
        pin: String,
        amount: i64,
    ) -> AppResult<AccountProfile> {
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .create_account(name, &pin, amount)
            .map_err(|e| Self::account_error("Account creation failed", e))?;

        Ok(Self::account_profile(&account))
    }

    pub fn login(&self, name: String, pin: String) -> AppResult<LoginResult> {
        self.login_detail(name, pin).map(|(result, _)| result)
    }

    fn login_detail(
        &self,
        name: String,
        pin: String,
    ) -> AppResult<(LoginResult, Option<SessionId>)> {
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .login(&name, &pin)
            .map_err(|e| Self::auth_error("Login failed", e))?;

        let creation = self
            .sessions
            .lock()
            .expect("session store lock poisoned")
            .create_session(&account);
        Ok((
            LoginResult {
                session_id: creation.session.session_id,
                account: Self::account_profile(&account),
            },
            creation.replaced_session_id,
        ))
    }

    pub fn logout(&self, session_id: &str) -> AppResult<LogoutResult> {
        self.logout_detail(session_id)
            .map(|(result, _table_changes)| result)
    }

    fn logout_detail(&self, session_id: &str) -> AppResult<(LogoutResult, Vec<TableChangeNotice>)> {
        let session = self.require_session(session_id)?;
        let mut table_changes = Vec::new();
        for (table_id, handle) in self.table_handles() {
            let snapshot = handle
                .snapshot()
                .map_err(|e| Self::table_worker_error(table_id, e))?;
            let is_seated = snapshot
                .table
                .seated_players()
                .iter()
                .any(|(_, seated_id)| *seated_id == session.account_id);
            if !is_seated {
                continue;
            }

            let events = handle
                .apply(Command::Lobby(LobbyCmd::Leave {
                    player_name: session.player_name.clone(),
                }))
                .map_err(|e| Self::table_worker_error(table_id, e))?;
            Self::ensure_no_error(&events)?;
            if !events.is_empty() {
                table_changes.push(Self::table_change(table_id, events));
            }
        }

        let removed = self
            .sessions
            .lock()
            .expect("session store lock poisoned")
            .remove(session_id)
            .ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::SessionNotFound,
                    format!("Session {session_id} was not found"),
                )
            })?;

        Ok((
            LogoutResult {
                session_id: removed.session_id,
            },
            table_changes,
        ))
    }

    pub fn handle_disconnect(&self, session_id: &str) -> Vec<TableChangeNotice> {
        let session = match self
            .sessions
            .lock()
            .expect("session store lock poisoned")
            .remove(session_id)
        {
            Some(session) => session,
            None => return Vec::new(),
        };

        let mut table_changes = Vec::new();
        for (table_id, handle) in self.table_handles() {
            let snapshot = match handle.snapshot() {
                Ok(snapshot) => snapshot,
                Err(_) => continue,
            };
            let is_seated = snapshot
                .table
                .seated_players()
                .iter()
                .any(|(_, seated_id)| *seated_id == session.account_id);
            if !is_seated {
                continue;
            }

            let events =
                match handle.disconnect_player(session.account_id, session.player_name.clone()) {
                    Ok(events) => events,
                    Err(_) => continue,
                };
            if Self::ensure_no_error(&events).is_err() {
                continue;
            }
            if !events.is_empty() {
                table_changes.push(Self::table_change(table_id, events));
            }
        }

        table_changes
    }

    pub fn deposit(&self, session_id: &str, amount: i64) -> AppResult<AccountProfile> {
        let session = self.require_session(session_id)?;
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .deposit(&session.player_name, amount)
            .map_err(|e| Self::account_error("Deposit failed", e))?;
        Ok(Self::account_profile(&account))
    }

    pub fn withdraw(&self, session_id: &str, amount: i64) -> AppResult<AccountProfile> {
        let session = self.require_session(session_id)?;
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .withdraw(&session.player_name, amount)
            .map_err(|e| Self::account_error("Withdraw failed", e))?;
        Ok(Self::account_profile(&account))
    }

    pub fn list_tables(&self) -> TableList {
        let mut tables: Vec<TableSummary> = self
            .table_handles()
            .into_iter()
            .filter_map(|(table_id, handle)| {
                handle
                    .snapshot()
                    .ok()
                    .map(|snapshot| Self::table_summary_from_table(table_id, &snapshot.table))
            })
            .collect();
        tables.sort_by_key(|table| table.table_id);
        TableList { tables }
    }

    pub fn create_table(&self, session_id: &str, variant: PokerVariant) -> AppResult<TableSummary> {
        let _session = self.require_session(session_id)?;

        let table_id = self.next_table_id.fetch_add(1, Ordering::Relaxed);
        let mut runtime = TableRuntime::new(Arc::clone(&self.accounts));
        runtime
            .engine
            .table
            .set_variant_spec(variants::spec_for(variant));

        let summary = Self::table_summary_from_table(table_id, &runtime.engine.table);
        let handle = TableHandle::new_with_runtime(runtime);
        self.tables
            .write()
            .expect("table registry lock poisoned")
            .insert(table_id, handle);
        Ok(summary)
    }

    pub fn join_table(&self, session_id: &str, table_id: TableId) -> AppResult<JoinTableResult> {
        self.join_table_detail(session_id, table_id)
            .map(|(result, _events)| result)
    }

    fn join_table_detail(
        &self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<(JoinTableResult, Vec<Event>)> {
        let session = self.require_session(session_id)?;
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .get_by_id(session.account_id)
            .cloned()
            .map_err(|e| Self::account_error("Join failed", e))?;
        let handle = self.table_handle(table_id)?;

        let events = handle
            .join_authenticated_player(account.id, account.name.clone())
            .map_err(|e| Self::table_worker_error(table_id, e))?;
        Self::ensure_no_error(&events)?;

        let seat = events.iter().find_map(|event| match event {
            Event::PlayerJoined {
                player_name, seat, ..
            } if player_name == &session.player_name => Some(*seat),
            _ => None,
        });

        Ok((
            JoinTableResult {
                table_id,
                status: SeatAssignmentStatus::Seated,
                seat,
            },
            events,
        ))
    }

    pub fn leave_table(&self, session_id: &str, table_id: TableId) -> AppResult<LeaveTableResult> {
        self.leave_table_detail(session_id, table_id)
            .map(|(result, _events)| result)
    }

    fn leave_table_detail(
        &self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<(LeaveTableResult, Vec<Event>)> {
        let session = self.require_session(session_id)?;
        let handle = self.table_handle(table_id)?;
        let snapshot = handle
            .snapshot()
            .map_err(|e| Self::table_worker_error(table_id, e))?;

        let immediate_leave = snapshot.table.current_step() == HandStep::WaitingForPlayers;
        let events = handle
            .apply(Command::Lobby(LobbyCmd::Leave {
                player_name: session.player_name.clone(),
            }))
            .map_err(|e| Self::table_worker_error(table_id, e))?;
        Self::ensure_no_error(&events)?;

        let status = if immediate_leave {
            LeaveTableStatus::Left
        } else {
            LeaveTableStatus::LeavingAfterHand
        };

        Ok((LeaveTableResult { table_id, status }, events))
    }

    pub fn start_hand(&self, session_id: &str, table_id: TableId) -> AppResult<OperationReceipt> {
        self.start_hand_detail(session_id, table_id)
            .map(|(result, _events)| result)
    }

    fn start_hand_detail(
        &self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<(OperationReceipt, Vec<Event>)> {
        let session = self.require_session(session_id)?;
        let handle = self.table_handle(table_id)?;
        let snapshot = handle
            .snapshot()
            .map_err(|e| Self::table_worker_error(table_id, e))?;

        Self::require_player_seated(&snapshot.table, session.account_id, &session.player_name)?;

        let events = handle
            .apply(Command::Lobby(LobbyCmd::StartHand))
            .map_err(|e| Self::table_worker_error(table_id, e))?;
        Self::ensure_no_error(&events)?;
        Ok((
            OperationReceipt {
                table_id,
                event_count: events.len(),
            },
            events,
        ))
    }

    pub fn get_table_state(&self, table_id: TableId) -> AppResult<PublicTableState> {
        let handle = self.table_handle(table_id)?;
        let snapshot = handle
            .snapshot()
            .map_err(|e| Self::table_worker_error(table_id, e))?;
        Ok(
            self.build_public_table_state(
                table_id,
                &snapshot.table,
                &snapshot.disconnected_players,
            ),
        )
    }

    pub fn get_player_view(&self, session_id: &str, table_id: TableId) -> AppResult<PlayerView> {
        let session = self.require_session(session_id)?;
        let handle = self.table_handle(table_id)?;
        let state = handle
            .player_view_state(session.account_id, session.player_name.clone())
            .map_err(|e| Self::table_worker_error(table_id, e))?;

        if !state.is_seated {
            return Err(ErrorPayload::new(
                ErrorCode::PermissionDenied,
                format!("{} is not seated at the table", session.player_name),
            ));
        }

        Ok(Self::build_player_view(
            table_id,
            session.account_id,
            &session.player_name,
            state,
        ))
    }

    pub fn submit_action(
        &self,
        session_id: &str,
        table_id: TableId,
        action: PlayerAction,
    ) -> AppResult<OperationReceipt> {
        self.submit_action_detail(session_id, table_id, action)
            .map(|(result, _events)| result)
    }

    fn submit_action_detail(
        &self,
        session_id: &str,
        table_id: TableId,
        action: PlayerAction,
    ) -> AppResult<(OperationReceipt, Vec<Event>)> {
        let session = self.require_session(session_id)?;
        let handle = self.table_handle(table_id)?;
        let snapshot = handle
            .snapshot()
            .map_err(|e| Self::table_worker_error(table_id, e))?;

        Self::require_player_seated(&snapshot.table, session.account_id, &session.player_name)?;
        let command = Command::Action(Self::to_action_cmd(&session.player_name, action));
        let events = handle
            .apply(command)
            .map_err(|e| Self::table_worker_error(table_id, e))?;
        Self::ensure_no_error(&events)?;

        Ok((
            OperationReceipt {
                table_id,
                event_count: events.len(),
            },
            events,
        ))
    }

    pub fn get_history(&self, table_id: TableId) -> AppResult<TableHistory> {
        let handle = self.table_handle(table_id)?;
        let history = handle
            .history()
            .map_err(|e| Self::table_worker_error(table_id, e))?;

        let entries = history
            .iter()
            .enumerate()
            .filter_map(|(index, event)| Self::history_entry(index as u64, event))
            .collect();

        Ok(TableHistory { table_id, entries })
    }

    pub fn flush_accounts(&self) -> Result<(), AccountError> {
        self.accounts
            .lock()
            .expect("account store lock poisoned")
            .flush()
    }

    pub fn handle_request(&self, request: ClientRequest) -> ServerResponse {
        self.handle_request_outcome(request).response
    }

    pub fn handle_request_outcome(&self, request: ClientRequest) -> RequestOutcome {
        let request_id = request.request_id.clone();
        match self.dispatch_request_outcome(request) {
            Ok((payload, table_changes, invalidated_sessions)) => RequestOutcome {
                response: ServerResponse::ok(request_id, payload),
                table_changes,
                invalidated_sessions,
            },
            Err(error) => RequestOutcome {
                response: ServerResponse::error(request_id, error),
                table_changes: Vec::new(),
                invalidated_sessions: Vec::new(),
            },
        }
    }

    fn apply_account_cmd(&self, cmd: AccountCmd) -> Vec<Event> {
        let mut accounts = self.accounts.lock().expect("account store lock poisoned");

        match cmd {
            AccountCmd::Create { name, pin, amount } => {
                match accounts.create_account(&name, &pin, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account creation failed: {:?}", e),
                    }],
                }
            }
            AccountCmd::Delete { player_name, pin } => {
                match accounts.delete_account(&player_name, &pin) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account deletion failed: {:?}", e),
                    }],
                }
            }
            AccountCmd::Deposit {
                player_name,
                amount,
            } => match accounts.deposit(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Deposit failed: {:?}", e),
                }],
            },
            AccountCmd::Withdraw {
                player_name,
                amount,
            } => match accounts.withdraw(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Withdraw failed: {:?}", e),
                }],
            },
            AccountCmd::Get { player_name } => match accounts.get_by_name(&player_name) {
                Ok(acct) => {
                    let summary = acct.summary();
                    vec![Event::Info {
                        message: format!("Account info:\n{summary}"),
                    }]
                }
                Err(e) => vec![Event::Error {
                    message: format!("Get account failed: {:?}", e),
                }],
            },
        }
    }

    fn dispatch_request_outcome(
        &self,
        request: ClientRequest,
    ) -> AppResult<(ResponsePayload, Vec<TableChangeNotice>, Vec<SessionId>)> {
        let session_id = request.session_id.as_deref();

        match request.op {
            RequestOp::CreateAccount { name, pin, amount } => self
                .create_account(name, pin, amount)
                .map(|payload| (ResponsePayload::Account(payload), Vec::new(), Vec::new())),
            RequestOp::Login { name, pin } => {
                let (result, replaced_session_id) = self.login_detail(name, pin)?;
                Ok((
                    ResponsePayload::Login(result),
                    Vec::new(),
                    replaced_session_id.into_iter().collect(),
                ))
            }
            RequestOp::Logout => {
                let session_id = Self::require_session_id(session_id)?;
                let (result, table_changes) = self.logout_detail(session_id)?;
                Ok((ResponsePayload::Logout(result), table_changes, Vec::new()))
            }
            RequestOp::Deposit { amount } => {
                let session_id = Self::require_session_id(session_id)?;
                let session = self.require_session(session_id)?;
                let account = self.deposit(session_id, amount)?;
                Ok((
                    ResponsePayload::Account(account),
                    self.visible_table_changes_for_player(session.account_id),
                    Vec::new(),
                ))
            }
            RequestOp::Withdraw { amount } => {
                let session_id = Self::require_session_id(session_id)?;
                let session = self.require_session(session_id)?;
                let account = self.withdraw(session_id, amount)?;
                Ok((
                    ResponsePayload::Account(account),
                    self.visible_table_changes_for_player(session.account_id),
                    Vec::new(),
                ))
            }
            RequestOp::ListTables => Ok((
                ResponsePayload::Tables(self.list_tables()),
                Vec::new(),
                Vec::new(),
            )),
            RequestOp::CreateTable { variant } => self
                .create_table(Self::require_session_id(session_id)?, variant)
                .map(|payload| (ResponsePayload::Table(payload), Vec::new(), Vec::new())),
            RequestOp::JoinTable { table_id } => {
                let (result, events) =
                    self.join_table_detail(Self::require_session_id(session_id)?, table_id)?;
                Ok((
                    ResponsePayload::JoinTable(result),
                    vec![Self::table_change(table_id, events)],
                    Vec::new(),
                ))
            }
            RequestOp::LeaveTable { table_id } => {
                let (result, events) =
                    self.leave_table_detail(Self::require_session_id(session_id)?, table_id)?;
                Ok((
                    ResponsePayload::LeaveTable(result),
                    vec![Self::table_change(table_id, events)],
                    Vec::new(),
                ))
            }
            RequestOp::StartHand { table_id } => {
                let (result, events) =
                    self.start_hand_detail(Self::require_session_id(session_id)?, table_id)?;
                Ok((
                    ResponsePayload::Receipt(result),
                    vec![Self::table_change(table_id, events)],
                    Vec::new(),
                ))
            }
            RequestOp::GetTableState { table_id } => self
                .get_table_state(table_id)
                .map(|payload| (ResponsePayload::TableState(payload), Vec::new(), Vec::new())),
            RequestOp::GetPlayerView { table_id } => self
                .get_player_view(Self::require_session_id(session_id)?, table_id)
                .map(|payload| (ResponsePayload::PlayerView(payload), Vec::new(), Vec::new())),
            RequestOp::SubmitAction { table_id, action } => {
                let (result, events) = self.submit_action_detail(
                    Self::require_session_id(session_id)?,
                    table_id,
                    action,
                )?;
                Ok((
                    ResponsePayload::Receipt(result),
                    vec![Self::table_change(table_id, events)],
                    Vec::new(),
                ))
            }
            RequestOp::GetHistory { table_id } => self
                .get_history(table_id)
                .map(|payload| (ResponsePayload::History(payload), Vec::new(), Vec::new())),
            RequestOp::SubscribeTable { .. } | RequestOp::UnsubscribeTable { .. } => {
                Err(ErrorPayload::new(
                    ErrorCode::InvalidRequest,
                    "subscribe requests are handled by the TCP transport",
                ))
            }
        }
    }

    fn table_handle(&self, table_id: TableId) -> AppResult<TableHandle> {
        self.tables
            .read()
            .expect("table registry lock poisoned")
            .get(&table_id)
            .cloned()
            .ok_or_else(|| Self::table_not_found(table_id))
    }

    fn table_handles(&self) -> Vec<(TableId, TableHandle)> {
        self.tables
            .read()
            .expect("table registry lock poisoned")
            .iter()
            .map(|(table_id, handle)| (*table_id, handle.clone()))
            .collect()
    }

    fn require_session(&self, session_id: &str) -> AppResult<Session> {
        self.sessions
            .lock()
            .expect("session store lock poisoned")
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                ErrorPayload::new(
                    ErrorCode::SessionNotFound,
                    format!("Session {session_id} was not found"),
                )
            })
    }

    fn require_session_id(session_id: Option<&str>) -> AppResult<&str> {
        session_id.ok_or_else(|| {
            ErrorPayload::new(
                ErrorCode::InvalidRequest,
                "session_id is required for this operation",
            )
        })
    }

    fn require_player_seated(
        table: &PokerTable,
        player_id: PlayerId,
        player_name: &str,
    ) -> AppResult<()> {
        let is_seated = table
            .seated_players()
            .iter()
            .any(|(_, seated_id)| *seated_id == player_id);
        if is_seated {
            Ok(())
        } else {
            Err(ErrorPayload::new(
                ErrorCode::PermissionDenied,
                format!("{player_name} is not seated at the table"),
            ))
        }
    }

    fn account_profile(account: &Account) -> AccountProfile {
        AccountProfile {
            player_id: account.id,
            player_name: account.name.clone(),
            balance: account.balance,
            games_played: account.stats.games_played,
            pots_won: account.stats.pots_won,
            folds: account.stats.folds,
        }
    }

    fn account_map(&self) -> HashMap<PlayerId, Account> {
        self.accounts
            .lock()
            .expect("account store lock poisoned")
            .list_accounts()
            .iter()
            .cloned()
            .map(|account| (account.id, account))
            .collect()
    }

    fn table_summary_from_table(table_id: TableId, table: &PokerTable) -> TableSummary {
        TableSummary {
            table_id,
            variant: Self::variant_for_table(table),
            step: table.current_step(),
            seated_count: table.seated_players().len(),
            max_players: table.variant_spec().max_players,
        }
    }

    pub fn build_table_update_for_session(
        &self,
        table_id: TableId,
        session_id: Option<&str>,
        events: &[Event],
    ) -> AppResult<TableUpdateMessage> {
        let table_state = self.get_table_state(table_id)?;
        let player_view = match session_id {
            Some(session_id) => self.get_player_view(session_id, table_id).ok(),
            None => None,
        };

        Ok(TableUpdateMessage {
            table_id,
            table_state,
            player_view,
            events: Self::history_entries(events),
        })
    }

    fn build_public_table_state(
        &self,
        table_id: TableId,
        table: &PokerTable,
        disconnected_players: &std::collections::HashSet<PlayerId>,
    ) -> PublicTableState {
        let accounts = self.account_map();
        let hand_state = table.hand_state();

        let seats = table
            .seated_players()
            .into_iter()
            .filter_map(|(seat, player_id)| {
                let account = accounts.get(&player_id)?;
                let total_cards = hand_state
                    .player_hole_cards(&player_id)
                    .map(|cards| cards.len())
                    .unwrap_or(0);
                let up_cards = hand_state
                    .player_up_cards(&player_id)
                    .cloned()
                    .unwrap_or_default();

                Some(PublicSeatState {
                    seat,
                    player_id,
                    player_name: account.name.clone(),
                    balance: account.balance,
                    is_connected: !disconnected_players.contains(&player_id),
                    is_in_hand: hand_state.player_hole_cards(&player_id).is_some(),
                    has_folded: hand_state.has_player_folded(&player_id),
                    hidden_card_count: total_cards.saturating_sub(up_cards.len()),
                    up_cards,
                })
            })
            .collect();

        PublicTableState {
            table_id,
            variant: Self::variant_for_table(table),
            step: table.current_step(),
            dealer_seat: table.dealer_position(),
            acting_player_id: Self::acting_player_id(table),
            pot: hand_state.pot_size() as i64,
            current_bet: hand_state.current_bet() as i64,
            board_cards: hand_state.board_cards().to_vec(),
            seats,
        }
    }

    fn build_player_view(
        table_id: TableId,
        player_id: PlayerId,
        player_name: &str,
        state: PlayerViewState,
    ) -> PlayerView {
        PlayerView {
            table_id,
            player_id,
            player_name: player_name.to_string(),
            seat: state.seat,
            private_cards: state.private_cards,
            up_cards: state.up_cards,
            board_cards: state.board_cards,
            action_prompt: state.action_prompt,
        }
    }

    fn acting_player_id(table: &PokerTable) -> Option<PlayerId> {
        match table.current_step() {
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => table
                .hand_state()
                .betting()
                .map(|betting| betting.current_player()),
            HandStep::DrawPhase => table.hand_state().draw().map(|draw| draw.current_player()),
            _ => None,
        }
    }

    fn variant_for_table(table: &PokerTable) -> PokerVariant {
        let spec = table.variant_spec();
        if spec == variants::texas_holdem::SPEC {
            PokerVariant::TexasHoldEm
        } else if spec == variants::seven_card_stud::SPEC {
            PokerVariant::SevenCardStud
        } else {
            PokerVariant::FiveCardDraw
        }
    }

    fn ensure_no_error(events: &[Event]) -> AppResult<()> {
        if let Some(message) = events.iter().find_map(|event| match event {
            Event::Error { message } => Some(message.clone()),
            _ => None,
        }) {
            return Err(ErrorPayload::new(ErrorCode::GameError, message));
        }

        Ok(())
    }

    fn history_entry(index: u64, event: &Event) -> Option<HistoryEntry> {
        let (kind, message, showdown_results, payout_results) = match event {
            Event::Ok => return None,
            Event::Info { message } => (HistoryKind::Info, message.clone(), None, None),
            Event::Error { message } => (HistoryKind::Error, message.clone(), None, None),
            Event::StepChanged { step } => (
                HistoryKind::StepChanged,
                match step{
                    HandStep::WaitingForPlayers => "Phase: Waiting for Players".to_string(),
                    HandStep::CollectForcedBets => "Phase: Collecting Antes".to_string(),
                    HandStep::DealCommunity => "Phase: Dealing Community Cards".to_string(),
                    HandStep::DealPrivate => "Phase: Dealing Private Cards".to_string(),
                    HandStep::DealFaceUp => "Phase: Dealing Face-Up Cards".to_string(),
                    HandStep::BettingRound1 => "Phase: Betting Round 1".to_string(),
                    HandStep::DrawPhase => "Phase: Draw Phase".to_string(),
                    HandStep::BettingRound2 => "Phase: Betting Round 2".to_string(),
                    HandStep::BettingRound3 => "Phase: Betting Round 3".to_string(),
                    HandStep::BettingRound4 => "Phase: Betting Round 4".to_string(),
                    HandStep::BettingRound5 => "Phase: Betting Round 5".to_string(),
                    HandStep::Showdown => "Phase: Showdown".to_string(),
                    HandStep::Payout => "Phase: Payout".to_string(),
                },
                None,
                None,
            ),
            Event::PlayerJoined {
                player_name, seat, ..
            } => (
                HistoryKind::PlayerJoined,
                format!("{player_name} joined seat {seat}"),
                None,
                None,
            ),
            Event::PlayerLeft { player_name, .. } => (
                HistoryKind::PlayerLeft,
                format!("{player_name} left the table"),
                None,
                None,
            ),
            Event::PotUpdated { pot } => (
                HistoryKind::PotUpdated,
                format!("Pot is now {pot}"),
                None,
                None,
            ),
            Event::ActionRequested { player_name, .. } => (
                HistoryKind::ActionPrompt,
                format!("{player_name} to act"),
                None,
                None,
            ),
            Event::DrawRequested { player_name, .. } => (
                HistoryKind::DrawPrompt,
                format!("{player_name} to draw/discard"),
                None,
                None,
            ),
            Event::PrivateHand { .. } => return None,
            Event::Showdown { results } => (
                HistoryKind::Showdown,
                format!("Showdown: {} result(s)", results.len()),
                Some(results.clone()),
                None,
            ),
            Event::Payout { winners } => (
                HistoryKind::Payout,
                format!("Payout: {} winner(s)", winners.len()),
                None,
                Some(winners.clone()),
            ),
            Event::LobbyState { seated } => (
                HistoryKind::Info,
                format!("Lobby State Updated: {} seated", seated.len()),
                None,
                None,
            ),
        };

        Some(HistoryEntry {
            index,
            kind,
            message,
            showdown_results,
            payout_results,
        })
    }

    fn history_entries(events: &[Event]) -> Vec<HistoryEntry> {
        events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| Self::history_entry(index as u64, event))
            .collect()
    }

    fn table_change(table_id: TableId, events: Vec<Event>) -> TableChangeNotice {
        TableChangeNotice { table_id, events }
    }

    fn visible_table_changes_for_player(&self, player_id: PlayerId) -> Vec<TableChangeNotice> {
        self.table_handles()
            .into_iter()
            .filter_map(|(table_id, handle)| {
                let snapshot = handle.snapshot().ok()?;
                snapshot
                    .table
                    .seated_players()
                    .iter()
                    .any(|(_, seated_id)| *seated_id == player_id)
                    .then(|| Self::table_change(table_id, Vec::new()))
            })
            .collect()
    }

    fn to_action_cmd(player_name: &str, action: PlayerAction) -> ActionCmd {
        match action {
            PlayerAction::Check => ActionCmd::Check {
                player_name: player_name.to_string(),
            },
            PlayerAction::Bet { amount } => ActionCmd::Bet {
                player_name: player_name.to_string(),
                amount,
            },
            PlayerAction::Call => ActionCmd::Call {
                player_name: player_name.to_string(),
            },
            PlayerAction::Raise { amount } => ActionCmd::Raise {
                player_name: player_name.to_string(),
                amount,
            },
            PlayerAction::Fold => ActionCmd::Fold {
                player_name: player_name.to_string(),
            },
            PlayerAction::Draw { discard } => ActionCmd::Draw {
                player_name: player_name.to_string(),
                discard,
            },
        }
    }

    fn table_not_found(table_id: TableId) -> ErrorPayload {
        ErrorPayload::new(
            ErrorCode::TableNotFound,
            format!("Table {table_id} was not found"),
        )
    }

    fn table_worker_error(table_id: TableId, error: TableWorkerError) -> ErrorPayload {
        ErrorPayload::new(
            ErrorCode::Internal,
            format!("Table {table_id} worker error: {error}"),
        )
    }

    fn auth_error(prefix: &str, error: AccountError) -> ErrorPayload {
        ErrorPayload::new(
            ErrorCode::AuthenticationFailed,
            format!("{prefix}: {error}"),
        )
    }

    fn account_error(prefix: &str, error: AccountError) -> ErrorPayload {
        ErrorPayload::new(ErrorCode::AccountError, format!("{prefix}: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, tempdir};

    struct TestFixture {
        _tmp: TempDir,
        app: ServerApp,
    }

    fn test_app() -> TestFixture {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("accounts.json");
        let store = AccountStore::load(path).expect("load account store");
        TestFixture {
            _tmp: dir,
            app: ServerApp::new(store),
        }
    }

    #[test]
    fn shared_api_account_and_login_flow() {
        let fx = test_app();

        let account = fx
            .app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create account");
        assert_eq!(account.player_name, "alice");

        let login = fx.app.login("alice".into(), "1234".into()).expect("login");
        assert_eq!(login.account.player_name, "alice");
        assert!(login.session_id.starts_with("session-"));

        let updated = fx.app.deposit(&login.session_id, 25).expect("deposit");
        assert_eq!(updated.balance, 525);
    }

    #[test]
    fn shared_api_table_state_and_player_view() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");
        let login = fx.app.login("alice".into(), "1234".into()).expect("login");

        let table = fx.app.list_tables().tables[0].clone();
        let joined = fx
            .app
            .join_table(&login.session_id, table.table_id)
            .expect("join table");
        assert_eq!(joined.status, SeatAssignmentStatus::Seated);

        let state = fx.app.get_table_state(table.table_id).expect("table state");
        assert_eq!(state.table_id, table.table_id);
        assert_eq!(state.seats.len(), 1);

        let view = fx
            .app
            .get_player_view(&login.session_id, table.table_id)
            .expect("player view");
        assert_eq!(view.player_name, "alice");
        assert_eq!(view.seat, Some(0));
    }

    #[test]
    fn shared_api_relogin_invalidates_old_session() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");

        let first = fx
            .app
            .login("alice".into(), "1234".into())
            .expect("first login");
        let second = fx
            .app
            .login("alice".into(), "1234".into())
            .expect("second login");

        assert_ne!(first.session_id, second.session_id);

        let stale = fx
            .app
            .deposit(&first.session_id, 10)
            .expect_err("stale session should fail");
        assert_eq!(stale.code, ErrorCode::SessionNotFound);

        let fresh = fx
            .app
            .deposit(&second.session_id, 10)
            .expect("fresh session should work");
        assert_eq!(fresh.balance, 510);
    }

    #[test]
    fn shared_api_logout_invalidates_session_and_leaves_waiting_table() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");
        let login = fx.app.login("alice".into(), "1234".into()).expect("login");

        let table = fx.app.list_tables().tables[0].clone();
        fx.app
            .join_table(&login.session_id, table.table_id)
            .expect("join table");

        let logout = fx.app.logout(&login.session_id).expect("logout");
        assert_eq!(logout.session_id, login.session_id);

        let stale = fx
            .app
            .deposit(&login.session_id, 10)
            .expect_err("logged out session should fail");
        assert_eq!(stale.code, ErrorCode::SessionNotFound);

        let state = fx.app.get_table_state(table.table_id).expect("table state");
        assert!(state.seats.is_empty());
    }

    #[test]
    fn shared_api_create_table_and_history_filter_private_hands() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");
        fx.app
            .create_account("bob".into(), "1234".into(), 500)
            .expect("create bob");

        let alice = fx
            .app
            .login("alice".into(), "1234".into())
            .expect("login alice");
        let bob = fx
            .app
            .login("bob".into(), "1234".into())
            .expect("login bob");

        let table = fx
            .app
            .create_table(&alice.session_id, PokerVariant::TexasHoldEm)
            .expect("create table");
        fx.app
            .join_table(&alice.session_id, table.table_id)
            .expect("join alice");
        fx.app
            .join_table(&bob.session_id, table.table_id)
            .expect("join bob");

        let _ = fx
            .app
            .start_hand(&alice.session_id, table.table_id)
            .expect("start hand");
        let history = fx.app.get_history(table.table_id).expect("history");
        assert!(
            history
                .entries
                .iter()
                .all(|entry| !entry.message.contains("hand(")),
            "private hand events should not be exposed via public history"
        );
    }

    #[test]
    fn showdown_events_keep_structured_results_in_history_and_updates() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");
        let login = fx.app.login("alice".into(), "1234".into()).expect("login");
        let table = fx.app.list_tables().tables[0].clone();
        fx.app
            .join_table(&login.session_id, table.table_id)
            .expect("join table");

        let showdown = Event::Showdown {
            results: vec![
                common::protocol::ShowdownLine {
                    player_name: "alice".into(),
                    player_id: 1,
                    hand_desc: "A♠ K♠ => Flush".into(),
                },
                common::protocol::ShowdownLine {
                    player_name: "bob".into(),
                    player_id: 2,
                    hand_desc: "K♥ K♦ => Pair".into(),
                },
            ],
        };

        let history_entry = ServerApp::history_entry(5, &showdown).expect("history entry");
        assert_eq!(history_entry.kind, HistoryKind::Showdown);
        assert_eq!(history_entry.message, "showdown: 2 result(s)");
        let results = history_entry
            .showdown_results
            .as_ref()
            .expect("showdown results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].player_name, "alice");
        assert!(results[0].hand_desc.contains("Flush"));

        let update = fx
            .app
            .build_table_update_for_session(table.table_id, Some(&login.session_id), &[showdown])
            .expect("table update");
        assert_eq!(update.events.len(), 1);
        assert_eq!(update.events[0].kind, HistoryKind::Showdown);
        let update_results = update.events[0]
            .showdown_results
            .as_ref()
            .expect("update showdown results");
        assert_eq!(update_results.len(), 2);
        assert_eq!(update_results[1].player_name, "bob");
    }

    #[test]
    fn payout_events_keep_structured_results_in_history_and_updates() {
        let fx = test_app();
        fx.app
            .create_account("alice".into(), "1234".into(), 500)
            .expect("create alice");
        let login = fx.app.login("alice".into(), "1234".into()).expect("login");
        let table = fx.app.list_tables().tables[0].clone();
        fx.app
            .join_table(&login.session_id, table.table_id)
            .expect("join table");

        let payout = Event::Payout {
            winners: vec![
                common::protocol::PayoutLine {
                    player_name: "alice".into(),
                    player_id: 1,
                    amount: 60,
                },
                common::protocol::PayoutLine {
                    player_name: "bob".into(),
                    player_id: 2,
                    amount: 40,
                },
            ],
        };

        let history_entry = ServerApp::history_entry(6, &payout).expect("history entry");
        assert_eq!(history_entry.kind, HistoryKind::Payout);
        assert_eq!(history_entry.message, "payout: 2 winner(s)");
        let results = history_entry
            .payout_results
            .as_ref()
            .expect("payout results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].player_id, 1);
        assert_eq!(results[1].amount, 40);

        let update = fx
            .app
            .build_table_update_for_session(table.table_id, Some(&login.session_id), &[payout])
            .expect("table update");
        assert_eq!(update.events.len(), 1);
        assert_eq!(update.events[0].kind, HistoryKind::Payout);
        let update_results = update.events[0]
            .payout_results
            .as_ref()
            .expect("update payout results");
        assert_eq!(update_results.len(), 2);
        assert_eq!(update_results[1].player_id, 2);
    }
}

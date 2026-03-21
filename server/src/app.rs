use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use common::{
    protocol::{AccountCmd, ActionCmd, Command, Event, HandStep, LobbyCmd, PlayerId, PokerVariant},
    wire::{
        AccountProfile, ClientRequest, ErrorCode, ErrorPayload, HistoryEntry, HistoryKind,
        JoinTableResult, LeaveTableResult, LeaveTableStatus, LoginResult, OperationReceipt,
        PlayerAction, PlayerView, PublicSeatState, PublicTableState, RequestOp, ResponsePayload,
        SeatAssignmentStatus, ServerResponse, TableHistory, TableId, TableList, TableSummary,
    },
};

use crate::{
    account_store::{AccountError, AccountStore},
    accounts::Account,
    engine::SharedAccountStore,
    sessions::{Session, SessionStore},
    table_runtime::TableRuntime,
    variants,
};

pub type AppResult<T> = Result<T, ErrorPayload>;

const DEFAULT_TABLE_ID: TableId = 1;

pub struct ServerApp {
    pub accounts: SharedAccountStore,
    sessions: SessionStore,
    pub tables: HashMap<TableId, TableRuntime>,
    default_table_id: TableId,
    next_table_id: TableId,
}

impl ServerApp {
    pub fn new(store: AccountStore) -> Self {
        let accounts = Arc::new(Mutex::new(store));
        let default_table_id = DEFAULT_TABLE_ID;
        let mut tables = HashMap::new();
        tables.insert(default_table_id, TableRuntime::new(Arc::clone(&accounts)));

        Self {
            accounts,
            sessions: SessionStore::new(),
            tables,
            default_table_id,
            next_table_id: default_table_id + 1,
        }
    }

    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::Account(account_cmd) => self.apply_account_cmd(account_cmd),
            other => self.apply_to_table(self.default_table_id, other),
        }
    }

    pub fn apply_to_table(&mut self, table_id: TableId, cmd: Command) -> Vec<Event> {
        match self.tables.get_mut(&table_id) {
            Some(runtime) => runtime.apply(cmd),
            None => vec![Event::Error {
                message: format!("Table {table_id} not found"),
            }],
        }
    }

    pub fn create_account(
        &mut self,
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

    pub fn login(&mut self, name: String, pin: String) -> AppResult<LoginResult> {
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .login(&name, &pin)
            .map_err(|e| Self::auth_error("Login failed", e))?;

        let session = self.sessions.create_session(&account, &pin);
        Ok(LoginResult {
            session_id: session.session_id,
            account: Self::account_profile(&account),
        })
    }

    pub fn deposit(&mut self, session_id: &str, amount: i64) -> AppResult<AccountProfile> {
        let session = self.require_session(session_id)?;
        let account = self
            .accounts
            .lock()
            .expect("account store lock poisoned")
            .deposit(&session.player_name, amount)
            .map_err(|e| Self::account_error("Deposit failed", e))?;
        Ok(Self::account_profile(&account))
    }

    pub fn withdraw(&mut self, session_id: &str, amount: i64) -> AppResult<AccountProfile> {
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
            .tables
            .iter()
            .map(|(table_id, runtime)| self.table_summary(*table_id, runtime))
            .collect();
        tables.sort_by_key(|table| table.table_id);
        TableList { tables }
    }

    pub fn create_table(
        &mut self,
        session_id: &str,
        variant: PokerVariant,
    ) -> AppResult<TableSummary> {
        let _session = self.require_session(session_id)?;

        let table_id = self.next_table_id;
        self.next_table_id = self.next_table_id.saturating_add(1);

        let mut runtime = TableRuntime::new(Arc::clone(&self.accounts));
        runtime
            .engine
            .table
            .set_variant_spec(variants::spec_for(variant));

        let summary = self.table_summary(table_id, &runtime);
        self.tables.insert(table_id, runtime);
        Ok(summary)
    }

    pub fn join_table(
        &mut self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<JoinTableResult> {
        let session = self.require_session(session_id)?;
        let runtime = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        let events = runtime.apply(Command::Lobby(LobbyCmd::Join {
            player_name: session.player_name.clone(),
            pin: session.pin.clone(),
        }));
        Self::ensure_no_error(&events)?;

        let seat = events.iter().find_map(|event| match event {
            Event::PlayerJoined {
                player_name, seat, ..
            } if player_name == &session.player_name => Some(*seat),
            _ => None,
        });

        Ok(JoinTableResult {
            table_id,
            status: SeatAssignmentStatus::Seated,
            seat,
        })
    }

    pub fn leave_table(
        &mut self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<LeaveTableResult> {
        let session = self.require_session(session_id)?;
        let runtime = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        let immediate_leave = runtime.engine.table.current_step() == HandStep::WaitingForPlayers;
        let events = runtime.apply(Command::Lobby(LobbyCmd::Leave {
            player_name: session.player_name.clone(),
        }));
        Self::ensure_no_error(&events)?;

        let status = if immediate_leave {
            LeaveTableStatus::Left
        } else {
            LeaveTableStatus::LeavingAfterHand
        };

        Ok(LeaveTableResult { table_id, status })
    }

    pub fn start_hand(
        &mut self,
        session_id: &str,
        table_id: TableId,
    ) -> AppResult<OperationReceipt> {
        let session = self.require_session(session_id)?;
        let runtime = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        Self::require_player_seated(runtime, session.account_id, &session.player_name)?;

        let events = runtime.apply(Command::Lobby(LobbyCmd::StartHand));
        Self::ensure_no_error(&events)?;
        Ok(OperationReceipt {
            table_id,
            event_count: events.len(),
        })
    }

    pub fn get_table_state(&self, table_id: TableId) -> AppResult<PublicTableState> {
        let runtime = self
            .tables
            .get(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;
        Ok(self.build_public_table_state(table_id, runtime))
    }

    pub fn get_player_view(&self, session_id: &str, table_id: TableId) -> AppResult<PlayerView> {
        let session = self.require_session(session_id)?;
        let runtime = self
            .tables
            .get(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        Self::require_player_seated(runtime, session.account_id, &session.player_name)?;
        Ok(self.build_player_view(table_id, runtime, session.account_id, &session.player_name))
    }

    pub fn submit_action(
        &mut self,
        session_id: &str,
        table_id: TableId,
        action: PlayerAction,
    ) -> AppResult<OperationReceipt> {
        let session = self.require_session(session_id)?;
        let runtime = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        Self::require_player_seated(runtime, session.account_id, &session.player_name)?;
        let command = Command::Action(Self::to_action_cmd(&session.player_name, action));
        let events = runtime.apply(command);
        Self::ensure_no_error(&events)?;

        Ok(OperationReceipt {
            table_id,
            event_count: events.len(),
        })
    }

    pub fn get_history(&self, table_id: TableId) -> AppResult<TableHistory> {
        let runtime = self
            .tables
            .get(&table_id)
            .ok_or_else(|| Self::table_not_found(table_id))?;

        let entries = runtime
            .history()
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

    pub fn handle_request(&mut self, request: ClientRequest) -> ServerResponse {
        let request_id = request.request_id.clone();
        match self.dispatch_request(request) {
            Ok(payload) => ServerResponse::ok(request_id, payload),
            Err(error) => ServerResponse::error(request_id, error),
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

    fn dispatch_request(&mut self, request: ClientRequest) -> AppResult<ResponsePayload> {
        let session_id = request.session_id.as_deref();

        match request.op {
            RequestOp::CreateAccount { name, pin, amount } => self
                .create_account(name, pin, amount)
                .map(ResponsePayload::Account),
            RequestOp::Login { name, pin } => self.login(name, pin).map(ResponsePayload::Login),
            RequestOp::Deposit { amount } => {
                let session_id = Self::require_session_id(session_id)?;
                self.deposit(session_id, amount)
                    .map(ResponsePayload::Account)
            }
            RequestOp::Withdraw { amount } => {
                let session_id = Self::require_session_id(session_id)?;
                self.withdraw(session_id, amount)
                    .map(ResponsePayload::Account)
            }
            RequestOp::ListTables => Ok(ResponsePayload::Tables(self.list_tables())),
            RequestOp::CreateTable { variant } => {
                let session_id = Self::require_session_id(session_id)?;
                self.create_table(session_id, variant)
                    .map(ResponsePayload::Table)
            }
            RequestOp::JoinTable { table_id } => {
                let session_id = Self::require_session_id(session_id)?;
                self.join_table(session_id, table_id)
                    .map(ResponsePayload::JoinTable)
            }
            RequestOp::LeaveTable { table_id } => {
                let session_id = Self::require_session_id(session_id)?;
                self.leave_table(session_id, table_id)
                    .map(ResponsePayload::LeaveTable)
            }
            RequestOp::StartHand { table_id } => {
                let session_id = Self::require_session_id(session_id)?;
                self.start_hand(session_id, table_id)
                    .map(ResponsePayload::Receipt)
            }
            RequestOp::GetTableState { table_id } => self
                .get_table_state(table_id)
                .map(ResponsePayload::TableState),
            RequestOp::GetPlayerView { table_id } => {
                let session_id = Self::require_session_id(session_id)?;
                self.get_player_view(session_id, table_id)
                    .map(ResponsePayload::PlayerView)
            }
            RequestOp::SubmitAction { table_id, action } => {
                let session_id = Self::require_session_id(session_id)?;
                self.submit_action(session_id, table_id, action)
                    .map(ResponsePayload::Receipt)
            }
            RequestOp::GetHistory { table_id } => {
                self.get_history(table_id).map(ResponsePayload::History)
            }
        }
    }

    fn require_session(&self, session_id: &str) -> AppResult<Session> {
        self.sessions.get(session_id).cloned().ok_or_else(|| {
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
        runtime: &TableRuntime,
        player_id: PlayerId,
        player_name: &str,
    ) -> AppResult<()> {
        let is_seated = runtime
            .engine
            .table
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

    fn table_summary(&self, table_id: TableId, runtime: &TableRuntime) -> TableSummary {
        TableSummary {
            table_id,
            variant: Self::variant_for_runtime(runtime),
            step: runtime.engine.table.current_step(),
            seated_count: runtime.engine.table.seated_players().len(),
            max_players: runtime.engine.table.variant_spec().max_players,
        }
    }

    fn build_public_table_state(
        &self,
        table_id: TableId,
        runtime: &TableRuntime,
    ) -> PublicTableState {
        let accounts = self.account_map();
        let hand_state = runtime.engine.table.hand_state();

        let seats = runtime
            .engine
            .table
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
                    is_in_hand: hand_state.player_hole_cards(&player_id).is_some(),
                    has_folded: hand_state.has_player_folded(&player_id),
                    hidden_card_count: total_cards.saturating_sub(up_cards.len()),
                    up_cards,
                })
            })
            .collect();

        PublicTableState {
            table_id,
            variant: Self::variant_for_runtime(runtime),
            step: runtime.engine.table.current_step(),
            dealer_seat: runtime.engine.table.dealer_position(),
            acting_player_id: Self::acting_player_id(runtime),
            pot: hand_state.pot_size() as i64,
            current_bet: hand_state.current_bet() as i64,
            board_cards: hand_state.board_cards().to_vec(),
            seats,
        }
    }

    fn build_player_view(
        &self,
        table_id: TableId,
        runtime: &TableRuntime,
        player_id: PlayerId,
        player_name: &str,
    ) -> PlayerView {
        let hand_state = runtime.engine.table.hand_state();
        let seat = runtime
            .engine
            .table
            .seated_players()
            .into_iter()
            .find_map(|(seat, seated_id)| (seated_id == player_id).then_some(seat));

        PlayerView {
            table_id,
            player_id,
            player_name: player_name.to_string(),
            seat,
            private_cards: hand_state
                .player_hole_cards(&player_id)
                .cloned()
                .unwrap_or_default(),
            up_cards: hand_state
                .player_up_cards(&player_id)
                .cloned()
                .unwrap_or_default(),
            board_cards: hand_state.board_cards().to_vec(),
            action_prompt: runtime.engine.action_prompt_for_player(player_id),
        }
    }

    fn acting_player_id(runtime: &TableRuntime) -> Option<PlayerId> {
        match runtime.engine.table.current_step() {
            HandStep::BettingRound1
            | HandStep::BettingRound2
            | HandStep::BettingRound3
            | HandStep::BettingRound4
            | HandStep::BettingRound5 => runtime
                .engine
                .table
                .hand_state()
                .betting()
                .map(|betting| betting.current_player()),
            HandStep::DrawPhase => runtime
                .engine
                .table
                .hand_state()
                .draw()
                .map(|draw| draw.current_player()),
            _ => None,
        }
    }

    fn variant_for_runtime(runtime: &TableRuntime) -> PokerVariant {
        let spec = runtime.engine.table.variant_spec();
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
        let (kind, message) = match event {
            Event::Ok => {
                return None;
            }
            Event::Info { message } => (HistoryKind::Info, message.clone()),
            Event::Error { message } => (HistoryKind::Error, message.clone()),
            Event::StepChanged { step } => (
                HistoryKind::StepChanged,
                format!("step changed to {:?}", step),
            ),
            Event::PlayerJoined {
                player_name, seat, ..
            } => (
                HistoryKind::PlayerJoined,
                format!("{player_name} joined seat {seat}"),
            ),
            Event::PlayerLeft { player_name, .. } => (
                HistoryKind::PlayerLeft,
                format!("{player_name} left the table"),
            ),
            Event::PotUpdated { pot } => (HistoryKind::PotUpdated, format!("pot updated to {pot}")),
            Event::ActionRequested { player_name, .. } => (
                HistoryKind::ActionPrompt,
                format!("action requested for {player_name}"),
            ),
            Event::DrawRequested { player_name, .. } => (
                HistoryKind::DrawPrompt,
                format!("draw requested for {player_name}"),
            ),
            Event::PrivateHand { .. } => {
                return None;
            }
            Event::Showdown { results } => (
                HistoryKind::Showdown,
                format!("showdown: {} result(s)", results.len()),
            ),
            Event::Payout { winners } => (
                HistoryKind::Payout,
                format!("payout: {} winner(s)", winners.len()),
            ),
            Event::LobbyState { seated } => (
                HistoryKind::Info,
                format!("lobby state updated: {} seated", seated.len()),
            ),
        };

        Some(HistoryEntry {
            index,
            kind,
            message,
        })
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
        let mut fx = test_app();

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
        let mut fx = test_app();
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
    fn shared_api_create_table_and_history_filter_private_hands() {
        let mut fx = test_app();
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
}

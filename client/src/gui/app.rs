use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender};

use common::protocol::{AnimationStreet, AnimationTarget, HandStep, PokerVariant};
use common::wire::{
    AccountProfile, AnimationCue, HistoryEntry, HistoryKind, LeaveTableStatus, PlayerView,
    PublicTableState, RequestOp, ResponsePayload, SeatAssignmentStatus, SessionId, TableId,
    TableSummary, TableUpdateMessage,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableViewMode {
    Seated,
    Spectating,
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
    ChangeVariant,
    SitOut,
    SitIn,
}

// ---------------------------------------------------------------------------
// GuiCommand — sent from the GUI to the network worker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum GuiCommand {
    SendOp(RequestOp),
}

#[derive(Debug, Clone)]
/// Holds an incoming server update while the UI animates toward it.
///
/// The final table state, player view, and history entries are only committed once every cue has
/// been played so the on-screen state stays internally consistent.
struct DealAnimationState {
    target_table_state: PublicTableState,
    target_player_view: Option<PlayerView>,
    pending_events: Vec<HistoryEntry>,
    cues: Vec<AnimationCue>,
    cue_index: usize,
    cue_started_at: f64,
}

impl DealAnimationState {
    fn current_cue(&self) -> Option<&AnimationCue> {
        self.cues.get(self.cue_index)
    }
}

// ---------------------------------------------------------------------------
// PokerApp
// ---------------------------------------------------------------------------

/// Top-level GUI state machine for the client crate.
///
/// `PokerApp` bridges three timelines:
/// - immediate user intent from egui
/// - request/response traffic coming back from the network worker
/// - push-driven table updates that may need to be animated before they are committed
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
    pub table_view_mode: Option<TableViewMode>,
    pub table_state: Option<PublicTableState>,
    pub player_view: Option<PlayerView>,
    pub event_log: Vec<HistoryEntry>,
    queued_table_updates: VecDeque<TableUpdateMessage>,
    active_deal_animation: Option<DealAnimationState>,

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

    // Small-screen guidance can be dismissed while the viewport remains cramped.
    pub small_screen_notice_dismissed: bool,
    // Tracks whether the local player has requested to sit out the next hand.
    pub sit_out_pending: bool,
    // Tracks whether the hand rankings reference window is currently open.
    pub show_hand_rankings: bool,
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
            table_view_mode: None,
            table_state: None,
            player_view: None,
            event_log: Vec::new(),
            queued_table_updates: VecDeque::new(),
            active_deal_animation: None,
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
            small_screen_notice_dismissed: false,
            sit_out_pending: false,
            show_hand_rankings: false,
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
        self.table_view_mode = None;
        self.screen = Screen::Lobby;
        self.last_error = None;
        self.send_op(PendingKind::ListTables, RequestOp::ListTables);
    }

    pub fn open_table(&mut self, table_id: TableId, mode: TableViewMode) {
        self.table_view_mode = Some(mode);
        self.reset_table_view_state();
        self.last_error = None;
        self.screen = Screen::Table { table_id };
    }

    pub fn watch_table(&mut self, table_id: TableId) {
        self.open_table(table_id, TableViewMode::Spectating);
        self.send_op(
            PendingKind::Subscribe,
            RequestOp::SubscribeTable { table_id },
        );
    }

    pub fn leave_spectated_table(&mut self, table_id: TableId) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(GuiCommand::SendOp(RequestOp::UnsubscribeTable { table_id }));
        }
        self.reset_table_view_state();
        self.enter_lobby();
    }

    /// Queues one operation onto the network worker.
    ///
    /// The current client intentionally serializes requests through a single sender thread, so the
    /// GUI only needs coarse "something is in flight" tracking here.
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

    pub fn is_deal_animation_active(&self) -> bool {
        self.active_deal_animation.is_some()
    }

    pub fn is_action_locked(&self) -> bool {
        self.is_request_pending() || self.is_deal_animation_active()
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

    /// Handles solicited request/response traffic.
    ///
    /// Push-driven table changes still arrive on `push_rx`; this method is mainly responsible for
    /// navigation, optimistic follow-up requests, and syncing point-in-time response payloads into
    /// the local view model.
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
            // Authentication/account responses shape the top-level session state.
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
                self.table_view_mode = None;
                self.screen = Screen::Login;
            }
            // Lobby navigation lives off direct responses because it does not need per-cue
            // animation like table updates do.
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
                match result.status {
                    SeatAssignmentStatus::Seated => {
                        self.open_table(table_id, TableViewMode::Seated);
                        // Subscribe then fetch initial state — these go through the channel
                        // in order without blocking the UI.
                        if let Some(tx) = &self.cmd_tx {
                            let _ =
                                tx.send(GuiCommand::SendOp(RequestOp::SubscribeTable { table_id }));
                            let _ =
                                tx.send(GuiCommand::SendOp(RequestOp::GetTableState { table_id }));
                            let _ =
                                tx.send(GuiCommand::SendOp(RequestOp::GetPlayerView { table_id }));
                        }
                    }
                    SeatAssignmentStatus::Queued => {
                        self.last_error = Some(
                            "Table is full — you are queued and will be seated when a spot opens."
                                .to_string(),
                        );
                    }
                }
            }
            Some(ResponsePayload::LeaveTable(result)) => match result.status {
                LeaveTableStatus::Left => {
                    self.reset_table_view_state();
                    self.enter_lobby();
                }
                LeaveTableStatus::LeavingAfterHand => {
                    self.last_error = Some(
                        "You will leave the table automatically after this hand finishes."
                            .to_string(),
                    );
                }
            },
            // Direct state fetches are authoritative snapshots and can be committed immediately.
            Some(ResponsePayload::TableState(state)) => {
                self.table_state = Some(state);
            }
            Some(ResponsePayload::PlayerView(view)) => {
                self.table_view_mode = Some(TableViewMode::Seated);
                self.player_view = Some(view);
            }
            _ => {}
        }
    }

    /// Handles unsolicited transport and subscription events.
    fn handle_push_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::TableUpdate(update) => {
                if update.table_state.step == HandStep::CollectForcedBets {
                    self.last_showdown = None;
                    self.last_payout = None;
                }
                self.queued_table_updates.push_back(update);
            }
            ClientEvent::SessionClosed(msg) => {
                self.session_id = None;
                self.account = None;
                self.table_view_mode = None;
                self.reset_table_view_state();
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
                self.table_view_mode = None;
                self.reset_table_view_state();
                self.status_msg = "Connection lost — please reconnect".to_string();
                self.screen = Screen::Connect;
            }
            ClientEvent::TransportError(msg) => {
                self.connected = false;
                self.cmd_tx = None;
                self.resp_rx = None;
                self.push_rx = None;
                self.table_view_mode = None;
                self.reset_table_view_state();
                self.status_msg = format!("Connection error: {msg}");
                self.screen = Screen::Connect;
            }
            ClientEvent::UnexpectedResponse(_) => {}
        }
    }

    fn apply_table_update_immediately(&mut self, update: TableUpdateMessage) {
        self.commit_table_update(update.table_state, update.player_view, update.events);
    }

    fn commit_table_update(
        &mut self,
        table_state: PublicTableState,
        player_view: Option<PlayerView>,
        events: Vec<HistoryEntry>,
    ) {
        self.sync_account_balance(&table_state);
        self.table_state = Some(table_state);
        if let Some(view) = player_view {
            self.selected_discards.clear();
            self.input_bet.clear();
            self.table_view_mode = Some(TableViewMode::Seated);
            self.player_view = Some(view);
        }
        for entry in &events {
            match entry.kind {
                HistoryKind::Showdown => self.last_showdown = Some(entry.clone()),
                HistoryKind::Payout => self.last_payout = Some(entry.clone()),
                _ => {}
            }
        }
        self.event_log.extend(events);

        // Detect a completed deferred leave: we were seated but the server no longer lists us
        // in any seat after the hand finished and applied pending leave requests.
        if self.table_view_mode == Some(TableViewMode::Seated) {
            let my_player_id = self.account.as_ref().map(|a| a.player_id);
            if let Some(my_id) = my_player_id {
                let still_seated = self
                    .table_state
                    .as_ref()
                    .map(|s| s.seats.iter().any(|seat| seat.player_id == my_id))
                    .unwrap_or(true);
                if !still_seated {
                    self.reset_table_view_state();
                    self.enter_lobby();
                }
            }
        }
    }

    fn sync_account_balance(&mut self, table_state: &PublicTableState) {
        if let Some(account) = &mut self.account
            && let Some(seat) = table_state
                .seats
                .iter()
                .find(|seat| seat.player_id == account.player_id)
        {
            account.balance = seat.balance;
        }
    }

    /// Applies queued table updates one animation cue at a time.
    ///
    /// The server sends authoritative end-state snapshots plus optional deal cues. We rewind to a
    /// safe starting point, animate toward that end state, then commit the history entries only
    /// once the visual transition is complete.
    fn update_deal_animation(&mut self, now: f64) {
        loop {
            if let Some(animation) = &self.active_deal_animation {
                // A cue is already in progress: either finish it, advance to the next cue, or
                // commit the final target state once the sequence is exhausted.
                let Some(cue) = animation.current_cue().cloned() else {
                    let finished = self.active_deal_animation.take().expect("animation exists");
                    self.commit_table_update(
                        finished.target_table_state,
                        finished.target_player_view,
                        finished.pending_events,
                    );
                    continue;
                };

                let elapsed = now - animation.cue_started_at;
                if elapsed < Self::cue_duration_seconds(&cue) {
                    break;
                }

                let target_table_state = animation.target_table_state.clone();
                let target_player_view = animation.target_player_view.clone();
                self.apply_animation_cue(&cue, &target_table_state, target_player_view.as_ref());

                let mut finished = false;
                if let Some(animation) = &mut self.active_deal_animation {
                    animation.cue_index += 1;
                    if animation.cue_index >= animation.cues.len() {
                        finished = true;
                    } else {
                        animation.cue_started_at = now;
                    }
                }

                if finished {
                    let finished = self.active_deal_animation.take().expect("animation exists");
                    self.commit_table_update(
                        finished.target_table_state,
                        finished.target_player_view,
                        finished.pending_events,
                    );
                    continue;
                }
            } else {
                // No animation is active, so pull the next queued server update and either apply
                // it immediately or convert it into a staged animation sequence.
                let Some(update) = self.queued_table_updates.pop_front() else {
                    break;
                };

                if update.animation_cues.is_empty() {
                    self.apply_table_update_immediately(update);
                    continue;
                }

                let (base_table_state, base_player_view) = self.animation_base_for_update(&update);
                self.table_state = Some(base_table_state);
                self.player_view = base_player_view;
                if self.player_view.is_some() {
                    self.table_view_mode = Some(TableViewMode::Seated);
                }
                self.active_deal_animation = Some(DealAnimationState {
                    target_table_state: update.table_state,
                    target_player_view: update.player_view,
                    pending_events: update.events,
                    cues: update.animation_cues,
                    cue_index: 0,
                    cue_started_at: now,
                });
                break;
            }
        }
    }

    /// Chooses the state we should animate from for an incoming update.
    ///
    /// If we already have a matching live snapshot for this table/viewer, we animate from that.
    /// Otherwise we reconstruct the pre-deal baseline by rewinding the target snapshot with the
    /// provided animation cues.
    fn animation_base_for_update(
        &self,
        update: &TableUpdateMessage,
    ) -> (PublicTableState, Option<PlayerView>) {
        let table_state = match &self.table_state {
            Some(state) if state.table_id == update.table_id => state.clone(),
            _ => Self::rewind_table_state(&update.table_state, &update.animation_cues),
        };

        let player_view = match (&self.player_view, &update.player_view) {
            (Some(view), Some(target))
                if view.table_id == target.table_id && view.player_id == target.player_id =>
            {
                Some(view.clone())
            }
            (_, Some(target)) => Some(Self::rewind_player_view(target, &update.animation_cues)),
            _ => None,
        };

        (table_state, player_view)
    }

    /// Reconstructs the pre-animation public table state by removing the dealt cards in reverse
    /// cue order from the authoritative target snapshot.
    fn rewind_table_state(target: &PublicTableState, cues: &[AnimationCue]) -> PublicTableState {
        let mut state = target.clone();

        for cue in cues.iter().rev() {
            match cue.target {
                AnimationTarget::Board => {
                    if let Some(index) = cue.target_index
                        && index < state.board_cards.len()
                    {
                        state.board_cards.remove(index);
                    }
                }
                AnimationTarget::Seat => {
                    let Some(seat_index) = cue.seat else {
                        continue;
                    };
                    let Some(seat) = state.seats.iter_mut().find(|seat| seat.seat == seat_index)
                    else {
                        continue;
                    };

                    match cue.street {
                        AnimationStreet::FiveCardDrawInitial
                        | AnimationStreet::HoldemPreflop
                        | AnimationStreet::StudThirdDown
                        | AnimationStreet::StudSeventh => {
                            seat.hidden_card_count = seat.hidden_card_count.saturating_sub(1);
                        }
                        AnimationStreet::StudThirdUp
                        | AnimationStreet::StudFourth
                        | AnimationStreet::StudFifth
                        | AnimationStreet::StudSixth => {
                            if let Some(index) = cue.target_index {
                                if index < seat.up_cards.len() {
                                    seat.up_cards.remove(index);
                                }
                            } else {
                                let _ = seat.up_cards.pop();
                            }
                        }
                        AnimationStreet::FiveCardDrawReplacement => {}
                        AnimationStreet::HoldemFlop
                        | AnimationStreet::HoldemTurn
                        | AnimationStreet::HoldemRiver => {}
                    }

                    seat.is_in_hand = seat.hidden_card_count > 0 || !seat.up_cards.is_empty();
                }
            }
        }

        state
    }

    /// Reconstructs the seated player's private view before the incoming animation plays.
    fn rewind_player_view(target: &PlayerView, cues: &[AnimationCue]) -> PlayerView {
        let mut view = target.clone();

        for cue in cues.iter().rev() {
            match cue.target {
                AnimationTarget::Board => {
                    if let Some(index) = cue.target_index
                        && index < view.board_cards.len()
                    {
                        view.board_cards.remove(index);
                    }
                }
                AnimationTarget::Seat => {
                    if cue.card.is_none() || cue.seat != view.seat {
                        continue;
                    }

                    match cue.street {
                        AnimationStreet::FiveCardDrawReplacement => {}
                        AnimationStreet::FiveCardDrawInitial
                        | AnimationStreet::HoldemPreflop
                        | AnimationStreet::StudThirdDown
                        | AnimationStreet::StudSeventh => {
                            if let Some(index) = cue.target_index
                                && index < view.private_cards.len()
                            {
                                view.private_cards.remove(index);
                            }
                        }
                        AnimationStreet::StudThirdUp
                        | AnimationStreet::StudFourth
                        | AnimationStreet::StudFifth
                        | AnimationStreet::StudSixth => {
                            if let Some(index) = cue.target_index
                                && index < view.up_cards.len()
                            {
                                view.up_cards.remove(index);
                            }
                            let _ = view.private_cards.pop();
                        }
                        AnimationStreet::HoldemFlop
                        | AnimationStreet::HoldemTurn
                        | AnimationStreet::HoldemRiver => {}
                    }
                }
            }
        }

        view
    }

    /// Applies one cue to the live UI projection without committing the full update yet.
    fn apply_animation_cue(
        &mut self,
        cue: &AnimationCue,
        target_table_state: &PublicTableState,
        target_player_view: Option<&PlayerView>,
    ) {
        match cue.target {
            AnimationTarget::Board => {
                if let Some(card) = cue.card
                    && let Some(state) = &mut self.table_state
                    && let Some(index) = cue.target_index
                    && index <= state.board_cards.len()
                    && state.board_cards.len() < target_table_state.board_cards.len()
                {
                    state.board_cards.insert(index, card);
                }

                if let Some(card) = cue.card
                    && let Some(view) = &mut self.player_view
                    && let Some(index) = cue.target_index
                    && index <= view.board_cards.len()
                    && let Some(target_view) = target_player_view
                    && view.board_cards.len() < target_view.board_cards.len()
                {
                    view.board_cards.insert(index, card);
                }
            }
            AnimationTarget::Seat => {
                // First update the public seat projection that everyone sees.
                if let Some(state) = &mut self.table_state
                    && let Some(seat_index) = cue.seat
                    && let Some(seat) = state.seats.iter_mut().find(|seat| seat.seat == seat_index)
                    && let Some(target_seat) = target_table_state
                        .seats
                        .iter()
                        .find(|target_seat| target_seat.seat == seat_index)
                {
                    match cue.street {
                        AnimationStreet::FiveCardDrawInitial
                        | AnimationStreet::HoldemPreflop
                        | AnimationStreet::StudThirdDown
                        | AnimationStreet::StudSeventh => {
                            if seat.hidden_card_count < target_seat.hidden_card_count {
                                seat.hidden_card_count += 1;
                            }
                        }
                        AnimationStreet::StudThirdUp
                        | AnimationStreet::StudFourth
                        | AnimationStreet::StudFifth
                        | AnimationStreet::StudSixth => {
                            if let Some(card) = cue.card
                                && seat.up_cards.len() < target_seat.up_cards.len()
                            {
                                let index = cue.target_index.unwrap_or(seat.up_cards.len());
                                if index <= seat.up_cards.len() {
                                    seat.up_cards.insert(index, card);
                                }
                            }
                        }
                        AnimationStreet::FiveCardDrawReplacement => {}
                        AnimationStreet::HoldemFlop
                        | AnimationStreet::HoldemTurn
                        | AnimationStreet::HoldemRiver => {}
                    }

                    seat.is_in_hand = target_seat.is_in_hand
                        || seat.hidden_card_count > 0
                        || !seat.up_cards.is_empty();
                }

                // Then, if this cue belongs to the local user, mirror the matching private cards
                // into the seat owner's personal view.
                if let Some(card) = cue.card
                    && let Some(view) = &mut self.player_view
                    && cue.seat == view.seat
                {
                    match cue.street {
                        AnimationStreet::FiveCardDrawReplacement => {
                            if let Some(index) = cue.target_index {
                                if index < view.private_cards.len() {
                                    view.private_cards[index] = card;
                                } else if index == view.private_cards.len() {
                                    view.private_cards.push(card);
                                }
                            }
                        }
                        AnimationStreet::FiveCardDrawInitial
                        | AnimationStreet::HoldemPreflop
                        | AnimationStreet::StudThirdDown
                        | AnimationStreet::StudSeventh => {
                            let index = cue.target_index.unwrap_or(view.private_cards.len());
                            if index <= view.private_cards.len() {
                                view.private_cards.insert(index, card);
                            }
                        }
                        AnimationStreet::StudThirdUp
                        | AnimationStreet::StudFourth
                        | AnimationStreet::StudFifth
                        | AnimationStreet::StudSixth => {
                            let up_index = cue.target_index.unwrap_or(view.up_cards.len());
                            if up_index <= view.up_cards.len() {
                                view.up_cards.insert(up_index, card);
                            }
                            if let Some(target_view) = target_player_view
                                && view.private_cards.len() < target_view.private_cards.len()
                            {
                                view.private_cards.push(card);
                            }
                        }
                        AnimationStreet::HoldemFlop
                        | AnimationStreet::HoldemTurn
                        | AnimationStreet::HoldemRiver => {}
                    }
                }
            }
        }
    }

    fn cue_duration_seconds(cue: &AnimationCue) -> f64 {
        match cue.target {
            AnimationTarget::Board => 0.24,
            AnimationTarget::Seat => match cue.street {
                AnimationStreet::FiveCardDrawReplacement => 0.16,
                _ => 0.18,
            },
        }
    }

    pub fn current_animation_overlay(&self, now: f64) -> Option<(AnimationCue, f32)> {
        let animation = self.active_deal_animation.as_ref()?;
        let cue = animation.current_cue()?.clone();
        let duration = Self::cue_duration_seconds(&cue).max(f64::EPSILON);
        let progress = ((now - animation.cue_started_at) / duration).clamp(0.0, 1.0) as f32;
        Some((cue, progress))
    }
}

impl eframe::App for PokerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        super::theme::apply_theme(ctx);
        self.drain_channels();
        self.update_deal_animation(ctx.input(|input| input.time));

        if self.is_deal_animation_active() || !self.queued_table_updates.is_empty() {
            ctx.request_repaint();
        }

        match self.screen.clone() {
            Screen::Connect => super::screens::connect::show(self, ctx),
            Screen::Login => super::screens::login::show(self, ctx),
            Screen::Lobby => super::screens::lobby::show(self, ctx),
            Screen::Table { table_id } => {
                super::screens::table::show(self, ctx, table_id);
            }
        }

        super::theme::show_small_screen_notice(ctx, &mut self.small_screen_notice_dismissed);
    }
}

impl PokerApp {
    fn reset_table_view_state(&mut self) {
        self.table_state = None;
        self.player_view = None;
        self.event_log.clear();
        self.last_showdown = None;
        self.last_payout = None;
        self.queued_table_updates.clear();
        self.active_deal_animation = None;
        self.selected_discards.clear();
        self.input_bet.clear();
        self.sit_out_pending = false;
        self.show_hand_rankings = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::card_deck::{Card, Rank, Suit};
    use common::protocol::{AllowedAction, AnimationFace};

    fn sample_table_state(step: HandStep, hidden_card_count: usize) -> PublicTableState {
        PublicTableState {
            table_id: 1,
            variant: PokerVariant::TexasHoldEm,
            step,
            dealer_seat: 0,
            acting_player_id: Some(7),
            pot: 15,
            current_bet: 5,
            board_cards: vec![],
            seats: vec![common::wire::PublicSeatState {
                seat: 0,
                player_id: 7,
                player_name: "alice".into(),
                balance: 495,
                is_connected: true,
                is_in_hand: hidden_card_count > 0,
                has_folded: false,
                hidden_card_count,
                up_cards: vec![],
            }],
        }
    }

    fn sample_player_view(private_cards: Vec<Card>) -> PlayerView {
        PlayerView {
            table_id: 1,
            player_id: 7,
            player_name: "alice".into(),
            seat: Some(0),
            private_cards,
            up_cards: vec![],
            board_cards: vec![],
            action_prompt: Some(common::wire::ActionPrompt {
                allowed_actions: vec![AllowedAction::Call],
                call_amount: Some(5),
                min_raise: None,
                max_amount: None,
                max_discards: None,
            }),
        }
    }

    fn sample_history_entry(index: u64, message: &str) -> HistoryEntry {
        HistoryEntry {
            index,
            kind: HistoryKind::Info,
            message: message.to_string(),
            showdown_results: None,
            payout_results: None,
        }
    }

    fn sample_animation_update() -> TableUpdateMessage {
        TableUpdateMessage {
            table_id: 1,
            table_state: sample_table_state(HandStep::BettingRound1, 1),
            player_view: Some(sample_player_view(vec![Card::new(Rank::Ace, Suit::Spades)])),
            events: vec![sample_history_entry(1, "alice to act")],
            animation_cues: vec![AnimationCue {
                street: AnimationStreet::HoldemPreflop,
                target: AnimationTarget::Seat,
                seat: Some(0),
                face: AnimationFace::Down,
                card: Some(Card::new(Rank::Ace, Suit::Spades)),
                target_index: Some(0),
            }],
        }
    }

    #[test]
    fn table_update_with_animation_cues_waits_for_commit() {
        let mut app = PokerApp::default();
        app.table_state = Some(sample_table_state(HandStep::WaitingForPlayers, 0));
        app.player_view = Some(sample_player_view(vec![]));

        app.handle_push_event(ClientEvent::TableUpdate(sample_animation_update()));
        assert_eq!(
            app.table_state.as_ref().map(|state| state.step),
            Some(HandStep::WaitingForPlayers)
        );

        app.update_deal_animation(0.0);
        assert!(app.is_deal_animation_active());
        assert_eq!(app.event_log.len(), 0);
        assert_eq!(
            app.table_state
                .as_ref()
                .map(|state| state.hidden_card_count_for_test()),
            Some(0)
        );

        app.update_deal_animation(1.0);
        assert!(!app.is_deal_animation_active());
        assert_eq!(
            app.table_state.as_ref().map(|state| state.step),
            Some(HandStep::BettingRound1)
        );
        assert_eq!(app.event_log.len(), 1);
        assert_eq!(
            app.player_view
                .as_ref()
                .map(|view| view.private_cards.len()),
            Some(1)
        );
    }

    #[test]
    fn action_lock_stays_active_while_animation_is_running() {
        let mut app = PokerApp::default();
        app.table_state = Some(sample_table_state(HandStep::WaitingForPlayers, 0));
        app.player_view = Some(sample_player_view(vec![]));

        app.handle_push_event(ClientEvent::TableUpdate(sample_animation_update()));
        app.update_deal_animation(0.0);
        assert!(app.is_action_locked());

        app.update_deal_animation(1.0);
        assert!(!app.is_deal_animation_active());
        assert!(!app.is_request_pending());
    }

    #[test]
    fn table_update_without_animation_cues_applies_immediately() {
        let mut app = PokerApp::default();
        let update = TableUpdateMessage {
            table_id: 1,
            table_state: sample_table_state(HandStep::BettingRound1, 0),
            player_view: Some(sample_player_view(vec![])),
            events: vec![sample_history_entry(2, "alice checked")],
            animation_cues: vec![],
        };

        app.handle_push_event(ClientEvent::TableUpdate(update));
        app.update_deal_animation(0.0);

        assert!(!app.is_deal_animation_active());
        assert_eq!(
            app.table_state.as_ref().map(|state| state.step),
            Some(HandStep::BettingRound1)
        );
        assert_eq!(app.event_log.len(), 1);
    }

    trait TestSeatCount {
        fn hidden_card_count_for_test(&self) -> usize;
    }

    impl TestSeatCount for PublicTableState {
        fn hidden_card_count_for_test(&self) -> usize {
            self.seats
                .iter()
                .find(|seat| seat.seat == 0)
                .map(|seat| seat.hidden_card_count)
                .unwrap_or(0)
        }
    }
}

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use common::{
    card_deck::Card,
    protocol::{Command, Event, PlayerId},
    wire::ActionPrompt,
};

use crate::{engine::SharedAccountStore, table::PokerTable, table_runtime::TableRuntime};

#[derive(Debug, Clone)]
/// Cloneable handle to a dedicated table worker thread.
///
/// The worker owns all mutable table/runtime state so the rest of the server can query or mutate
/// a table without sharing that state across threads.
pub struct TableHandle {
    tx: Sender<TableRequest>,
}

#[derive(Debug, Clone)]
pub struct TableSnapshot {
    pub table: PokerTable,
    pub disconnected_players: HashSet<PlayerId>,
}

#[derive(Debug, Clone)]
pub struct PlayerViewState {
    pub is_seated: bool,
    pub seat: Option<usize>,
    pub private_cards: Vec<Card>,
    pub up_cards: Vec<Card>,
    pub board_cards: Vec<Card>,
    pub action_prompt: Option<ActionPrompt>,
}

#[derive(Debug)]
pub enum TableWorkerError {
    Closed,
}

impl std::fmt::Display for TableWorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableWorkerError::Closed => write!(f, "table worker is unavailable"),
        }
    }
}

impl std::error::Error for TableWorkerError {}

type TableWorkerResult<T> = Result<T, TableWorkerError>;

enum TableRequest {
    Apply {
        cmd: Command,
        reply_tx: Sender<Vec<Event>>,
    },
    JoinAuthenticated {
        player_id: PlayerId,
        player_name: String,
        reply_tx: Sender<Vec<Event>>,
    },
    DisconnectPlayer {
        player_id: PlayerId,
        player_name: String,
        reply_tx: Sender<Vec<Event>>,
    },
    Snapshot {
        reply_tx: Sender<TableSnapshot>,
    },
    History {
        reply_tx: Sender<Vec<Event>>,
    },
    PlayerView {
        player_id: PlayerId,
        player_name: String,
        reply_tx: Sender<PlayerViewState>,
    },
}

impl TableHandle {
    pub fn new(accounts: SharedAccountStore) -> Self {
        Self::new_with_runtime(TableRuntime::new(accounts))
    }

    pub fn new_with_runtime(runtime: TableRuntime) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || worker_loop(runtime, rx));
        Self { tx }
    }

    pub fn apply(&self, cmd: Command) -> TableWorkerResult<Vec<Event>> {
        self.request(|reply_tx| TableRequest::Apply { cmd, reply_tx })
    }

    pub fn join_authenticated_player(
        &self,
        player_id: PlayerId,
        player_name: String,
    ) -> TableWorkerResult<Vec<Event>> {
        self.request(|reply_tx| TableRequest::JoinAuthenticated {
            player_id,
            player_name,
            reply_tx,
        })
    }

    pub fn snapshot(&self) -> TableWorkerResult<TableSnapshot> {
        self.request(|reply_tx| TableRequest::Snapshot { reply_tx })
    }

    pub fn disconnect_player(
        &self,
        player_id: PlayerId,
        player_name: String,
    ) -> TableWorkerResult<Vec<Event>> {
        self.request(|reply_tx| TableRequest::DisconnectPlayer {
            player_id,
            player_name,
            reply_tx,
        })
    }

    pub fn history(&self) -> TableWorkerResult<Vec<Event>> {
        self.request(|reply_tx| TableRequest::History { reply_tx })
    }

    pub fn player_view_state(
        &self,
        player_id: PlayerId,
        player_name: String,
    ) -> TableWorkerResult<PlayerViewState> {
        self.request(|reply_tx| TableRequest::PlayerView {
            player_id,
            player_name,
            reply_tx,
        })
    }

    fn request<T>(&self, build: impl FnOnce(Sender<T>) -> TableRequest) -> TableWorkerResult<T> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(build(reply_tx))
            .map_err(|_| TableWorkerError::Closed)?;
        reply_rx.recv().map_err(|_| TableWorkerError::Closed)
    }
}

/// Serializes all table access for one table instance.
///
/// The server can freely clone `TableHandle`, but every mutation still funnels through this loop
/// so game state, history, and disconnect bookkeeping stay consistent.
fn worker_loop(mut runtime: TableRuntime, rx: Receiver<TableRequest>) {
    let mut disconnected_players = HashSet::new();

    while let Ok(request) = rx.recv() {
        match request {
            TableRequest::Apply { cmd, reply_tx } => {
                let events = runtime.apply(cmd);
                prune_disconnected_players(&runtime, &mut disconnected_players);
                let _ = reply_tx.send(events);
            }
            TableRequest::JoinAuthenticated {
                player_id,
                player_name,
                reply_tx,
            } => {
                let events = runtime
                    .engine
                    .join_authenticated_player(player_id, player_name);
                runtime.record_events(&events);
                disconnected_players.remove(&player_id);
                prune_disconnected_players(&runtime, &mut disconnected_players);
                let _ = reply_tx.send(events);
            }
            TableRequest::DisconnectPlayer {
                player_id,
                player_name,
                reply_tx,
            } => {
                // A transport disconnect is treated like a leave attempt. If the engine cannot
                // remove the seat yet, we keep the player marked as disconnected in snapshots.
                let is_seated = runtime
                    .engine
                    .table
                    .seated_players()
                    .iter()
                    .any(|(_, seated_id)| *seated_id == player_id);

                let events = if is_seated {
                    runtime.apply(Command::Lobby(common::protocol::LobbyCmd::Leave {
                        player_name,
                    }))
                } else {
                    Vec::new()
                };

                let still_seated = runtime
                    .engine
                    .table
                    .seated_players()
                    .iter()
                    .any(|(_, seated_id)| *seated_id == player_id);
                if still_seated {
                    disconnected_players.insert(player_id);
                } else {
                    disconnected_players.remove(&player_id);
                }

                prune_disconnected_players(&runtime, &mut disconnected_players);
                let _ = reply_tx.send(events);
            }
            TableRequest::Snapshot { reply_tx } => {
                let _ = reply_tx.send(TableSnapshot {
                    table: runtime.engine.table.clone(),
                    disconnected_players: disconnected_players.clone(),
                });
            }
            TableRequest::History { reply_tx } => {
                let _ = reply_tx.send(runtime.history().to_vec());
            }
            TableRequest::PlayerView {
                player_id,
                player_name,
                reply_tx,
            } => {
                let hand_state = runtime.engine.table.hand_state();
                let seat = runtime
                    .engine
                    .table
                    .seated_players()
                    .into_iter()
                    .find_map(|(seat, seated_id)| (seated_id == player_id).then_some(seat));

                let state = PlayerViewState {
                    is_seated: seat.is_some(),
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
                    action_prompt: runtime
                        .engine
                        .action_prompt_for_player_named(player_id, &player_name),
                };

                let _ = reply_tx.send(state);
            }
        }
    }
}

/// Removes stale disconnect markers once a player fully leaves the table.
fn prune_disconnected_players(
    runtime: &TableRuntime,
    disconnected_players: &mut HashSet<PlayerId>,
) {
    let seated_players: HashSet<PlayerId> = runtime
        .engine
        .table
        .seated_players()
        .into_iter()
        .map(|(_, player_id)| player_id)
        .collect();
    disconnected_players.retain(|player_id| seated_players.contains(player_id));
}

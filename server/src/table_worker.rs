use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use common::{
    card_deck::Card,
    protocol::{Command, Event, PlayerId},
    wire::ActionPrompt,
};

use crate::{engine::SharedAccountStore, table::PokerTable, table_runtime::TableRuntime};

#[derive(Debug, Clone)]
pub struct TableHandle {
    tx: Sender<TableRequest>,
}

#[derive(Debug, Clone)]
pub struct TableSnapshot {
    pub table: PokerTable,
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

fn worker_loop(mut runtime: TableRuntime, rx: Receiver<TableRequest>) {
    while let Ok(request) = rx.recv() {
        match request {
            TableRequest::Apply { cmd, reply_tx } => {
                let events = runtime.apply(cmd);
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
                let _ = reply_tx.send(events);
            }
            TableRequest::Snapshot { reply_tx } => {
                let _ = reply_tx.send(TableSnapshot {
                    table: runtime.engine.table.clone(),
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

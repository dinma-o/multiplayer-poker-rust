use common::protocol::{Command, Event};

use crate::{
    engine::{GameEngine, SharedAccountStore},
    table::PokerTable,
};

/// Mutable per-table runtime owned by a single worker thread.
///
/// This keeps the game engine and the emitted event history together so snapshots, history
/// requests, and state mutations all refer to the same serialized timeline.
pub struct TableRuntime {
    pub engine: GameEngine,
    history: Vec<Event>,
}

impl TableRuntime {
    pub fn new(accounts: SharedAccountStore) -> Self {
        Self::new_with_table(accounts, PokerTable::new())
    }

    pub fn new_with_table(accounts: SharedAccountStore, table: PokerTable) -> Self {
        Self {
            engine: GameEngine::new(accounts, table),
            history: Vec::new(),
        }
    }

    /// Applies a command and appends the emitted events to the table-local history in the same
    /// order subscribers observed them.
    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        let events = self.engine.apply(cmd);
        self.record_events(&events);
        events
    }

    pub fn record_events(&mut self, events: &[Event]) {
        self.history.extend(events.iter().cloned());
    }

    #[allow(dead_code)]
    pub fn history(&self) -> &[Event] {
        &self.history
    }
}

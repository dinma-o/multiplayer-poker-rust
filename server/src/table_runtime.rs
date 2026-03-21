use common::protocol::{Command, Event};

use crate::{
    engine::{GameEngine, SharedAccountStore},
    table::PokerTable,
};

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

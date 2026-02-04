use common::{card_deck::Deck, protocol::{Command, Event}};

use crate::{account_store::AccountStore, table::FiveCardTable};

pub struct ServerEngine {
    pub accounts: AccountStore,
    pub table: FiveCardTable,
    pub deck: Deck,
}

impl ServerEngine {
    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        // stub first, then implement command-by-command
        vec![Event::Error { message: "unimplemented".into() }]
    }
}

use std::collections::HashMap;

use crate::accounts::Account;
use common::wire::SessionId;

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: SessionId,
    pub account_id: u64,
    pub player_name: String,
    // Temporary bridge for reusing the current engine join flow.
    pub pin: String,
}

#[derive(Debug, Default)]
pub struct SessionStore {
    next_id: u64,
    sessions: HashMap<SessionId, Session>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            sessions: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, account: &Account, pin: &str) -> Session {
        let session_id = format!("session-{}-{}", account.id, self.next_id);
        self.next_id = self.next_id.saturating_add(1);

        let session = Session {
            session_id: session_id.clone(),
            account_id: account.id,
            player_name: account.name.clone(),
            pin: pin.to_string(),
        };
        self.sessions.insert(session_id, session.clone());
        session
    }

    pub fn get(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }
}

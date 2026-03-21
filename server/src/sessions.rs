use std::collections::HashMap;

use crate::accounts::Account;
use common::wire::SessionId;

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: SessionId,
    pub account_id: u64,
    pub player_name: String,
}

#[derive(Debug, Default)]
pub struct SessionStore {
    next_id: u64,
    sessions: HashMap<SessionId, Session>,
    session_by_account: HashMap<u64, SessionId>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            sessions: HashMap::new(),
            session_by_account: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, account: &Account) -> Session {
        if let Some(previous_session_id) = self.session_by_account.remove(&account.id) {
            self.sessions.remove(&previous_session_id);
        }

        let session_id = format!("session-{}-{}", account.id, self.next_id);
        self.next_id = self.next_id.saturating_add(1);

        let session = Session {
            session_id: session_id.clone(),
            account_id: account.id,
            player_name: account.name.clone(),
        };
        self.sessions.insert(session_id, session.clone());
        self.session_by_account
            .insert(account.id, session.session_id.clone());
        session
    }

    pub fn get(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn remove(&mut self, session_id: &str) -> Option<Session> {
        let session = self.sessions.remove(session_id)?;
        if self.session_by_account.get(&session.account_id) == Some(&session.session_id) {
            self.session_by_account.remove(&session.account_id);
        }
        Some(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_session_replaces_existing_session_for_same_account() {
        let account = Account::new(7, "alice", "1234", 500);
        let mut store = SessionStore::new();

        let first = store.create_session(&account);
        let second = store.create_session(&account);

        assert_ne!(first.session_id, second.session_id);
        assert!(store.get(&first.session_id).is_none());
        assert_eq!(
            store
                .get(&second.session_id)
                .map(|session| session.account_id),
            Some(account.id)
        );
    }

    #[test]
    fn remove_deletes_session_and_reverse_lookup() {
        let account = Account::new(8, "bob", "9999", 250);
        let mut store = SessionStore::new();
        let session = store.create_session(&account);

        let removed = store.remove(&session.session_id).expect("remove session");
        assert_eq!(removed.account_id, account.id);
        assert!(store.get(&session.session_id).is_none());

        let replacement = store.create_session(&account);
        assert_ne!(replacement.session_id, session.session_id);
        assert!(store.get(&replacement.session_id).is_some());
    }
}

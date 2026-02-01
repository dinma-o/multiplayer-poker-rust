//! Account storage and authentication service.
//!
//! This module provides server-side account management backed by a JSON file.
//! Responsibilities include:
//! - Loading and saving accounts to persistent storage
//! - Creating new accounts with uniqueness guarantees
//! - Authenticating accounts via name + PIN
//!
//! This module does NOT handle UI, CLI input, or game logic. It is intended
//! to be called by higher-level application layers.

use crate::accounts::{Account, AccountId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Errors that can occur during account creation, login, or persistence.
#[derive(Debug)]
pub enum AccountError {
    /// An account with the given name already exists.
    NameTaken,
    /// PIN does not meet format requirements.
    InvalidPinFormat,
    /// No account exists with the given identifier.
    AccountNotFound,
    /// PIN does not match the stored PIN for the account.
    InvalidPin,
    /// Underlying filesystem error.
    Io(std::io::Error),
    /// JSON serialization or deserialization error.
    Json(serde_json::Error),
}

impl From<std::io::Error> for AccountError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for AccountError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Serializable on-disk representation of the account store.
///
/// This wrapper allows the JSON format to evolve without directly coupling
/// it to the in-memory `AccountStore` layout.
#[derive(Debug, Serialize, Deserialize)]
struct StoreData {
    accounts: Vec<Account>,
}

/// In-memory account store with JSON-backed persistence.
///
/// All mutations are immediately written to disk.
/// ID generation is managed internally to ensure uniqueness.
#[derive(Debug)]
pub struct AccountStore {
    path: PathBuf,
    accounts: Vec<Account>,
    next_id: AccountId,
}

impl AccountStore {
    /// Loads accounts from a JSON file if it exists.
    ///
    /// If the file does not exist or is empty, an empty store is created.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, AccountError> {
        let path = path.into();

        let accounts = if Path::new(&path).exists() {
            let contents = fs::read_to_string(&path)?;
            if contents.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str::<StoreData>(&contents)?.accounts
            }
        } else {
            Vec::new()
        };

        // Generate the next ID based on existing accounts
        let next_id = accounts
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        Ok(Self {
            path,
            accounts,
            next_id,
        })
    }

    /// Persists the current store state to disk.
    ///
    /// This is called after every successful mutation.
    fn save(&self) -> Result<(), AccountError> {
        let data = StoreData {
            accounts: self.accounts.clone(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(&self.path, json)?;
        Ok(())
    }

    /// Creates a new account with a unique name and generated ID.
    ///
    /// Fails if the name already exists or the PIN format is invalid.
    pub fn create_account(
        &mut self,
        name: impl Into<String>,
        pin: &str,
        initial_balance: i64,
    ) -> Result<Account, AccountError> {
        let name = name.into();

        if self.accounts.iter().any(|a| a.name == name) {
            return Err(AccountError::NameTaken);
        }

        if !is_valid_pin(pin) {
            return Err(AccountError::InvalidPinFormat);
        }

        let account = Account::new(self.next_id, name, pin.to_string(), initial_balance);

        self.next_id = self.next_id.saturating_add(1);
        self.accounts.push(account.clone());
        self.save()?;

        Ok(account)
    }

    /// Authenticates an account using name and PIN.
    ///
    /// Returns a cloned account on success.
    pub fn login(&self, name: &str, pin: &str) -> Result<Account, AccountError> {
        if !is_valid_pin(pin) {
            return Err(AccountError::InvalidPinFormat);
        }

        let account = self
            .accounts
            .iter()
            .find(|a| a.name == name)
            .ok_or(AccountError::AccountNotFound)?;

        if account.pin != pin {
            return Err(AccountError::InvalidPin);
        }

        Ok(account.clone())
    }

    /// Returns a read-only view of all stored accounts.
    ///
    /// Intended for debugging, testing, or future admin features.
    pub fn list_accounts(&self) -> &[Account] {
        &self.accounts
    }
}

/// Validates PIN format.
///
/// Current rule: exactly four numeric digits.
/// This is intentionally isolated to make policy changes easy.
fn is_valid_pin(pin: &str) -> bool {
    pin.len() == 4 && pin.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_and_login_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        let created = store.create_account("alice", "1234", 1000).unwrap();

        let logged_in = store.login("alice", "1234").unwrap();
        assert_eq!(created.id, logged_in.id);
        assert_eq!(logged_in.name, "alice");
    }

    #[test]
    fn reject_invalid_pin_format_on_create() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        let err = store.create_account("alice", "12ab", 0).unwrap_err();
        matches!(err, AccountError::InvalidPinFormat);
    }

    #[test]
    fn reject_invalid_pin_on_login() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 0).unwrap();

        let err = store.login("alice", "9999").unwrap_err();
        matches!(err, AccountError::InvalidPin);
    }

    #[test]
    fn enforce_unique_name() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 0).unwrap();

        let err = store.create_account("alice", "0000", 0).unwrap_err();
        matches!(err, AccountError::NameTaken);
    }

    #[test]
    fn loads_from_json_and_allows_login_after_restart() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        {
            let mut store = AccountStore::load(&path).unwrap();
            store.create_account("alice", "1234", 100).unwrap();
        }

        let store2 = AccountStore::load(&path).unwrap();
        let logged_in = store2.login("alice", "1234").unwrap();
        assert_eq!(logged_in.balance, 100);
    }
}
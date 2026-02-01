//! Account storage and authentication service.
//!
//! This module provides server-side account management backed by a JSON file.
//! Responsibilities include:
//! - Loading and saving accounts to persistent storage
//! - Creating new accounts with uniqueness guarantees
//! - Authenticating accounts via name + PIN
//! - Deposit/withdraw with validation and persistence
//!
//! This module does NOT handle UI, CLI input, or game logic. It is intended
//! to be called by higher-level application layers.

use crate::accounts::{Account, AccountId};
use serde::{Deserialize, Serialize};
use std::fmt;
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
    /// Amount must be positive and within safe bounds.
    InvalidAmount,
    /// Withdraw would make balance negative.
    InsufficientFunds,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountError::NameTaken => write!(f, "account name already exists"),
            AccountError::InvalidPinFormat => write!(f, "invalid PIN format"),
            AccountError::AccountNotFound => write!(f, "account not found"),
            AccountError::InvalidPin => write!(f, "invalid PIN"),
            AccountError::InvalidAmount => write!(f, "invalid amount"),
            AccountError::InsufficientFunds => write!(f, "insufficient funds"),
            AccountError::Io(e) => write!(f, "I/O error: {e}"),
            AccountError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for AccountError {}

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

    /// Deposits a positive amount into the account balance and persists the update.
    pub fn deposit(&mut self, name: &str, amount: i64) -> Result<Account, AccountError> {
        if amount <= 0 {
            return Err(AccountError::InvalidAmount);
        }
        // mutate in a short scope, clone result, then save
        let updated = {
            let account = self
                .accounts
                .iter_mut()
                .find(|a| a.name == name)
                .ok_or(AccountError::AccountNotFound)?;

            account.balance = account
                .balance
                .checked_add(amount)
                .ok_or(AccountError::InvalidAmount)?;

            account.clone()
        }; // <- mutable borrow ends here

        self.save()?;
        Ok(updated)
    }

    /// Withdraws a positive amount from the account balance if sufficient funds exist.
    ///
    /// This operation will not allow the balance to become negative.
    pub fn withdraw(&mut self, name: &str, amount: i64) -> Result<Account, AccountError> {
        if amount <= 0 {
            return Err(AccountError::InvalidAmount);
        }

        let updated = {
            let account = self
                .accounts
                .iter_mut()
                .find(|a| a.name == name)
                .ok_or(AccountError::AccountNotFound)?;

            if amount > account.balance {
                return Err(AccountError::InsufficientFunds);
            }

            account.balance = account
                .balance
                .checked_sub(amount)
                .ok_or(AccountError::InvalidAmount)?;

            account.clone()
        }; // <- mutable borrow ends here

        self.save()?;
        Ok(updated)
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
    #[test]
    fn deposit_increases_balance() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 100).unwrap();

        let updated = store.deposit("alice", 50).unwrap();
        assert_eq!(updated.balance, 150);
    }

    #[test]
    fn withdraw_decreases_balance() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 100).unwrap();

        let updated = store.withdraw("alice", 40).unwrap();
        assert_eq!(updated.balance, 60);
    }

    #[test]
    fn withdraw_cannot_go_negative() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 30).unwrap();

        let err = store.withdraw("alice", 31).unwrap_err();
        assert!(matches!(err, AccountError::InsufficientFunds));
    }

    #[test]
    fn reject_invalid_amounts() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 30).unwrap();

        assert!(matches!(
            store.deposit("alice", 0).unwrap_err(),
            AccountError::InvalidAmount
        ));
        assert!(matches!(
            store.withdraw("alice", -5).unwrap_err(),
            AccountError::InvalidAmount
        ));
    }

    #[test]
    fn error_then_success_has_no_stale_error_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.create_account("alice", "1234", 30).unwrap();

        let err = store.withdraw("alice", 999).unwrap_err();
        assert!(matches!(err, AccountError::InsufficientFunds));

        let ok = store.deposit("alice", 10).unwrap();
        assert_eq!(ok.balance, 40);
    }
}


mod accounts;

mod account_store;
mod table;
mod dealer;
mod game_states;
mod variants;

use accounts::Account;
use std::fs;
use std::path::Path;

use crate::account_store::AccountStore;

const ACCOUNTS_PATH: &str = "server/data/accounts.json";

fn main() {
    // Ensure the data directory exists (fresh clones won't have it).
    if let Some(parent) = Path::new(ACCOUNTS_PATH).parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    // Load at startup.
    let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");

    let mut a = Account::new(1, "demo", "1234", 1000);
    a.record_game_played();
    println!("{}", a.summary());

    // Save on exit (in addition to save-on-change).
    store.flush().expect("flush accounts");
}

mod account_store;
mod accounts;
mod app;
mod cli;
mod dealer;
mod engine;
mod game_states;
mod sessions;
mod table;
mod table_runtime;
mod variants;

use std::fs;
use std::path::Path;

use crate::account_store::AccountStore;
use crate::app::ServerApp;

const ACCOUNTS_PATH: &str = "server/server/data/accounts.json";

fn main() {
    if let Some(parent) = Path::new(ACCOUNTS_PATH).parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");
    let mut app = ServerApp::new(store);

    cli::run(&mut app);

    app.flush_accounts().expect("flush accounts");
}

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
mod table_worker;
mod transport;
mod variants;

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::account_store::AccountStore;
use crate::app::ServerApp;

const ACCOUNTS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/accounts.json");

fn main() {
    if let Some(parent) = Path::new(ACCOUNTS_PATH).parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    let mode = env::args().nth(1);

    match mode.as_deref() {
        Some("serve") | Some("tcp") => {
            let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");
            let app = Arc::new(ServerApp::new(store));
            if let Err(error) = transport::tcp::serve(Arc::clone(&app)) {
                eprintln!("tcp server error: {error}");
            }

            app.flush_accounts().expect("flush accounts");
        }
        Some("cli") | None => {
            let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");
            let mut app = ServerApp::new(store);
            cli::run(&mut app);
            app.flush_accounts().expect("flush accounts");
        }
        Some(other) => {
            eprintln!("unknown mode '{other}'. use 'cli' or 'serve'.");
        }
    }
}

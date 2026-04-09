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
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::account_store::AccountStore;
use crate::app::ServerApp;

fn main() {
    let accounts_path = accounts_path();

    if let Some(parent) = accounts_path.parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    let mode = env::args().nth(1);

    match mode.as_deref() {
        Some("serve") | Some("tcp") => {
            let store = AccountStore::load(&accounts_path).expect("load accounts");
            let app = Arc::new(ServerApp::new(store));
            if let Err(error) = transport::tcp::serve(Arc::clone(&app)) {
                eprintln!("tcp server error: {error}");
            }

            app.flush_accounts().expect("flush accounts");
        }
        Some("cli") | None => {
            let store = AccountStore::load(&accounts_path).expect("load accounts");
            let mut app = ServerApp::new(store);
            cli::run(&mut app);
            app.flush_accounts().expect("flush accounts");
        }
        Some(other) => {
            eprintln!("unknown mode '{other}'. use 'cli' or 'serve'.");
        }
    }
}

fn accounts_path() -> PathBuf {
    if let Ok(path) = env::var("POKER_ACCOUNTS_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    let repo_relative = PathBuf::from("server").join("data").join("accounts.json");
    if repo_relative.parent().is_some_and(Path::exists) {
        return repo_relative;
    }

    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("data").join("accounts.json")))
        .unwrap_or_else(|| PathBuf::from("data").join("accounts.json"))
}

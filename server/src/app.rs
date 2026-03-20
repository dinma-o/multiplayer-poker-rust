use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use common::protocol::{AccountCmd, Command, Event};

use crate::{
    account_store::{AccountError, AccountStore},
    engine::SharedAccountStore,
    table_runtime::TableRuntime,
};

pub type TableId = u64;

const DEFAULT_TABLE_ID: TableId = 1;

pub struct ServerApp {
    pub accounts: SharedAccountStore,
    pub tables: HashMap<TableId, TableRuntime>,
    default_table_id: TableId,
    next_table_id: TableId,
}

impl ServerApp {
    pub fn new(store: AccountStore) -> Self {
        let accounts = Arc::new(Mutex::new(store));
        let default_table_id = DEFAULT_TABLE_ID;
        let mut tables = HashMap::new();
        tables.insert(default_table_id, TableRuntime::new(Arc::clone(&accounts)));

        Self {
            accounts,
            tables,
            default_table_id,
            next_table_id: default_table_id + 1,
        }
    }

    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::Account(account_cmd) => self.apply_account_cmd(account_cmd),
            other => self.apply_to_table(self.default_table_id, other),
        }
    }

    pub fn apply_to_table(&mut self, table_id: TableId, cmd: Command) -> Vec<Event> {
        match self.tables.get_mut(&table_id) {
            Some(runtime) => runtime.apply(cmd),
            None => vec![Event::Error {
                message: format!("Table {table_id} not found"),
            }],
        }
    }

    #[allow(dead_code)]
    pub fn create_table(&mut self) -> TableId {
        let table_id = self.next_table_id;
        self.next_table_id = self.next_table_id.saturating_add(1);
        self.tables
            .insert(table_id, TableRuntime::new(Arc::clone(&self.accounts)));
        table_id
    }

    #[allow(dead_code)]
    pub fn default_table_id(&self) -> TableId {
        self.default_table_id
    }

    pub fn flush_accounts(&self) -> Result<(), AccountError> {
        self.accounts
            .lock()
            .expect("account store lock poisoned")
            .flush()
    }

    fn apply_account_cmd(&self, cmd: AccountCmd) -> Vec<Event> {
        let mut accounts = self.accounts.lock().expect("account store lock poisoned");

        match cmd {
            AccountCmd::Create { name, pin, amount } => {
                match accounts.create_account(&name, &pin, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account creation failed: {:?}", e),
                    }],
                }
            }
            AccountCmd::Delete { player_name, pin } => {
                match accounts.delete_account(&player_name, &pin) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error {
                        message: format!("Account deletion failed: {:?}", e),
                    }],
                }
            }
            AccountCmd::Deposit {
                player_name,
                amount,
            } => match accounts.deposit(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Deposit failed: {:?}", e),
                }],
            },
            AccountCmd::Withdraw {
                player_name,
                amount,
            } => match accounts.withdraw(&player_name, amount) {
                Ok(_) => vec![Event::Ok],
                Err(e) => vec![Event::Error {
                    message: format!("Withdraw failed: {:?}", e),
                }],
            },
            AccountCmd::Get { player_name } => match accounts.get_by_name(&player_name) {
                Ok(acct) => {
                    let summary = acct.summary();
                    vec![Event::Info {
                        message: format!("Account info:\n{summary}"),
                    }]
                }
                Err(e) => vec![Event::Error {
                    message: format!("Get account failed: {:?}", e),
                }],
            },
        }
    }
}

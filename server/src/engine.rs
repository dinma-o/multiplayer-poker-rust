use common::{
    card_deck::Deck,
    constants::five_card_draw,
    protocol::{ AccountCmd, ActionCmd, Command, Event, HandStep, LobbyCmd, PlayerSummary },
};
use std::collections::HashMap;

use crate::{ account_store::{ AccountStore }, table::FiveCardTable };

pub struct ServerEngine {
    pub accounts: AccountStore,
    pub table: FiveCardTable,
    pub deck: Deck,
}

impl ServerEngine {
    pub fn new(accounts: AccountStore, table: FiveCardTable) -> Self {
        Self {
            accounts,
            table,
            deck: Deck::new_shuffled(),
        }
    }

    pub fn lobby_state_event(&self) -> Event {
        let accounts = self.accounts.list_accounts();
        let mut account_by_id = HashMap::with_capacity(accounts.len());
        for acct in accounts {
            account_by_id.insert(acct.id, acct);
        }

        let mut seated = Vec::new();
        for (seat, player_id) in self.table.seated_players() {
            if let Some(acct) = account_by_id.get(&player_id) {
                seated.push(PlayerSummary {
                    player_id,
                    player_name: acct.name.clone(),
                    seat,
                    balance: acct.balance,
                    games_played: acct.stats.games_played,
                    pots_won: acct.stats.pots_won,
                    folds: acct.stats.folds,
                });
            }
        }

        Event::LobbyState { seated }
    }

    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::Account(ac) => self.apply_account_cmd(ac),
            Command::Lobby(lc) => self.apply_lobby_cmd(lc),
            Command::Action(ac) => self.apply_action_cmd(ac),
        }
    }

    pub fn apply_account_cmd(&mut self, cmd: AccountCmd) -> Vec<Event> {
        match cmd {
            AccountCmd::Create { name, pin, amount } => {
                match self.accounts.create_account(&name, &pin, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) =>
                        vec![Event::Error { message: format!("Account creation failed: {:?}", e) }],
                }
            }

            AccountCmd::Delete { player_name, pin } => {
                match self.accounts.delete_account(&player_name, &pin) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) =>
                        vec![Event::Error { message: format!("Account deletion failed: {:?}", e) }],
                }
            }

            AccountCmd::Deposit { player_name, amount } => {
                match self.accounts.deposit(&player_name, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error { message: format!("Deposit failed: {:?}", e) }],
                }
            }

            AccountCmd::Withdraw { player_name, amount } => {
                match self.accounts.withdraw(&player_name, amount) {
                    Ok(_) => vec![Event::Ok],
                    Err(e) => vec![Event::Error { message: format!("Withdraw failed: {:?}", e) }],
                }
            }

            AccountCmd::Get { player_name } => {
                match self.accounts.get_by_name(&player_name) {
                    Ok(acct) => vec![Event::Info { message: format!("Account info: {:?}", acct) }],
                    Err(e) =>
                        vec![Event::Error { message: format!("Get account failed: {:?}", e) }],
                }
            }
        }
    }

    pub fn apply_lobby_cmd(&mut self, cmd: LobbyCmd) -> Vec<Event> {
        match cmd {
            LobbyCmd::SetVariant { variant } => {
                // Nothing fancy yet
                vec![Event::Info { message: format!("Set poker variant to {:?}", variant) }]
            }

            LobbyCmd::GetLobbyState => { vec![self.lobby_state_event()] }

            LobbyCmd::Join { player_name, pin } => {
                if self.table.current_step() != common::protocol::HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Cannot join table during hand".into() }];
                }

                match self.accounts.login(&player_name, &pin) {
                    Ok(pa) => {
                        match self.table.join_table(pa.id) {
                            Ok(seat) =>
                                vec![
                                    Event::PlayerJoined {
                                        player_id: pa.id,
                                        player_name: pa.name.clone(),
                                        seat: seat,
                                    },
                                    self.lobby_state_event()
                                ],
                            Err(e) =>
                                vec![Event::Error { message: format!("Seating failed: {:?}", e) }],
                        }
                    }
                    Err(e) => { vec![Event::Error { message: format!("Login failed: {:?}", e) }] }
                }
            }

            LobbyCmd::Leave { player_name } => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Cannot leave table during hand".into() }];
                }

                match self.accounts.get_by_name(&player_name) {
                    Ok(pa) => {
                        match self.table.leave_table(pa.id) {
                            Ok(_) =>
                                vec![
                                    Event::PlayerLeft {
                                        player_id: pa.id,
                                        player_name: pa.name.clone(),
                                    },
                                    self.lobby_state_event()
                                ],
                            Err(e) =>
                                vec![Event::Error { message: format!("Leave failed: {:?}", e) }],
                        }
                    }
                    Err(e) => {
                        vec![Event::Error { message: format!("Get account failed: {:?}", e) }]
                    }
                }
            }

            LobbyCmd::StartHand => {
                if self.table.current_step() != HandStep::WaitingForPlayers {
                    return vec![Event::Error { message: "Hand already in progress".into() }];
                }

                let seated = self.table.seated_players().count();

                if
                    seated < (five_card_draw::MIN_PLAYERS as usize) ||
                    seated > (five_card_draw::MAX_PLAYERS as usize)
                {
                    return vec![Event::Error { message: "need 2–5 players to start".into() }];
                }

                self.deck = Deck::new_shuffled();

                self.table.advance_table_state();

                vec![Event::StepChanged { step: self.table.current_step() }]
            }

            _ => vec![Event::Error { message: "Lobby command not implemented yet".into() }],
        }
    }

    pub fn apply_action_cmd(&mut self, _cmd: ActionCmd) -> Vec<Event> {
        // stub
        vec![Event::Info { message: "Unimplemented action command".into() }]
    }
}

mod accounts;
mod account_store;
mod table;
mod dealer;
mod game_states;
mod variants;
mod engine; // <-- your ServerEngine

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use common::protocol::{Command, Event, LobbyCmd, AccountCmd, ActionCmd, PokerVariant};

use crate::account_store::AccountStore;
use crate::engine::ServerEngine;

const ACCOUNTS_PATH: &str = "server/data/accounts.json";

fn main() {
    if let Some(parent) = Path::new(ACCOUNTS_PATH).parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");

    let table = crate::table::FiveCardTable::new();
    let mut engine = ServerEngine::new(store, table);

    println!("PokerDUD CLI. Type 'help' for commands.");

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("Error reading input.");
            continue;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case("quit") || line.eq_ignore_ascii_case("exit") {
            break;
        }
        if line.eq_ignore_ascii_case("help") {
            print_help();
            continue;
        }

        match parse_command(line) {
            Ok(cmd) => {
                let events = engine.apply(cmd);
                print_events(&events);
            }
            Err(msg) => {
                println!("Parse error: {msg}");
            }
        }
    }

    // flush accounts on exit
    engine.accounts.flush().expect("flush accounts");
    println!("Bye.");
}

fn print_help() {
    println!("Commands:");
    println!("  join <player_name> <pin>");
    println!("  leave <player_name>");
    println!("  start");
    println!("  variant five|holdem|stud");
    println!("  lobbystate");
    println!("  acct get <player_name>");
    println!("  acct create <name> <pin> <balance>");
    println!("  acct deposit <player_name> <amount>");
    println!("  acct withdraw <player_name> <amount>");
    println!("  check <player_name> | call <player_name> | fold <player_name>");
    println!("  bet <player_name> <amount> | raise <player_name> <amount>");
    println!("  draw <player_name> <i1> <i2> ...");
    println!("  help | quit");
}

fn parse_command(line: &str) -> Result<Command, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let head = parts[0].to_lowercase();

    match head.as_str() {
        "join" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let pin = parts.get(2).ok_or("missing pin")?.to_string();
            Ok(Command::Lobby(LobbyCmd::Join { player_name: pname, pin }))
        }
        "leave" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            Ok(Command::Lobby(LobbyCmd::Leave { player_name: pname }))
        }
        "start" => Ok(Command::Lobby(LobbyCmd::StartHand)),
        "variant" => {
            let v = parts.get(1).ok_or("missing variant")?;
            let variant = match v.to_lowercase().as_str() {
                "five" | "fivecarddraw" => PokerVariant::FiveCardDraw,
                "holdem" | "texasholdem" => PokerVariant::TexasHoldEm,
                "stud" | "sevencardstud" => PokerVariant::SevenCardStud,
                _ => return Err("unknown variant (five|holdem|stud)".into()),
            };
            Ok(Command::Lobby(LobbyCmd::SetVariant { variant }))
        }
        "lobbystate" => Ok(Command::Lobby(LobbyCmd::GetLobbyState)),

        "acct" => parse_account_cmd(&parts),

        "check" => Ok(Command::Action(ActionCmd::Check { player_name: parts.get(1).ok_or("missing player_name")?.to_string() })),
        "call"  => Ok(Command::Action(ActionCmd::Call  { player_name: parts.get(1).ok_or("missing player_name")?.to_string() })),
        "fold"  => Ok(Command::Action(ActionCmd::Fold  { player_name: parts.get(1).ok_or("missing player_name")?.to_string() })),

        "bet" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(2))?;
            Ok(Command::Action(ActionCmd::Bet { player_name: pname, amount: amt }))
        }
        "raise" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(2))?;
            Ok(Command::Action(ActionCmd::Raise { player_name: pname, amount: amt }))
        }
        "draw" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let mut discard = Vec::new();
            for p in parts.iter().skip(2) {
                discard.push(p.parse::<usize>().map_err(|_| "invalid discard index")?);
            }
            Ok(Command::Action(ActionCmd::Draw { player_name: pname, discard }))
        }

        _ => Err("unknown command (type 'help')".into()),
    }
}

fn parse_account_cmd(parts: &[&str]) -> Result<Command, String> {
    let sub = parts.get(1).ok_or("missing acct subcommand")?.to_lowercase();
    match sub.as_str() {
        "get" => Ok(Command::Account(AccountCmd::Get { player_name: parts.get(2).ok_or("missing player_name")?.to_string() })),
        "create" => {
            let name = parts.get(2).ok_or("missing name")?.to_string();
            let pin  = parts.get(3).ok_or("missing pin")?.to_string();
            let bal  = parse_i64(parts.get(4))?;

            Ok(Command::Account(AccountCmd::Create { name, pin, amount: bal }))
        }
        "deposit" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(3))?;
            Ok(Command::Account(AccountCmd::Deposit { player_name: pname, amount: amt }))
        }
        "withdraw" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(3))?;
            Ok(Command::Account(AccountCmd::Withdraw { player_name: pname, amount: amt }))
        }
        "delete" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let pin = parts.get(3).ok_or("missing pin")?.to_string();
            Ok(Command::Account(AccountCmd::Delete { player_name: pname, pin }))
        }
        _ => Err("unknown acct subcommand".into()),
    }
}

fn parse_i64(v: Option<&&str>) -> Result<i64, String> {
    v.ok_or("missing amount")?
        .parse::<i64>()
        .map_err(|_| "invalid amount".into())
}

fn print_events(events: &[Event]) {
    for e in events {
        match e {
            Event::Ok => println!("ok"),
            Event::Info { message } => println!("info: {message}"),
            Event::Error { message } => println!("error: {message}"),
            Event::StepChanged { step } => println!("step: {:?}", step),
            Event::PlayerJoined { player_name, player_id, seat } => println!("joined: {player_name} (ID: {player_id}) at seat {seat}"),
            Event::PlayerLeft { player_id,player_name } => println!("left: {player_name} (ID: {player_id})"),
            Event::LobbyState { seated } => {
                println!("seated players:");
                for p in seated {
                    println!("  seat {}: name {} | bal {} | GP {} | PW {} | F {}",
                        p.seat, p.player_name, p.balance, p.games_played, p.pots_won, p.folds
                    );
                }
            }
            Event::PotUpdated { pot } => println!("pot: {pot}"),
            Event::PrivateHand { player_name, cards } => println!("hand({player_name}): {:?}", cards),
            Event::ActionRequested { player_name, allowed, call_amount, min_raise } => {
                println!("action requested for {player_name}: allowed={allowed:?} call={call_amount:?} min_raise={min_raise:?}");
            }
            Event::DrawRequested { player_name, max_discards } => {
                println!("draw requested for {player_name}: max_discards={max_discards}");
            }
            Event::Showdown { results } => {
                println!("showdown:");
                for r in results {
                    println!("  {} -> {}", r.player_name, r.hand_desc);
                }
            }
            Event::Payout { winners } => {
                println!("payout:");
                for (player_name, amt) in winners {
                    println!("  {} +{}", player_name, amt);
                }
            }
        }
    }
}

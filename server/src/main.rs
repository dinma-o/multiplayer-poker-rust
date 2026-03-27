mod account_store;
mod accounts;
mod app;
mod dealer;
mod engine;
mod game_states;
mod sessions;
mod table;
mod table_runtime;
mod variants;

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use common::constants::five_card_draw;
use common::protocol::{AccountCmd, ActionCmd, Command, Event, LobbyCmd, PokerVariant};

use crate::account_store::AccountStore;
use crate::app::ServerApp;

const ACCOUNTS_PATH: &str = "server/server/data/accounts.json";

fn main() {
    if let Some(parent) = Path::new(ACCOUNTS_PATH).parent() {
        fs::create_dir_all(parent).expect("create data directory");
    }

    let store = AccountStore::load(ACCOUNTS_PATH).expect("load accounts");
    let mut app = ServerApp::new(store);

    println!("PokerDUD CLI. Type 'help' or 'help <command>' for details.");

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

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts[0].eq_ignore_ascii_case("help") {
            print_help_for(&parts);
            continue;
        }

        match parse_command(&parts) {
            Ok(cmd) => {
                let events = app.apply(cmd);
                print_events(&events);
            }
            Err(msg) => {
                println!("Parse error: {msg}");
            }
        }
    }

    // flush accounts on exit
    app.flush_accounts().expect("flush accounts");
    println!("Bye.");
}

fn print_help() {
    println!("Commands:");
    println!("  help [command] [subcommand]");
    println!("  join <player_name> <pin>");
    println!("  leave <player_name> (during hand = fold + leave after hand)");
    println!("  sitout <player_name> | sitin <player_name>");
    println!("  start");
    println!("  variant five|holdem|stud");
    println!("  seed <u64> | seed clear");
    println!("  lobbystate");
    println!("  handstate | view state");
    println!("  view hand <player_name>");
    println!("  acct get <player_name>");
    println!("  acct create <name> <pin> <balance>");
    println!("  acct deposit <player_name> <amount>");
    println!("  acct withdraw <player_name> <amount>");
    println!("  acct delete <player_name> <pin>");
    println!("  check <player_name> | call <player_name> | fold <player_name>");
    println!("  bet <player_name> <amount> | raise <player_name> <amount> (raise-by)");
    println!("  draw <player_name> <i1> <i2> ...");
    println!("  help | quit | exit");
    println!("Tip: use 'help <command>' for detailed usage (e.g., 'help bet').");
}

fn print_help_for(parts: &[&str]) {
    if parts.len() == 1 {
        print_help();
        return;
    }

    let cmd = parts[1].to_lowercase();
    match cmd.as_str() {
        "join" => {
            println!("Usage: join <player_name> <pin>");
            println!("  Log in and seat a player at the table.");
            println!("  Only allowed while waiting for players.");
            println!("Example: join alice 1234");
        }
        "leave" => {
            println!("Usage: leave <player_name>");
            println!("  If no hand is in progress, leaves immediately.");
            println!("  If a hand is in progress, folds and leaves after the hand.");
        }
        "sitout" => {
            println!("Usage: sitout <player_name>");
            println!("  Marks the player to sit out the next hand.");
            println!("  Only allowed while waiting for players.");
        }
        "sitin" => {
            println!("Usage: sitin <player_name>");
            println!("  Clears a pending sit-out so the player participates next hand.");
            println!("  Only allowed while waiting for players.");
        }
        "start" => {
            println!("Usage: start");
            println!(
                "  Starts a new hand with {}-{} eligible seated players.",
                five_card_draw::MIN_PLAYERS,
                five_card_draw::MAX_PLAYERS
            );
            println!(
                "  Each player must have at least {} chips for the ante.",
                five_card_draw::ANTE_AMOUNT
            );
            println!("  Shuffles, deals, and prompts the first player to act.");
        }
        "variant" => {
            println!("Usage: variant five|holdem|stud");
            println!("  Sets the lobby variant for future hands.");
            println!(
                "  Supported variants: five (Five-card Draw), holdem (Texas Hold'em), stud (Seven-card Stud)."
            );
        }
        "seed" => {
            println!("Usage: seed <u64> | seed clear");
            println!("  Sets a deterministic RNG seed for shuffles.");
            println!("  Use 'seed clear' to return to random shuffles.");
        }
        "lobbystate" => {
            println!("Usage: lobbystate");
            println!("  Prints seated players, balances, and basic stats.");
        }
        "handstate" => {
            println!("Usage: handstate | view state");
            println!("  Prints a friendly summary of the current hand state.");
        }
        "view" => {
            print_help_view(parts.get(2).copied());
        }
        "acct" | "account" => {
            print_help_account(parts.get(2).copied());
        }
        "check" => {
            println!("Usage: check <player_name>");
            println!("  Pass your turn without betting. Only allowed when call amount is 0.");
        }
        "call" => {
            println!("Usage: call <player_name>");
            println!("  Match the current bet amount.");
        }
        "fold" => {
            println!("Usage: fold <player_name>");
            println!("  Fold your hand and stop participating in this hand.");
        }
        "bet" => {
            println!("Usage: bet <player_name> <amount>");
            println!("  Place the first bet in a betting round.");
            println!(
                "  Amount must be between {} and {} and a multiple of {}.",
                five_card_draw::MIN_BET_PER_ROUND,
                five_card_draw::MAX_BET_PER_ROUND,
                five_card_draw::BET_INCREMENT
            );
        }
        "raise" => {
            println!("Usage: raise <player_name> <amount>");
            println!("  Raise an existing bet. Amount is raise-by, not total.");
            println!(
                "  Min raise {} (multiple of {}). Max total bet {}.",
                five_card_draw::MIN_RAISE_AMOUNT,
                five_card_draw::RAISE_INCREMENT,
                five_card_draw::MAX_RAISE_TO_AMOUNT
            );
            println!(
                "  Max raises per round: {}.",
                five_card_draw::MAX_RAISES_PER_ROUND
            );
        }
        "draw" => {
            println!("Usage: draw <player_name> [i1 i2 ...]");
            println!("  Discard the listed card indices (0-4) and draw replacements.");
            println!(
                "  Max discards: {}. No duplicate indices.",
                five_card_draw::MAX_DISCARDS
            );
            println!("  Provide no indices to stand pat.");
        }
        "help" => {
            println!("Usage: help [command] [subcommand]");
            println!("  Show command list or detailed help for a specific command.");
        }
        "quit" | "exit" => {
            println!("Usage: quit | exit");
            println!("  Exit the CLI (accounts are flushed on exit).");
        }
        _ => {
            println!(
                "No detailed help for '{}'. Type 'help' to list commands.",
                parts[1]
            );
        }
    }
}

fn print_help_account(sub: Option<&str>) {
    match sub.map(|s| s.to_lowercase()) {
        None => {
            println!("Account commands:");
            println!("  acct get <player_name>");
            println!("  acct create <name> <pin> <balance>");
            println!("  acct deposit <player_name> <amount>");
            println!("  acct withdraw <player_name> <amount>");
            println!("  acct delete <player_name> <pin>");
            println!("Tip: use 'help acct <subcommand>' for details.");
        }
        Some(ref s) if s == "get" => {
            println!("Usage: acct get <player_name>");
            println!("  Show account summary and stats for a player.");
        }
        Some(ref s) if s == "create" => {
            println!("Usage: acct create <name> <pin> <balance>");
            println!("  Create a new account with the given starting balance.");
            println!(
                "  Typical starting balance: {} chips.",
                five_card_draw::INITIAL_CHIPS
            );
        }
        Some(ref s) if s == "deposit" => {
            println!("Usage: acct deposit <player_name> <amount>");
            println!("  Add chips to the player's account balance.");
        }
        Some(ref s) if s == "withdraw" => {
            println!("Usage: acct withdraw <player_name> <amount>");
            println!("  Remove chips from the player's account balance.");
        }
        Some(ref s) if s == "delete" => {
            println!("Usage: acct delete <player_name> <pin>");
            println!("  Delete an account. Requires the correct PIN.");
        }
        Some(other) => {
            println!("Unknown acct subcommand '{}'.", other);
            println!("Type 'help acct' to list subcommands.");
        }
    }
}

fn print_help_view(sub: Option<&str>) {
    match sub.map(|s| s.to_lowercase()) {
        None => {
            println!("View commands:");
            println!("  view hand <player_name>");
            println!("  view state");
            println!("Tip: use 'help view hand' or 'help view state' for details.");
        }
        Some(ref s) if s == "hand" => {
            println!("Usage: view hand <player_name>");
            println!("  Prints the player's current private hand (if dealt).");
        }
        Some(ref s) if s == "state" || s == "handstate" => {
            println!("Usage: view state");
            println!("  Same as 'handstate' - shows a friendly summary of the hand.");
        }
        Some(other) => {
            println!("Unknown view subcommand '{}'.", other);
            println!("Type 'help view' to list subcommands.");
        }
    }
}

fn parse_command(parts: &[&str]) -> Result<Command, String> {
    let head = parts[0].to_lowercase();

    match head.as_str() {
        "join" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let pin = parts.get(2).ok_or("missing pin")?.to_string();
            Ok(Command::Lobby(LobbyCmd::Join {
                player_name: pname,
                pin,
            }))
        }
        "leave" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            Ok(Command::Lobby(LobbyCmd::Leave { player_name: pname }))
        }
        "sitout" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            Ok(Command::Lobby(LobbyCmd::SitOutNext { player_name: pname }))
        }
        "sitin" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            Ok(Command::Lobby(LobbyCmd::SitIn { player_name: pname }))
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
        "seed" => {
            let arg = parts.get(1).ok_or("missing seed value or 'clear'")?;
            if arg.eq_ignore_ascii_case("clear") {
                Ok(Command::Lobby(LobbyCmd::ClearRngSeed))
            } else {
                let seed = arg.parse::<u64>().map_err(|_| "invalid seed (u64)")?;
                Ok(Command::Lobby(LobbyCmd::SetRngSeed { seed }))
            }
        }
        "lobbystate" => Ok(Command::Lobby(LobbyCmd::GetLobbyState)),
        "handstate" => Ok(Command::Lobby(LobbyCmd::GetHandState)),
        "view" => parse_view_cmd(&parts),

        "acct" => parse_account_cmd(&parts),

        "check" => Ok(Command::Action(ActionCmd::Check {
            player_name: parts.get(1).ok_or("missing player_name")?.to_string(),
        })),
        "call" => Ok(Command::Action(ActionCmd::Call {
            player_name: parts.get(1).ok_or("missing player_name")?.to_string(),
        })),
        "fold" => Ok(Command::Action(ActionCmd::Fold {
            player_name: parts.get(1).ok_or("missing player_name")?.to_string(),
        })),

        "bet" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(2))?;
            Ok(Command::Action(ActionCmd::Bet {
                player_name: pname,
                amount: amt,
            }))
        }
        "raise" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(2))?;
            Ok(Command::Action(ActionCmd::Raise {
                player_name: pname,
                amount: amt,
            }))
        }
        "draw" => {
            let pname = parts.get(1).ok_or("missing player_name")?.to_string();
            let mut discard = Vec::new();
            for p in parts.iter().skip(2) {
                discard.push(p.parse::<usize>().map_err(|_| "invalid discard index")?);
            }
            Ok(Command::Action(ActionCmd::Draw {
                player_name: pname,
                discard,
            }))
        }

        _ => Err("unknown command (type 'help' or 'help <command>')".into()),
    }
}

fn parse_account_cmd(parts: &[&str]) -> Result<Command, String> {
    let sub = parts
        .get(1)
        .ok_or("missing acct subcommand")?
        .to_lowercase();
    match sub.as_str() {
        "get" => Ok(Command::Account(AccountCmd::Get {
            player_name: parts.get(2).ok_or("missing player_name")?.to_string(),
        })),
        "create" => {
            let name = parts.get(2).ok_or("missing name")?.to_string();
            let pin = parts.get(3).ok_or("missing pin")?.to_string();
            let bal = parse_i64(parts.get(4))?;

            Ok(Command::Account(AccountCmd::Create {
                name,
                pin,
                amount: bal,
            }))
        }
        "deposit" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(3))?;
            Ok(Command::Account(AccountCmd::Deposit {
                player_name: pname,
                amount: amt,
            }))
        }
        "withdraw" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let amt = parse_i64(parts.get(3))?;
            Ok(Command::Account(AccountCmd::Withdraw {
                player_name: pname,
                amount: amt,
            }))
        }
        "delete" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            let pin = parts.get(3).ok_or("missing pin")?.to_string();
            Ok(Command::Account(AccountCmd::Delete {
                player_name: pname,
                pin,
            }))
        }
        _ => Err("unknown acct subcommand".into()),
    }
}

fn parse_i64(v: Option<&&str>) -> Result<i64, String> {
    v.ok_or("missing amount")?
        .parse::<i64>()
        .map_err(|_| "invalid amount".into())
}

fn parse_view_cmd(parts: &[&str]) -> Result<Command, String> {
    let sub = parts
        .get(1)
        .ok_or("missing view subcommand")?
        .to_lowercase();
    match sub.as_str() {
        "hand" => {
            let pname = parts.get(2).ok_or("missing player_name")?.to_string();
            Ok(Command::Lobby(LobbyCmd::ViewHand { player_name: pname }))
        }
        "state" | "handstate" => Ok(Command::Lobby(LobbyCmd::GetHandState)),
        _ => Err("unknown view subcommand (hand|state)".into()),
    }
}

fn print_events(events: &[Event]) {
    for e in events {
        match e {
            Event::Ok => println!("ok"),
            Event::Info { message } => println!("info: {message}"),
            Event::Error { message } => println!("error: {message}"),
            Event::StepChanged { step } => println!("step: {:?}", step),
            Event::PlayerJoined {
                player_name,
                player_id,
                seat,
            } => println!("joined: {player_name} (ID: {player_id}) at seat {seat}"),
            Event::PlayerLeft {
                player_id,
                player_name,
            } => println!("left: {player_name} (ID: {player_id})"),
            Event::LobbyState { seated } => {
                println!("seated players:");
                for p in seated {
                    println!(
                        "  seat {}: name {} | bal {} | GP {} | PW {} | F {}",
                        p.seat, p.player_name, p.balance, p.games_played, p.pots_won, p.folds
                    );
                }
            }
            Event::PotUpdated { pot } => println!("pot: {pot}"),
            Event::PrivateHand { player_name, cards } => println!(
                "hand({player_name}): {:?}",
                cards.iter().map(|c| c.to_string()).collect::<Vec<_>>()
            ),
            Event::ActionRequested {
                player_name,
                allowed,
                call_amount,
                min_raise,
            } => {
                println!(
                    "action requested for {player_name}: allowed={allowed:?} call={call_amount:?} min_raise={min_raise:?}"
                );
            }
            Event::DrawRequested {
                player_name,
                max_discards,
            } => {
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

use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;

use common::protocol::PokerVariant;
use common::wire::{
    ClientRequest, LoginResult, LogoutResult, PlayerAction, RequestOp, ResponsePayload,
    ServerResponse, SessionId, TableId,
};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8080;

fn main() {
    let address = parse_address(env::args().skip(1));
    let mut client = match TcpCliClient::connect(&address) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to connect to {address}: {error}");
            return;
        }
    };

    println!("PokerDUD TCP client connected to {address}");
    println!("Type 'help' for commands.");

    loop {
        print!("client> ");
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
        if matches!(line.to_ascii_lowercase().as_str(), "quit" | "exit") {
            break;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts[0].eq_ignore_ascii_case("help") {
            print_help();
            continue;
        }
        if parts[0].eq_ignore_ascii_case("session") {
            match &client.session_id {
                Some(session_id) => println!("session: {session_id}"),
                None => println!("session: <none>"),
            }
            continue;
        }

        let op = match parse_request_op(&parts) {
            Ok(op) => op,
            Err(error) => {
                println!("Parse error: {error}");
                continue;
            }
        };

        match client.send(op) {
            Ok(response) => {
                client.capture_session(&response);
                print_response(&response);
            }
            Err(error) => {
                println!("Request failed: {error}");
                break;
            }
        }
    }
}

struct TcpCliClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_request_id: u64,
    session_id: Option<SessionId>,
}

impl TcpCliClient {
    fn connect(address: &str) -> io::Result<Self> {
        let writer = TcpStream::connect(address)?;
        let reader = BufReader::new(writer.try_clone()?);

        Ok(Self {
            reader,
            writer,
            next_request_id: 1,
            session_id: None,
        })
    }

    fn send(&mut self, op: RequestOp) -> io::Result<ServerResponse> {
        let request_id = format!("req-{}", self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);

        let request = ClientRequest {
            request_id,
            session_id: self.session_id.clone(),
            op,
        };

        let json = serde_json::to_string(&request).map_err(io::Error::other)?;
        self.writer.write_all(json.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "server closed the connection",
            ));
        }

        serde_json::from_str(line.trim()).map_err(io::Error::other)
    }

    fn capture_session(&mut self, response: &ServerResponse) {
        if !response.ok {
            return;
        }

        match &response.payload {
            Some(ResponsePayload::Login(LoginResult { session_id, .. })) => {
                self.session_id = Some(session_id.clone());
            }
            Some(ResponsePayload::Logout(LogoutResult { .. })) => {
                self.session_id = None;
            }
            _ => {}
        }
    }
}

fn parse_address(args: impl Iterator<Item = String>) -> String {
    let mut host = DEFAULT_HOST.to_string();
    let mut port = DEFAULT_PORT;
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => {
                if let Some(value) = args.next() {
                    host = value;
                }
            }
            "--port" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<u16>() {
                        port = parsed;
                    }
                }
            }
            other if other.contains(':') => {
                return other.to_string();
            }
            _ => {}
        }
    }

    format!("{host}:{port}")
}

fn parse_request_op(parts: &[&str]) -> Result<RequestOp, String> {
    match parts[0].to_ascii_lowercase().as_str() {
        "create_account" => Ok(RequestOp::CreateAccount {
            name: required_arg(parts, 1, "name")?.to_string(),
            pin: required_arg(parts, 2, "pin")?.to_string(),
            amount: parse_i64(parts.get(3), "amount")?,
        }),
        "login" => Ok(RequestOp::Login {
            name: required_arg(parts, 1, "name")?.to_string(),
            pin: required_arg(parts, 2, "pin")?.to_string(),
        }),
        "logout" => Ok(RequestOp::Logout),
        "deposit" => Ok(RequestOp::Deposit {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "withdraw" => Ok(RequestOp::Withdraw {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "list_tables" => Ok(RequestOp::ListTables),
        "create_table" => Ok(RequestOp::CreateTable {
            variant: parse_variant(required_arg(parts, 1, "variant")?)?,
        }),
        "join_table" => Ok(RequestOp::JoinTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "leave_table" => Ok(RequestOp::LeaveTable {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "start_hand" => Ok(RequestOp::StartHand {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_table_state" => Ok(RequestOp::GetTableState {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_player_view" => Ok(RequestOp::GetPlayerView {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "get_history" => Ok(RequestOp::GetHistory {
            table_id: parse_u64(parts.get(1), "table_id")?,
        }),
        "submit_action" => Ok(RequestOp::SubmitAction {
            table_id: parse_u64(parts.get(1), "table_id")?,
            action: parse_action(&parts[2..])?,
        }),
        other => Err(format!("unknown command '{other}'")),
    }
}

fn parse_variant(value: &str) -> Result<PokerVariant, String> {
    match value.to_ascii_lowercase().as_str() {
        "five" | "five_card_draw" | "fivecarddraw" => Ok(PokerVariant::FiveCardDraw),
        "holdem" | "hold'em" | "texas_holdem" | "texasholdem" => Ok(PokerVariant::TexasHoldEm),
        "stud" | "seven_card_stud" | "sevencardstud" => Ok(PokerVariant::SevenCardStud),
        _ => Err("unknown variant (five|holdem|stud)".into()),
    }
}

fn parse_action(parts: &[&str]) -> Result<PlayerAction, String> {
    let action = parts
        .first()
        .ok_or("missing action (check|call|fold|bet|raise|draw)")?
        .to_ascii_lowercase();

    match action.as_str() {
        "check" => Ok(PlayerAction::Check),
        "call" => Ok(PlayerAction::Call),
        "fold" => Ok(PlayerAction::Fold),
        "bet" => Ok(PlayerAction::Bet {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "raise" => Ok(PlayerAction::Raise {
            amount: parse_i64(parts.get(1), "amount")?,
        }),
        "draw" => {
            let discard = parts
                .iter()
                .skip(1)
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "invalid discard index".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PlayerAction::Draw { discard })
        }
        _ => Err("unknown action (check|call|fold|bet|raise|draw)".into()),
    }
}

fn parse_i64(value: Option<&&str>, name: &str) -> Result<i64, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .parse::<i64>()
        .map_err(|_| format!("invalid {name}"))
}

fn parse_u64(value: Option<&&str>, name: &str) -> Result<TableId, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid {name}"))
}

fn required_arg<'a>(parts: &'a [&str], index: usize, name: &str) -> Result<&'a str, String> {
    parts
        .get(index)
        .copied()
        .ok_or_else(|| format!("missing {name}"))
}

fn print_response(response: &ServerResponse) {
    match serde_json::to_string_pretty(response) {
        Ok(json) => println!("{json}"),
        Err(error) => println!("failed to render response: {error}"),
    }
}

fn print_help() {
    println!("Commands:");
    println!("  help");
    println!("  session");
    println!("  create_account <name> <pin> <amount>");
    println!("  login <name> <pin>");
    println!("  logout");
    println!("  deposit <amount>");
    println!("  withdraw <amount>");
    println!("  list_tables");
    println!("  create_table <five|holdem|stud>");
    println!("  join_table <table_id>");
    println!("  leave_table <table_id>");
    println!("  start_hand <table_id>");
    println!("  get_table_state <table_id>");
    println!("  get_player_view <table_id>");
    println!("  get_history <table_id>");
    println!("  submit_action <table_id> check");
    println!("  submit_action <table_id> call");
    println!("  submit_action <table_id> fold");
    println!("  submit_action <table_id> bet <amount>");
    println!("  submit_action <table_id> raise <amount>");
    println!("  submit_action <table_id> draw [i1 i2 ...]");
    println!("  quit | exit");
    println!("Startup flags:");
    println!("  --host <host> --port <port>");
    println!("  or pass <host>:<port>");
}

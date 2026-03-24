use std::env;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8080;

fn main() {
    let address = parse_address(env::args().skip(1));
    poker_gui::cli::run(&address);
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

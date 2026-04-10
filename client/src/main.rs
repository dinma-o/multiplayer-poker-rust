use std::env;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8080;

#[derive(Debug)]
struct PreviewRequest {
    screen: String,
    variant: Option<String>,
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();

    if let Some(preview) = parse_preview_args(&args).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    }) {
        if let Err(err) = poker_gui::gui::run_preview(&preview.screen, preview.variant.as_deref()) {
            eprintln!("{err}");
            std::process::exit(2);
        }
        return;
    }

    if args.iter().any(|a| a == "--gui") {
        poker_gui::gui::run();
        return;
    }

    args.retain(|a| a != "--gui");
    let address = parse_address(args.into_iter());
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

fn parse_preview_args(args: &[String]) -> Result<Option<PreviewRequest>, String> {
    let Some(index) = args.iter().position(|arg| arg == "--gui-preview") else {
        return Ok(None);
    };

    let preview_args: Vec<&String> = args[index + 1..]
        .iter()
        .take_while(|arg| !arg.starts_with("--"))
        .collect();

    let Some(screen) = preview_args.first() else {
        return Err(preview_help(
            "Expected a preview target after --gui-preview.",
        ));
    };

    match screen.trim().to_ascii_lowercase().as_str() {
        "rankings" => {
            if preview_args.len() > 1 {
                Err(preview_help(
                    "Preview 'rankings' does not take an extra argument.",
                ))
            } else {
                Ok(Some(PreviewRequest {
                    screen: "table".to_string(),
                    variant: Some("rankings".to_string()),
                }))
            }
        }
        "connect" | "login" | "lobby" => {
            if preview_args.len() > 1 {
                Err(preview_help(&format!(
                    "Preview '{}' does not take an extra argument.",
                    screen
                )))
            } else {
                Ok(Some(PreviewRequest {
                    screen: (*screen).clone(),
                    variant: None,
                }))
            }
        }
        "table" => {
            let Some(variant) = preview_args.get(1) else {
                return Err(preview_help(
                    "Expected a table variant after '--gui-preview table'.",
                ));
            };

            if preview_args.len() > 2 {
                return Err(preview_help(
                    "Too many preview arguments supplied for '--gui-preview table'.",
                ));
            }

            Ok(Some(PreviewRequest {
                screen: (*screen).clone(),
                variant: Some((**variant).clone()),
            }))
        }
        _ => Ok(Some(PreviewRequest {
            screen: (*screen).clone(),
            variant: preview_args.get(1).map(|variant| (**variant).clone()),
        })),
    }
}

fn preview_help(prefix: &str) -> String {
    format!(
        "{prefix} Available previews: connect, login, lobby, table <holdem|draw|stud|rankings>."
    )
}

#[cfg(test)]
mod tests {
    use super::parse_preview_args;

    #[test]
    fn parses_table_preview_with_variant() {
        let args = vec![
            "--gui-preview".to_string(),
            "table".to_string(),
            "stud".to_string(),
        ];

        let preview = parse_preview_args(&args)
            .expect("parse preview args")
            .expect("preview request");

        assert_eq!(preview.screen, "table");
        assert_eq!(preview.variant.as_deref(), Some("stud"));
    }

    #[test]
    fn rejects_table_preview_without_variant() {
        let args = vec!["--gui-preview".to_string(), "table".to_string()];

        let err = parse_preview_args(&args).expect_err("missing variant should fail");

        assert!(err.contains("table variant"));
    }

    #[test]
    fn rejects_extra_arg_for_non_table_preview() {
        let args = vec![
            "--gui-preview".to_string(),
            "lobby".to_string(),
            "stud".to_string(),
        ];

        let err = parse_preview_args(&args).expect_err("extra preview arg should fail");

        assert!(err.contains("does not take an extra argument"));
    }

    #[test]
    fn parses_rankings_shortcut_preview() {
        let args = vec!["--gui-preview".to_string(), "rankings".to_string()];

        let preview = parse_preview_args(&args)
            .expect("parse preview args")
            .expect("preview request");

        assert_eq!(preview.screen, "table");
        assert_eq!(preview.variant.as_deref(), Some("rankings"));
    }
}

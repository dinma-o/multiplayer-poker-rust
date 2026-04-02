use common::card_deck::{Card, Rank, Suit};
use common::protocol::{AllowedAction, HandStep, PayoutLine, PokerVariant, ShowdownLine};
use common::wire::{
    AccountProfile, ActionPrompt, HistoryEntry, HistoryKind, PlayerView, PublicSeatState,
    PublicTableState, TableSummary,
};

use super::app::{LoginTab, PokerApp, Screen};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewScene {
    Connect,
    Login,
    Lobby,
    Table(TablePreviewVariant),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePreviewVariant {
    Holdem,
    Draw,
    Stud,
}

impl TablePreviewVariant {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "holdem" | "hold'em" | "texas_holdem" | "texasholdem" => Ok(Self::Holdem),
            "draw" | "five" | "five_card_draw" | "fivecarddraw" => Ok(Self::Draw),
            "stud" | "seven_card_stud" | "sevencardstud" => Ok(Self::Stud),
            other => Err(preview_help(&format!(
                "Unknown table preview variant '{other}'."
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Holdem => "Texas Hold'em",
            Self::Draw => "Five Card Draw",
            Self::Stud => "Seven Card Stud",
        }
    }
}

impl PreviewScene {
    pub fn parse(name: &str, variant: Option<&str>) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "connect" => Ok(Self::Connect),
            "login" => Ok(Self::Login),
            "lobby" => Ok(Self::Lobby),
            "table" => {
                let variant = variant.ok_or_else(|| {
                    preview_help("Table preview requires a variant after '--gui-preview table'.")
                })?;
                Ok(Self::Table(TablePreviewVariant::parse(variant)?))
            }
            other => Err(preview_help(&format!("Unknown preview screen '{other}'."))),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Connect => "Connect".to_string(),
            Self::Login => "Login".to_string(),
            Self::Lobby => "Lobby".to_string(),
            Self::Table(variant) => format!("Table - {}", variant.label()),
        }
    }
}

pub fn build_preview_app(scene: PreviewScene) -> PokerApp {
    let mut app = PokerApp::default();
    app.connected = true;

    match scene {
        PreviewScene::Connect => {
            app.screen = Screen::Connect;
            app.connected = false;
            app.server_address = "127.0.0.1:8080".to_string();
        }
        PreviewScene::Login => {
            app.screen = Screen::Login;
            app.input_name = "demo_player".to_string();
            app.input_pin = "1234".to_string();
            app.input_amount = "1000".to_string();
            app.login_tab = LoginTab::Login;
        }
        PreviewScene::Lobby => {
            apply_lobby_preview(&mut app);
        }
        PreviewScene::Table(variant) => {
            apply_table_preview(&mut app, variant);
        }
    }

    app
}

fn apply_lobby_preview(app: &mut PokerApp) {
    app.screen = Screen::Lobby;
    app.account = Some(preview_account());
    app.selected_variant = PokerVariant::TexasHoldEm;
    app.tables = vec![
        TableSummary {
            table_id: 7,
            variant: PokerVariant::TexasHoldEm,
            step: HandStep::BettingRound3,
            seated_count: 4,
            max_players: 5,
        },
        TableSummary {
            table_id: 12,
            variant: PokerVariant::FiveCardDraw,
            step: HandStep::DrawPhase,
            seated_count: 4,
            max_players: 5,
        },
        TableSummary {
            table_id: 19,
            variant: PokerVariant::SevenCardStud,
            step: HandStep::BettingRound3,
            seated_count: 4,
            max_players: 5,
        },
    ];
}

fn apply_table_preview(app: &mut PokerApp, variant: TablePreviewVariant) {
    match variant {
        TablePreviewVariant::Holdem => apply_holdem_table_preview(app),
        TablePreviewVariant::Draw => apply_draw_table_preview(app),
        TablePreviewVariant::Stud => apply_stud_table_preview(app),
    }
}

fn apply_holdem_table_preview(app: &mut PokerApp) {
    let table_id = 7;
    let board_cards = vec![
        card(Rank::Queen, Suit::Clubs),
        card(Rank::Ten, Suit::Diamonds),
        card(Rank::Seven, Suit::Spades),
        card(Rank::Two, Suit::Hearts),
    ];

    app.screen = Screen::Table { table_id };
    app.account = Some(preview_account());
    app.table_state = Some(PublicTableState {
        table_id,
        variant: PokerVariant::TexasHoldEm,
        step: HandStep::BettingRound3,
        dealer_seat: 1,
        acting_player_id: Some(1),
        pot: 320,
        current_bet: 20,
        board_cards: board_cards.clone(),
        seats: vec![
            holdem_seat(0, 3, "Maya", 980, true, false),
            holdem_seat(1, 2, "Noah", 1430, true, false),
            PublicSeatState {
                seat: 2,
                player_id: 4,
                player_name: "Ivy".to_string(),
                balance: 760,
                is_connected: true,
                is_in_hand: false,
                has_folded: true,
                hidden_card_count: 0,
                up_cards: vec![],
            },
            holdem_seat(3, 1, "You", 1240, true, false),
        ],
    });
    app.player_view = Some(PlayerView {
        table_id,
        player_id: 1,
        player_name: "You".to_string(),
        seat: Some(3),
        private_cards: vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Spades),
        ],
        up_cards: vec![],
        board_cards,
        action_prompt: Some(ActionPrompt {
            allowed_actions: vec![
                AllowedAction::Fold,
                AllowedAction::Call,
                AllowedAction::Raise,
            ],
            call_amount: Some(20),
            min_raise: Some(5),
            max_amount: Some(20),
            max_discards: None,
        }),
    });
    app.event_log = vec![
        history_info(1, "Noah led 20 on the turn"),
        history_info(2, "Maya called 20"),
        history_info(3, "Ivy folded"),
        history_prompt(4),
    ];
    app.last_showdown = Some(HistoryEntry {
        index: 90,
        kind: HistoryKind::Showdown,
        message: "Preview showdown".to_string(),
        showdown_results: Some(vec![
            ShowdownLine {
                player_name: "Noah".to_string(),
                player_id: 2,
                hand_desc: "A♣ A♥ => Pair of aces".to_string(),
            },
            ShowdownLine {
                player_name: "You".to_string(),
                player_id: 1,
                hand_desc: "A♠ K♠ => Ace high".to_string(),
            },
        ]),
        payout_results: None,
    });
    app.last_payout = Some(HistoryEntry {
        index: 91,
        kind: HistoryKind::Payout,
        message: "Preview payout".to_string(),
        showdown_results: None,
        payout_results: Some(vec![PayoutLine {
            player_name: "Noah".to_string(),
            player_id: 2,
            amount: 320,
        }]),
    });
}

fn apply_draw_table_preview(app: &mut PokerApp) {
    let table_id = 12;

    app.screen = Screen::Table { table_id };
    app.account = Some(preview_account());
    app.selected_discards = vec![1, 3];
    app.table_state = Some(PublicTableState {
        table_id,
        variant: PokerVariant::FiveCardDraw,
        step: HandStep::DrawPhase,
        dealer_seat: 0,
        acting_player_id: Some(1),
        pot: 75,
        current_bet: 10,
        board_cards: vec![],
        seats: vec![
            draw_seat(0, 2, "Noah", 1130, true, false),
            draw_seat(1, 1, "You", 1240, true, false),
            PublicSeatState {
                seat: 2,
                player_id: 4,
                player_name: "Ivy".to_string(),
                balance: 690,
                is_connected: true,
                is_in_hand: false,
                has_folded: true,
                hidden_card_count: 0,
                up_cards: vec![],
            },
            draw_seat(3, 3, "Maya", 930, true, false),
        ],
    });
    app.player_view = Some(PlayerView {
        table_id,
        player_id: 1,
        player_name: "You".to_string(),
        seat: Some(1),
        private_cards: vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Ace, Suit::Diamonds),
            card(Rank::Ten, Suit::Clubs),
            card(Rank::Seven, Suit::Hearts),
            card(Rank::Three, Suit::Clubs),
        ],
        up_cards: vec![],
        board_cards: vec![],
        action_prompt: Some(ActionPrompt {
            allowed_actions: vec![AllowedAction::Draw],
            call_amount: None,
            min_raise: None,
            max_amount: None,
            max_discards: Some(3),
        }),
    });
    app.event_log = vec![
        history_info(1, "Noah opened for 10"),
        history_info(2, "You called 10"),
        history_info(3, "Maya called 10"),
        history_info(4, "You marked 2 cards to discard"),
        history_prompt(5),
    ];
    app.last_showdown = Some(HistoryEntry {
        index: 90,
        kind: HistoryKind::Showdown,
        message: "Preview showdown".to_string(),
        showdown_results: Some(vec![
            ShowdownLine {
                player_name: "You".to_string(),
                player_id: 1,
                hand_desc: "A♠ A♦ 10♣ 7♥ 3♣ => Pair of aces".to_string(),
            },
            ShowdownLine {
                player_name: "Noah".to_string(),
                player_id: 2,
                hand_desc: "K♠ K♥ 9♦ 6♠ 2♣ => Pair of kings".to_string(),
            },
        ]),
        payout_results: None,
    });
    app.last_payout = Some(HistoryEntry {
        index: 91,
        kind: HistoryKind::Payout,
        message: "Preview payout".to_string(),
        showdown_results: None,
        payout_results: Some(vec![PayoutLine {
            player_name: "You".to_string(),
            player_id: 1,
            amount: 75,
        }]),
    });
}

fn apply_stud_table_preview(app: &mut PokerApp) {
    let table_id = 19;

    app.screen = Screen::Table { table_id };
    app.account = Some(preview_account());
    app.table_state = Some(PublicTableState {
        table_id,
        variant: PokerVariant::SevenCardStud,
        step: HandStep::BettingRound3,
        dealer_seat: 1,
        acting_player_id: Some(1),
        pot: 210,
        current_bet: 20,
        board_cards: vec![],
        seats: vec![
            stud_seat(
                0,
                3,
                "Maya",
                980,
                true,
                false,
                &[
                    card(Rank::Nine, Suit::Hearts),
                    card(Rank::Nine, Suit::Clubs),
                    card(Rank::King, Suit::Hearts),
                ],
            ),
            stud_seat(
                1,
                2,
                "Noah",
                1430,
                true,
                false,
                &[
                    card(Rank::Ace, Suit::Hearts),
                    card(Rank::Ten, Suit::Hearts),
                    card(Rank::Ace, Suit::Clubs),
                ],
            ),
            PublicSeatState {
                seat: 2,
                player_id: 4,
                player_name: "Ivy".to_string(),
                balance: 760,
                is_connected: true,
                is_in_hand: false,
                has_folded: true,
                hidden_card_count: 0,
                up_cards: vec![
                    card(Rank::Five, Suit::Spades),
                    card(Rank::Jack, Suit::Diamonds),
                    card(Rank::Three, Suit::Spades),
                ],
            },
            stud_seat(
                3,
                1,
                "You",
                1240,
                true,
                false,
                &[
                    card(Rank::Queen, Suit::Spades),
                    card(Rank::Jack, Suit::Spades),
                    card(Rank::Eight, Suit::Spades),
                ],
            ),
        ],
    });
    app.player_view = Some(PlayerView {
        table_id,
        player_id: 1,
        player_name: "You".to_string(),
        seat: Some(3),
        private_cards: vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Spades),
        ],
        up_cards: vec![
            card(Rank::Queen, Suit::Spades),
            card(Rank::Jack, Suit::Spades),
            card(Rank::Eight, Suit::Spades),
        ],
        board_cards: vec![],
        action_prompt: Some(ActionPrompt {
            allowed_actions: vec![
                AllowedAction::Fold,
                AllowedAction::Call,
                AllowedAction::Raise,
            ],
            call_amount: Some(20),
            min_raise: Some(20),
            max_amount: Some(20),
            max_discards: None,
        }),
    });
    app.event_log = vec![
        history_info(1, "Noah bet 20 on fifth street"),
        history_info(2, "Maya called 20"),
        history_info(3, "Ivy folded"),
        history_prompt(4),
    ];
    app.last_showdown = Some(HistoryEntry {
        index: 90,
        kind: HistoryKind::Showdown,
        message: "Preview showdown".to_string(),
        showdown_results: Some(vec![
            ShowdownLine {
                player_name: "Noah".to_string(),
                player_id: 2,
                hand_desc: "A♥ 10♥ A♣ => Visible aces".to_string(),
            },
            ShowdownLine {
                player_name: "You".to_string(),
                player_id: 1,
                hand_desc: "Q♠ J♠ 8♠ => Flush draw pressure".to_string(),
            },
        ]),
        payout_results: None,
    });
    app.last_payout = Some(HistoryEntry {
        index: 91,
        kind: HistoryKind::Payout,
        message: "Preview payout".to_string(),
        showdown_results: None,
        payout_results: Some(vec![PayoutLine {
            player_name: "Noah".to_string(),
            player_id: 2,
            amount: 210,
        }]),
    });
}

fn preview_account() -> AccountProfile {
    AccountProfile {
        player_id: 1,
        player_name: "You".to_string(),
        balance: 1240,
        games_played: 18,
        pots_won: 5,
        folds: 7,
    }
}

fn holdem_seat(
    seat: usize,
    player_id: u64,
    player_name: &str,
    balance: i64,
    is_in_hand: bool,
    has_folded: bool,
) -> PublicSeatState {
    PublicSeatState {
        seat,
        player_id,
        player_name: player_name.to_string(),
        balance,
        is_connected: true,
        is_in_hand,
        has_folded,
        hidden_card_count: if has_folded { 0 } else { 2 },
        up_cards: vec![],
    }
}

fn draw_seat(
    seat: usize,
    player_id: u64,
    player_name: &str,
    balance: i64,
    is_in_hand: bool,
    has_folded: bool,
) -> PublicSeatState {
    PublicSeatState {
        seat,
        player_id,
        player_name: player_name.to_string(),
        balance,
        is_connected: true,
        is_in_hand,
        has_folded,
        hidden_card_count: if has_folded { 0 } else { 5 },
        up_cards: vec![],
    }
}

fn stud_seat(
    seat: usize,
    player_id: u64,
    player_name: &str,
    balance: i64,
    is_in_hand: bool,
    has_folded: bool,
    up_cards: &[Card],
) -> PublicSeatState {
    PublicSeatState {
        seat,
        player_id,
        player_name: player_name.to_string(),
        balance,
        is_connected: true,
        is_in_hand,
        has_folded,
        hidden_card_count: if has_folded { 0 } else { 2 },
        up_cards: up_cards.to_vec(),
    }
}

fn history_info(index: u64, message: &str) -> HistoryEntry {
    HistoryEntry {
        index,
        kind: HistoryKind::Info,
        message: message.to_string(),
        showdown_results: None,
        payout_results: None,
    }
}

fn history_prompt(index: u64) -> HistoryEntry {
    HistoryEntry {
        index,
        kind: HistoryKind::ActionPrompt,
        message: "Your turn to act".to_string(),
        showdown_results: None,
        payout_results: None,
    }
}

fn card(rank: Rank, suit: Suit) -> Card {
    Card::new(rank, suit)
}

fn preview_help(prefix: &str) -> String {
    format!("{prefix} Available previews: connect, login, lobby, table <holdem|draw|stud>.")
}

#[cfg(test)]
mod tests {
    use super::{PreviewScene, TablePreviewVariant};

    #[test]
    fn parses_stud_table_preview() {
        let scene = PreviewScene::parse("table", Some("stud")).expect("parse stud table preview");

        assert_eq!(scene, PreviewScene::Table(TablePreviewVariant::Stud));
    }

    #[test]
    fn rejects_table_preview_without_variant() {
        let err = PreviewScene::parse("table", None).expect_err("missing variant should fail");

        assert!(err.contains("table <holdem|draw|stud>"));
    }

    #[test]
    fn labels_table_preview_with_variant() {
        let label = PreviewScene::Table(TablePreviewVariant::Draw).label();

        assert_eq!(label, "Table - Five Card Draw");
    }
}

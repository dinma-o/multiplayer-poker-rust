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
    Table,
}

impl PreviewScene {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "connect" => Ok(Self::Connect),
            "login" => Ok(Self::Login),
            "lobby" => Ok(Self::Lobby),
            "table" => Ok(Self::Table),
            other => Err(format!(
                "Unknown preview screen '{other}'. Available previews: connect, login, lobby, table."
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Connect => "Connect",
            Self::Login => "Login",
            Self::Lobby => "Lobby",
            Self::Table => "Table",
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
        PreviewScene::Table => {
            apply_table_preview(&mut app);
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
            step: HandStep::BettingRound2,
            seated_count: 3,
            max_players: 5,
        },
        TableSummary {
            table_id: 12,
            variant: PokerVariant::FiveCardDraw,
            step: HandStep::WaitingForPlayers,
            seated_count: 1,
            max_players: 5,
        },
        TableSummary {
            table_id: 19,
            variant: PokerVariant::SevenCardStud,
            step: HandStep::DealFaceUp,
            seated_count: 4,
            max_players: 5,
        },
    ];
}

fn apply_table_preview(app: &mut PokerApp) {
    let table_id = 7;
    app.screen = Screen::Table { table_id };
    app.account = Some(preview_account());
    app.table_state = Some(preview_table_state(table_id));
    app.player_view = Some(preview_player_view(table_id));
    app.event_log = vec![
        HistoryEntry {
            index: 1,
            kind: HistoryKind::Info,
            message: "Noah posted big blind 40".to_string(),
            showdown_results: None,
            payout_results: None,
        },
        HistoryEntry {
            index: 2,
            kind: HistoryKind::Info,
            message: "Maya called 40".to_string(),
            showdown_results: None,
            payout_results: None,
        },
        HistoryEntry {
            index: 3,
            kind: HistoryKind::Info,
            message: "Ivy folded".to_string(),
            showdown_results: None,
            payout_results: None,
        },
        HistoryEntry {
            index: 4,
            kind: HistoryKind::ActionPrompt,
            message: "Your turn to act".to_string(),
            showdown_results: None,
            payout_results: None,
        },
    ];
    app.last_showdown = Some(HistoryEntry {
        index: 90,
        kind: HistoryKind::Showdown,
        message: "Preview showdown".to_string(),
        showdown_results: Some(vec![
            ShowdownLine {
                player_name: "Noah".to_string(),
                player_id: 2,
                hand_desc: "A♠ A♥ => Pair of aces".to_string(),
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

fn preview_table_state(table_id: u64) -> PublicTableState {
    PublicTableState {
        table_id,
        variant: PokerVariant::TexasHoldEm,
        step: HandStep::BettingRound2,
        dealer_seat: 1,
        acting_player_id: Some(1),
        pot: 320,
        current_bet: 40,
        board_cards: vec![
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Ten, Suit::Diamonds),
            card(Rank::Seven, Suit::Spades),
            card(Rank::Two, Suit::Hearts),
        ],
        seats: vec![
            PublicSeatState {
                seat: 0,
                player_id: 3,
                player_name: "Maya".to_string(),
                balance: 980,
                is_connected: true,
                is_in_hand: true,
                has_folded: false,
                hidden_card_count: 2,
                up_cards: vec![],
            },
            PublicSeatState {
                seat: 1,
                player_id: 2,
                player_name: "Noah".to_string(),
                balance: 1430,
                is_connected: true,
                is_in_hand: true,
                has_folded: false,
                hidden_card_count: 2,
                up_cards: vec![],
            },
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
            PublicSeatState {
                seat: 3,
                player_id: 1,
                player_name: "You".to_string(),
                balance: 1240,
                is_connected: true,
                is_in_hand: true,
                has_folded: false,
                hidden_card_count: 2,
                up_cards: vec![],
            },
        ],
    }
}

fn preview_player_view(table_id: u64) -> PlayerView {
    PlayerView {
        table_id,
        player_id: 1,
        player_name: "You".to_string(),
        seat: Some(3),
        private_cards: vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Spades),
        ],
        up_cards: vec![],
        board_cards: vec![
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Ten, Suit::Diamonds),
            card(Rank::Seven, Suit::Spades),
            card(Rank::Two, Suit::Hearts),
        ],
        action_prompt: Some(ActionPrompt {
            allowed_actions: vec![
                AllowedAction::Fold,
                AllowedAction::Call,
                AllowedAction::Raise,
            ],
            call_amount: Some(40),
            min_raise: Some(80),
            max_discards: None,
        }),
    }
}

fn card(rank: Rank, suit: Suit) -> Card {
    Card::new(rank, suit)
}

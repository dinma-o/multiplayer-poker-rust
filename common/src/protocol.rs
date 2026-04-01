use crate::card_deck::Card;
use serde::{Deserialize, Serialize};

pub type PlayerId = u64;
pub type Chips = i64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PokerVariant {
    FiveCardDraw,
    TexasHoldEm,
    SevenCardStud,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandStep {
    WaitingForPlayers,
    CollectForcedBets, // ante or blinds later
    DealPrivate,       // draw5/hole cards/stud down cards
    DealFaceUp,        // stud up cards later
    DealCommunity,     // hold'em later
    BettingRound1,
    DrawPhase, // draw5 only (no-op for others)
    BettingRound2,
    BettingRound3,
    BettingRound4,
    BettingRound5,
    Showdown,
    Payout,
}

// --- Commands ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Account(AccountCmd),
    Lobby(LobbyCmd),
    Action(ActionCmd),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountCmd {
    Create {
        name: String,
        pin: String,
        amount: Chips,
    },
    Delete {
        player_name: String,
        pin: String,
    },
    Deposit {
        player_name: String,
        amount: Chips,
    },
    Withdraw {
        player_name: String,
        amount: Chips,
    },
    Get {
        player_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyCmd {
    SetVariant { variant: PokerVariant },
    Join { player_name: String, pin: String },
    Leave { player_name: String },
    StartHand,
    EndSession, // optional convenience for CLI
    GetLobbyState,
    GetHandState,
    ViewHand { player_name: String },
    SetRngSeed { seed: u64 },
    ClearRngSeed,
    SitOutNext { player_name: String },
    SitIn { player_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionCmd {
    Check {
        player_name: String,
    },
    Bet {
        player_name: String,
        amount: Chips,
    },
    Call {
        player_name: String,
    },
    Raise {
        player_name: String,
        amount: Chips,
    },
    Fold {
        player_name: String,
    },
    Draw {
        player_name: String,
        discard: Vec<usize>,
    }, // indices into 5-card hand
}

// -------------------- Events --------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Ok,
    Info {
        message: String,
    },
    Error {
        message: String,
    },

    StepChanged {
        step: HandStep,
    },

    LobbyState {
        seated: Vec<PlayerSummary>,
    },
    PlayerJoined {
        player_id: PlayerId,
        player_name: String,
        seat: usize,
    },
    PlayerLeft {
        player_id: PlayerId,
        player_name: String,
    },

    PotUpdated {
        pot: Chips,
    },

    ActionRequested {
        player_name: String,
        allowed: Vec<AllowedAction>,
        call_amount: Option<Chips>,
        min_raise: Option<Chips>,
        max_amount: Option<Chips>,
    },

    DrawRequested {
        player_name: String,
        max_discards: usize,
    },

    /// Only send to the relevant player in Phase 2;
    /// in Phase 1 CLI you can just print it.
    PrivateHand {
        player_name: String,
        cards: Vec<Card>,
    },

    Showdown {
        results: Vec<ShowdownLine>,
    },

    Payout {
        winners: Vec<PayoutLine>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub player_id: PlayerId,
    pub player_name: String,
    pub seat: usize,
    pub balance: Chips,
    pub games_played: u32,
    pub pots_won: u32,
    pub folds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowedAction {
    Check,
    Bet,
    Call,
    Raise,
    Fold,
    Draw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShowdownLine {
    pub player_name: String,
    pub player_id: PlayerId,
    pub hand_desc: String, // keep string for now; we will add structured later
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayoutLine {
    pub player_name: String,
    pub player_id: PlayerId,
    pub amount: Chips,
}

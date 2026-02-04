use crate::card_deck::Card;

pub type PlayerId = u64;
pub type Chips = u32;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PokerVariant {
    FiveCardDraw,
    TexasHoldEm,
    SevenCardStud,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HandStep {
    WaitingForPlayers,
    CollectForcedBets,        // ante or blinds later
    DealPrivate,              // draw5/hole cards/stud down cards
    DealCommunity,            // hold'em later
    BettingRound1,
    DrawPhase,                // draw5 only (no-op for others)
    BettingRound2,
    Showdown,
    Payout,
}

// --- Commands ---

#[derive(Debug, Clone)]
pub enum Command {
    Account(AccountCmd),
    Lobby(LobbyCmd),
    Action(ActionCmd),
}

#[derive(Debug, Clone)]
pub enum AccountCmd {
    Create { name: String, pin: String },
    Delete { player_id: PlayerId, pin: String },
    Deposit { player_id: PlayerId, amount: Chips },
    Withdraw { player_id: PlayerId, amount: Chips },
    Get { player_id: PlayerId },
}


#[derive(Debug, Clone)]
pub enum LobbyCmd {
    SetVariant { variant: PokerVariant },
    Join { player_id: PlayerId, pin: String },
    Leave { player_id: PlayerId },
    StartHand,
    EndSession, // optional convenience for CLI
}

#[derive(Debug, Clone)]
pub enum ActionCmd {
    Check { player_id: PlayerId },
    Bet { player_id: PlayerId, amount: Chips },
    Call { player_id: PlayerId },
    Raise { player_id: PlayerId, amount: Chips },
    Fold { player_id: PlayerId },
    Draw { player_id: PlayerId, discard: Vec<usize> }, // indices into 5-card hand
}


// -------------------- Events --------------------

#[derive(Debug, Clone)]
pub enum Event {
    Ok,
    Error { message: String },

    StepChanged { step: HandStep },

    LobbyState { seated: Vec<PlayerSummary> },
    PlayerJoined { player_id: PlayerId, seat: u8 },
    PlayerLeft { player_id: PlayerId },

    PotUpdated { pot: Chips },

    ActionRequested {
        player_id: PlayerId,
        allowed: Vec<AllowedAction>,
        call_amount: Option<Chips>,
        min_raise: Option<Chips>,
    },

    DrawRequested {
        player_id: PlayerId,
        max_discards: usize,
    },

    /// Only send to the relevant player in Phase 2;
    /// in Phase 1 CLI you can just print it.
    PrivateHand {
        player_id: PlayerId,
        cards: Vec<Card>,
    },

    Showdown {
        results: Vec<ShowdownLine>,
    },

    Payout {
        winners: Vec<(PlayerId, Chips)>,
    },
}

#[derive(Debug, Clone)]
pub struct PlayerSummary {
    pub player_id: PlayerId,
    pub seat: u8,
    pub balance: Chips,
    pub games_played: u32,
    pub pots_won: u32,
    pub folds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedAction {
    Check,
    Bet,
    Call,
    Raise,
    Fold,
    Draw,
}

#[derive(Debug, Clone)]
pub struct ShowdownLine {
    pub player_id: PlayerId,
    pub hand_desc: String, // keep string for now; we will add structured later
}
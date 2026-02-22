use common::protocol::HandStep;

use super::spec::VariantSpec;

pub const SMALL_BLIND: u32 = 5;
pub const BIG_BLIND: u32 = 10;

// Metadata + step plan only. Betting/showdown semantics are still mostly shared with the
// current engine, but these steps drive Hold'em-style dealing progression.
pub const TEXAS_HOLDEM_STEPS: &[HandStep] = &[
    HandStep::WaitingForPlayers,
    HandStep::CollectForcedBets,
    HandStep::DealPrivate,    // preflop hole cards
    HandStep::BettingRound1,  // preflop betting
    HandStep::DealCommunity,  // flop
    HandStep::BettingRound2,  // flop betting
    HandStep::DealCommunity,  // turn
    HandStep::BettingRound2,  // turn betting
    HandStep::DealCommunity,  // river
    HandStep::BettingRound2,  // river betting
    HandStep::Showdown,
    HandStep::Payout,
];

pub const SPEC: VariantSpec = VariantSpec {
    name: "Texas Hold'em",
    steps: TEXAS_HOLDEM_STEPS,
    max_players: 5,
};

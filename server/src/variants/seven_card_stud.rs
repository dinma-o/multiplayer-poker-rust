use common::protocol::HandStep;

use super::spec::VariantSpec;

// Placeholder engine-flow metadata only. Gameplay/dealing remains unimplemented.
pub const SEVEN_CARD_STUD_STEPS: &[HandStep] = &[
    HandStep::WaitingForPlayers,
    HandStep::CollectForcedBets, // antes
    HandStep::DealPrivate,       // 3rd street (2 down + 1 up)
    HandStep::BettingRound1,     // bring-in / first betting
    HandStep::DealFaceUp,        // 4th street
    HandStep::BettingRound2,     // second betting
    HandStep::DealFaceUp,        // 5th street
    HandStep::BettingRound3,     // third betting
    HandStep::DealFaceUp,        // 6th street
    HandStep::BettingRound4,     // fourth betting
    HandStep::DealPrivate,       // 7th street (river, down)
    HandStep::BettingRound5,     // final betting
    HandStep::Showdown,
    HandStep::Payout,
];

pub const SPEC: VariantSpec = VariantSpec {
    name: "Seven Card Stud",
    steps: SEVEN_CARD_STUD_STEPS,
    min_players: 2,
    max_players: 5,
};

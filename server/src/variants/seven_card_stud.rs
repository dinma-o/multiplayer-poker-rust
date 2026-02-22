use common::protocol::HandStep;

use super::spec::VariantSpec;

// Placeholder engine-flow metadata only. Gameplay/dealing remains unimplemented.
pub const SEVEN_CARD_STUD_STEPS: &[HandStep] = &[
    HandStep::WaitingForPlayers,
    HandStep::CollectForcedBets,
    HandStep::DealPrivate,
    HandStep::BettingRound1,
    HandStep::DrawPhase,
    HandStep::BettingRound2,
    HandStep::Showdown,
    HandStep::Payout,
];

pub const SPEC: VariantSpec = VariantSpec {
    name: "Seven Card Stud",
    steps: SEVEN_CARD_STUD_STEPS,
    max_players: 5,
};

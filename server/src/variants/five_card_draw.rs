// variants/five_card_draw.rs

use common::protocol::HandStep;

use super::spec::VariantSpec;

pub const FIVE_CARD_DRAW_STEPS: &[HandStep] = &[
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
    name: "Five Card Draw",
    steps: FIVE_CARD_DRAW_STEPS,
    max_players: 5,
};

pub fn five_card_draw_plan() -> Vec<HandStep> {
    SPEC.steps.to_vec()
}

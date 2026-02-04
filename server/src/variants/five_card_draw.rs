// variants/five_card_draw.rs

use common::protocol::HandStep;

pub fn five_card_draw_plan() -> Vec<HandStep> {
    vec![
        HandStep::WaitingForPlayers,
        HandStep::CollectForcedBets,
        HandStep::DealPrivate,
        HandStep::BettingRound1,
        HandStep::DrawPhase,
        HandStep::BettingRound2,
        HandStep::Showdown,
        HandStep::Payout,
    ]
}

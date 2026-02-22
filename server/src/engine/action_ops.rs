use super::*;

impl ServerEngine {
    pub fn apply_action_cmd(&mut self, cmd: ActionCmd) -> Vec<Event> {
        // 1) Must be in a decision step that accepts actions
        match self.table.current_step() {
            HandStep::BettingRound1 | HandStep::BettingRound2 => self.apply_betting_action(cmd),
            HandStep::DrawPhase => self.apply_draw_action(cmd), // later
            _ => vec![Event::Error {
                message: "Actions not allowed in current step".into(),
            }],
        }
    }
}


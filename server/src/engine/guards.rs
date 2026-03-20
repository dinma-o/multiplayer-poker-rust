use super::*;

impl GameEngine {
    pub(super) fn require_step_with_message(
        &self,
        expected_step: HandStep,
        message: &'static str,
    ) -> Result<(), Vec<Event>> {
        if self.table.current_step() != expected_step {
            return Err(vec![self.emit_error(message)]);
        }

        Ok(())
    }

    pub(super) fn require_pid_by_name_get_account(
        &self,
        player_name: &str,
    ) -> Result<PlayerId, Vec<Event>> {
        match self.account_by_name(player_name) {
            Ok(acct) => Ok(acct.id),
            Err(e) => Err(vec![
                self.emit_error(format!("Get account failed: {:?}", e)),
            ]),
        }
    }

    pub(super) fn require_pid_and_name_by_name_get_account(
        &self,
        player_name: &str,
    ) -> Result<(PlayerId, String), Vec<Event>> {
        match self.account_by_name(player_name) {
            Ok(acct) => Ok((acct.id, acct.name.clone())),
            Err(e) => Err(vec![
                self.emit_error(format!("Get account failed: {:?}", e)),
            ]),
        }
    }

    pub(super) fn require_seated(
        &self,
        pid: PlayerId,
        player_name: &str,
    ) -> Result<(), Vec<Event>> {
        let is_seated = self
            .table
            .seated_players()
            .iter()
            .any(|(_, spid)| *spid == pid);
        if !is_seated {
            return Err(vec![
                self.emit_error(format!("Player {} is not seated", player_name)),
            ]);
        }

        Ok(())
    }

    pub(super) fn require_betting_turn(&self, pid: PlayerId) -> Result<(), Vec<Event>> {
        let current_player = match self.table.hand_state().betting() {
            Some(br) => br.current_player(),
            None => {
                return Err(vec![self.emit_error("Betting round not initialized")]);
            }
        };

        if pid != current_player {
            return Err(vec![self.emit_error(format!(
                "Not your turn. Expected {}",
                self.account_name(current_player)
            ))]);
        }

        Ok(())
    }

    pub(super) fn require_draw_turn(&self, pid: PlayerId) -> Result<(), Vec<Event>> {
        let expected = match self.table.hand_state().draw() {
            Some(dr) => dr.current_player(),
            None => {
                return Err(vec![self.emit_error("Draw round not initialized")]);
            }
        };

        if pid != expected {
            let expected_name = self.account_name(expected);
            return Err(vec![self.emit_error(format!(
                "Not your draw turn. Expected {expected_name}"
            ))]);
        }

        Ok(())
    }
}

use super::*;

impl ServerEngine {
    pub(super) fn emit_lobby_state(&self) -> Event {
        self.lobby_state_event()
    }

    pub(super) fn emit_hand_summary(&self) -> Event {
        self.hand_state_summary_event()
    }

    pub(super) fn emit_action_requested(&self, pid: PlayerId) -> Event {
        let pname = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
        self.action_requested_event_for(pid, &pname)
    }

    pub(super) fn emit_draw_requested(&self, pid: PlayerId) -> Event {
        let pname = self.accounts.name_from_id(pid).unwrap_or("Unknown".into());
        Event::DrawRequested {
            player_name: pname,
            max_discards: five_card_draw::MAX_DISCARDS,
        }
    }

    pub(super) fn emit_error(&self, msg: impl Into<String>) -> Event {
        Event::Error {
            message: msg.into(),
        }
    }

    pub(super) fn emit_info(&self, msg: impl Into<String>) -> Event {
        Event::Info {
            message: msg.into(),
        }
    }

    pub(super) fn emit_step_changed(&self) -> Event {
        Event::StepChanged {
            step: self.table.current_step(),
        }
    }

    pub(super) fn emit_step_changed_to(&self, step: HandStep) -> Event {
        Event::StepChanged { step }
    }

    pub(super) fn emit_pot_updated(&self, pot: i64) -> Event {
        Event::PotUpdated { pot }
    }

    pub(super) fn emit_current_pot_updated(&self) -> Event {
        self.emit_pot_updated(self.table.hand_state().pot_size() as i64)
    }
}

use std::collections::HashMap;
use crate::game_states::{PokerStateMachine, HandStep, HandState};
use crate::dealer::DealerManager;
use crate::variants::{self};
use common::constants::five_card_draw as five_card_draw_constants;

#[derive(Debug, Clone)]
pub struct FiveCardTable {
    game_state_machine: PokerStateMachine,
    hand_state: HandState<5>,
    dealer_manager: DealerManager,
    seats: Vec<Option<usize>>,
    player_lookup: HashMap<usize, PlayerAtTable>,
    pending_join_requests: Vec<usize>,
    pending_leave_requests: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerAtTable {
    pub player_id: usize,
    pub times_folded: u32,
    pub pots_won: u32,
    pub rounds_played: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    PlayerAlreadyAtTable,
    TableFull,
    RoundInProgress,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LeaveError {
    PlayerNotAtTable,
    RoundInProgress,
}

impl FiveCardTable {
    pub fn new() -> Self {
        FiveCardTable {
            game_state_machine: PokerStateMachine::new(variants::five_card_draw::five_card_draw_plan()),
            hand_state: HandState::new(five_card_draw_constants::ANTE_AMOUNT),
            dealer_manager: DealerManager::new(),
            seats: vec![None; 5],
            player_lookup: HashMap::new(),
            pending_join_requests: Vec::new(),
            pending_leave_requests: Vec::new(),
        }
    }

    pub fn join_table(&mut self, player_id: usize) -> Result<(), JoinError> {
        if self.player_lookup.contains_key(&player_id) {
            return Err(JoinError::PlayerAlreadyAtTable);
        }
        if self.player_lookup.len() >= self.seats.len() {
            self.pending_join_requests.push(player_id);
            return Err(JoinError::TableFull);
        }
        if let HandStep::WaitingForPlayers = self.game_state_machine.get_current_state() {
            self.player_lookup.insert(player_id, PlayerAtTable {
                player_id,
                times_folded: 0,
                pots_won: 0,
                rounds_played: 0,
            });
            if let Some(empty_seat) = self.seats.iter_mut().find(|seat| seat.is_none()) {
                *empty_seat = Some(player_id);
            }
            Ok(())
        } else {
            self.pending_join_requests.push(player_id);
            Err(JoinError::RoundInProgress)
        }
    }

    pub fn leave_table(&mut self, player_id: usize) -> Result<PlayerAtTable, LeaveError> {
        if !self.player_lookup.contains_key(&player_id) {
            return Err(LeaveError::PlayerNotAtTable);
        }
        if let HandStep::WaitingForPlayers = self.game_state_machine.get_current_state() {
            let player = self.player_lookup.remove(&player_id).unwrap();
            if let Some(seat) = self.seats.iter_mut().find(|seat| seat.as_ref() == Some(&player_id)) {
                *seat = None;
            }
            Ok(player)
        } else {
            self.pending_leave_requests.push(player_id);
            Err(LeaveError::RoundInProgress)
        }
    }
    pub fn apply_pending_requests(&mut self) {
        if *self.game_state_machine.get_current_state() != HandStep::WaitingForPlayers {
            return;
        }
        let pending_joins = std::mem::take(&mut self.pending_join_requests);
        let pending_leaves = std::mem::take(&mut self.pending_leave_requests); 
        
        for player_id in pending_leaves {let _ = self.leave_table(player_id);}

        for player_id in pending_joins {
            if self.join_table(player_id).is_err() {
                self.pending_join_requests.push(player_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_and_leave_table() {
        let mut table = FiveCardTable::new();
        assert_eq!(table.join_table(1), Ok(()));
        assert_eq!(table.join_table(1), Err(JoinError::PlayerAlreadyAtTable));
        assert_eq!(table.leave_table(1), Ok(PlayerAtTable {
            player_id: 1,
            times_folded: 0,
            pots_won: 0,
            rounds_played: 0,
        }));
        assert_eq!(table.leave_table(1), Err(LeaveError::PlayerNotAtTable));
    }

    #[test]
    fn test_table_full() {
        let mut table = FiveCardTable::new();
        for player_id in 1..=5 {
            assert_eq!(table.join_table(player_id), Ok(()));
        }
        assert_eq!(table.join_table(6), Err(JoinError::TableFull));
    }

    #[test]
    fn test_pending_requests() {
        let mut table = FiveCardTable::new();
        for player_id in 1..=5 {
            assert_eq!(table.join_table(player_id), Ok(()));
        }
        assert_eq!(table.join_table(6), Err(JoinError::TableFull));
        assert_eq!(table.leave_table(1), Ok(PlayerAtTable {
            player_id: 1,
            times_folded: 0,
            pots_won: 0,
            rounds_played: 0,
        }));
        assert_eq!(table.join_table(7), Ok(()));
    }

    #[test]
    fn test_apply_pending_requests() {
        let mut table = FiveCardTable::new();
        for player_id in 1..=5 {
            assert_eq!(table.join_table(player_id), Ok(()));
        }
        assert_eq!(table.join_table(6), Err(JoinError::TableFull));
        assert_eq!(table.leave_table(1), Ok(PlayerAtTable {
            player_id: 1,
            times_folded: 0,
            pots_won: 0,
            rounds_played: 0,
        }));
        table.apply_pending_requests();
        assert!(table.player_lookup.contains_key(&6));
        assert!(!table.player_lookup.contains_key(&1));
    }
}

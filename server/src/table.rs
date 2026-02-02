std::collections::HashMap;
use crate::game_state_machines::five_card_draw_poker::FiveCardDrawPokerStateMachine;
use crate::player::Player;
pub struct FiveCardTable {
    game_state_machine: FiveCardDrawPokerStateMachine,
    dealer_manager: DealerManager,
    player_ids_in_game: Vec<usize>,
    player_lookup: HashMap<usize, PlayerAtTable>,
}
pub struct PlayerAtTable {
    player_id: usize,
    player: Player,
    times_folded: u32,
    pots_won: u32,
    rounds_played: u32,
    is_folded: bool,
    is_dealer: bool,
    is_all_in: bool,
}

pub enum JoinError {
    PlayerAlreadyAtTable,
    TableFull,
    RoundInProgress,
}

pub enum LeaveError {
    PlayerNotAtTable,
    RoundInProgress,
}

impl FiveCardTable {
    pub fn new(ante_amount: u32) -> Self {
        FiveCardTable {
            game_state_machine: FiveCardDrawPokerStateMachine::new(ante_amount),
            dealer_manager: DealerManager::new(),
            player_ids_in_game: Vec::new(),
            player_lookup: HashMap::new(),
        }
    }

    pub fn join_table(&mut self, player_id: usize, player: Player) -> Result<(), JoinError> {
        if self.player_lookup.contains_key(&player_id) {
            return Err(JoinError::PlayerAlreadyAtTable);
        }
        if self.player_ids_in_game.len() > 5 {
            return Err(JoinError::TableFull);
        }
        if let FiveCardDrawPokerGameState::WaitingForPlayers = self.game_state_machine.get_current_state() {
            self.player_ids_in_game.push(player_id);
            self.player_lookup.insert(player_id, PlayerAtTable {
                player_id,
                player,
                times_folded: 0,
                pots_won: 0,
                rounds_played: 0,
                is_folded: false,
                is_dealer: false,
                is_all_in: false,
            });
            Ok(())
        } else {
            Err(JoinError::RoundInProgress)
        }
    }

    pub fn leave_table(&mut self, player_id: usize) -> Result<(), LeaveError> {
        if !self.player_lookup.contains_key(&player_id) {
            return Err(LeaveError::PlayerNotAtTable);
        }
        if let FiveCardDrawPokerGameState::WaitingForPlayers = self.game_state_machine.get_current_state() {
            self.player_lookup.remove(&player_id);
            self.player_ids_in_game.retain(|&id| id != player_id);
            Ok(())
        } else {
            Err(LeaveError::RoundInProgress)
        }
    }
}
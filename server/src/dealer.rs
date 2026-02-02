#[derive(Debug)]
pub struct DealerManager {
    dealer_position: usize,
}

impl DealerManager {
    pub fn new() -> Self {
        DealerManager { dealer_position: 0 }
    }

    pub fn rotate_dealer(&mut self, total_players: usize) {
        self.dealer_position = (self.dealer_position + 1) % total_players;
    }

    pub fn get_dealer_position(&self) -> usize {
        self.dealer_position
    }

    pub fn set_dealer_position(&mut self, position: usize) {
        self.dealer_position = position;
    }

    pub fn reset_dealer(&mut self) {
        self.dealer_position = 0;
    }

    pub fn is_dealer(&self, player_position: usize) -> bool {
        self.dealer_position == player_position
    }

    pub fn get_action_order(&self, total_players: usize) -> Vec<usize> {
        let mut order = Vec::new();
        for i in 1..=total_players {
            order.push((self.dealer_position + i) % total_players);
        }
        order
    }
}
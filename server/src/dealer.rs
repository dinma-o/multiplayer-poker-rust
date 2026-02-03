#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealerManager {
    dealer_position: usize,
}

impl DealerManager {
    pub fn new() -> Self {
        DealerManager { dealer_position: 0 }
    }

    pub fn advance_dealer(&mut self, seats: &Vec<Option<usize>>) {
        let total_seats = seats.len();
        for _ in 0..total_seats {
            self.dealer_position = (self.dealer_position + 1) % total_seats;
            if seats[self.dealer_position].is_some() {
                return;
            }
        }   
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

    pub fn is_dealer_seat(&self, player_position: usize) -> bool {
        self.dealer_position == player_position
    }

    pub fn get_action_order(&self, seats: &Vec<Option<usize>>) -> Vec<usize> {
        let mut order = Vec::new();
        let total_seats = seats.len();
        let mut index = (self.dealer_position + 1) % total_seats;

        while order.len() < seats.iter().filter(|s| s.is_some()).count() {
            if let Some(player_id) = seats[index] {
                order.push(player_id);
            }
            index = (index + 1) % total_seats;
        }
        order
    }

    pub fn ensure_dealer_in_seats(&mut self, seats: &Vec<Option<usize>>) {
        if seats[self.dealer_position].is_some() {
            return;
        }
        self.advance_dealer(seats);
    }

}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advance_dealer() {
        let mut dealer_manager = DealerManager::new();
        let seats = vec![Some(1), None, Some(2), Some(3), None];

        assert_eq!(dealer_manager.get_dealer_position(), 0);

        dealer_manager.advance_dealer(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 2);

        dealer_manager.advance_dealer(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 3);

        dealer_manager.advance_dealer(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 0);
    }

    #[test]
    fn test_get_action_order() {
        let mut dealer_manager = DealerManager::new();
        let seats = vec![Some(1), None, Some(2), Some(3), None];

        dealer_manager.set_dealer_position(0);
        let order = dealer_manager.get_action_order(&seats);
        assert_eq!(order, vec![2, 3, 1]);

        dealer_manager.set_dealer_position(2);
        let order = dealer_manager.get_action_order(&seats);
        assert_eq!(order, vec![3, 1, 2]);

        dealer_manager.set_dealer_position(3);
        let order = dealer_manager.get_action_order(&seats);
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn test_ensure_dealer_in_seats() {
        let mut dealer_manager = DealerManager::new();
        let seats = vec![Some(1), None, Some(2), Some(3), None];

        dealer_manager.set_dealer_position(1);
        dealer_manager.ensure_dealer_in_seats(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 2);

        dealer_manager.set_dealer_position(4);
        dealer_manager.ensure_dealer_in_seats(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 0);
    }

    #[test]
    fn test_is_dealer_seat() {
        let mut dealer_manager = DealerManager::new();
        dealer_manager.set_dealer_position(2);

        assert!(dealer_manager.is_dealer_seat(2));
        assert!(!dealer_manager.is_dealer_seat(0));
        assert!(!dealer_manager.is_dealer_seat(1));
        assert!(!dealer_manager.is_dealer_seat(3));
    }

    #[test]
    fn test_reset_dealer() {
        let mut dealer_manager = DealerManager::new();
        dealer_manager.set_dealer_position(3);
        dealer_manager.reset_dealer();
        assert_eq!(dealer_manager.get_dealer_position(), 0);
    }

    #[test]
    fn test_new_dealer_manager() {
        let dealer_manager = DealerManager::new();
        assert_eq!(dealer_manager.get_dealer_position(), 0);
    }

    #[test]
    fn test_set_dealer_position() {
        let mut dealer_manager = DealerManager::new();
        dealer_manager.set_dealer_position(4);
        assert_eq!(dealer_manager.get_dealer_position(), 4);
    }
    fn test_advance_dealer_all_seats_empty() {
        let mut dealer_manager = DealerManager::new();
        let seats = vec![None, None, None];

        dealer_manager.advance_dealer(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 0);
    }
    fn test_get_action_order_no_players() {
        let dealer_manager = DealerManager::new();
        let seats = vec![None, None, None];

        let order = dealer_manager.get_action_order(&seats);
        assert!(order.is_empty());
    }

    #[test]
    fn test_ensure_dealer_in_seats_all_empty() {
        let mut dealer_manager = DealerManager::new();
        let seats = vec![None, None, None];

        dealer_manager.set_dealer_position(1);
        dealer_manager.ensure_dealer_in_seats(&seats);
        assert_eq!(dealer_manager.get_dealer_position(), 1);
    }
}
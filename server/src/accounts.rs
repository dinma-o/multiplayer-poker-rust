use serde::{Deserialize, Serialize};

/// Stable identifier for an account.
pub type AccountId = u64;

/// Lightweight counters tracked per account.
/// These are updated by the game engine as outcomes occur.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AccountStats {
    /// Number of games the player participated in.
    pub games_played: u32,
    /// Number of pots the player won.
    pub pots_won: u32,
    /// Number of times the player folded.
    pub folds: u32,
}

/// Player account record.
///
/// Notes:
/// - `pin` is stored as a String to preserve leading zeros.
/// - `balance` is signed for simple arithmetic; validation is enforced in later stories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub pin: String,
    pub balance: i64,
    pub stats: AccountStats,
}

impl Account {
    /// Creates a new account with default stats.
    pub fn new(
        id: AccountId,
        name: impl Into<String>,
        pin: impl Into<String>,
        balance: i64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            pin: pin.into(),
            balance,
            stats: AccountStats::default(),
        }
    }

    /// Increments "games played" by 1.
    pub fn record_game_played(&mut self) {
        self.stats.games_played = self.stats.games_played.saturating_add(1);
    }

    /// Increments "pots won" by 1.
    pub fn record_pot_won(&mut self) {
        self.stats.pots_won = self.stats.pots_won.saturating_add(1);
    }

    /// Increments "folds" by 1.
    pub fn record_fold(&mut self) {
        self.stats.folds = self.stats.folds.saturating_add(1);
    }

    /// Returns a readable summary for CLI display and logging.
    /// Keep this stable because tests and demos may rely on it.
    pub fn summary(&self) -> String {
        format!(
            "Account Summary\n\
             - id: {}\n\
             - name: {}\n\
             - balance: {}\n\
             - stats: games_played={}, pots_won={}, folds={}",
            self.id,
            self.name,
            self.balance,
            self.stats.games_played,
            self.stats.pots_won,
            self.stats.folds
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_new_has_required_fields_and_default_stats() {
        let a = Account::new(1, "alice", "1234", 1000);
        assert_eq!(a.id, 1);
        assert_eq!(a.name, "alice");
        assert_eq!(a.pin, "1234");
        assert_eq!(a.balance, 1000);
        assert_eq!(a.stats, AccountStats::default());
    }

    #[test]
    fn stats_increment_correctly() {
        let mut a = Account::new(2, "bob", "0007", 500);

        a.record_game_played();
        a.record_game_played();
        a.record_pot_won();
        a.record_fold();
        a.record_fold();
        a.record_fold();

        assert_eq!(a.stats.games_played, 2);
        assert_eq!(a.stats.pots_won, 1);
        assert_eq!(a.stats.folds, 3);
    }

    #[test]
    fn summary_includes_all_acceptance_criteria_fields() {
        let mut a = Account::new(3, "carol", "4321", 250);
        a.record_game_played();
        a.record_pot_won();

        let s = a.summary();
        assert!(s.contains("id: 3"));
        assert!(s.contains("name: carol"));
        assert!(s.contains("balance: 250"));
        assert!(s.contains("games_played=1"));
        assert!(s.contains("pots_won=1"));
        assert!(s.contains("folds=0"));
    }
}

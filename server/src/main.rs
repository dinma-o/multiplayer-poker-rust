mod accounts;

use accounts::Account;

fn main() {
    let mut a = Account::new(1, "demo", "1234", 1000);
    a.record_game_played();
    println!("{}", a.summary());
}

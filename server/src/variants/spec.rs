use common::protocol::HandStep;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantSpec {
    pub name: &'static str,
    pub steps: &'static [HandStep],
    pub max_players: usize,
}

// server/src/variants/mod.rs
pub mod five_card_draw;
pub mod seven_card_stud;
pub mod spec;
pub mod texas_holdem;

use common::protocol::PokerVariant;

use self::spec::VariantSpec;

pub fn spec_for(variant: PokerVariant) -> VariantSpec {
    match variant {
        PokerVariant::FiveCardDraw => five_card_draw::SPEC,
        PokerVariant::TexasHoldEm => texas_holdem::SPEC,
        PokerVariant::SevenCardStud => seven_card_stud::SPEC,
    }
}

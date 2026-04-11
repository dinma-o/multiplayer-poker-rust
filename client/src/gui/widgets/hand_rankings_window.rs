use std::collections::HashMap;

use common::{
    card_deck::{Card, Rank, Suit},
    hand_rank::HAND_RANKING_GUIDE,
};

use crate::gui::theme;

use super::card::show_card;

pub fn show_hand_rankings_window(
    ctx: &egui::Context,
    open: &mut bool,
    texture_cache: &mut HashMap<String, egui::TextureHandle>,
) {
    egui::Window::new("Hand Rankings")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .default_height(620.0)
        .show(ctx, |ui| {
            ui.label(theme::section_title("Ranked From Strongest to Weakest"));
            ui.label(theme::muted(
                "Use this quick reference during betting and showdown decisions.",
            ));
            ui.add_space(10.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, entry) in HAND_RANKING_GUIDE.iter().enumerate() {
                        theme::soft_panel_frame().show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                theme::show_badge(
                                    ui,
                                    &(index + 1).to_string(),
                                    theme::PANEL_SOFT,
                                    theme::BORDER_SOFT,
                                    theme::TEXT_MUTED,
                                );
                                ui.label(theme::section_title(entry.label));
                            });

                            ui.add_space(6.0);
                            ui.label(theme::muted(entry.description));
                            ui.label(theme::small_muted(format!("Example")));
                            ui.horizontal(|ui| {
                                for card in ranking_example_cards(index) {
                                    show_card(ui, ctx, texture_cache, card);
                                }
                            });

                            ui.add_space(4.0);
                        });
                        ui.add_space(6.0);
                    }
                });
        });
}

fn ranking_example_cards(index: usize) -> [Card; 5] {
    match index {
        0 => [
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
        ],
        1 => [
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
        ],
        2 => [
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
        ],
        3 => [
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Two, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
        ],
        4 => [
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Three, Suit::Spades),
        ],
        5 => [
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::King, Suit::Spades),
        ],
        6 => [
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
        ],
        7 => [
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Diamonds),
            Card::new(Rank::Nine, Suit::Clubs),
        ],
        8 => [
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        _ => [
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Clubs),
        ],
    }
}

use std::collections::HashMap;

use common::card_deck::{Card, Rank, Suit};

/// Rendered size of every card image.
pub const CARD_SIZE: egui::Vec2 = egui::vec2(60.0, 84.0);

/// Render a face-up card by loading its PNG from disk, uploading it to the GPU
/// once, and caching the resulting `TextureHandle` in `cache`.
pub fn show_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    card: Card,
) {
    let key = format!("{}_of_{}", rank_str(card.rank()), suit_str(card.suit()));

    if !cache.contains_key(&key) {
        let path = format!(
            "{}/assets/cards/{}.png",
            env!("CARGO_MANIFEST_DIR"),
            key,
        );
        match load_texture(ctx, &path, &key) {
            Some(handle) => {
                cache.insert(key.clone(), handle);
            }
            None => {
                // Asset missing or unreadable — render a red placeholder.
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(160, 30, 30))
                    .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(200)))
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.set_min_size(CARD_SIZE);
                        ui.centered_and_justified(|ui| {
                            ui.label(egui::RichText::new("?").color(egui::Color32::WHITE));
                        });
                    });
                return;
            }
        }
    }

    if let Some(handle) = cache.get(&key) {
        ui.add(egui::Image::new(handle).fit_to_exact_size(CARD_SIZE));
    }
}

/// Render a face-down card as a solid blue rounded frame.
pub fn show_card_back(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(30, 60, 160))
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(200)))
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.set_min_size(CARD_SIZE);
        });
}

/// Read a PNG from disk, decode it, and upload it as an egui texture.
/// Returns `None` if the file cannot be read or decoded.
fn load_texture(ctx: &egui::Context, path: &str, name: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    );
    Some(ctx.load_texture(name, color_image, egui::TextureOptions::default()))
}

fn rank_str(rank: Rank) -> &'static str {
    match rank {
        Rank::Ace => "ace",
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "10",
        Rank::Jack => "jack",
        Rank::Queen => "queen",
        Rank::King => "king",
    }
}

fn suit_str(suit: Suit) -> &'static str {
    match suit {
        Suit::Hearts => "hearts",
        Suit::Diamonds => "diamonds",
        Suit::Clubs => "clubs",
        Suit::Spades => "spades",
    }
}

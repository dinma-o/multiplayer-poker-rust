use std::collections::HashMap;

use common::card_deck::{Card, Rank, Suit};

use crate::gui::theme;

/// Rendered size of every card image.
pub const CARD_SIZE: egui::Vec2 = egui::vec2(54.0, 76.0);
pub const COMPACT_CARD_SIZE: egui::Vec2 = egui::vec2(34.0, 48.0);
const CARD_BACK_TEXTURE_KEY: &str = "card_back";
const CARD_BACK_BYTES: &[u8] = include_bytes!("../../../assets/cards/back_of_card.png");

/// Render a face-up card. PNG bytes are embedded in the binary at compile time
/// via `include_bytes!`, so no filesystem access is required at runtime.
pub fn show_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    card: Card,
) {
    show_card_at_size(ui, ctx, cache, card, CARD_SIZE);
}

pub fn show_card_compact(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    card: Card,
) {
    show_card_at_size(ui, ctx, cache, card, COMPACT_CARD_SIZE);
}

pub fn show_card_back(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
) {
    show_card_back_at_size(ui, ctx, cache, CARD_SIZE);
}

fn show_card_at_size(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    card: Card,
    size: egui::Vec2,
) {
    let key = format!("{}_of_{}", rank_str(card.rank()), suit_str(card.suit()));

    if !cache.contains_key(&key) {
        match card_bytes(&key) {
            Some(bytes) => {
                if let Some(handle) = load_texture(ctx, bytes, &key) {
                    cache.insert(key.clone(), handle);
                }
            }
            None => {
                show_placeholder(ui);
                return;
            }
        }
    }

    if let Some(handle) = cache.get(&key) {
        ui.add(egui::Image::new(handle).fit_to_exact_size(size));
    } else {
        show_placeholder_at_size(ui, size);
    }
}

pub fn show_card_back_compact(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
) {
    show_card_back_at_size(ui, ctx, cache, COMPACT_CARD_SIZE);
}

fn show_card_back_at_size(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    size: egui::Vec2,
) {
    if !cache.contains_key(CARD_BACK_TEXTURE_KEY)
        && let Some(handle) = load_texture(ctx, CARD_BACK_BYTES, CARD_BACK_TEXTURE_KEY)
    {
        cache.insert(CARD_BACK_TEXTURE_KEY.to_string(), handle);
    }

    if let Some(handle) = cache.get(CARD_BACK_TEXTURE_KEY) {
        ui.add(egui::Image::new(handle).fit_to_exact_size(size));
    } else {
        show_card_back_fallback(ui, size);
    }
}

fn show_card_back_fallback(ui: &mut egui::Ui, size: egui::Vec2) {
    egui::Frame::new()
        .fill(theme::PANEL_SOFT)
        .stroke(egui::Stroke::new(1.5, theme::BORDER))
        .corner_radius(12.0)
        .show(ui, |ui| {
            ui.set_min_size(size);
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("POKER")
                        .small()
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
            });
        });
}

pub fn show_card_slot(ui: &mut egui::Ui, label: &str) {
    show_card_slot_at_size(ui, label, CARD_SIZE, 16.0);
}

pub fn show_card_slot_compact(ui: &mut egui::Ui, label: &str) {
    show_card_slot_at_size(ui, label, COMPACT_CARD_SIZE, 12.0);
}

fn show_card_slot_at_size(ui: &mut egui::Ui, label: &str, size: egui::Vec2, corner_radius: f32) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_premultiplied(255, 255, 255, 10))
        .stroke(egui::Stroke::new(
            1.5,
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 45),
        ))
        .corner_radius(corner_radius)
        .show(ui, |ui| {
            ui.set_min_size(size);
            ui.centered_and_justified(|ui| {
                ui.label(theme::small_muted(label));
            });
        });
}

fn show_placeholder(ui: &mut egui::Ui) {
    show_placeholder_at_size(ui, CARD_SIZE);
}

fn show_placeholder_at_size(ui: &mut egui::Ui, size: egui::Vec2) {
    egui::Frame::new()
        .fill(theme::DANGER_SOFT)
        .stroke(egui::Stroke::new(1.5, theme::DANGER))
        .corner_radius(16.0)
        .show(ui, |ui| {
            ui.set_min_size(size);
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("?").color(theme::TEXT).strong());
            });
        });
}

/// Decode PNG bytes and upload as an egui texture. Returns `None` on failure.
fn load_texture(ctx: &egui::Context, bytes: &[u8], name: &str) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    Some(ctx.load_texture(name, color_image, egui::TextureOptions::default()))
}

/// Returns the compile-time embedded PNG bytes for a card key such as
/// `"ace_of_hearts"` or `"10_of_clubs"`. Returns `None` for unknown keys.
fn card_bytes(key: &str) -> Option<&'static [u8]> {
    match key {
        // ── Aces ─────────────────────────────────────────────────────────────
        "ace_of_clubs" => Some(include_bytes!("../../../assets/cards/ace_of_clubs.png")),
        "ace_of_diamonds" => Some(include_bytes!("../../../assets/cards/ace_of_diamonds.png")),
        "ace_of_hearts" => Some(include_bytes!("../../../assets/cards/ace_of_hearts.png")),
        "ace_of_spades" => Some(include_bytes!("../../../assets/cards/ace_of_spades.png")),
        // ── 2s ───────────────────────────────────────────────────────────────
        "2_of_clubs" => Some(include_bytes!("../../../assets/cards/2_of_clubs.png")),
        "2_of_diamonds" => Some(include_bytes!("../../../assets/cards/2_of_diamonds.png")),
        "2_of_hearts" => Some(include_bytes!("../../../assets/cards/2_of_hearts.png")),
        "2_of_spades" => Some(include_bytes!("../../../assets/cards/2_of_spades.png")),
        // ── 3s ───────────────────────────────────────────────────────────────
        "3_of_clubs" => Some(include_bytes!("../../../assets/cards/3_of_clubs.png")),
        "3_of_diamonds" => Some(include_bytes!("../../../assets/cards/3_of_diamonds.png")),
        "3_of_hearts" => Some(include_bytes!("../../../assets/cards/3_of_hearts.png")),
        "3_of_spades" => Some(include_bytes!("../../../assets/cards/3_of_spades.png")),
        // ── 4s ───────────────────────────────────────────────────────────────
        "4_of_clubs" => Some(include_bytes!("../../../assets/cards/4_of_clubs.png")),
        "4_of_diamonds" => Some(include_bytes!("../../../assets/cards/4_of_diamonds.png")),
        "4_of_hearts" => Some(include_bytes!("../../../assets/cards/4_of_hearts.png")),
        "4_of_spades" => Some(include_bytes!("../../../assets/cards/4_of_spades.png")),
        // ── 5s ───────────────────────────────────────────────────────────────
        "5_of_clubs" => Some(include_bytes!("../../../assets/cards/5_of_clubs.png")),
        "5_of_diamonds" => Some(include_bytes!("../../../assets/cards/5_of_diamonds.png")),
        "5_of_hearts" => Some(include_bytes!("../../../assets/cards/5_of_hearts.png")),
        "5_of_spades" => Some(include_bytes!("../../../assets/cards/5_of_spades.png")),
        // ── 6s ───────────────────────────────────────────────────────────────
        "6_of_clubs" => Some(include_bytes!("../../../assets/cards/6_of_clubs.png")),
        "6_of_diamonds" => Some(include_bytes!("../../../assets/cards/6_of_diamonds.png")),
        "6_of_hearts" => Some(include_bytes!("../../../assets/cards/6_of_hearts.png")),
        "6_of_spades" => Some(include_bytes!("../../../assets/cards/6_of_spades.png")),
        // ── 7s ───────────────────────────────────────────────────────────────
        "7_of_clubs" => Some(include_bytes!("../../../assets/cards/7_of_clubs.png")),
        "7_of_diamonds" => Some(include_bytes!("../../../assets/cards/7_of_diamonds.png")),
        "7_of_hearts" => Some(include_bytes!("../../../assets/cards/7_of_hearts.png")),
        "7_of_spades" => Some(include_bytes!("../../../assets/cards/7_of_spades.png")),
        // ── 8s ───────────────────────────────────────────────────────────────
        "8_of_clubs" => Some(include_bytes!("../../../assets/cards/8_of_clubs.png")),
        "8_of_diamonds" => Some(include_bytes!("../../../assets/cards/8_of_diamonds.png")),
        "8_of_hearts" => Some(include_bytes!("../../../assets/cards/8_of_hearts.png")),
        "8_of_spades" => Some(include_bytes!("../../../assets/cards/8_of_spades.png")),
        // ── 9s ───────────────────────────────────────────────────────────────
        "9_of_clubs" => Some(include_bytes!("../../../assets/cards/9_of_clubs.png")),
        "9_of_diamonds" => Some(include_bytes!("../../../assets/cards/9_of_diamonds.png")),
        "9_of_hearts" => Some(include_bytes!("../../../assets/cards/9_of_hearts.png")),
        "9_of_spades" => Some(include_bytes!("../../../assets/cards/9_of_spades.png")),
        // ── 10s ──────────────────────────────────────────────────────────────
        "10_of_clubs" => Some(include_bytes!("../../../assets/cards/10_of_clubs.png")),
        "10_of_diamonds" => Some(include_bytes!("../../../assets/cards/10_of_diamonds.png")),
        "10_of_hearts" => Some(include_bytes!("../../../assets/cards/10_of_hearts.png")),
        "10_of_spades" => Some(include_bytes!("../../../assets/cards/10_of_spades.png")),
        // ── Jacks ────────────────────────────────────────────────────────────
        "jack_of_clubs" => Some(include_bytes!("../../../assets/cards/jack_of_clubs.png")),
        "jack_of_diamonds" => Some(include_bytes!("../../../assets/cards/jack_of_diamonds.png")),
        "jack_of_hearts" => Some(include_bytes!("../../../assets/cards/jack_of_hearts.png")),
        "jack_of_spades" => Some(include_bytes!("../../../assets/cards/jack_of_spades.png")),
        // ── Queens ───────────────────────────────────────────────────────────
        "queen_of_clubs" => Some(include_bytes!("../../../assets/cards/queen_of_clubs.png")),
        "queen_of_diamonds" => Some(include_bytes!(
            "../../../assets/cards/queen_of_diamonds.png"
        )),
        "queen_of_hearts" => Some(include_bytes!("../../../assets/cards/queen_of_hearts.png")),
        "queen_of_spades" => Some(include_bytes!("../../../assets/cards/queen_of_spades.png")),
        // ── Kings ────────────────────────────────────────────────────────────
        "king_of_clubs" => Some(include_bytes!("../../../assets/cards/king_of_clubs.png")),
        "king_of_diamonds" => Some(include_bytes!("../../../assets/cards/king_of_diamonds.png")),
        "king_of_hearts" => Some(include_bytes!("../../../assets/cards/king_of_hearts.png")),
        "king_of_spades" => Some(include_bytes!("../../../assets/cards/king_of_spades.png")),
        _ => None,
    }
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

use common::wire::{HistoryEntry, HistoryKind};

use crate::gui::theme;

/// Render a scrollable event log panel.
///
/// Auto-scrolls to the bottom when new entries arrive.
pub fn show_event_log(ui: &mut egui::Ui, entries: &[HistoryEntry]) {
    ui.label(theme::section_title("Hand log"));
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("event_log_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if entries.is_empty() {
                theme::soft_panel_frame().show(ui, |ui| {
                    ui.label(theme::small_muted("No events yet."));
                });
                return;
            }

            for entry in entries {
                let color = kind_color(&entry.kind);
                egui::Frame::new()
                    .fill(theme::PANEL_ALT)
                    .stroke(egui::Stroke::new(1.0, color))
                    .corner_radius(16.0)
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&entry.message)
                                .small()
                                .color(theme::TEXT),
                        );
                    });
                ui.add_space(6.0);
            }
        });
}

fn kind_color(kind: &HistoryKind) -> egui::Color32 {
    match kind {
        HistoryKind::PlayerJoined | HistoryKind::PlayerLeft => theme::TEXT_MUTED,
        HistoryKind::PotUpdated => theme::GOLD,
        HistoryKind::ActionPrompt | HistoryKind::DrawPrompt => {
            egui::Color32::from_rgb(255, 160, 60)
        }
        HistoryKind::Showdown | HistoryKind::Payout => theme::ACCENT,
        HistoryKind::StepChanged => theme::INFO,
        HistoryKind::Info => theme::TEXT_MUTED,
        HistoryKind::Error => theme::DANGER,
    }
}

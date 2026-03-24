use common::wire::{HistoryEntry, HistoryKind};

/// Render a scrollable event log panel.
///
/// Auto-scrolls to the bottom when new entries arrive.
pub fn show_event_log(ui: &mut egui::Ui, entries: &[HistoryEntry]) {
    ui.label(egui::RichText::new("Event Log").strong());
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt("event_log_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if entries.is_empty() {
                ui.label(
                    egui::RichText::new("No events yet.")
                        .small()
                        .color(egui::Color32::from_gray(100)),
                );
                return;
            }

            for entry in entries {
                let color = kind_color(&entry.kind);
                ui.label(
                    egui::RichText::new(&entry.message)
                        .small()
                        .color(color),
                );
            }
        });
}

fn kind_color(kind: &HistoryKind) -> egui::Color32 {
    match kind {
        HistoryKind::PlayerJoined | HistoryKind::PlayerLeft => {
            egui::Color32::from_gray(140)
        }
        HistoryKind::PotUpdated => egui::Color32::from_rgb(220, 200, 80),
        HistoryKind::ActionPrompt | HistoryKind::DrawPrompt => egui::Color32::from_rgb(255, 160, 60),
        HistoryKind::Showdown | HistoryKind::Payout => {
            egui::Color32::from_rgb(255, 200, 50)
        }
        HistoryKind::StepChanged => egui::Color32::from_rgb(100, 180, 255),
        HistoryKind::Info => egui::Color32::from_gray(180),
        HistoryKind::Error => egui::Color32::from_rgb(220, 80, 80),
    }
}

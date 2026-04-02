use common::wire::HistoryEntry;

use crate::gui::theme;

/// Render showdown hand descriptions and payout amounts.
///
/// Displayed as a highlighted section above the event log.
/// Pass `None` for either to skip that section.
pub fn show_results(
    ui: &mut egui::Ui,
    showdown: Option<&HistoryEntry>,
    payout: Option<&HistoryEntry>,
) {
    if showdown.is_none() && payout.is_none() {
        return;
    }

    theme::info_frame().show(ui, |ui| {
        if let Some(entry) = showdown {
            ui.label(
                egui::RichText::new("Showdown")
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.add_space(2.0);

            if let Some(results) = &entry.showdown_results {
                for line in results {
                    theme::soft_panel_frame().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                line.player_name, line.hand_desc
                            ))
                            .small()
                            .color(theme::TEXT),
                        );
                    });
                }
            } else {
                ui.label(theme::small_muted(&entry.message));
            }

            if payout.is_some() {
                ui.add_space(6.0);
            }
        }

        if let Some(entry) = payout {
            ui.label(egui::RichText::new("Payout").strong().color(theme::GOLD));
            ui.add_space(2.0);

            if let Some(results) = &entry.payout_results {
                for line in results {
                    theme::soft_panel_frame().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} wins {}",
                                line.player_name, line.amount
                            ))
                            .small()
                            .color(theme::ACCENT),
                        );
                    });
                }
            } else {
                ui.label(theme::small_muted(&entry.message));
            }
        }
    });

    ui.add_space(6.0);
}

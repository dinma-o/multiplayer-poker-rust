use common::wire::HistoryEntry;

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

    egui::Frame::new()
        .fill(egui::Color32::from_rgb(40, 35, 10))
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(180, 150, 30)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            // ── Showdown ──────────────────────────────────────────────────
            if let Some(entry) = showdown {
                ui.label(
                    egui::RichText::new("Showdown")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 200, 50)),
                );
                ui.add_space(2.0);

                if let Some(results) = &entry.showdown_results {
                    for line in results {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                line.player_name, line.hand_desc
                            ))
                            .small()
                            .color(egui::Color32::WHITE),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new(&entry.message)
                            .small()
                            .color(egui::Color32::from_gray(180)),
                    );
                }

                if payout.is_some() {
                    ui.add_space(6.0);
                }
            }

            // ── Payout ────────────────────────────────────────────────────
            if let Some(entry) = payout {
                ui.label(
                    egui::RichText::new("Payout")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 200, 50)),
                );
                ui.add_space(2.0);

                if let Some(results) = &entry.payout_results {
                    for line in results {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} wins ${}",
                                line.player_name, line.amount
                            ))
                            .small()
                            .color(egui::Color32::from_rgb(100, 220, 100)),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new(&entry.message)
                            .small()
                            .color(egui::Color32::from_gray(180)),
                    );
                }
            }
        });

    ui.add_space(6.0);
}

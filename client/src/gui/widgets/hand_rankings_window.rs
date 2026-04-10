use common::hand_rank::HAND_RANKING_GUIDE;

use crate::gui::theme;

pub fn show_hand_rankings_window(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("Hand Rankings")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_width(440.0)
        .default_height(520.0)
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
                            ui.add_space(4.0);
                            ui.label(theme::muted(entry.description));
                            ui.label(theme::small_muted(format!("Example: {}", entry.example)));
                        });
                        ui.add_space(6.0);
                    }
                });
        });
}

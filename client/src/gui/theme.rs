use egui::{Color32, FontId, Frame, Margin, RichText, Stroke, TextStyle, Ui, epaint::Shadow};

pub const BACKGROUND: Color32 = Color32::from_rgb(10, 13, 12);
pub const BACKGROUND_LAYER: Color32 = Color32::from_rgb(15, 19, 17);
pub const PANEL: Color32 = Color32::from_rgb(22, 27, 25);
pub const PANEL_ALT: Color32 = Color32::from_rgb(32, 40, 36);
pub const PANEL_SOFT: Color32 = Color32::from_rgb(18, 23, 21);
pub const BORDER: Color32 = Color32::from_rgb(59, 71, 66);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(42, 51, 47);
pub const ACCENT: Color32 = Color32::from_rgb(126, 231, 179);
pub const ACCENT_STRONG: Color32 = Color32::from_rgb(52, 211, 153);
pub const ACCENT_GLOW: Color32 = Color32::from_rgba_premultiplied(52, 211, 153, 32);
pub const GOLD: Color32 = Color32::from_rgb(245, 194, 66);
pub const GOLD_SOFT: Color32 = Color32::from_rgba_premultiplied(245, 194, 66, 28);
pub const INFO: Color32 = Color32::from_rgb(96, 165, 250);
pub const TEXT: Color32 = Color32::from_rgb(242, 247, 244);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(163, 173, 168);
pub const DANGER: Color32 = Color32::from_rgb(248, 113, 113);
pub const DANGER_SOFT: Color32 = Color32::from_rgb(58, 28, 28);
pub const TABLE_WOOD: Color32 = Color32::from_rgb(120, 53, 15);
pub const TABLE_FELT: Color32 = Color32::from_rgb(11, 58, 42);
pub const TABLE_EDGE: Color32 = Color32::from_rgb(18, 94, 67);
pub const RECOMMENDED_VIEW_WIDTH: f32 = 1120.0;
pub const RECOMMENDED_VIEW_HEIGHT: f32 = 720.0;

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size = egui::vec2(40.0, 34.0);
    style.spacing.text_edit_width = 200.0;
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(24.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));

    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = BACKGROUND_LAYER;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.extreme_bg_color = PANEL_SOFT;
    visuals.faint_bg_color = PANEL_ALT;
    visuals.code_bg_color = PANEL_SOFT;
    visuals.selection.bg_fill = Color32::from_rgb(34, 82, 65);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke.color = TEXT_MUTED;
    visuals.widgets.inactive.bg_fill = PANEL_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke.color = TEXT;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 54, 49);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.fg_stroke.color = TEXT;
    visuals.widgets.active.bg_fill = Color32::from_rgb(49, 64, 58);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke.color = TEXT;
    visuals.widgets.open.bg_fill = PANEL_ALT;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.open.fg_stroke.color = TEXT;

    style.visuals = visuals;
    ctx.set_style(style);
}

pub fn paint_screen_background(ui: &mut Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, BACKGROUND);

    let large_radius = rect.width().max(rect.height()) * 0.36;
    painter.circle_filled(
        egui::pos2(rect.center().x, rect.top() + rect.height() * 0.22),
        large_radius,
        ACCENT_GLOW,
    );
    painter.circle_filled(
        egui::pos2(
            rect.right() - rect.width() * 0.12,
            rect.top() + rect.height() * 0.18,
        ),
        rect.width().min(rect.height()) * 0.17,
        GOLD_SOFT,
    );
    painter.circle_filled(
        egui::pos2(
            rect.left() + rect.width() * 0.18,
            rect.bottom() - rect.height() * 0.1,
        ),
        rect.width().min(rect.height()) * 0.2,
        Color32::from_rgba_premultiplied(18, 23, 21, 160),
    );
}

pub fn panel_frame() -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(20.0)
        .inner_margin(Margin::same(14))
        .shadow(Shadow {
            offset: [0, 10],
            blur: 24,
            spread: 0,
            color: Color32::from_rgba_premultiplied(0, 0, 0, 72),
        })
}

pub fn accent_panel_frame() -> Frame {
    panel_frame().stroke(Stroke::new(
        1.0,
        Color32::from_rgba_premultiplied(126, 231, 179, 100),
    ))
}

pub fn soft_panel_frame() -> Frame {
    Frame::new()
        .fill(PANEL_ALT)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(16.0)
        .inner_margin(Margin::same(12))
}

pub fn info_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgb(19, 38, 32))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(126, 231, 179, 110),
        ))
        .corner_radius(16.0)
        .inner_margin(Margin::same(12))
}

pub fn error_frame() -> Frame {
    Frame::new()
        .fill(DANGER_SOFT)
        .stroke(Stroke::new(1.0, DANGER))
        .corner_radius(16.0)
        .inner_margin(Margin::same(12))
}

pub fn table_shell_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgb(15, 18, 16))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(126, 231, 179, 75),
        ))
        .corner_radius(28.0)
        .inner_margin(Margin::same(14))
        .shadow(Shadow {
            offset: [0, 12],
            blur: 28,
            spread: 0,
            color: Color32::from_rgba_premultiplied(0, 0, 0, 96),
        })
}

pub fn badge_frame(fill: Color32, stroke: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(255.0)
        .inner_margin(Margin::symmetric(8, 5))
}

pub fn eyebrow(text: impl Into<String>) -> RichText {
    RichText::new(text.into().to_uppercase())
        .size(10.0)
        .color(ACCENT)
        .strong()
}

pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(20.0).color(TEXT).strong()
}

pub fn hero(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(28.0).color(TEXT).strong()
}

pub fn section_title(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(16.0).color(TEXT).strong()
}

pub fn stat_value(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(16.0).color(TEXT).strong()
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(13.0).color(TEXT_MUTED)
}

pub fn small_muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(11.0).color(TEXT_MUTED)
}

pub fn show_badge(ui: &mut Ui, label: &str, fill: Color32, stroke: Color32, text: Color32) {
    badge_frame(fill, stroke).show(ui, |ui| {
        ui.label(RichText::new(label).size(12.0).color(text).strong());
    });
}

pub fn show_stat_card(ui: &mut Ui, label: &str, value: impl ToString) {
    soft_panel_frame().show(ui, |ui| {
        ui.label(eyebrow(label));
        ui.add_space(6.0);
        ui.label(stat_value(value.to_string()));
    });
}

pub fn show_stat_row(ui: &mut Ui, label: &str, value: impl ToString) {
    soft_panel_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(muted(label));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(value.to_string()).color(TEXT).strong());
            });
        });
    });
}

pub fn show_small_screen_notice(ctx: &egui::Context, dismissed: &mut bool) {
    let size = ctx.screen_rect().size();
    let is_small = size.x < RECOMMENDED_VIEW_WIDTH || size.y < RECOMMENDED_VIEW_HEIGHT;

    if !is_small {
        *dismissed = false;
        return;
    }

    if *dismissed {
        return;
    }

    egui::Area::new("small_screen_notice".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 12.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            info_frame().show(ui, |ui| {
                ui.set_max_width(460.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(section_title("Window is a bit tight for this layout"));
                        ui.label(muted(format!(
                            "Current viewport: {:.0} x {:.0}. Recommended minimum: {:.0} x {:.0}.",
                            size.x, size.y, RECOMMENDED_VIEW_WIDTH, RECOMMENDED_VIEW_HEIGHT
                        )));
                        ui.label(small_muted(
                            "You can keep using the app, but some panels may feel cramped.",
                        ));
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("Dismiss").clicked() {
                            *dismissed = true;
                        }
                    });
                });
            });
        });
}

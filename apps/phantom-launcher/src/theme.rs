//! Dark Obsidian UI theme matching Phantom EQ (`#101117` panel, `#161820` cards, `#38BDF8` cyan).

use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Ui, Visuals};

pub const BG_PANEL: Color32 = Color32::from_rgb(16, 17, 23); // #101117
pub const BG_CARD: Color32 = Color32::from_rgb(22, 24, 32); // #161820
pub const BG_ELEVATED: Color32 = Color32::from_rgb(28, 31, 43); // #1C1F2B
pub const BG_DISPLAY: Color32 = Color32::from_rgb(13, 14, 20); // #0D0E14
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(38, 42, 56); // #262A38
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(56, 63, 84); // #383F54

pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(241, 245, 249); // #F1F5F9
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184); // #94A3B8
pub const TEXT_DIMMED: Color32 = Color32::from_rgb(100, 116, 139); // #64748B

pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248); // #38BDF8
pub const ACCENT_CYAN_BG: Color32 = Color32::from_rgb(14, 52, 74);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(16, 185, 129); // #10B981
pub const ACCENT_GREEN_BG: Color32 = Color32::from_rgb(12, 54, 44);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B
pub const ACCENT_AMBER_BG: Color32 = Color32::from_rgb(66, 44, 12);
pub const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444

pub fn apply_obsidian_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = BG_DISPLAY;
    visuals.faint_bg_color = BG_ELEVATED;

    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.weak_bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(36, 41, 56);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(36, 41, 56);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = ACCENT_CYAN_BG;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(56, 189, 248, 55);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_CYAN);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    ctx.set_style(style);
}

pub fn draw_pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32, border: Color32) {
    egui::Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(10.5))
                    .strong()
                    .color(fg),
            );
        });
}

pub fn primary_button(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .font(FontId::proportional(12.0))
                .strong()
                .color(fg),
        )
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, bg))
        .corner_radius(CornerRadius::same(6))
        .min_size(egui::vec2(102.0, 28.0)),
    )
}

pub fn subtle_button(ui: &mut Ui, text: &str, fg: Color32, border: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .font(FontId::proportional(11.5))
                .color(fg),
        )
        .fill(BG_ELEVATED)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(6))
        .min_size(egui::vec2(0.0, 28.0)),
    )
}

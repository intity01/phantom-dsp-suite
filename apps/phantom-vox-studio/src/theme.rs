//! Dark Obsidian UI theme & optional Windows Thai/Unicode fallback font loader.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Response,
    RichText, Stroke, Ui, Vec2, Visuals,
};
use phantom_vox_dsp::{ConfidenceLevel, FrameClass};
use std::sync::Arc;

pub const BG_PANEL: Color32 = Color32::from_rgb(15, 16, 22);
pub const BG_CARD: Color32 = Color32::from_rgb(22, 24, 32);
pub const BG_DISPLAY: Color32 = Color32::from_rgb(12, 13, 19);
pub const BG_TRACK: Color32 = Color32::from_rgb(18, 20, 28);
pub const BG_HEADER: Color32 = Color32::from_rgb(25, 28, 38);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(38, 42, 56);
pub const BORDER_ACTIVE: Color32 = Color32::from_rgb(66, 76, 102);

pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(241, 245, 249);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);
pub const TEXT_DIMMED: Color32 = Color32::from_rgb(100, 116, 139);

pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(16, 185, 129);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
pub const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68);
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(167, 139, 250);
pub const ACCENT_PINK: Color32 = Color32::from_rgb(244, 114, 182);
pub const WAVE_ORIG: Color32 = Color32::from_rgb(100, 116, 139);

pub const LAYER_COLORS: [Color32; 6] = [
    Color32::from_rgb(52, 211, 153),  // Emerald
    Color32::from_rgb(251, 191, 36),  // Amber Gold
    Color32::from_rgb(192, 132, 252), // Violet
    Color32::from_rgb(244, 114, 182), // Rose
    Color32::from_rgb(96, 165, 250),  // Sky Blue
    Color32::from_rgb(251, 146, 60),  // Orange
];

#[inline]
pub fn layer_color(idx: usize) -> Color32 {
    LAYER_COLORS[idx % LAYER_COLORS.len()]
}

#[inline]
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    if alpha == 0 {
        return Color32::TRANSPARENT;
    }
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

pub fn confidence_color(level: ConfidenceLevel) -> Color32 {
    match level {
        ConfidenceLevel::High => ACCENT_GREEN,
        ConfidenceLevel::Medium => ACCENT_AMBER,
        ConfidenceLevel::Low => ACCENT_RED,
    }
}

pub fn frame_class_color(cls: FrameClass) -> Color32 {
    match cls {
        FrameClass::Voiced => ACCENT_CYAN,
        FrameClass::Sibilance => ACCENT_AMBER,
        FrameClass::Breath => ACCENT_PURPLE,
        FrameClass::Transient => ACCENT_PINK,
        FrameClass::UnvoicedConsonant => TEXT_MUTED,
        FrameClass::Silence => Color32::TRANSPARENT,
    }
}

pub fn apply_obsidian_theme(ctx: &egui::Context) {
    install_fallback_fonts(ctx);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = BG_DISPLAY;
    visuals.faint_bg_color = BG_TRACK;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 34, 46);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(30, 34, 46);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 48, 66);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 48, 66);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_ACTIVE);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(48, 62, 88);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    visuals.selection.bg_fill = with_alpha(ACCENT_CYAN, 70);
    visuals.selection.stroke = Stroke::new(1.2_f32, ACCENT_CYAN);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.scroll.bar_width = 7.0;
    ctx.set_style(style);
}

/// Gracefully loads a Windows Thai/Unicode fallback font if installed; silently skips if absent.
pub fn install_fallback_fonts(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\leelawui.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        "/mnt/c/Windows/Fonts/leelawui.ttf",
        "/mnt/c/Windows/Fonts/tahoma.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if bytes.len() > 1024 {
                let mut fonts = FontDefinitions::default();
                fonts.font_data.insert(
                    "thai_fallback".to_owned(),
                    Arc::new(FontData::from_owned(bytes)),
                );
                if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
                    prop.push("thai_fallback".to_owned());
                }
                if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                    mono.push("thai_fallback".to_owned());
                }
                ctx.set_fonts(fonts);
                return;
            }
        }
    }
}

pub fn pill_badge(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(10.5))
                    .strong()
                    .color(fg),
            );
        });
}

pub fn toggle_chip(ui: &mut Ui, label: &str, active: bool, accent: Color32) -> Response {
    let fill = if active {
        with_alpha(accent, 48)
    } else {
        Color32::from_rgb(28, 31, 42)
    };
    let stroke = if active {
        Stroke::new(1.0_f32, accent)
    } else {
        Stroke::new(1.0_f32, BORDER_SUBTLE)
    };
    let fg = if active { accent } else { TEXT_MUTED };
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(11.5))
                .strong()
                .color(fg),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 24.0)),
    )
}

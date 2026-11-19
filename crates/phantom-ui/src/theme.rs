//! Phantom UI Design System & Theme Engine
//! Sleek Dark Obsidian palette inspired by professional audio workstations.

use nih_plug_egui::egui::{Color32, Visuals};

pub const BG_PANEL: Color32 = Color32::from_rgb(16, 17, 23);
pub const BG_CARD: Color32 = Color32::from_rgb(22, 24, 32);
pub const BG_DISPLAY: Color32 = Color32::from_rgb(13, 14, 20);
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(241, 245, 249);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);
pub const TEXT_DIMMED: Color32 = Color32::from_rgb(100, 116, 139);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(16, 185, 129);
pub const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(167, 139, 250);

pub const BAND_COLORS: [Color32; 8] = [
    Color32::from_rgb(239, 68, 68),  // Coral Red
    Color32::from_rgb(249, 115, 22), // Orange
    Color32::from_rgb(245, 158, 11), // Amber
    Color32::from_rgb(16, 185, 129), // Emerald Green
    Color32::from_rgb(6, 182, 212),  // Electric Cyan
    Color32::from_rgb(59, 130, 246), // Royal Blue
    Color32::from_rgb(139, 92, 246), // Purple
    Color32::from_rgb(236, 72, 153), // Rose Pink
];

/// Returns the accent color assigned to band `index` (cycles cleanly across 24+ bands).
#[inline]
pub fn band_color(index: usize) -> Color32 {
    BAND_COLORS[index % BAND_COLORS.len()]
}

/// Applies non-premultiplied `alpha` (0..=255) to an opaque or translucent color safely.
/// Uses `Color32::from_rgba_unmultiplied` so OpenGL blending never produces additive glow artifacts.
#[inline]
pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    if alpha == 0 {
        return Color32::TRANSPARENT;
    }
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

pub fn configure_visuals(ctx: &nih_plug_egui::egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    ctx.set_visuals(visuals);
}

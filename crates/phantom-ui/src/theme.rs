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
    Color32::from_rgb(239, 68, 68),  // Band 1: Coral Red
    Color32::from_rgb(249, 115, 22), // Band 2: Orange
    Color32::from_rgb(245, 158, 11), // Band 3: Amber
    Color32::from_rgb(16, 185, 129), // Band 4: Emerald Green
    Color32::from_rgb(6, 182, 212),  // Band 5: Electric Cyan
    Color32::from_rgb(59, 130, 246), // Band 6: Royal Blue
    Color32::from_rgb(139, 92, 246), // Band 7: Purple
    Color32::from_rgb(236, 72, 153), // Band 8: Rose Pink
];

pub fn configure_visuals(ctx: &nih_plug_egui::egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    ctx.set_visuals(visuals);
}

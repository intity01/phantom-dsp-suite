//! Phantom UI — Modular Audio Graphics & Design System
//! Provides reusable views, themes, and audio widgets for the Phantom DSP Suite.

pub mod icons;
pub mod theme;
pub mod views;
pub mod widgets;

pub use theme::{
    configure_visuals, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED,
    BAND_COLORS, BG_CARD, BG_PANEL, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};
pub use views::curve_plot::{
    draw_band_curve, draw_canvas_grid, draw_combined_curve, EqCanvasMapping,
};
pub use views::node_handle::{draw_node, show_node_tooltip, NodeRenderData};
pub use views::spectrum_view::draw_spectrum;
pub use widgets::band_inspector::{draw_band_inspector, BandInspectorData, BandInspectorEvents};
pub use widgets::master_bar::{draw_master_bar, MasterBarEvents};

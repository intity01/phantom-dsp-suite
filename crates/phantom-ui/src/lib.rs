//! Phantom UI — Modular Audio Graphics & Design System
//! Provides reusable views, themes, Lucide vector icons, and audio widgets for Phantom DSP Suite.

pub mod filter_icons;
pub mod icons;
pub mod theme;
pub mod views;
pub mod widgets;

pub use icons::{
    draw_chip_icon, draw_close_icon, draw_filter_shape_icon, draw_headphones_icon, draw_lock_icon,
    draw_phase_icon, draw_power_icon,
};
pub use theme::{
    band_color, configure_visuals, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN,
    ACCENT_PURPLE, ACCENT_RED, BAND_COLORS, BG_CARD, BG_DISPLAY, BG_PANEL, TEXT_BRIGHT,
    TEXT_DIMMED, TEXT_MUTED,
};
pub use views::curve_plot::{
    draw_band_curve, draw_band_curve_fn, draw_canvas_grid, draw_combined_curve,
    draw_combined_curve_fn, draw_empty_canvas_hint, EqCanvasMapping, DB_SCALE_OPTIONS,
};
pub use views::node_handle::{
    draw_node, show_node_tooltip, solo_badge_center, NodeRenderData, SOLO_BADGE_RADIUS,
};
pub use views::spectrum_view::{
    draw_dual_spectrum, draw_spectrum, draw_spectrum_line, draw_spectrum_trace,
};
pub use widgets::band_inspector::{
    draw_band_inspector, draw_empty_band_inspector, BandInspectorData, BandInspectorEvents,
};
pub use widgets::master_bar::{draw_master_bar, MasterBarData, MasterBarEvents};
pub use widgets::output_meter::{draw_output_meter, OutputMeterData, OUTPUT_METER_WIDTH};

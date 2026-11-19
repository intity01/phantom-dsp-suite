//! EQ Canvas Hit-Testing Helpers & Ghost "Click To Create" Preview Node
//! Combined-curve distance tests, floating solo-badge hits, and the translucent cyan `+` ghost
//! node that shows exactly where a click / drag / double-click will create the next band.

use nih_plug_egui::egui::{Color32, Painter, Pos2, Shape, Stroke};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{solo_badge_center, with_alpha, EqCanvasMapping, ACCENT_CYAN, SOLO_BADGE_RADIUS};
use std::f32::consts::TAU;

use crate::params::NUM_BANDS;

/// A single click within this vertical distance of the combined EQ curve creates a band on it.
pub const CURVE_CLICK_HIT_PX: f32 = 28.0;
/// A click-and-drag starting within this vertical distance of the combined EQ curve creates a band.
pub const CURVE_DRAG_HIT_PX: f32 = 40.0;
/// Pointer travel that promotes a curve press into click-and-drag band creation.
pub const DRAG_CREATE_START_PX: f32 = 3.5;
/// Maximum pointer travel for a press/release to still count as a single click.
pub const CLICK_MAX_TRAVEL_PX: f32 = 3.0;
/// A double-click on a band created this recently keeps its gain (it was the first click).
pub const FRESH_BAND_DOUBLE_CLICK_SEC: f64 = 0.40;
pub const GHOST_NODE_RADIUS: f32 = 8.0;

/// Combined (all enabled bands) EQ response in dB at `freq_hz`.
pub fn curve_db_at(freq_hz: f32, active_coeffs: &[CascadedCoeffs], sample_rate: f32) -> f32 {
    active_coeffs
        .iter()
        .map(|c| c.magnitude_db_at(freq_hz, sample_rate))
        .sum()
}

/// Returns the curve gain (dB) under `pos.x` when `pos` lies within `max_dist_px` of the curve.
pub fn curve_hit_db_within(
    pos: Pos2,
    map: &EqCanvasMapping,
    active_coeffs: &[CascadedCoeffs],
    sample_rate: f32,
    max_dist_px: f32,
) -> Option<f32> {
    let freq = map.x_to_freq(pos.x);
    let db = curve_db_at(freq, active_coeffs, sample_rate);
    ((pos.y - map.db_to_y(db)).abs() <= max_dist_px).then_some(db)
}

/// Returns the curve gain (dB) under `pos.x` when `pos` lies within [`CURVE_CLICK_HIT_PX`].
pub fn curve_hit_db(
    pos: Pos2,
    map: &EqCanvasMapping,
    active_coeffs: &[CascadedCoeffs],
    sample_rate: f32,
) -> Option<f32> {
    curve_hit_db_within(pos, map, active_coeffs, sample_rate, CURVE_CLICK_HIT_PX)
}

pub fn hit_solo_badge(
    pos: Pos2,
    soloed_band: Option<usize>,
    node_positions: &[Option<Pos2>; NUM_BANDS],
) -> bool {
    let Some(s_idx) = soloed_band else {
        return false;
    };
    let Some(node_pos) = node_positions.get(s_idx).copied().flatten() else {
        return false;
    };
    let badge_pos = solo_badge_center(node_pos, 11.0);
    pos.distance(badge_pos) <= SOLO_BADGE_RADIUS + 2.5
}

/// Where a click at `pointer` would create a band on the curve, or `None` when away from it.
pub fn ghost_position(
    pointer: Pos2,
    map: &EqCanvasMapping,
    active_coeffs: &[CascadedCoeffs],
    sample_rate: f32,
) -> Option<Pos2> {
    curve_hit_db(pointer, map, active_coeffs, sample_rate)
        .map(|db| Pos2::new(pointer.x, map.db_to_y(db)))
}

/// Subtle dashed translucent cyan circle with a faint `+`: "click here to create a band".
pub fn draw_ghost_node(painter: &Painter, center: Pos2) {
    let r = GHOST_NODE_RADIUS;
    painter.circle_filled(center, r, with_alpha(ACCENT_CYAN, 22));
    let segments = 16;
    for s in (0..segments).step_by(2) {
        let a0 = s as f32 / segments as f32 * TAU;
        let a1 = (s + 1) as f32 / segments as f32 * TAU;
        let p0 = center + r * nih_plug_egui::egui::vec2(a0.cos(), a0.sin());
        let p1 = center + r * nih_plug_egui::egui::vec2(a1.cos(), a1.sin());
        painter.add(Shape::line_segment(
            [p0, p1],
            Stroke::new(1.2_f32, with_alpha(ACCENT_CYAN, 170)),
        ));
    }
    let arm = r * 0.45;
    let plus = Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(220, 245, 255, 150));
    painter.line_segment([center - egui_x(arm), center + egui_x(arm)], plus);
    painter.line_segment([center - egui_y(arm), center + egui_y(arm)], plus);
}

#[inline]
fn egui_x(v: f32) -> nih_plug_egui::egui::Vec2 {
    nih_plug_egui::egui::vec2(v, 0.0)
}

#[inline]
fn egui_y(v: f32) -> nih_plug_egui::egui::Vec2 {
    nih_plug_egui::egui::vec2(0.0, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::EditorState;
    use nih_plug_egui::egui::Rect;

    #[test]
    fn curve_hit_thresholds_and_ghost_visibility() {
        let map = EqCanvasMapping::with_db_range(
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(800.0, 400.0)),
            12.0,
        );
        let y0 = map.db_to_y(0.0); // 200.0

        // Within CURVE_CLICK_HIT_PX (28 px): ghost snaps to curve
        let near = Pos2::new(400.0, y0 + 20.0);
        let ghost = ghost_position(near, &map, &[], 48000.0).expect("ghost expected near curve");
        assert!((ghost.y - y0).abs() < 1e-3);

        // Between 28 px and 40 px: no click-ghost, but drag-hit succeeds
        let mid = Pos2::new(400.0, y0 + 35.0);
        assert!(ghost_position(mid, &map, &[], 48000.0).is_none());
        assert!(curve_hit_db_within(mid, &map, &[], 48000.0, CURVE_DRAG_HIT_PX).is_some());

        // Far from curve (> 40 px): neither click nor drag creates a band
        let far = Pos2::new(400.0, y0 + 60.0);
        assert!(ghost_position(far, &map, &[], 48000.0).is_none());
        assert!(curve_hit_db_within(far, &map, &[], 48000.0, CURVE_DRAG_HIT_PX).is_none());
    }

    #[test]
    fn editor_default_starts_clean_without_piano_roll_or_hud() {
        let state = EditorState::default();
        assert!(!state.show_piano_roll);
        assert!(state.selected_band.is_none());
    }
}

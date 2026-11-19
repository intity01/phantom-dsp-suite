//! Pro-Q 4 Mouse & Gesture Handler for the Main EQ Frequency Canvas
//! - Ignores canvas clicks/drags when pointer is inside the Floating Band HUD Popover
//! - Musical Note Snapping (`state.snap_to_notes` or holding `Ctrl` while dragging)
//! - Spectrum Grab peak click-and-drag: creates/selects a Bell band at `(freq_hz, estimated_q)`
//! - Double-click empty canvas / curve drag / node drag / Alt-Solo / Scroll Q

use super::display_helpers::{
    create_band_at, find_hit_used_node, smart_detect_shape, CURVE_DRAG_HIT_PX,
};
use super::{EditorShared, EditorState};
use crate::params::{FilterTypeParam, PhantomEqParams, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{Pos2, Rect, Response, Ui};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{
    snap_freq_to_nearest_semitone, solo_badge_center, EqCanvasMapping, SpectrumPeak,
    SOLO_BADGE_RADIUS,
};

#[allow(clippy::too_many_arguments)]
pub fn handle_canvas_gestures(
    ui: &Ui,
    response: &Response,
    map: &EqCanvasMapping,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
    node_positions: &[Option<Pos2>; NUM_BANDS],
    active_coeffs: &[CascadedCoeffs],
    hovered_node: Option<usize>,
    soloed_band: Option<usize>,
    hud_rect: Option<Rect>,
    hovered_grab_peak: Option<SpectrumPeak>,
) {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let interact_pos = response.interact_pointer_pos().or(pointer_pos);
    let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
    let primary_down = ui.input(|i| i.pointer.primary_down());
    let pointer_moved = ui.input(|i| i.pointer.delta().length_sq() > 0.0);
    let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
    let alt_down = ui.input(|i| i.modifiers.alt);
    let ctrl_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);

    let inside_hud = hud_rect.is_some_and(|r| pointer_pos.is_some_and(|p| r.contains(p)));

    // 1. Continue or finish active node drag (with Alt+Drag Solo audition & Ctrl/SNAP note snap)
    if let Some(drag_idx) = state.dragging_band {
        let band = &params.bands[drag_idx];
        if primary_down && band.used.value() {
            state.alt_solo_band = if alt_down { Some(drag_idx) } else { None };
            if pointer_moved {
                if let Some(pos) = pointer_pos {
                    let raw_freq = map.x_to_freq(pos.x).clamp(10.0, 30000.0);
                    let freq = if state.snap_to_notes || ctrl_down {
                        snap_freq_to_nearest_semitone(raw_freq).clamp(10.0, 30000.0)
                    } else {
                        raw_freq
                    };
                    let gain = map.y_to_db(pos.y).clamp(-30.0, 30.0);
                    setter.set_parameter(&band.frequency, freq);
                    setter.set_parameter(&band.gain, gain);

                    // Upgrade newly curve-dragged Bell -> LowShelf/HighShelf once dragged past 1.5 dB
                    if state.drag_created_band == Some(drag_idx)
                        && band.filter_type.value() == FilterTypeParam::Bell
                        && gain.abs() > 1.5
                    {
                        let upgraded = smart_detect_shape(freq, gain);
                        if upgraded != FilterTypeParam::Bell {
                            setter.begin_set_parameter(&band.filter_type);
                            setter.set_parameter(&band.filter_type, upgraded);
                            setter.end_set_parameter(&band.filter_type);
                            state.drag_created_band = None;
                        }
                    }
                }
            }
        } else {
            setter.end_set_parameter(&band.frequency);
            setter.end_set_parameter(&band.gain);
            state.dragging_band = None;
            state.drag_created_band = None;
            state.alt_solo_band = None;
        }
    }

    // Never initiate new canvas node gestures if the pointer is interacting inside the Floating HUD
    if inside_hud && state.dragging_band.is_none() {
        state.pending_curve_press = None;
        return;
    }

    // 2. Promote click-and-drag near 0 dB / EQ curve into instant band creation + drag
    if let Some(press_pos) = state.pending_curve_press {
        if !primary_down {
            state.pending_curve_press = None;
        } else if let Some(cur_pos) = pointer_pos {
            if cur_pos.distance(press_pos) >= 3.0 {
                state.pending_curve_press = None;
                let raw_freq = map.x_to_freq(cur_pos.x).clamp(10.0, 30000.0);
                let freq = if state.snap_to_notes || ctrl_down {
                    snap_freq_to_nearest_semitone(raw_freq).clamp(10.0, 30000.0)
                } else {
                    raw_freq
                };
                let gain = map.y_to_db(cur_pos.y).clamp(-30.0, 30.0);
                if let Some(new_idx) = create_band_at(setter, params, state, freq, gain) {
                    let band = &params.bands[new_idx];
                    state.dragging_band = Some(new_idx);
                    state.drag_created_band = Some(new_idx);
                    setter.begin_set_parameter(&band.frequency);
                    setter.begin_set_parameter(&band.gain);
                    if alt_down {
                        state.alt_solo_band = Some(new_idx);
                    }
                }
            }
        }
    }

    // 3. Double-click: node -> reset gain to 0 dB; empty canvas -> create band with Smart Shape
    if response.double_clicked() {
        state.pending_curve_press = None;
        if let Some(drag_idx) = state.dragging_band.take() {
            let drag_band = &params.bands[drag_idx];
            setter.end_set_parameter(&drag_band.frequency);
            setter.end_set_parameter(&drag_band.gain);
            state.drag_created_band = None;
            state.alt_solo_band = None;
        }
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                state.selected_band = Some(hit_idx);
                let band = &params.bands[hit_idx];
                setter.begin_set_parameter(&band.gain);
                setter.set_parameter(&band.gain, 0.0);
                setter.end_set_parameter(&band.gain);
            } else if map.rect.contains(pos) {
                let raw_freq = map.x_to_freq(pos.x);
                let click_freq = if state.snap_to_notes || ctrl_down {
                    snap_freq_to_nearest_semitone(raw_freq)
                } else {
                    raw_freq
                };
                let click_gain = map.y_to_db(pos.y);
                let _ = create_band_at(setter, params, state, click_freq, click_gain);
            }
        }
    } else if response.secondary_clicked() {
        // 4. Right-click existing node -> toggle Band On/Off
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                let band = &params.bands[hit_idx];
                setter.begin_set_parameter(&band.enabled);
                setter.set_parameter(&band.enabled, !band.enabled.value());
                setter.end_set_parameter(&band.enabled);
            }
        }
    } else if primary_pressed && state.dragging_band.is_none() {
        // 5. Primary press: floating solo badge | existing node | Spectrum Grab peak | curve drag
        if let Some(pos) = pointer_pos {
            if hit_solo_badge(pos, soloed_band, node_positions) {
                state.latched_solo = None;
                state.solo_hold_band = None;
                shared.clear_solo();
            } else if let Some(hit_idx) = hovered_node {
                let band = &params.bands[hit_idx];
                state.selected_band = Some(hit_idx);
                state.dragging_band = Some(hit_idx);
                setter.begin_set_parameter(&band.frequency);
                setter.begin_set_parameter(&band.gain);
                if alt_down {
                    state.alt_solo_band = Some(hit_idx);
                }
            } else if let Some(peak) = hovered_grab_peak {
                grab_spectrum_peak_band(setter, params, state, peak, alt_down);
            } else if map.rect.contains(pos)
                && is_near_eq_curve(pos, map, active_coeffs, state.sample_rate)
            {
                state.pending_curve_press = Some(pos);
            }
        }
    }

    // 6. Scroll wheel adjusts Q of hovered used band (or selected used band)
    if scroll_delta.abs() > 0.05 && pointer_pos.is_some_and(|p| map.rect.contains(p)) {
        if let Some(q_idx) = hovered_node.or(state.selected_band) {
            if q_idx < NUM_BANDS && params.bands[q_idx].used.value() {
                let band = &params.bands[q_idx];
                let mult = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
                let new_q = (band.q.value() * mult).clamp(0.1, 30.0);
                setter.begin_set_parameter(&band.q);
                setter.set_parameter(&band.q, new_q);
                setter.end_set_parameter(&band.q);
            }
        }
    }
}

fn grab_spectrum_peak_band(
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    peak: SpectrumPeak,
    alt_down: bool,
) {
    let existing = (0..NUM_BANDS).find(|&i| {
        params.bands[i].used.value()
            && ((params.bands[i].frequency.value() - peak.freq_hz).abs() / peak.freq_hz.max(1.0))
                < 0.03
    });
    let target_idx = existing.or_else(|| create_band_at(setter, params, state, peak.freq_hz, 0.0));
    if let Some(idx) = target_idx {
        let band = &params.bands[idx];
        setter.begin_set_parameter(&band.filter_type);
        setter.set_parameter(&band.filter_type, FilterTypeParam::Bell);
        setter.end_set_parameter(&band.filter_type);

        setter.begin_set_parameter(&band.q);
        setter.set_parameter(&band.q, peak.estimated_q.clamp(0.1, 30.0));
        setter.end_set_parameter(&band.q);

        state.selected_band = Some(idx);
        state.dragging_band = Some(idx);
        setter.begin_set_parameter(&band.frequency);
        setter.set_parameter(&band.frequency, peak.freq_hz.clamp(10.0, 30000.0));
        setter.begin_set_parameter(&band.gain);
        if alt_down {
            state.alt_solo_band = Some(idx);
        }
    }
}

fn hit_solo_badge(
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

fn is_near_eq_curve(
    pos: Pos2,
    map: &EqCanvasMapping,
    active_coeffs: &[CascadedCoeffs],
    sample_rate: f32,
) -> bool {
    let zero_y = map.db_to_y(0.0);
    if (pos.y - zero_y).abs() <= CURVE_DRAG_HIT_PX {
        return true;
    }
    if active_coeffs.is_empty() {
        return false;
    }
    let freq = map.x_to_freq(pos.x);
    let total_db: f32 = active_coeffs
        .iter()
        .map(|c| c.magnitude_db_at(freq, sample_rate))
        .sum();
    (pos.y - map.db_to_y(total_db)).abs() <= CURVE_DRAG_HIT_PX
}

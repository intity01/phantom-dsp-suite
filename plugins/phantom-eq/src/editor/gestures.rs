//! Pro-Q 4 Mouse & Gesture Handler for the Main EQ Frequency Canvas
//! - Single-click near the EQ curve (<= 36 px) creates & selects a band on the curve
//! - Click-and-drag anywhere on empty canvas creates a band & drags it immediately
//! - Double-click anywhere creates a band (and won't reset gain if double-clicking a fresh band)
//! - Double-click existing band (>= 0.40s old) resets gain to 0 dB; Right-click toggles On/Off
//! - Spectrum Grab peak click-and-drag, Alt+Drag Solo audition, Ctrl/SNAP semitone snap, Scroll Q

use super::canvas_hit::{
    curve_hit_db, hit_solo_badge, CLICK_MAX_TRAVEL_PX, DRAG_CREATE_START_PX,
    FRESH_BAND_DOUBLE_CLICK_SEC,
};
use super::display_helpers::{create_band_at, find_hit_used_node, smart_detect_shape};
use super::window_size::local_pointer;
use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::{FilterTypeParam, PhantomEqParams, NUM_BANDS};
use nih_plug_egui::egui::{Pos2, Rect, Response, Ui};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{snap_freq_to_nearest_semitone, EqCanvasMapping, SpectrumPeak};

#[allow(clippy::too_many_arguments)]
pub fn handle_canvas_gestures(
    ui: &Ui,
    response: &Response,
    map: &EqCanvasMapping,
    setter: &LiveSetter,
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
    let pointer_pos = local_pointer(ui, ui.input(|i| i.pointer.hover_pos()));
    let interact_pos = response.interact_pointer_pos().or(pointer_pos);
    let now = ui.input(|i| i.time);
    let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
    let primary_down = ui.input(|i| i.pointer.primary_down());
    let pointer_moved = ui.input(|i| i.pointer.delta().length_sq() > 0.0);
    let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
    let alt_down = ui.input(|i| i.modifiers.alt);
    let ctrl_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let snap = state.snap_to_notes || ctrl_down;
    let inside_hud = hud_rect.is_some_and(|r| pointer_pos.is_some_and(|p| r.contains(p)));

    // 1. Continue or finish active node drag
    if let Some(drag_idx) = state.dragging_band {
        let band = &params.bands[drag_idx];
        if primary_down && setter.read(&band.used) {
            state.alt_solo_band = if alt_down { Some(drag_idx) } else { None };
            if pointer_moved {
                if let Some(pos) = pointer_pos {
                    let freq = resolve_freq(map.x_to_freq(pos.x), snap);
                    let gain = map.y_to_db(pos.y).clamp(-30.0, 30.0);
                    setter.set_parameter(&band.frequency, freq);
                    setter.set_parameter(&band.gain, gain);
                    if state.drag_created_band == Some(drag_idx)
                        && setter.read(&band.filter_type) == FilterTypeParam::Bell
                        && gain.abs() > 1.5
                    {
                        let upgraded = smart_detect_shape(freq, gain);
                        if upgraded != FilterTypeParam::Bell {
                            setter.set_once(&band.filter_type, upgraded);
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

    if inside_hud && state.dragging_band.is_none() {
        state.pending_curve_press = None;
        return;
    }

    // 2. Empty canvas press: drag >= 2.5px creates & drags anywhere; release < 3px near curve creates
    if let Some(press_pos) = state.pending_curve_press {
        if !primary_down {
            state.pending_curve_press = None;
            let rel_pos = interact_pos.unwrap_or(press_pos);
            if rel_pos.distance(press_pos) < CLICK_MAX_TRAVEL_PX && !response.double_clicked() {
                if let Some(curve_db) =
                    curve_hit_db(press_pos, map, active_coeffs, state.sample_rate)
                {
                    let freq = resolve_freq(map.x_to_freq(press_pos.x), snap);
                    let _ = create_band_at(setter, params, state, freq, curve_db, now);
                }
            }
        } else if let Some(cur_pos) = pointer_pos {
            if cur_pos.distance(press_pos) >= DRAG_CREATE_START_PX {
                state.pending_curve_press = None;
                let freq = resolve_freq(map.x_to_freq(cur_pos.x), snap);
                let gain = map.y_to_db(cur_pos.y).clamp(-30.0, 30.0);
                if let Some(new_idx) = create_band_at(setter, params, state, freq, gain, now) {
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

    // 3. Double-click: fresh band (<0.40s) keeps gain; older band resets to 0 dB; empty creates
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
                let just_created =
                    (now - state.last_band_created_time) < FRESH_BAND_DOUBLE_CLICK_SEC;
                if just_created {
                    let shape = setter.read(&band.filter_type);
                    if !matches!(shape, FilterTypeParam::LowCut | FilterTypeParam::HighCut) {
                        setter.set_once(&band.gain, map.y_to_db(pos.y).clamp(-30.0, 30.0));
                    }
                } else {
                    setter.set_once(&band.gain, 0.0);
                }
            } else if map.rect.contains(pos) {
                let click_freq = resolve_freq(map.x_to_freq(pos.x), snap);
                let click_gain = map.y_to_db(pos.y);
                let _ = create_band_at(setter, params, state, click_freq, click_gain, now);
            }
        }
    } else if response.secondary_clicked() {
        // 4. Right-click existing node -> toggle Band On/Off
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                let band = &params.bands[hit_idx];
                let en = setter.read(&band.enabled);
                setter.set_once(&band.enabled, !en);
            }
        }
    } else if primary_pressed && state.dragging_band.is_none() {
        // 5. Primary press: solo badge | existing node | Spectrum Grab peak | any empty canvas spot
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
                grab_spectrum_peak_band(setter, params, state, peak, alt_down, now);
            } else if map.rect.contains(pos) {
                state.pending_curve_press = Some(pos);
            }
        }
    }

    // 6. Scroll wheel adjusts Q of hovered used band (or selected used band)
    if scroll_delta.abs() > 0.05 && pointer_pos.is_some_and(|p| map.rect.contains(p)) {
        if let Some(q_idx) = hovered_node.or(state.selected_band) {
            if q_idx < NUM_BANDS && setter.read(&params.bands[q_idx].used) {
                let band = &params.bands[q_idx];
                let mult = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
                let new_q = (setter.read(&band.q) * mult).clamp(0.1, 30.0);
                setter.set_once(&band.q, new_q);
            }
        }
    }
}

#[inline]
fn resolve_freq(raw_freq: f32, snap: bool) -> f32 {
    let clamped = raw_freq.clamp(10.0, 30000.0);
    if snap {
        snap_freq_to_nearest_semitone(clamped).clamp(10.0, 30000.0)
    } else {
        clamped
    }
}

fn grab_spectrum_peak_band(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    peak: SpectrumPeak,
    alt_down: bool,
    now_sec: f64,
) {
    let existing = (0..NUM_BANDS).find(|&i| {
        setter.read(&params.bands[i].used)
            && ((setter.read(&params.bands[i].frequency) - peak.freq_hz).abs()
                / peak.freq_hz.max(1.0))
                < 0.03
    });
    let target_idx =
        existing.or_else(|| create_band_at(setter, params, state, peak.freq_hz, 0.0, now_sec));
    if let Some(idx) = target_idx {
        let band = &params.bands[idx];
        setter.set_once(&band.filter_type, FilterTypeParam::Bell);
        setter.set_once(&band.q, peak.estimated_q.clamp(0.1, 30.0));
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

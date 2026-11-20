//! Pro-Q 4 Mouse & Gesture Handler for the Main EQ Frequency Canvas
//! - Single-click near EQ curve (<= 28 px) creates & selects a band; away from curve deselects
//! - Click-and-drag near curve (<= 40 px, travel >= 3.5 px) creates & drags a band
//! - Shift + Drag enters fine precision mode (0.15x pointer scaling with zero coordinate jump)
//! - Double-click existing band (>= 0.40s old) resets gain to 0 dB; empty double-click creates
//! - Right-click node opens Pro-Q 4 Context Menu; Middle-click node toggles Band Bypass
//! - Spectrum Grab peak drag, Alt+Drag Solo audition, Ctrl/SNAP semitone snap, Scroll Q

use super::canvas_hit::{
    curve_hit_db, curve_hit_db_within, hit_solo_badge, CLICK_MAX_TRAVEL_PX, CURVE_DRAG_HIT_PX,
    DRAG_CREATE_START_PX, FRESH_BAND_DOUBLE_CLICK_SEC,
};
use super::display_helpers::{
    create_band_at, find_hit_used_node, grab_spectrum_peak_band, resolve_freq, smart_detect_shape,
};
use super::window_size::local_pointer;
use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::{BandParams, FilterTypeParam, PhantomEqParams, NUM_BANDS};
use nih_plug_egui::egui::{PointerButton, Pos2, Rect, Response, Ui, Vec2};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{EqCanvasMapping, SpectrumPeak};

const SHIFT_FINE_SCALE: f32 = 0.15;

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
    let (raw_hover, now, primary_pressed, primary_down, pointer_moved, scroll_delta, mods) = ui
        .input(|i| {
            (
                i.pointer.hover_pos(),
                i.time,
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.delta().length_sq() > 0.0,
                i.raw_scroll_delta.y,
                i.modifiers,
            )
        });
    let pointer_pos = local_pointer(ui, raw_hover);
    let interact_pos = response.interact_pointer_pos().or(pointer_pos);
    let (alt_down, shift_down) = (mods.alt, mods.shift);
    let snap = state.snap_to_notes || mods.ctrl || mods.command;
    let inside_hud = hud_rect.is_some_and(|r| pointer_pos.is_some_and(|p| r.contains(p)));

    // 1. Continue or finish active node drag (with seamless Shift+Drag 0.15x fine mode)
    if let Some(drag_idx) = state.dragging_band {
        let band = &params.bands[drag_idx];
        if primary_down && setter.read(&band.used) {
            state.alt_solo_band = alt_down.then_some(drag_idx);
            if let Some(tp) = compute_drag_target(
                map,
                setter,
                band,
                state,
                pointer_pos,
                pointer_moved,
                shift_down,
            ) {
                let freq = resolve_freq(map.x_to_freq(tp.x), snap);
                let gain = map.y_to_db(tp.y).clamp(-30.0, 30.0);
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
        } else {
            end_active_drag(setter, params, state, drag_idx);
        }
    }

    if inside_hud && state.dragging_band.is_none() {
        state.pending_curve_press = None;
        return;
    }

    // 2. Empty canvas press: near curve creates/drags; away from curve deselects (hides HUD)
    if let Some(press_pos) = state.pending_curve_press {
        if !primary_down {
            state.pending_curve_press = None;
            let rel_pos = interact_pos.unwrap_or(press_pos);
            if !response.double_clicked() {
                if rel_pos.distance(press_pos) < CLICK_MAX_TRAVEL_PX {
                    if let Some(db) = curve_hit_db(press_pos, map, active_coeffs, state.sample_rate)
                    {
                        let f = resolve_freq(map.x_to_freq(press_pos.x), snap);
                        let _ = create_band_at(setter, params, state, f, db, now);
                    } else {
                        state.selected_band = None;
                    }
                } else if curve_hit_db(press_pos, map, active_coeffs, state.sample_rate).is_none() {
                    state.selected_band = None;
                }
            }
        } else if let Some(cur_pos) = pointer_pos {
            if cur_pos.distance(press_pos) >= DRAG_CREATE_START_PX {
                state.pending_curve_press = None;
                let near = curve_hit_db_within(
                    press_pos,
                    map,
                    active_coeffs,
                    state.sample_rate,
                    CURVE_DRAG_HIT_PX,
                )
                .is_some();
                if near {
                    let f = resolve_freq(map.x_to_freq(cur_pos.x), snap);
                    let g = map.y_to_db(cur_pos.y).clamp(-30.0, 30.0);
                    if let Some(new_idx) = create_band_at(setter, params, state, f, g, now) {
                        begin_node_drag(setter, params, state, new_idx, Some(cur_pos), alt_down);
                        state.drag_created_band = Some(new_idx);
                    }
                } else {
                    state.selected_band = None;
                }
            }
        }
    }

    // 3. Double-click / Middle-click / Right-click / Primary press
    if response.double_clicked() {
        state.pending_curve_press = None;
        if let Some(drag_idx) = state.dragging_band {
            end_active_drag(setter, params, state, drag_idx);
        }
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                state.selected_band = Some(hit_idx);
                let band = &params.bands[hit_idx];
                if (now - state.last_band_created_time) < FRESH_BAND_DOUBLE_CLICK_SEC {
                    let shape = setter.read(&band.filter_type);
                    if !matches!(shape, FilterTypeParam::LowCut | FilterTypeParam::HighCut) {
                        setter.set_once(&band.gain, map.y_to_db(pos.y).clamp(-30.0, 30.0));
                    }
                } else {
                    setter.set_once(&band.gain, 0.0);
                }
            } else if map.rect.contains(pos) {
                let f = resolve_freq(map.x_to_freq(pos.x), snap);
                let _ = create_band_at(setter, params, state, f, map.y_to_db(pos.y), now);
            }
        }
    } else if response.clicked_by(PointerButton::Middle) {
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                let band = &params.bands[hit_idx];
                setter.set_once(&band.enabled, !setter.read(&band.enabled));
            }
        }
    } else if response.secondary_clicked() {
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_used_node(pos, node_positions, state.selected_band) {
                state.selected_band = Some(hit_idx);
                state.node_menu = Some((hit_idx, pos));
            }
        }
    } else if primary_pressed
        && state.dragging_band.is_none()
        && (response.hovered() || response.is_pointer_button_down_on())
    {
        if let Some(pos) = pointer_pos {
            if hit_solo_badge(pos, soloed_band, node_positions) {
                state.latched_solo = None;
                state.solo_hold_band = None;
                shared.clear_solo();
            } else if let Some(hit_idx) = hovered_node {
                begin_node_drag(setter, params, state, hit_idx, Some(pos), alt_down);
            } else if let Some(peak) = hovered_grab_peak {
                grab_spectrum_peak_band(setter, params, state, peak, Some(pos), alt_down, now);
            } else if map.rect.contains(pos) {
                state.pending_curve_press = Some(pos);
            }
        }
    }

    // 4. Scroll wheel adjusts Q of hovered used band (or selected used band)
    if scroll_delta.abs() > 0.05
        && response.hovered()
        && pointer_pos.is_some_and(|p| map.rect.contains(p))
    {
        if let Some(q_idx) = hovered_node.or(state.selected_band) {
            if q_idx < NUM_BANDS && setter.read(&params.bands[q_idx].used) {
                let band = &params.bands[q_idx];
                let mult = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
                setter.set_once(&band.q, (setter.read(&band.q) * mult).clamp(0.1, 30.0));
            }
        }
    }
}

fn compute_drag_target(
    map: &EqCanvasMapping,
    setter: &LiveSetter,
    band: &BandParams,
    state: &mut EditorState,
    pointer_pos: Option<Pos2>,
    pointer_moved: bool,
    shift_down: bool,
) -> Option<Pos2> {
    let pos = pointer_pos?;
    let delta = state
        .drag_last_pointer
        .map(|prev| pos - prev)
        .unwrap_or(Vec2::ZERO);
    state.drag_last_pointer = Some(pos);
    let moved = pointer_moved || delta.length_sq() > 0.0;
    if !shift_down && state.drag_virtual_pos.is_none() {
        return moved.then_some(pos);
    }
    let cur = Pos2::new(
        map.freq_to_x(setter.read(&band.frequency)),
        map.db_to_y(setter.read(&band.gain)),
    );
    let base = state.drag_virtual_pos.unwrap_or(cur);
    let scale = if shift_down { SHIFT_FINE_SCALE } else { 1.0 };
    let next = Pos2::new(
        (base.x + delta.x * scale).clamp(map.rect.min.x, map.rect.max.x),
        (base.y + delta.y * scale).clamp(map.rect.min.y, map.rect.max.y),
    );
    state.drag_virtual_pos = Some(next);
    moved.then_some(next)
}

fn begin_node_drag(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    idx: usize,
    start: Option<Pos2>,
    alt: bool,
) {
    let band = &params.bands[idx];
    state.selected_band = Some(idx);
    state.dragging_band = Some(idx);
    state.drag_virtual_pos = None;
    state.drag_last_pointer = start;
    setter.begin_set_parameter(&band.frequency);
    setter.begin_set_parameter(&band.gain);
    if alt {
        state.alt_solo_band = Some(idx);
    }
}

fn end_active_drag(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    idx: usize,
) {
    let band = &params.bands[idx];
    setter.end_set_parameter(&band.frequency);
    setter.end_set_parameter(&band.gain);
    state.dragging_band = None;
    state.drag_created_band = None;
    state.drag_virtual_pos = None;
    state.drag_last_pointer = None;
    state.alt_solo_band = None;
}

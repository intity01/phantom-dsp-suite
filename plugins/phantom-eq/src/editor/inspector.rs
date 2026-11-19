//! Selected Band Inspector & Floating Band HUD Popover Bridge
//! Connects `phantom-ui`'s Floating Band HUD Popover (`draw_floating_band_hud`) and Docked
//! `BandInspector` (`draw_band_inspector`) to Phantom EQ parameters, Headphone Solo Audition,
//! Spectral Dynamics (`SPEC`), and band deletion.

use super::display_helpers::delete_band;
use super::{EditorShared, EditorState};
use crate::params::{
    ChannelPlacementParam, FilterSlopeParam, FilterTypeParam, PhantomEqParams, FILTER_NAMES,
    NUM_BANDS, ROUTING_NAMES, SLOPE_NAMES,
};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{Pos2, Rect, Ui};
use phantom_ui::{
    draw_band_inspector, draw_empty_band_inspector, draw_floating_band_hud, BandInspectorData,
    BandInspectorEvents, EqCanvasMapping,
};

const SOLO_HOLD_THRESHOLD_SEC: f64 = 0.25;

pub fn build_band_inspector_data(
    params: &PhantomEqParams,
    shared: &EditorShared,
    state: &EditorState,
    sel_idx: usize,
) -> BandInspectorData {
    let mut used_mask = 0u32;
    for (i, band) in params.bands.iter().enumerate() {
        if band.used.value() {
            used_mask |= 1u32 << i;
        }
    }
    let band = &params.bands[sel_idx];
    let soloed_band = state.effective_solo(params);
    BandInspectorData {
        selected_idx: sel_idx,
        num_bands: NUM_BANDS,
        used_bands_mask: used_mask,
        is_enabled: band.enabled.value(),
        is_soloed: soloed_band == Some(sel_idx),
        soloed_band,
        filter_shape_idx: band.filter_type.value().to_idx(),
        slope_idx: band.slope.value().to_idx(),
        routing_idx: band.placement.value().to_ui_idx(),
        freq: band.frequency.value(),
        gain: band.gain.value(),
        q: band.q.value(),
        dyn_range: band.dynamic_range.value(),
        dyn_threshold: band.dynamic_threshold.value(),
        dyn_attack_ms: band.dynamic_attack.value(),
        dyn_release_ms: band.dynamic_release.value(),
        dyn_auto_thresh: band.dynamic_auto_thresh.value(),
        dyn_ext_sc: band.dynamic_ext_sc.value(),
        spectral_enabled: band.spectral_dynamics.value(),
        live_gr_db: shared.band_gr.load(sel_idx),
    }
}

/// Renders the Pro-Q 4 Floating Band HUD Popover near `node_pos` on the EQ canvas.
/// Returns `Some(hud_rect)` so canvas gestures ignore pointer clicks/drags inside the HUD.
#[allow(clippy::too_many_arguments)]
pub fn draw_floating_hud_for_selected(
    ui: &mut Ui,
    map: &EqCanvasMapping,
    node_pos: Pos2,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
    sel_idx: usize,
) -> Rect {
    let data = build_band_inspector_data(params, shared, state, sel_idx);
    let (events, hud_rect) = draw_floating_band_hud(
        ui,
        map,
        node_pos,
        &data,
        &FILTER_NAMES,
        &SLOPE_NAMES,
        &ROUTING_NAMES,
    );
    apply_band_inspector_events(ui, setter, state, params, shared, sel_idx, events);
    hud_rect
}

/// Renders the Docked Bottom Band Inspector section when `state.show_docked_inspector` is active.
pub fn draw_inspector_section(
    ui: &mut Ui,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
) {
    let Some(sel_idx) = state
        .selected_band
        .filter(|&i| i < NUM_BANDS && params.bands[i].used.value())
    else {
        if !state.show_floating_hud {
            state.solo_hold_band = None;
            state.solo_press_start = None;
        }
        draw_empty_band_inspector(ui);
        return;
    };

    let data = build_band_inspector_data(params, shared, state, sel_idx);
    let events = draw_band_inspector(ui, &data, &FILTER_NAMES, &SLOPE_NAMES, &ROUTING_NAMES);
    apply_band_inspector_events(ui, setter, state, params, shared, sel_idx, events);
}

pub fn apply_band_inspector_events(
    ui: &Ui,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
    sel_idx: usize,
    events: BandInspectorEvents,
) {
    let band = &params.bands[sel_idx];
    let now = ui.input(|i| i.time);

    // Headphone Solo: press-and-hold (>= 0.25s auditions while held) + quick click (toggle latch)
    if events.solo_held {
        if state.solo_press_start.is_none() {
            state.solo_press_start = Some(now);
            state.solo_was_latched_at_press = state.latched_solo == Some(sel_idx);
        }
        state.solo_hold_band = Some(sel_idx);
    } else if events.toggle_solo {
        if let Some(press_t) = state.solo_press_start.take() {
            let held_sec = now - press_t;
            state.solo_hold_band = None;
            if state.solo_was_latched_at_press {
                state.latched_solo = None;
            } else if held_sec < SOLO_HOLD_THRESHOLD_SEC {
                state.latched_solo = Some(sel_idx);
            }
        } else {
            state.latched_solo = if state.latched_solo == Some(sel_idx) {
                None
            } else {
                Some(sel_idx)
            };
        }
    } else if !ui.input(|i| i.pointer.primary_down()) {
        if let Some(press_t) = state.solo_press_start.take() {
            let held_sec = now - press_t;
            state.solo_hold_band = None;
            if state.solo_was_latched_at_press {
                state.latched_solo = None;
            } else if held_sec < SOLO_HOLD_THRESHOLD_SEC {
                state.latched_solo = Some(sel_idx);
            }
        }
    }

    if let Some(idx) = events.select_band {
        if idx < NUM_BANDS && params.bands[idx].used.value() {
            state.selected_band = Some(idx);
        }
    }
    if events.delete_band {
        delete_band(setter, params, state, shared, sel_idx);
        return;
    }
    if events.toggle_enabled {
        setter.begin_set_parameter(&band.enabled);
        setter.set_parameter(&band.enabled, !band.enabled.value());
        setter.end_set_parameter(&band.enabled);
    }
    if events.toggle_spectral {
        setter.begin_set_parameter(&band.spectral_dynamics);
        setter.set_parameter(&band.spectral_dynamics, !band.spectral_dynamics.value());
        setter.end_set_parameter(&band.spectral_dynamics);
    }
    if let Some(shape_idx) = events.set_filter_shape {
        setter.begin_set_parameter(&band.filter_type);
        setter.set_parameter(&band.filter_type, FilterTypeParam::from_idx(shape_idx));
        setter.end_set_parameter(&band.filter_type);
    }
    if let Some(slope_idx) = events.set_slope {
        setter.begin_set_parameter(&band.slope);
        setter.set_parameter(&band.slope, FilterSlopeParam::from_idx(slope_idx));
        setter.end_set_parameter(&band.slope);
    }
    if let Some(rout_idx) = events.set_routing {
        setter.begin_set_parameter(&band.placement);
        setter.set_parameter(
            &band.placement,
            ChannelPlacementParam::from_ui_idx(rout_idx),
        );
        setter.end_set_parameter(&band.placement);
    }
    if let Some(freq) = events.set_freq {
        setter.begin_set_parameter(&band.frequency);
        setter.set_parameter(&band.frequency, freq);
        setter.end_set_parameter(&band.frequency);
    }
    if let Some(gain) = events.set_gain {
        setter.begin_set_parameter(&band.gain);
        setter.set_parameter(&band.gain, gain);
        setter.end_set_parameter(&band.gain);
    }
    if let Some(q) = events.set_q {
        setter.begin_set_parameter(&band.q);
        setter.set_parameter(&band.q, q);
        setter.end_set_parameter(&band.q);
    }
    if let Some(dyn_r) = events.set_dyn_range {
        setter.begin_set_parameter(&band.dynamic_range);
        setter.set_parameter(&band.dynamic_range, dyn_r);
        setter.end_set_parameter(&band.dynamic_range);
    }
    if let Some(thresh) = events.set_dyn_threshold {
        setter.begin_set_parameter(&band.dynamic_threshold);
        setter.set_parameter(&band.dynamic_threshold, thresh);
        setter.end_set_parameter(&band.dynamic_threshold);
    }
    if let Some(atk) = events.set_dyn_attack_ms {
        setter.begin_set_parameter(&band.dynamic_attack);
        setter.set_parameter(&band.dynamic_attack, atk);
        setter.end_set_parameter(&band.dynamic_attack);
    }
    if let Some(rel) = events.set_dyn_release_ms {
        setter.begin_set_parameter(&band.dynamic_release);
        setter.set_parameter(&band.dynamic_release, rel);
        setter.end_set_parameter(&band.dynamic_release);
    }
    if events.toggle_dyn_auto_thresh {
        setter.begin_set_parameter(&band.dynamic_auto_thresh);
        setter.set_parameter(&band.dynamic_auto_thresh, !band.dynamic_auto_thresh.value());
        setter.end_set_parameter(&band.dynamic_auto_thresh);
    }
    if events.toggle_dyn_ext_sc {
        setter.begin_set_parameter(&band.dynamic_ext_sc);
        setter.set_parameter(&band.dynamic_ext_sc, !band.dynamic_ext_sc.value());
        setter.end_set_parameter(&band.dynamic_ext_sc);
    }
}

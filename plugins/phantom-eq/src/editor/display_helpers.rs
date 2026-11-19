//! Pure Helper Functions for the EQ Canvas Display
//! Smart shape auto-detection, band creation/deletion, hit-testing, and render-data builders.
//! All reads go through `LiveParamMirror` and all writes go through `LiveSetter` (0 ms feedback).

use super::{EditorShared, EditorState};
use crate::live_mirror::{LiveParamMirror, LiveSetter};
use crate::params::{
    BandParams, ChannelPlacementParam, CharacterModeParam, FilterSlopeParam, FilterTypeParam,
    PhantomEqParams, NUM_BANDS,
};
use nih_plug_egui::egui::Pos2;
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{MasterBarEvents, NodeRenderData};
use std::f32::consts::FRAC_1_SQRT_2;

pub const NODE_HIT_RADIUS: f32 = 15.0;
const SMART_BAND_FREQS: [f32; 8] = [100.0, 320.0, 1000.0, 3200.0, 10000.0, 60.0, 600.0, 6000.0];

pub fn band_cascaded_coeffs(
    band: &BandParams,
    live: &LiveParamMirror,
    sample_rate: f32,
    natural_phase: bool,
) -> CascadedCoeffs {
    CascadedCoeffs::calculate(
        live.read(&band.filter_type).into(),
        live.read(&band.slope).into(),
        sample_rate,
        live.read(&band.frequency),
        live.read(&band.gain),
        live.read(&band.q),
        natural_phase,
    )
}

/// Pro-Q 4 Smart Shape Auto-Detection based on cursor frequency and gain:
/// - `< 35 Hz` -> LowCut (12 dB/oct)
/// - `> 16500 Hz` -> HighCut (12 dB/oct)
/// - `< 100 Hz && |gain| > 1.5 dB` -> LowShelf
/// - `> 10000 Hz && |gain| > 1.5 dB` -> HighShelf
/// - otherwise -> Bell
pub fn smart_detect_shape(freq_hz: f32, gain_db: f32) -> FilterTypeParam {
    if freq_hz < 35.0 {
        FilterTypeParam::LowCut
    } else if freq_hz > 16500.0 {
        FilterTypeParam::HighCut
    } else if freq_hz < 100.0 && gain_db.abs() > 1.5 {
        FilterTypeParam::LowShelf
    } else if freq_hz > 10000.0 && gain_db.abs() > 1.5 {
        FilterTypeParam::HighShelf
    } else {
        FilterTypeParam::Bell
    }
}

pub fn find_first_unused_band(params: &PhantomEqParams, live: &LiveParamMirror) -> Option<usize> {
    (0..NUM_BANDS).find(|&i| !live.read(&params.bands[i].used))
}

pub fn find_hit_used_node(
    pos: Pos2,
    positions: &[Option<Pos2>; NUM_BANDS],
    selected: Option<usize>,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, pos_opt) in positions.iter().enumerate() {
        let Some(node_pos) = *pos_opt else {
            continue;
        };
        let dist = pos.distance(node_pos);
        if dist <= NODE_HIT_RADIUS {
            let effective = if selected == Some(i) {
                dist - 2.0
            } else {
                dist
            };
            if best.is_none_or(|(_, d)| effective < d) {
                best = Some((i, effective));
            }
        }
    }
    best.map(|(idx, _)| idx)
}

/// Allocates the first unused band at `(freq_hz, gain_db)` with Pro-Q Smart Shape Auto-Detection.
pub fn create_band_at(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    freq_hz: f32,
    gain_db: f32,
    now_sec: f64,
) -> Option<usize> {
    let idx = find_first_unused_band(params, setter.live)?;
    let freq = freq_hz.clamp(10.0, 30000.0);
    let gain = gain_db.clamp(-30.0, 30.0);
    let shape = smart_detect_shape(freq, gain);
    let init_gain = match shape {
        FilterTypeParam::LowCut | FilterTypeParam::HighCut => 0.0,
        _ => gain,
    };
    let init_q = match shape {
        FilterTypeParam::LowCut
        | FilterTypeParam::HighCut
        | FilterTypeParam::LowShelf
        | FilterTypeParam::HighShelf => FRAC_1_SQRT_2,
        _ => 1.0,
    };

    let band = &params.bands[idx];
    setter.set_once(&band.used, true);
    setter.set_once(&band.enabled, true);
    setter.set_once(&band.filter_type, shape);
    setter.set_once(&band.slope, FilterSlopeParam::Db12);
    setter.set_once(&band.placement, ChannelPlacementParam::Stereo);
    setter.set_once(&band.frequency, freq);
    setter.set_once(&band.gain, init_gain);
    setter.set_once(&band.q, init_q);
    setter.set_once(&band.dynamic_range, 0.0);
    setter.set_once(&band.spectral_dynamics, false);

    state.selected_band = Some(idx);
    state.last_band_created_time = now_sec;
    Some(idx)
}

/// Creates a Bell band at an uncrowded default frequency (for the `[+ BAND]` toolbar button).
pub fn create_smart_default_band(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    now_sec: f64,
) -> Option<usize> {
    let used_count = (0..NUM_BANDS)
        .filter(|&i| setter.read(&params.bands[i].used))
        .count();
    let freq = SMART_BAND_FREQS[used_count % SMART_BAND_FREQS.len()];
    create_band_at(setter, params, state, freq, 0.0, now_sec)
}

/// Removes a band cleanly: sets `used = false`, resets gain/dynamics to 0, and clears solo.
pub fn delete_band(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
    idx: usize,
) {
    if idx >= NUM_BANDS {
        return;
    }
    if state.dragging_band == Some(idx) {
        let band = &params.bands[idx];
        setter.end_set_parameter(&band.frequency);
        setter.end_set_parameter(&band.gain);
        state.dragging_band = None;
        state.drag_created_band = None;
    }

    let band = &params.bands[idx];
    setter.set_once(&band.used, false);
    setter.set_once(&band.gain, 0.0);
    setter.set_once(&band.dynamic_range, 0.0);
    setter.set_once(&band.spectral_dynamics, false);

    if state.latched_solo == Some(idx) {
        state.latched_solo = None;
    }
    if state.solo_hold_band == Some(idx) {
        state.solo_hold_band = None;
    }
    if state.alt_solo_band == Some(idx) {
        state.alt_solo_band = None;
    }
    if shared.solo_band() == Some(idx) {
        shared.clear_solo();
    }

    if state.selected_band == Some(idx) {
        state.selected_band =
            (0..NUM_BANDS).find(|&i| i != idx && setter.read(&params.bands[i].used));
    }
}

pub fn build_node_render_data(
    params: &PhantomEqParams,
    live: &LiveParamMirror,
    index: usize,
    is_soloed: bool,
    live_gr_db: f32,
) -> NodeRenderData {
    let band = &params.bands[index];
    NodeRenderData {
        index,
        freq: live.read(&band.frequency),
        gain: live.read(&band.gain),
        q: live.read(&band.q),
        dyn_range: live.read(&band.dynamic_range),
        live_gr_db,
        enabled: live.read(&band.enabled),
        is_soloed,
        filter_name: live.read(&band.filter_type).short_name(),
        target_name: live.read(&band.placement).short_name(),
    }
}

pub fn apply_master_events(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    events: MasterBarEvents,
) {
    if events.toggle_phase_invert {
        let cur = setter.read(&params.phase_invert);
        setter.set_once(&params.phase_invert, !cur);
    }
    if let Some(gain) = events.set_output_gain {
        setter.set_once(&params.output_gain, gain);
    }
    if events.toggle_auto_gain {
        let cur = setter.read(&params.auto_gain);
        setter.set_once(&params.auto_gain, !cur);
    }
    if let Some(idx) = events.set_character {
        setter.set_once(&params.character, CharacterModeParam::from_idx(idx));
    }
    if let Some(scale) = events.set_scale_db {
        state.scale_db = scale;
    }
}

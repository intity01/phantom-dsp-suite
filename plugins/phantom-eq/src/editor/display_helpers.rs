//! Pure Helper Functions for the EQ Canvas Display
//! Smart shape auto-detection, band creation/deletion, hit-testing, and render-data builders.
//! All reads go through `LiveParamMirror` and all writes go through `LiveSetter` (0 ms feedback).

use super::{EditorShared, EditorState};
use crate::dsp_state::scale_band_gain;
use crate::live_mirror::{LiveParamMirror, LiveSetter};
use crate::params::{
    BandParams, ChannelPlacementParam, FilterSlopeParam, FilterTypeParam, PhantomEqParams,
    NUM_BANDS,
};
use nih_plug_egui::egui::{Painter, Pos2, Rect, Response, Ui};
use phantom_dsp::{CascadedCoeffs, FilterType};
use phantom_ui::{
    draw_piano_roll, snap_freq_to_nearest_semitone, EqCanvasMapping, NodeRenderData, SpectrumPeak,
};
use std::f32::consts::FRAC_1_SQRT_2;

pub const NODE_HIT_RADIUS: f32 = 15.0;

pub fn band_cascaded_coeffs(
    band: &BandParams,
    live: &LiveParamMirror,
    sample_rate: f32,
    natural_phase: bool,
    gain_scale: f32,
) -> CascadedCoeffs {
    let ftype: FilterType = live.read(&band.filter_type).into();
    let scaled_gain = scale_band_gain(ftype, live.read(&band.gain), gain_scale);
    CascadedCoeffs::calculate(
        ftype,
        live.read(&band.slope).into(),
        sample_rate,
        live.read(&band.frequency),
        scaled_gain,
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
        state.drag_virtual_pos = None;
        state.drag_last_pointer = None;
    }
    if state.node_menu.is_some_and(|(b, _)| b == idx) {
        state.node_menu = None;
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

/// Renders the optional 20px C1..C8 Piano Roll bar and handles click-to-snap note selection.
/// Returns `true` when the pointer is hovering inside the piano roll strip.
#[allow(clippy::too_many_arguments)]
pub fn draw_piano_strip(
    ui: &Ui,
    painter: &Painter,
    response: &Response,
    map: &EqCanvasMapping,
    piano_rect: Rect,
    pointer_pos: Option<Pos2>,
    used_band_freqs: &[(usize, f32)],
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) -> bool {
    let hov_piano_freq = pointer_pos
        .filter(|p| piano_rect.contains(*p))
        .map(|p| map.x_to_freq(p.x));
    let in_piano = hov_piano_freq.is_some();
    let snapped_key = draw_piano_roll(
        painter,
        map,
        piano_rect,
        used_band_freqs,
        state.selected_band,
        hov_piano_freq,
    );
    if in_piano
        && state.node_menu.is_none()
        && response.contains_pointer()
        && ui.input(|i| i.pointer.primary_pressed())
    {
        if let Some(note_freq) = snapped_key {
            let f = note_freq.clamp(10.0, 30000.0);
            if let Some(sel) = state
                .selected_band
                .filter(|&i| i < NUM_BANDS && setter.read(&params.bands[i].used))
            {
                setter.set_once(&params.bands[sel].frequency, f);
            } else {
                let now = ui.input(|i| i.time);
                let _ = create_band_at(setter, params, state, f, 0.0, now);
            }
        }
    }
    in_piano
}

#[inline]
pub fn resolve_freq(raw_freq: f32, snap: bool) -> f32 {
    let c = raw_freq.clamp(10.0, 30000.0);
    if snap {
        snap_freq_to_nearest_semitone(c).clamp(10.0, 30000.0)
    } else {
        c
    }
}

pub fn grab_spectrum_peak_band(
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    peak: SpectrumPeak,
    start: Option<Pos2>,
    alt: bool,
    now_sec: f64,
) {
    let existing = (0..NUM_BANDS).find(|&i| {
        setter.read(&params.bands[i].used)
            && ((setter.read(&params.bands[i].frequency) - peak.freq_hz).abs()
                / peak.freq_hz.max(1.0))
                < 0.03
    });
    if let Some(idx) =
        existing.or_else(|| create_band_at(setter, params, state, peak.freq_hz, 0.0, now_sec))
    {
        let band = &params.bands[idx];
        setter.set_once(&band.filter_type, FilterTypeParam::Bell);
        setter.set_once(&band.q, peak.estimated_q.clamp(0.1, 30.0));
        state.selected_band = Some(idx);
        state.dragging_band = Some(idx);
        state.drag_virtual_pos = None;
        state.drag_last_pointer = start;
        setter.begin_set_parameter(&band.frequency);
        setter.set_parameter(&band.frequency, peak.freq_hz.clamp(10.0, 30000.0));
        setter.begin_set_parameter(&band.gain);
        if alt {
            state.alt_solo_band = Some(idx);
        }
    }
}

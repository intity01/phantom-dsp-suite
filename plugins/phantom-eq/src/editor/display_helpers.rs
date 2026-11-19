//! Pure Helper Functions for the EQ Canvas Display
//! Smart shape auto-detection, band creation/deletion, hit-testing, and render-data builders.

use super::{EditorShared, EditorState};
use crate::params::{
    BandParams, ChannelPlacementParam, FilterSlopeParam, FilterTypeParam, PhantomEqParams,
    NUM_BANDS,
};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::Pos2;
use phantom_dsp::CascadedCoeffs;
use phantom_ui::NodeRenderData;
use std::f32::consts::FRAC_1_SQRT_2;

pub const NODE_HIT_RADIUS: f32 = 15.0;
pub const CURVE_DRAG_HIT_PX: f32 = 22.0;

pub fn band_cascaded_coeffs(
    band: &BandParams,
    sample_rate: f32,
    natural_phase: bool,
) -> CascadedCoeffs {
    CascadedCoeffs::calculate(
        band.filter_type.value().into(),
        band.slope.value().into(),
        sample_rate,
        band.frequency.value(),
        band.gain.value(),
        band.q.value(),
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

pub fn find_first_unused_band(params: &PhantomEqParams) -> Option<usize> {
    (0..NUM_BANDS).find(|&i| !params.bands[i].used.value())
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
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    freq_hz: f32,
    gain_db: f32,
) -> Option<usize> {
    let idx = find_first_unused_band(params)?;
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
    setter.begin_set_parameter(&band.used);
    setter.set_parameter(&band.used, true);
    setter.end_set_parameter(&band.used);

    setter.begin_set_parameter(&band.enabled);
    setter.set_parameter(&band.enabled, true);
    setter.end_set_parameter(&band.enabled);

    setter.begin_set_parameter(&band.filter_type);
    setter.set_parameter(&band.filter_type, shape);
    setter.end_set_parameter(&band.filter_type);

    setter.begin_set_parameter(&band.slope);
    setter.set_parameter(&band.slope, FilterSlopeParam::Db12);
    setter.end_set_parameter(&band.slope);

    setter.begin_set_parameter(&band.placement);
    setter.set_parameter(&band.placement, ChannelPlacementParam::Stereo);
    setter.end_set_parameter(&band.placement);

    setter.begin_set_parameter(&band.frequency);
    setter.set_parameter(&band.frequency, freq);
    setter.end_set_parameter(&band.frequency);

    setter.begin_set_parameter(&band.gain);
    setter.set_parameter(&band.gain, init_gain);
    setter.end_set_parameter(&band.gain);

    setter.begin_set_parameter(&band.q);
    setter.set_parameter(&band.q, init_q);
    setter.end_set_parameter(&band.q);

    setter.begin_set_parameter(&band.dynamic_range);
    setter.set_parameter(&band.dynamic_range, 0.0);
    setter.end_set_parameter(&band.dynamic_range);

    state.selected_band = Some(idx);
    Some(idx)
}

/// Removes a band cleanly: sets `used = false`, resets gain/dynamics to 0, and clears solo.
pub fn delete_band(
    setter: &ParamSetter,
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
    setter.begin_set_parameter(&band.used);
    setter.set_parameter(&band.used, false);
    setter.end_set_parameter(&band.used);

    setter.begin_set_parameter(&band.gain);
    setter.set_parameter(&band.gain, 0.0);
    setter.end_set_parameter(&band.gain);

    setter.begin_set_parameter(&band.dynamic_range);
    setter.set_parameter(&band.dynamic_range, 0.0);
    setter.end_set_parameter(&band.dynamic_range);

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
        state.selected_band = (0..NUM_BANDS).find(|&i| i != idx && params.bands[i].used.value());
    }
}

pub fn build_node_render_data(
    params: &PhantomEqParams,
    index: usize,
    is_soloed: bool,
    live_gr_db: f32,
) -> NodeRenderData {
    let band = &params.bands[index];
    NodeRenderData {
        index,
        freq: band.frequency.value(),
        gain: band.gain.value(),
        q: band.q.value(),
        dyn_range: band.dynamic_range.value(),
        live_gr_db,
        enabled: band.enabled.value(),
        is_soloed,
        filter_name: band.filter_type.value().short_name(),
        target_name: band.placement.value().short_name(),
    }
}

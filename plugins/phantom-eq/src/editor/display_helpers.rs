//! Pure Helper Functions for the EQ Canvas Display
//! Node hit-testing, Pro-Q band targeting for canvas double-clicks, and render-data conversion.

use crate::params::{BandParams, ChannelPlacementParam, FilterTypeParam, PhantomEqParams, NUM_BANDS};
use nih_plug_egui::egui::Pos2;
use phantom_dsp::BiquadCoeffs;
use phantom_ui::NodeRenderData;

pub const NODE_HIT_RADIUS: f32 = 15.0;

pub fn band_coeffs(band: &BandParams, sample_rate: f32) -> BiquadCoeffs {
    BiquadCoeffs::calculate(
        band.filter_type.value().into(),
        sample_rate,
        band.frequency.value(),
        band.gain.value(),
        band.q.value(),
    )
}

pub fn find_hit_node(pos: Pos2, positions: &[Pos2; NUM_BANDS], selected: usize) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, &node_pos) in positions.iter().enumerate() {
        let dist = pos.distance(node_pos);
        if dist <= NODE_HIT_RADIUS {
            let effective = if i == selected { dist - 2.0 } else { dist };
            if best.map_or(true, |(_, d)| effective < d) {
                best = Some((i, effective));
            }
        }
    }
    best.map(|(idx, _)| idx)
}

pub fn find_band_for_canvas_double_click(params: &PhantomEqParams, target_freq: f32) -> usize {
    let target_ln = target_freq.max(10.0).ln();
    let log_dist = |i: usize| (params.bands[i].frequency.value().max(10.0).ln() - target_ln).abs();

    // Priority 1: nearest disabled band
    if let Some(idx) = (0..NUM_BANDS)
        .filter(|&i| !params.bands[i].enabled.value())
        .min_by(|&a, &b| log_dist(a).total_cmp(&log_dist(b)))
    {
        return idx;
    }

    // Priority 2: nearest flat unused band (Bell/Shelf at ~0 dB with no dynamic range)
    if let Some(idx) = (0..NUM_BANDS)
        .filter(|&i| {
            let b = &params.bands[i];
            matches!(
                b.filter_type.value(),
                FilterTypeParam::Bell | FilterTypeParam::LowShelf | FilterTypeParam::HighShelf
            ) && b.gain.value().abs() < 0.15
                && b.dynamic_range.value().abs() < 0.1
        })
        .min_by(|&a, &b| log_dist(a).total_cmp(&log_dist(b)))
    {
        return idx;
    }

    // Priority 3: nearest band overall
    (0..NUM_BANDS)
        .min_by(|&a, &b| log_dist(a).total_cmp(&log_dist(b)))
        .unwrap_or(0)
}

pub fn build_node_render_data(
    params: &PhantomEqParams,
    index: usize,
    is_soloed: bool,
) -> NodeRenderData {
    let band = &params.bands[index];
    NodeRenderData {
        index,
        freq: band.frequency.value(),
        gain: band.gain.value(),
        q: band.q.value(),
        dyn_range: band.dynamic_range.value(),
        enabled: band.enabled.value(),
        is_soloed,
        filter_name: match band.filter_type.value() {
            FilterTypeParam::Bell => "Bell",
            FilterTypeParam::LowCut => "Low Cut",
            FilterTypeParam::HighCut => "High Cut",
            FilterTypeParam::LowShelf => "Low Shelf",
            FilterTypeParam::HighShelf => "High Shelf",
            FilterTypeParam::Notch => "Notch",
            FilterTypeParam::BandPass => "Band Pass",
        },
        target_name: match band.placement.value() {
            ChannelPlacementParam::Stereo => "Stereo",
            ChannelPlacementParam::Left => "Left",
            ChannelPlacementParam::Right => "Right",
            ChannelPlacementParam::Mid => "Mid",
            ChannelPlacementParam::Side => "Side",
        },
    }
}

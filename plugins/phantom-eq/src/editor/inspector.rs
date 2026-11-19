//! Selected Band Inspector HUD Bridge
//! Connects phantom-ui's BandInspector widget to Phantom EQ parameters and Solo Audition state.

use super::{EditorShared, EditorState};
use crate::params::{ChannelPlacementParam, FilterTypeParam, PhantomEqParams, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::Ui;
use phantom_ui::{draw_band_inspector, BandInspectorData};

const FILTER_NAMES: [&str; 7] = [
    "Bell",
    "Low Cut",
    "High Cut",
    "Low Shelf",
    "High Shelf",
    "Notch",
    "Band Pass",
];

const ROUTING_NAMES: [&str; 5] = ["Stereo", "Mid", "Side", "Left", "Right"];

pub fn draw_inspector_section(
    ui: &mut Ui,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
) {
    let sel_idx = state.selected_band.clamp(0, NUM_BANDS - 1);
    let band = &params.bands[sel_idx];
    let soloed_band = shared.solo_band();

    let current_filter_idx = match band.filter_type.value() {
        FilterTypeParam::Bell => 0,
        FilterTypeParam::LowCut => 1,
        FilterTypeParam::HighCut => 2,
        FilterTypeParam::LowShelf => 3,
        FilterTypeParam::HighShelf => 4,
        FilterTypeParam::Notch => 5,
        FilterTypeParam::BandPass => 6,
    };

    let current_routing_idx = match band.placement.value() {
        ChannelPlacementParam::Stereo => 0,
        ChannelPlacementParam::Mid => 1,
        ChannelPlacementParam::Side => 2,
        ChannelPlacementParam::Left => 3,
        ChannelPlacementParam::Right => 4,
    };

    let data = BandInspectorData {
        selected_idx: sel_idx,
        num_bands: NUM_BANDS,
        is_enabled: band.enabled.value(),
        is_soloed: soloed_band == Some(sel_idx),
        soloed_band,
        filter_shape_idx: current_filter_idx,
        routing_idx: current_routing_idx,
        freq: band.frequency.value(),
        gain: band.gain.value(),
        q: band.q.value(),
        dyn_range: band.dynamic_range.value(),
        dyn_threshold: band.dynamic_threshold.value(),
    };

    let events = draw_band_inspector(ui, &data, &FILTER_NAMES, &ROUTING_NAMES);

    if let Some(idx) = events.select_band {
        state.selected_band = idx;
    }
    if events.toggle_enabled {
        setter.begin_set_parameter(&band.enabled);
        setter.set_parameter(&band.enabled, !band.enabled.value());
        setter.end_set_parameter(&band.enabled);
    }
    if events.toggle_solo {
        shared.toggle_solo(sel_idx);
    }
    if let Some(shape_idx) = events.set_filter_shape {
        let ftype = match shape_idx {
            0 => FilterTypeParam::Bell,
            1 => FilterTypeParam::LowCut,
            2 => FilterTypeParam::HighCut,
            3 => FilterTypeParam::LowShelf,
            4 => FilterTypeParam::HighShelf,
            5 => FilterTypeParam::Notch,
            _ => FilterTypeParam::BandPass,
        };
        setter.begin_set_parameter(&band.filter_type);
        setter.set_parameter(&band.filter_type, ftype);
        setter.end_set_parameter(&band.filter_type);
    }
    if let Some(rout_idx) = events.set_routing {
        let rout = match rout_idx {
            0 => ChannelPlacementParam::Stereo,
            1 => ChannelPlacementParam::Mid,
            2 => ChannelPlacementParam::Side,
            3 => ChannelPlacementParam::Left,
            _ => ChannelPlacementParam::Right,
        };
        setter.begin_set_parameter(&band.placement);
        setter.set_parameter(&band.placement, rout);
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
}

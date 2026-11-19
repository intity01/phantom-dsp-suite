//! Phantom EQ Parameters & Band Configuration
//! Automation-ready, sample-accurate DAW parameters.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use phantom_dsp::{ChannelTarget, FilterType};
use std::sync::Arc;

pub const NUM_BANDS: usize = 8;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingModeParam {
    #[name = "Zero Latency (Analog IIR)"]
    ZeroLatency,
    #[name = "Linear Phase (Mastering FIR)"]
    LinearPhase,
}

impl Default for ProcessingModeParam {
    fn default() -> Self {
        Self::ZeroLatency
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterTypeParam {
    #[name = "Bell (Peaking)"]
    Bell,
    #[name = "Low Cut (High Pass)"]
    LowCut,
    #[name = "High Cut (Low Pass)"]
    HighCut,
    #[name = "Low Shelf"]
    LowShelf,
    #[name = "High Shelf"]
    HighShelf,
    #[name = "Notch"]
    Notch,
    #[name = "Band Pass"]
    BandPass,
}

impl Default for FilterTypeParam {
    fn default() -> Self {
        Self::Bell
    }
}

impl From<FilterTypeParam> for FilterType {
    fn from(param: FilterTypeParam) -> Self {
        match param {
            FilterTypeParam::Bell => FilterType::Bell,
            FilterTypeParam::LowCut => FilterType::LowCut,
            FilterTypeParam::HighCut => FilterType::HighCut,
            FilterTypeParam::LowShelf => FilterType::LowShelf,
            FilterTypeParam::HighShelf => FilterType::HighShelf,
            FilterTypeParam::Notch => FilterType::Notch,
            FilterTypeParam::BandPass => FilterType::BandPass,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPlacementParam {
    #[name = "Stereo (L/R)"]
    Stereo,
    #[name = "Left Channel"]
    Left,
    #[name = "Right Channel"]
    Right,
    #[name = "Mid (Center)"]
    Mid,
    #[name = "Side (Stereo Width)"]
    Side,
}

impl Default for ChannelPlacementParam {
    fn default() -> Self {
        Self::Stereo
    }
}

impl From<ChannelPlacementParam> for ChannelTarget {
    fn from(param: ChannelPlacementParam) -> Self {
        match param {
            ChannelPlacementParam::Stereo => ChannelTarget::Stereo,
            ChannelPlacementParam::Left => ChannelTarget::Left,
            ChannelPlacementParam::Right => ChannelTarget::Right,
            ChannelPlacementParam::Mid => ChannelTarget::Mid,
            ChannelPlacementParam::Side => ChannelTarget::Side,
        }
    }
}

#[derive(Params)]
pub struct BandParams {
    #[id = "enabled"]
    pub enabled: BoolParam,

    #[id = "type"]
    pub filter_type: EnumParam<FilterTypeParam>,

    #[id = "channel"]
    pub placement: EnumParam<ChannelPlacementParam>,

    #[id = "freq"]
    pub frequency: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "q"]
    pub q: FloatParam,

    #[id = "dyn_range"]
    pub dynamic_range: FloatParam,

    #[id = "dyn_thresh"]
    pub dynamic_threshold: FloatParam,
}

impl BandParams {
    pub fn new(
        index: usize,
        default_type: FilterTypeParam,
        default_freq: f32,
        default_gain: f32,
        default_q: f32,
        default_placement: ChannelPlacementParam,
    ) -> Self {
        let name_prefix = format!("Band {index}");
        Self {
            enabled: BoolParam::new(format!("{name_prefix} On"), true),
            filter_type: EnumParam::new(format!("{name_prefix} Shape"), default_type),
            placement: EnumParam::new(format!("{name_prefix} Placement"), default_placement),
            frequency: FloatParam::new(
                format!("{name_prefix} Frequency"),
                default_freq,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 30000.0,
                    factor: FloatRange::skew_factor(-2.8),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1)),
            gain: FloatParam::new(
                format!("{name_prefix} Gain"),
                default_gain,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            q: FloatParam::new(
                format!("{name_prefix} Q"),
                default_q,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 30.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            dynamic_range: FloatParam::new(
                format!("{name_prefix} Dynamic Range"),
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            dynamic_threshold: FloatParam::new(
                format!("{name_prefix} Dynamic Threshold"),
                -18.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
        }
    }
}

#[derive(Params)]
pub struct PhantomEqParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "mode"]
    pub mode: EnumParam<ProcessingModeParam>,

    #[id = "anti_cramping"]
    pub anti_cramping: BoolParam,

    #[id = "out_gain"]
    pub output_gain: FloatParam,

    #[id = "bypass"]
    pub bypass: BoolParam,

    #[id = "phase_invert"]
    pub phase_invert: BoolParam,

    #[nested(array, group = "EQ Bands")]
    pub bands: [BandParams; NUM_BANDS],
}

impl Default for PhantomEqParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1020, 580),
            mode: EnumParam::new("Processing Mode", ProcessingModeParam::ZeroLatency),
            anti_cramping: BoolParam::new("Anti-Cramping (2x OS)", true),
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            bypass: BoolParam::new("Bypass", false),
            phase_invert: BoolParam::new("Phase Invert", false),
            bands: [
                BandParams::new(1, FilterTypeParam::LowCut, 25.0, 0.0, 0.707, ChannelPlacementParam::Stereo),
                BandParams::new(2, FilterTypeParam::LowShelf, 80.0, 0.0, 0.707, ChannelPlacementParam::Stereo),
                BandParams::new(3, FilterTypeParam::Bell, 250.0, 0.0, 1.0, ChannelPlacementParam::Stereo),
                BandParams::new(4, FilterTypeParam::Bell, 700.0, 0.0, 1.0, ChannelPlacementParam::Stereo),
                BandParams::new(5, FilterTypeParam::Bell, 2000.0, 0.0, 1.0, ChannelPlacementParam::Stereo),
                BandParams::new(6, FilterTypeParam::Bell, 4500.0, 0.0, 1.0, ChannelPlacementParam::Stereo),
                BandParams::new(7, FilterTypeParam::HighShelf, 8000.0, 0.0, 0.707, ChannelPlacementParam::Stereo),
                BandParams::new(8, FilterTypeParam::HighCut, 18000.0, 0.0, 0.707, ChannelPlacementParam::Stereo),
            ],
        }
    }
}

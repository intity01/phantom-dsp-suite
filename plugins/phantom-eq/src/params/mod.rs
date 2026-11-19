//! Phantom EQ Parameter Tree (24 Dynamic Bands + Mastering Global Controls)

pub mod band;
pub mod enums;

pub use band::BandParams;
pub use enums::{
    ChannelPlacementParam, CharacterModeParam, FilterSlopeParam, FilterTypeParam,
    OversamplingParam, ProcessingModeParam, FILTER_NAMES, ROUTING_NAMES, SLOPE_NAMES,
};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

pub const NUM_BANDS: usize = 24;

#[derive(Params)]
pub struct PhantomEqParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "mode"]
    pub mode: EnumParam<ProcessingModeParam>,

    #[id = "oversampling"]
    pub oversampling: EnumParam<OversamplingParam>,

    #[id = "character"]
    pub character: EnumParam<CharacterModeParam>,

    #[id = "auto_gain"]
    pub auto_gain: BoolParam,

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
            editor_state: EguiState::from_size(1060, 620),
            mode: EnumParam::new("Processing Mode", ProcessingModeParam::ZeroLatency),
            oversampling: EnumParam::new("Oversampling", OversamplingParam::X2),
            character: EnumParam::new("Character", CharacterModeParam::Clean),
            auto_gain: BoolParam::new("Auto Gain", false),
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
            bands: std::array::from_fn(|i| BandParams::new(i + 1)),
        }
    }
}

//! Per-Band Parameter Definition for Phantom EQ (24 Dynamic Slots)
//! All bands initialize with `used = false` for a clean Pro-Q 4 flat empty startup canvas.

use super::enums::{ChannelPlacementParam, FilterSlopeParam, FilterTypeParam};
use nih_plug::prelude::*;

#[derive(Params)]
pub struct BandParams {
    /// Whether this band slot has been created on the canvas (default `false` for all 24 bands).
    #[id = "used"]
    pub used: BoolParam,

    #[id = "enabled"]
    pub enabled: BoolParam,

    #[id = "type"]
    pub filter_type: EnumParam<FilterTypeParam>,

    #[id = "slope"]
    pub slope: EnumParam<FilterSlopeParam>,

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

    #[id = "dyn_attack"]
    pub dynamic_attack: FloatParam,

    #[id = "dyn_release"]
    pub dynamic_release: FloatParam,

    #[id = "dyn_auto"]
    pub dynamic_auto_thresh: BoolParam,

    #[id = "dyn_ext_sc"]
    pub dynamic_ext_sc: BoolParam,

    #[id = "spec_dyn"]
    pub spectral_dynamics: BoolParam,
}

impl BandParams {
    pub fn new(index: usize) -> Self {
        let p = format!("Band {index}");
        Self {
            used: BoolParam::new(format!("{p} Used"), false),
            enabled: BoolParam::new(format!("{p} On"), true),
            filter_type: EnumParam::new(format!("{p} Shape"), FilterTypeParam::Bell),
            slope: EnumParam::new(format!("{p} Slope"), FilterSlopeParam::Db12),
            placement: EnumParam::new(format!("{p} Placement"), ChannelPlacementParam::Stereo),
            frequency: FloatParam::new(
                format!("{p} Frequency"),
                1000.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 30000.0,
                    factor: FloatRange::skew_factor(-2.8),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1)),
            gain: FloatParam::new(
                format!("{p} Gain"),
                0.0,
                FloatRange::Linear {
                    min: -30.0,
                    max: 30.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            q: FloatParam::new(
                format!("{p} Q"),
                1.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 30.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            dynamic_range: FloatParam::new(
                format!("{p} Dynamic Range"),
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            dynamic_threshold: FloatParam::new(
                format!("{p} Dynamic Threshold"),
                -18.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            dynamic_attack: FloatParam::new(
                format!("{p} Dynamic Attack"),
                10.0,
                FloatRange::Skewed {
                    min: 0.5,
                    max: 250.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            dynamic_release: FloatParam::new(
                format!("{p} Dynamic Release"),
                100.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            dynamic_auto_thresh: BoolParam::new(format!("{p} Dynamic Auto Threshold"), true),
            dynamic_ext_sc: BoolParam::new(format!("{p} Dynamic External Sidechain"), false),
            spectral_dynamics: BoolParam::new(format!("{p} Spectral Dynamics"), false),
        }
    }
}

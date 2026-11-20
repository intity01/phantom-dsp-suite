//! Phantom Vox Parameter Tree (Real-Time Vocal Doubler & Sidechain Aligner)

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoxModeParam {
    #[default]
    #[name = "Doubler"]
    Doubler,
    #[name = "Live Align"]
    LiveAlign,
    #[name = "Align & Double"]
    AlignAndDouble,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoicesParam {
    #[default]
    #[name = "2 Voices"]
    TwoVoices,
    #[name = "4 Voices"]
    FourVoices,
}

#[derive(Params)]
pub struct PhantomVoxParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "mode"]
    pub mode: EnumParam<VoxModeParam>,

    #[id = "voices"]
    pub voices: EnumParam<VoicesParam>,

    #[id = "bypass"]
    pub bypass: BoolParam,

    #[id = "timing_ms"]
    pub timing_ms: FloatParam,

    #[id = "detune_cents"]
    pub detune_cents: FloatParam,

    #[id = "formant_st"]
    pub formant_st: FloatParam,

    #[id = "width_pct"]
    pub width_pct: FloatParam,

    #[id = "sibilance_safe"]
    pub sibilance_safe: BoolParam,

    #[id = "align_tightness"]
    pub align_tightness: FloatParam,

    #[id = "level_match"]
    pub level_match: FloatParam,

    #[id = "mono_check"]
    pub mono_check: BoolParam,

    #[id = "mix_pct"]
    pub mix_pct: FloatParam,

    #[id = "output_gain"]
    pub output_gain: FloatParam,
}

impl Default for PhantomVoxParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(860, 520),
            mode: EnumParam::new("Mode", VoxModeParam::Doubler),
            voices: EnumParam::new("Voices", VoicesParam::TwoVoices),
            // `.make_bypass()` sets `ParamFlags::BYPASS` (VST3 `kIsBypass`) for Cubase insert bypass.
            bypass: BoolParam::new("Bypass", false).make_bypass(),
            timing_ms: FloatParam::new(
                "Timing",
                12.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 35.0,
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            detune_cents: FloatParam::new(
                "Detune",
                10.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 35.0,
                },
            )
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            formant_st: FloatParam::new(
                "Formant",
                0.0,
                FloatRange::Linear {
                    min: -4.0,
                    max: 4.0,
                },
            )
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            width_pct: FloatParam::new(
                "Width",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 150.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            sibilance_safe: BoolParam::new("Sibilance Safe", true),
            align_tightness: FloatParam::new(
                "Align Tightness",
                80.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            level_match: FloatParam::new(
                "Level Match",
                30.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            mono_check: BoolParam::new("Mono Check", false),
            mix_pct: FloatParam::new(
                "Mix",
                55.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
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
        }
    }
}

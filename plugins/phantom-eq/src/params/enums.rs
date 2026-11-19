//! Automatable Enum Parameters & DSP Conversions for Phantom EQ
//! Covers Processing Modes, Oversampling, Character, 10 Filter Shapes, 8 Slopes, and M/S Routing.

use nih_plug::prelude::*;
use phantom_dsp::{ChannelTarget, CharacterMode, FilterSlope, FilterType, OversampleFactor};

pub const FILTER_NAMES: [&str; 10] = [
    "Bell",
    "Low Cut",
    "High Cut",
    "Low Shelf",
    "High Shelf",
    "Notch",
    "Band Pass",
    "Tilt Shelf",
    "Flat Tilt",
    "All Pass",
];

pub const SLOPE_NAMES: [&str; 8] = [
    "6 dB", "12 dB", "18 dB", "24 dB", "36 dB", "48 dB", "96 dB", "Brick",
];

pub const ROUTING_NAMES: [&str; 5] = ["Stereo", "Mid", "Side", "Left", "Right"];

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessingModeParam {
    #[default]
    #[name = "Zero Latency"]
    ZeroLatency,
    #[name = "Natural Phase"]
    NaturalPhase,
    #[name = "Linear Phase"]
    LinearPhase,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OversamplingParam {
    #[name = "Off"]
    Off,
    #[default]
    #[name = "2x HQ"]
    X2,
    #[name = "4x HQ"]
    X4,
}

impl From<OversamplingParam> for OversampleFactor {
    fn from(param: OversamplingParam) -> Self {
        match param {
            OversamplingParam::Off => OversampleFactor::Off,
            OversamplingParam::X2 => OversampleFactor::X2,
            OversamplingParam::X4 => OversampleFactor::X4,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterModeParam {
    #[default]
    #[name = "Clean"]
    Clean,
    #[name = "Subtle Analog"]
    SubtleAnalog,
    #[name = "Warm Tube"]
    WarmTube,
}

impl CharacterModeParam {
    pub fn to_idx(self) -> usize {
        match self {
            Self::Clean => 0,
            Self::SubtleAnalog => 1,
            Self::WarmTube => 2,
        }
    }

    pub fn from_idx(idx: usize) -> Self {
        match idx {
            1 => Self::SubtleAnalog,
            2 => Self::WarmTube,
            _ => Self::Clean,
        }
    }
}

impl From<CharacterModeParam> for CharacterMode {
    fn from(param: CharacterModeParam) -> Self {
        match param {
            CharacterModeParam::Clean => CharacterMode::Clean,
            CharacterModeParam::SubtleAnalog => CharacterMode::SubtleAnalog,
            CharacterModeParam::WarmTube => CharacterMode::WarmTube,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterTypeParam {
    #[default]
    #[name = "Bell"]
    Bell,
    #[name = "Low Cut"]
    LowCut,
    #[name = "High Cut"]
    HighCut,
    #[name = "Low Shelf"]
    LowShelf,
    #[name = "High Shelf"]
    HighShelf,
    #[name = "Notch"]
    Notch,
    #[name = "Band Pass"]
    BandPass,
    #[name = "Tilt Shelf"]
    TiltShelf,
    #[name = "Flat Tilt"]
    FlatTilt,
    #[name = "All Pass"]
    AllPass,
}

impl FilterTypeParam {
    pub fn to_idx(self) -> usize {
        match self {
            Self::Bell => 0,
            Self::LowCut => 1,
            Self::HighCut => 2,
            Self::LowShelf => 3,
            Self::HighShelf => 4,
            Self::Notch => 5,
            Self::BandPass => 6,
            Self::TiltShelf => 7,
            Self::FlatTilt => 8,
            Self::AllPass => 9,
        }
    }

    pub fn from_idx(idx: usize) -> Self {
        match idx {
            1 => Self::LowCut,
            2 => Self::HighCut,
            3 => Self::LowShelf,
            4 => Self::HighShelf,
            5 => Self::Notch,
            6 => Self::BandPass,
            7 => Self::TiltShelf,
            8 => Self::FlatTilt,
            9 => Self::AllPass,
            _ => Self::Bell,
        }
    }

    pub fn short_name(self) -> &'static str {
        FILTER_NAMES[self.to_idx()]
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
            FilterTypeParam::TiltShelf => FilterType::TiltShelf,
            FilterTypeParam::FlatTilt => FilterType::FlatTilt,
            FilterTypeParam::AllPass => FilterType::AllPass,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterSlopeParam {
    #[name = "6 dB/oct"]
    Db6,
    #[default]
    #[name = "12 dB/oct"]
    Db12,
    #[name = "18 dB/oct"]
    Db18,
    #[name = "24 dB/oct"]
    Db24,
    #[name = "36 dB/oct"]
    Db36,
    #[name = "48 dB/oct"]
    Db48,
    #[name = "96 dB/oct"]
    Db96,
    #[name = "Brickwall"]
    Brickwall,
}

impl FilterSlopeParam {
    pub fn to_idx(self) -> usize {
        match self {
            Self::Db6 => 0,
            Self::Db12 => 1,
            Self::Db18 => 2,
            Self::Db24 => 3,
            Self::Db36 => 4,
            Self::Db48 => 5,
            Self::Db96 => 6,
            Self::Brickwall => 7,
        }
    }

    pub fn from_idx(idx: usize) -> Self {
        match idx {
            0 => Self::Db6,
            2 => Self::Db18,
            3 => Self::Db24,
            4 => Self::Db36,
            5 => Self::Db48,
            6 => Self::Db96,
            7 => Self::Brickwall,
            _ => Self::Db12,
        }
    }
}

impl From<FilterSlopeParam> for FilterSlope {
    fn from(param: FilterSlopeParam) -> Self {
        match param {
            FilterSlopeParam::Db6 => FilterSlope::Db6,
            FilterSlopeParam::Db12 => FilterSlope::Db12,
            FilterSlopeParam::Db18 => FilterSlope::Db18,
            FilterSlopeParam::Db24 => FilterSlope::Db24,
            FilterSlopeParam::Db36 => FilterSlope::Db36,
            FilterSlopeParam::Db48 => FilterSlope::Db48,
            FilterSlopeParam::Db96 => FilterSlope::Db96,
            FilterSlopeParam::Brickwall => FilterSlope::Brickwall,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelPlacementParam {
    #[default]
    #[name = "Stereo"]
    Stereo,
    #[name = "Left"]
    Left,
    #[name = "Right"]
    Right,
    #[name = "Mid"]
    Mid,
    #[name = "Side"]
    Side,
}

impl ChannelPlacementParam {
    pub fn to_ui_idx(self) -> usize {
        match self {
            Self::Stereo => 0,
            Self::Mid => 1,
            Self::Side => 2,
            Self::Left => 3,
            Self::Right => 4,
        }
    }

    pub fn from_ui_idx(idx: usize) -> Self {
        match idx {
            1 => Self::Mid,
            2 => Self::Side,
            3 => Self::Left,
            4 => Self::Right,
            _ => Self::Stereo,
        }
    }

    pub fn short_name(self) -> &'static str {
        ROUTING_NAMES[self.to_ui_idx()]
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

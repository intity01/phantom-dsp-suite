//! Full 24-Band & Global Parameter Snapshot Engine for Phantom EQ
//! Supports host-automation-safe parameter application via `ParamSetter` and float-noise-tolerant comparison.

use crate::params::{
    BandParams, ChannelPlacementParam, CharacterModeParam, FilterSlopeParam, FilterTypeParam,
    OversamplingParam, PhantomEqParams, ProcessingModeParam, NUM_BANDS,
};
use nih_plug::prelude::*;

pub type OversampleParam = OversamplingParam;
pub type CharacterParam = CharacterModeParam;

#[inline]
fn f_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn set_bool(setter: &ParamSetter<'_>, param: &BoolParam, val: bool) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, val);
    setter.end_set_parameter(param);
}

fn set_float(setter: &ParamSetter<'_>, param: &FloatParam, val: f32) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, val);
    setter.end_set_parameter(param);
}

fn set_enum<T: Enum + PartialEq>(setter: &ParamSetter<'_>, param: &EnumParam<T>, val: T) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, val);
    setter.end_set_parameter(param);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandSnapshotData {
    pub used: bool,
    pub enabled: bool,
    pub filter_type: FilterTypeParam,
    pub slope: FilterSlopeParam,
    pub placement: ChannelPlacementParam,
    pub frequency: f32,
    pub gain: f32,
    pub q: f32,
    pub dynamic_range: f32,
    pub dynamic_threshold: f32,
    pub dynamic_attack: f32,
    pub dynamic_release: f32,
    pub dynamic_auto_thresh: bool,
    pub dynamic_ext_sc: bool,
    pub spectral_enabled: bool,
}

impl Default for BandSnapshotData {
    fn default() -> Self {
        Self {
            used: false,
            enabled: true,
            filter_type: FilterTypeParam::Bell,
            slope: FilterSlopeParam::Db12,
            placement: ChannelPlacementParam::Stereo,
            frequency: 1000.0,
            gain: 0.0,
            q: 1.0,
            dynamic_range: 0.0,
            dynamic_threshold: -18.0,
            dynamic_attack: 10.0,
            dynamic_release: 100.0,
            dynamic_auto_thresh: true,
            dynamic_ext_sc: false,
            spectral_enabled: false,
        }
    }
}

impl BandSnapshotData {
    pub const fn active(
        filter_type: FilterTypeParam,
        slope: FilterSlopeParam,
        placement: ChannelPlacementParam,
        frequency: f32,
        gain: f32,
        q: f32,
    ) -> Self {
        Self {
            used: true,
            enabled: true,
            filter_type,
            slope,
            placement,
            frequency,
            gain,
            q,
            dynamic_range: 0.0,
            dynamic_threshold: -18.0,
            dynamic_attack: 10.0,
            dynamic_release: 100.0,
            dynamic_auto_thresh: true,
            dynamic_ext_sc: false,
            spectral_enabled: false,
        }
    }

    pub const fn with_dynamics(mut self, range_db: f32, threshold_db: f32) -> Self {
        self.dynamic_range = range_db;
        self.dynamic_threshold = threshold_db;
        self
    }

    pub const fn with_ballistics(mut self, attack_ms: f32, release_ms: f32) -> Self {
        self.dynamic_attack = attack_ms;
        self.dynamic_release = release_ms;
        self
    }

    pub const fn with_spectral(mut self, enabled: bool) -> Self {
        self.spectral_enabled = enabled;
        self
    }

    pub fn capture_from(band: &BandParams) -> Self {
        Self {
            used: band.used.value(),
            enabled: band.enabled.value(),
            filter_type: band.filter_type.value(),
            slope: band.slope.value(),
            placement: band.placement.value(),
            frequency: band.frequency.value(),
            gain: band.gain.value(),
            q: band.q.value(),
            dynamic_range: band.dynamic_range.value(),
            dynamic_threshold: band.dynamic_threshold.value(),
            dynamic_attack: band.dynamic_attack.value(),
            dynamic_release: band.dynamic_release.value(),
            dynamic_auto_thresh: band.dynamic_auto_thresh.value(),
            dynamic_ext_sc: band.dynamic_ext_sc.value(),
            spectral_enabled: band.spectral_dynamics.value(),
        }
    }

    pub fn apply_to(&self, band: &BandParams, setter: &ParamSetter<'_>) {
        set_bool(setter, &band.used, self.used);
        set_bool(setter, &band.enabled, self.enabled);
        set_enum(setter, &band.filter_type, self.filter_type);
        set_enum(setter, &band.slope, self.slope);
        set_enum(setter, &band.placement, self.placement);
        set_float(setter, &band.frequency, self.frequency);
        set_float(setter, &band.gain, self.gain);
        set_float(setter, &band.q, self.q);
        set_float(setter, &band.dynamic_range, self.dynamic_range);
        set_float(setter, &band.dynamic_threshold, self.dynamic_threshold);
        set_float(setter, &band.dynamic_attack, self.dynamic_attack);
        set_float(setter, &band.dynamic_release, self.dynamic_release);
        set_bool(setter, &band.dynamic_auto_thresh, self.dynamic_auto_thresh);
        set_bool(setter, &band.dynamic_ext_sc, self.dynamic_ext_sc);
        set_bool(setter, &band.spectral_dynamics, self.spectral_enabled);
    }

    pub fn approx_eq(&self, other: &Self) -> bool {
        self.used == other.used
            && self.enabled == other.enabled
            && self.filter_type == other.filter_type
            && self.slope == other.slope
            && self.placement == other.placement
            && f_eq(self.frequency, other.frequency)
            && f_eq(self.gain, other.gain)
            && f_eq(self.q, other.q)
            && f_eq(self.dynamic_range, other.dynamic_range)
            && f_eq(self.dynamic_threshold, other.dynamic_threshold)
            && f_eq(self.dynamic_attack, other.dynamic_attack)
            && f_eq(self.dynamic_release, other.dynamic_release)
            && self.dynamic_auto_thresh == other.dynamic_auto_thresh
            && self.dynamic_ext_sc == other.dynamic_ext_sc
            && self.spectral_enabled == other.spectral_enabled
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EqPresetSnapshot {
    pub name: String,
    pub category: &'static str,
    pub mode: ProcessingModeParam,
    pub oversampling: OversampleParam,
    pub character: CharacterParam,
    pub auto_gain: bool,
    pub output_gain_db: f32,
    pub phase_invert: bool,
    pub bands: [BandSnapshotData; NUM_BANDS],
}

impl Default for EqPresetSnapshot {
    fn default() -> Self {
        Self::empty("Default (Flat Empty)", "Init")
    }
}

impl EqPresetSnapshot {
    pub fn empty(name: impl Into<String>, category: &'static str) -> Self {
        Self {
            name: name.into(),
            category,
            mode: ProcessingModeParam::ZeroLatency,
            oversampling: OversampleParam::X2,
            character: CharacterParam::Clean,
            auto_gain: false,
            output_gain_db: 0.0,
            phase_invert: false,
            bands: [BandSnapshotData::default(); NUM_BANDS],
        }
    }

    pub fn with_bands(
        name: impl Into<String>,
        category: &'static str,
        mode: ProcessingModeParam,
        character: CharacterParam,
        active_bands: &[BandSnapshotData],
    ) -> Self {
        let mut snap = Self::empty(name, category);
        snap.mode = mode;
        snap.character = character;
        for (i, b) in active_bands.iter().take(NUM_BANDS).enumerate() {
            snap.bands[i] = *b;
        }
        snap
    }

    pub fn capture_from(params: &PhantomEqParams) -> Self {
        Self {
            name: String::from("Current"),
            category: "User",
            mode: params.mode.value(),
            oversampling: params.oversampling.value(),
            character: params.character.value(),
            auto_gain: params.auto_gain.value(),
            output_gain_db: params.output_gain.value(),
            phase_invert: params.phase_invert.value(),
            bands: std::array::from_fn(|i| BandSnapshotData::capture_from(&params.bands[i])),
        }
    }

    pub fn apply_to(&self, params: &PhantomEqParams, setter: &ParamSetter<'_>) {
        set_enum(setter, &params.mode, self.mode);
        set_enum(setter, &params.oversampling, self.oversampling);
        set_enum(setter, &params.character, self.character);
        set_bool(setter, &params.auto_gain, self.auto_gain);
        set_float(setter, &params.output_gain, self.output_gain_db);
        set_bool(setter, &params.phase_invert, self.phase_invert);
        for (i, band_snap) in self.bands.iter().enumerate() {
            band_snap.apply_to(&params.bands[i], setter);
        }
    }

    pub fn approx_eq(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.oversampling == other.oversampling
            && self.character == other.character
            && self.auto_gain == other.auto_gain
            && f_eq(self.output_gain_db, other.output_gain_db)
            && self.phase_invert == other.phase_invert
            && self
                .bands
                .iter()
                .zip(other.bands.iter())
                .all(|(a, b)| a.approx_eq(b))
    }
}

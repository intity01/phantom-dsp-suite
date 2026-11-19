//! Full 24-Band & Global Parameter Snapshot Engine for Phantom EQ
//! Captures from the host params or the live GUI mirror (`ParamRead`), applies through the
//! automation-safe `LiveSetter`, and compares with float-noise-tolerant (relative) equality.

use crate::live_mirror::{LiveSetter, ParamRead};
use crate::params::{
    BandParams, ChannelPlacementParam, CharacterModeParam, FilterSlopeParam, FilterTypeParam,
    OversamplingParam, PhantomEqParams, ProcessingModeParam, NUM_BANDS,
};

pub type OversampleParam = OversamplingParam;
pub type CharacterParam = CharacterModeParam;

#[inline]
fn f_eq(a: f32, b: f32) -> bool {
    // Relative tolerance: normalized<->plain round trips of e.g. 10 kHz drift by ~1e-3 Hz.
    (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1.0)
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

    pub fn capture_with(band: &BandParams, r: &impl ParamRead) -> Self {
        Self {
            used: r.read(&band.used),
            enabled: r.read(&band.enabled),
            filter_type: r.read(&band.filter_type),
            slope: r.read(&band.slope),
            placement: r.read(&band.placement),
            frequency: r.read(&band.frequency),
            gain: r.read(&band.gain),
            q: r.read(&band.q),
            dynamic_range: r.read(&band.dynamic_range),
            dynamic_threshold: r.read(&band.dynamic_threshold),
            dynamic_attack: r.read(&band.dynamic_attack),
            dynamic_release: r.read(&band.dynamic_release),
            dynamic_auto_thresh: r.read(&band.dynamic_auto_thresh),
            dynamic_ext_sc: r.read(&band.dynamic_ext_sc),
            spectral_enabled: r.read(&band.spectral_dynamics),
        }
    }

    pub fn apply_to(&self, band: &BandParams, setter: &LiveSetter<'_>) {
        setter.set_once(&band.used, self.used);
        setter.set_once(&band.enabled, self.enabled);
        setter.set_once(&band.filter_type, self.filter_type);
        setter.set_once(&band.slope, self.slope);
        setter.set_once(&band.placement, self.placement);
        setter.set_once(&band.frequency, self.frequency);
        setter.set_once(&band.gain, self.gain);
        setter.set_once(&band.q, self.q);
        setter.set_once(&band.dynamic_range, self.dynamic_range);
        setter.set_once(&band.dynamic_threshold, self.dynamic_threshold);
        setter.set_once(&band.dynamic_attack, self.dynamic_attack);
        setter.set_once(&band.dynamic_release, self.dynamic_release);
        setter.set_once(&band.dynamic_auto_thresh, self.dynamic_auto_thresh);
        setter.set_once(&band.dynamic_ext_sc, self.dynamic_ext_sc);
        setter.set_once(&band.spectral_dynamics, self.spectral_enabled);
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

    /// Captures the values currently stored in the host parameter objects.
    pub fn capture_from(params: &PhantomEqParams) -> Self {
        Self::capture_with(params, &crate::live_mirror::HostRead)
    }

    /// Captures through any reader (e.g. the authoritative live GUI mirror).
    pub fn capture_with(params: &PhantomEqParams, r: &impl ParamRead) -> Self {
        Self {
            name: String::from("Current"),
            category: "User",
            mode: r.read(&params.mode),
            oversampling: r.read(&params.oversampling),
            character: r.read(&params.character),
            auto_gain: r.read(&params.auto_gain),
            output_gain_db: r.read(&params.output_gain),
            phase_invert: r.read(&params.phase_invert),
            bands: std::array::from_fn(|i| BandSnapshotData::capture_with(&params.bands[i], r)),
        }
    }

    pub fn apply_to(&self, params: &PhantomEqParams, setter: &LiveSetter<'_>) {
        setter.set_once(&params.mode, self.mode);
        setter.set_once(&params.oversampling, self.oversampling);
        setter.set_once(&params.character, self.character);
        setter.set_once(&params.auto_gain, self.auto_gain);
        setter.set_once(&params.output_gain, self.output_gain_db);
        setter.set_once(&params.phase_invert, self.phase_invert);
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

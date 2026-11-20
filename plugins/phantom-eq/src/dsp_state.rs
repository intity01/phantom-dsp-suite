//! Zero-Allocation DSP State Helpers for Phantom EQ
//! Provides fixed-size 24-band snapshots, lock-free live GR telemetry,
//! Solo Audition auditioning filters, and M/S-aware stereo filter routing.

use crate::live_mirror::{HostRead, ParamRead};
use crate::params::{BandParams, NUM_BANDS};
use phantom_dsp::{
    decode_mid_side, encode_mid_side, BiquadCoeffs, CascadedCoeffs, CascadedStereoFilter,
    ChannelTarget, FilterSlope, FilterType, SpectralBandProcessor, StereoBiquad,
};
use std::sync::atomic::{AtomicU32, Ordering};

/// Returns `true` for filter shapes whose amplitude scales with global `Gain Scale`.
#[inline]
pub const fn shape_uses_gain_scale(ftype: FilterType) -> bool {
    matches!(
        ftype,
        FilterType::Bell
            | FilterType::LowShelf
            | FilterType::HighShelf
            | FilterType::TiltShelf
            | FilterType::FlatTilt
            | FilterType::BandPass
    )
}

/// Applies normalized `gain_scale` (`0.0..=2.0`) to band gain or dynamic range for gain-bearing shapes.
#[inline]
pub fn scale_band_gain(ftype: FilterType, gain_db: f32, gain_scale: f32) -> f32 {
    if shape_uses_gain_scale(ftype) {
        (gain_db * gain_scale).clamp(-60.0, 60.0)
    } else {
        gain_db
    }
}

/// Applies normalized `gain_scale` (`0.0..=2.0`) to dynamic range for gain-bearing shapes only.
#[inline]
pub fn scale_band_dyn_range(ftype: FilterType, dyn_range_db: f32, gain_scale: f32) -> f32 {
    if shape_uses_gain_scale(ftype) {
        (dyn_range_db * gain_scale).clamp(-48.0, 48.0)
    } else {
        dyn_range_db
    }
}

/// Lock-free atomic live Dynamic Gain Reduction bus for all 24 bands (audio -> 60 fps GUI).
pub struct SharedBandGr {
    slots: [AtomicU32; NUM_BANDS],
}

impl Default for SharedBandGr {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBandGr {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
        }
    }

    #[inline]
    pub fn store(&self, idx: usize, gr_db: f32) {
        if let Some(slot) = self.slots.get(idx) {
            slot.store(gr_db.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn load(&self, idx: usize) -> f32 {
        self.slots
            .get(idx)
            .map(|s| f32::from_bits(s.load(Ordering::Relaxed)))
            .unwrap_or(0.0)
    }

    pub fn clear_all(&self) {
        let zero = 0.0_f32.to_bits();
        for slot in &self.slots {
            slot.store(zero, Ordering::Relaxed);
        }
    }
}

/// Stack-allocated snapshot of one EQ band's static parameters for zero-alloc dirty detection.
#[derive(Debug, Clone, Copy)]
pub struct BandSnapshot {
    pub used: bool,
    pub enabled: bool,
    pub filter_type: FilterType,
    pub slope: FilterSlope,
    pub placement: ChannelTarget,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
}

impl Default for BandSnapshot {
    fn default() -> Self {
        Self {
            used: false,
            enabled: false,
            filter_type: FilterType::Bell,
            slope: FilterSlope::Db12,
            placement: ChannelTarget::Stereo,
            freq: 1000.0,
            gain: 0.0,
            q: 1.0,
        }
    }
}

impl PartialEq for BandSnapshot {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.used == other.used
            && self.enabled == other.enabled
            && self.filter_type == other.filter_type
            && self.slope == other.slope
            && self.placement == other.placement
            && self.freq.to_bits() == other.freq.to_bits()
            && self.gain.to_bits() == other.gain.to_bits()
            && self.q.to_bits() == other.q.to_bits()
    }
}

impl Eq for BandSnapshot {}

impl BandSnapshot {
    #[inline]
    pub fn from_band(band: &BandParams) -> Self {
        Self::from_band_with(band, &HostRead)
    }

    #[inline]
    pub fn from_band_with(band: &BandParams, r: &impl ParamRead) -> Self {
        Self {
            used: r.read(&band.used),
            enabled: r.read(&band.enabled),
            filter_type: r.read(&band.filter_type).into(),
            slope: r.read(&band.slope).into(),
            placement: r.read(&band.placement).into(),
            freq: r.read(&band.frequency),
            gain: r.read(&band.gain),
            q: r.read(&band.q),
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.used && self.enabled
    }

    #[inline]
    pub fn cascaded_coeffs(&self, sample_rate: f32, natural_phase: bool) -> CascadedCoeffs {
        CascadedCoeffs::calculate(
            self.filter_type,
            self.slope,
            sample_rate,
            self.freq,
            self.gain,
            self.q,
            natural_phase,
        )
    }

    /// Computes Pro-Q style audition filter coefficients isolating this band's frequency range.
    #[inline]
    pub fn solo_coeffs(&self, sample_rate: f32) -> BiquadCoeffs {
        match self.filter_type {
            FilterType::Bell | FilterType::Notch | FilterType::BandPass | FilterType::AllPass => {
                BiquadCoeffs::calculate(
                    FilterType::BandPass,
                    sample_rate,
                    self.freq,
                    0.0,
                    self.q.clamp(0.5, 12.0),
                )
            }
            FilterType::LowCut | FilterType::LowShelf => BiquadCoeffs::calculate(
                FilterType::HighCut,
                sample_rate,
                self.freq,
                0.0,
                self.q.clamp(0.5, 4.0),
            ),
            FilterType::HighCut | FilterType::HighShelf => BiquadCoeffs::calculate(
                FilterType::LowCut,
                sample_rate,
                self.freq,
                0.0,
                self.q.clamp(0.5, 4.0),
            ),
            FilterType::TiltShelf | FilterType::FlatTilt => {
                BiquadCoeffs::calculate(FilterType::BandPass, sample_rate, self.freq, 0.0, 0.707)
            }
        }
    }
}

/// Captures all 24 bands into a fixed-size stack array (zero heap allocations).
#[inline]
pub fn capture_band_snapshots(bands: &[BandParams; NUM_BANDS]) -> [BandSnapshot; NUM_BANDS] {
    capture_band_snapshots_with(bands, &HostRead)
}

#[inline]
pub fn capture_band_snapshots_with(
    bands: &[BandParams; NUM_BANDS],
    r: &impl ParamRead,
) -> [BandSnapshot; NUM_BANDS] {
    std::array::from_fn(|i| BandSnapshot::from_band_with(&bands[i], r))
}

/// Processes one stereo sample pair through a cascaded filter respecting Stereo/L/R/M/S placement.
#[inline(always)]
pub fn process_cascaded_placement(
    filter: &mut CascadedStereoFilter,
    placement: ChannelTarget,
    l: f32,
    r: f32,
) -> (f32, f32) {
    match placement {
        ChannelTarget::Stereo => filter.process_stereo(l, r),
        ChannelTarget::Left => (filter.process_stereo(l, 0.0).0, r),
        ChannelTarget::Right => (l, filter.process_stereo(0.0, r).1),
        ChannelTarget::Mid => {
            let (mid, side) = encode_mid_side(l, r);
            let f_mid = filter.process_stereo(mid, 0.0).0;
            decode_mid_side(f_mid, side)
        }
        ChannelTarget::Side => {
            let (mid, side) = encode_mid_side(l, r);
            let f_side = filter.process_stereo(side, 0.0).0;
            decode_mid_side(mid, f_side)
        }
    }
}

/// Processes one stereo sample pair through a band's Solo Audition monitor respecting placement.
#[inline(always)]
pub fn process_solo_sample(
    filter: &mut StereoBiquad,
    placement: ChannelTarget,
    in_l: f32,
    in_r: f32,
) -> (f32, f32) {
    match placement {
        ChannelTarget::Stereo => filter.process_stereo(in_l, in_r),
        ChannelTarget::Left => (filter.process_stereo(in_l, 0.0).0, 0.0),
        ChannelTarget::Right => (0.0, filter.process_stereo(0.0, in_r).1),
        ChannelTarget::Mid => {
            let (mid, _) = encode_mid_side(in_l, in_r);
            let (f_mid, _) = filter.process_stereo(mid, 0.0);
            decode_mid_side(f_mid, 0.0)
        }
        ChannelTarget::Side => {
            let (_, side) = encode_mid_side(in_l, in_r);
            let (f_side, _) = filter.process_stereo(side, 0.0);
            decode_mid_side(0.0, f_side)
        }
    }
}

/// Processes one stereo sample pair through a band's Spectral Dynamics processor respecting placement.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn process_spectral_placement(
    spec: &mut SpectralBandProcessor,
    placement: ChannelTarget,
    l: f32,
    r: f32,
    sr: f32,
    freq: f32,
    q: f32,
    thresh: f32,
    range: f32,
) -> (f32, f32) {
    match placement {
        ChannelTarget::Stereo => spec.process_stereo(l, r, sr, freq, q, thresh, range),
        ChannelTarget::Left => (spec.process_stereo(l, 0.0, sr, freq, q, thresh, range).0, r),
        ChannelTarget::Right => (l, spec.process_stereo(0.0, r, sr, freq, q, thresh, range).1),
        ChannelTarget::Mid => {
            let (mid, side) = encode_mid_side(l, r);
            let f_mid = spec.process_stereo(mid, 0.0, sr, freq, q, thresh, range).0;
            decode_mid_side(f_mid, side)
        }
        ChannelTarget::Side => {
            let (mid, side) = encode_mid_side(l, r);
            let f_side = spec.process_stereo(side, 0.0, sr, freq, q, thresh, range).0;
            decode_mid_side(mid, f_side)
        }
    }
}

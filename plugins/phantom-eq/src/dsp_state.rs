//! Zero-Allocation DSP State Helpers for Phantom EQ
//! Provides fixed-size band snapshots for dirty-detection, Solo Audition filtering,
//! and lock-free spectrum feeding without heap allocations on the audio thread.

use crate::params::{BandParams, NUM_BANDS};
use phantom_dsp::{
    decode_mid_side, encode_mid_side, BiquadCoeffs, ChannelTarget, FilterType, SharedSpectrum,
    StereoBiquad,
};

/// Stack-allocated snapshot of one EQ band's static parameters for zero-alloc dirty detection.
#[derive(Debug, Clone, Copy)]
pub struct BandSnapshot {
    pub enabled: bool,
    pub filter_type: FilterType,
    pub placement: ChannelTarget,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
}

impl Default for BandSnapshot {
    fn default() -> Self {
        Self {
            enabled: false,
            filter_type: FilterType::Bell,
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
        self.enabled == other.enabled
            && self.filter_type == other.filter_type
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
        Self {
            enabled: band.enabled.value(),
            filter_type: band.filter_type.value().into(),
            placement: band.placement.value().into(),
            freq: band.frequency.value(),
            gain: band.gain.value(),
            q: band.q.value(),
        }
    }

    #[inline]
    pub fn biquad_coeffs(&self, sample_rate: f32) -> BiquadCoeffs {
        BiquadCoeffs::calculate(self.filter_type, sample_rate, self.freq, self.gain, self.q)
    }

    /// Computes audition filter coefficients for Pro-Q style band Solo monitoring:
    /// - Bell / Notch / BandPass -> 0 dB constant-peak BandPass at (freq, Q)
    /// - LowCut / LowShelf       -> Low-pass monitor isolating low region below `freq`
    /// - HighCut / HighShelf     -> High-pass monitor isolating high region above `freq`
    #[inline]
    pub fn solo_coeffs(&self, sample_rate: f32) -> BiquadCoeffs {
        match self.filter_type {
            FilterType::Bell | FilterType::Notch | FilterType::BandPass => BiquadCoeffs::calculate(
                FilterType::BandPass,
                sample_rate,
                self.freq,
                0.0,
                self.q.clamp(0.5, 12.0),
            ),
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
        }
    }
}

/// Captures all bands into a fixed-size stack array (zero heap allocations).
#[inline]
pub fn capture_band_snapshots(bands: &[BandParams; NUM_BANDS]) -> [BandSnapshot; NUM_BANDS] {
    std::array::from_fn(|i| BandSnapshot::from_band(&bands[i]))
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
        ChannelTarget::Left => {
            let (fl, _) = filter.process_stereo(in_l, 0.0);
            (fl, 0.0)
        }
        ChannelTarget::Right => {
            let (_, fr) = filter.process_stereo(0.0, in_r);
            (0.0, fr)
        }
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

/// Feeds stereo output to the lock-free spectrum ring buffer using a fixed stack buffer.
pub fn feed_spectrum(shared_spectrum: &SharedSpectrum, left: &[f32], right: &[f32]) {
    let total = left.len().min(right.len());
    let mut mono = [0.0_f32; 128];
    let mut offset = 0;
    while offset < total {
        let count = (total - offset).min(mono.len());
        for s in 0..count {
            mono[s] = (left[offset + s] + right[offset + s]) * 0.5;
        }
        shared_spectrum.push_samples(&mono[..count]);
        offset += count;
    }
}

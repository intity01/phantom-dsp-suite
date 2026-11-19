//! Pro-Q 4 Per-Band Spectral Dynamics Processor
//! Zero-latency, zero-allocation parallel multi-bin sub-band resonator bank.
//! Selectively compresses or expands only the offending resonant sub-frequencies
//! within an EQ band's bandwidth while leaving neighboring frequencies untouched.

use crate::biquad::{BiquadCoeffs, FilterType, StereoBiquad};

pub const NUM_SPECTRAL_SUBBANDS: usize = 9;
const DEFAULT_ATTACK_MS: f32 = 5.0;
const DEFAULT_RELEASE_MS: f32 = 60.0;
const SOFT_KNEE_DB: f32 = 4.0;
const SPECTRAL_WINDOW_DB: f32 = 12.0;
const LATERAL_INHIBIT_RATIO: f32 = 0.25;

/// Zero-latency, zero-allocation per-band spectral dynamics suppressor/expander.
#[derive(Debug, Clone)]
pub struct SpectralBandProcessor {
    audio_filters: [StereoBiquad; NUM_SPECTRAL_SUBBANDS],
    det_filters: [StereoBiquad; NUM_SPECTRAL_SUBBANDS],
    envelopes: [f32; NUM_SPECTRAL_SUBBANDS],
    subband_offsets_db: [f32; NUM_SPECTRAL_SUBBANDS],
    attack_coeff: f32,
    release_coeff: f32,
    last_gain_offset_db: f32,
    cached_sr_bits: u32,
    cached_freq_bits: u32,
    cached_q_bits: u32,
}

impl Default for SpectralBandProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectralBandProcessor {
    pub fn new() -> Self {
        let mut proc = Self {
            audio_filters: std::array::from_fn(|_| StereoBiquad::new()),
            det_filters: std::array::from_fn(|_| StereoBiquad::new()),
            envelopes: [0.0; NUM_SPECTRAL_SUBBANDS],
            subband_offsets_db: [0.0; NUM_SPECTRAL_SUBBANDS],
            attack_coeff: 0.004,
            release_coeff: 0.0003,
            last_gain_offset_db: 0.0,
            cached_sr_bits: 0,
            cached_freq_bits: 0,
            cached_q_bits: 0,
        };
        proc.update_tuning(48000.0, 1000.0, 1.0);
        proc
    }

    /// Resets all sub-band resonator states and envelope followers.
    pub fn reset(&mut self) {
        for f in &mut self.audio_filters {
            f.reset();
        }
        for f in &mut self.det_filters {
            f.reset();
        }
        self.envelopes = [0.0; NUM_SPECTRAL_SUBBANDS];
        self.subband_offsets_db = [0.0; NUM_SPECTRAL_SUBBANDS];
        self.last_gain_offset_db = 0.0;
    }

    /// Configures custom attack (0.5..250 ms) and release (5..2000 ms) envelope ballistics.
    pub fn set_ballistics(&mut self, sample_rate: f32, attack_ms: f32, release_ms: f32) {
        let sr = sample_rate.max(1000.0);
        let att_samples = (attack_ms.clamp(0.5, 250.0) * 0.001) * sr;
        let rel_samples = (release_ms.clamp(5.0, 2000.0) * 0.001) * sr;
        self.attack_coeff = 1.0 - (-1.0 / att_samples).exp();
        self.release_coeff = 1.0 - (-1.0 / rel_samples).exp();
    }

    /// Returns the dominant dynamic gain offset in dB across all sub-bands (for GUI metering).
    #[inline]
    pub fn last_gain_offset_db(&self) -> f32 {
        self.last_gain_offset_db
    }

    /// Returns the individual dynamic gain offsets in dB for each of the 9 sub-bands.
    #[inline]
    pub fn subband_gain_offsets_db(&self) -> &[f32; NUM_SPECTRAL_SUBBANDS] {
        &self.subband_offsets_db
    }

    #[inline]
    fn update_tuning(&mut self, sample_rate: f32, freq_hz: f32, q: f32) {
        let sr = sample_rate.max(1000.0);
        let freq = freq_hz.clamp(20.0, sr * 0.47);
        let clamped_q = q.clamp(0.1, 20.0);

        let sr_bits = sr.to_bits();
        let freq_bits = freq.to_bits();
        let q_bits = clamped_q.to_bits();

        if sr_bits != self.cached_sr_bits {
            self.set_ballistics(sr, DEFAULT_ATTACK_MS, DEFAULT_RELEASE_MS);
        }

        if sr_bits == self.cached_sr_bits
            && freq_bits == self.cached_freq_bits
            && q_bits == self.cached_q_bits
        {
            return;
        }

        // Distribute 9 narrow sub-resonators logarithmically across the band's target span
        let step_oct = (0.16 / clamped_q.clamp(0.4, 4.0).sqrt()).clamp(0.06, 0.25);
        let audio_q = (1.3 / step_oct).clamp(5.0, 16.0);
        let det_q = (2.0 / step_oct).clamp(8.0, 24.0);
        let center_idx = (NUM_SPECTRAL_SUBBANDS / 2) as f32;

        for k in 0..NUM_SPECTRAL_SUBBANDS {
            let offset_oct = (k as f32 - center_idx) * step_oct;
            let sub_freq = (freq * 2.0_f32.powf(offset_oct)).clamp(20.0, sr * 0.485);
            let audio_coeffs =
                BiquadCoeffs::calculate(FilterType::BandPass, sr, sub_freq, 0.0, audio_q);
            let det_coeffs =
                BiquadCoeffs::calculate(FilterType::BandPass, sr, sub_freq, 0.0, det_q);
            self.audio_filters[k].set_coeffs(audio_coeffs);
            self.det_filters[k].set_coeffs(det_coeffs);
        }

        self.cached_sr_bits = sr_bits;
        self.cached_freq_bits = freq_bits;
        self.cached_q_bits = q_bits;
    }

    /// Processes a stereo sample pair through the multi-bin spectral dynamics bank.
    /// Zero heap allocations and zero framing latency.
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn process_stereo(
        &mut self,
        l: f32,
        r: f32,
        sample_rate: f32,
        freq_hz: f32,
        q: f32,
        threshold_db: f32,
        range_db: f32,
    ) -> (f32, f32) {
        let in_l = if l.is_finite() { l } else { 0.0 };
        let in_r = if r.is_finite() { r } else { 0.0 };

        self.update_tuning(sample_rate, freq_hz, q);

        if range_db.abs() < 0.01 {
            self.last_gain_offset_db = 0.0;
            self.subband_offsets_db = [0.0; NUM_SPECTRAL_SUBBANDS];
            return (in_l, in_r);
        }

        let mut bp_l = [0.0_f32; NUM_SPECTRAL_SUBBANDS];
        let mut bp_r = [0.0_f32; NUM_SPECTRAL_SUBBANDS];

        for k in 0..NUM_SPECTRAL_SUBBANDS {
            let (al, ar) = self.audio_filters[k].process_stereo(in_l, in_r);
            bp_l[k] = al;
            bp_r[k] = ar;

            // Second-stage high-Q detector filter sharpens spectral selectivity
            let (dl, dr) = self.det_filters[k].process_stereo(al, ar);
            let peak = dl.abs().max(dr.abs());
            let coeff = if peak > self.envelopes[k] {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            let mut env = self.envelopes[k] + coeff * (peak - self.envelopes[k]);
            if env < 1e-15 {
                env = 0.0;
            }
            self.envelopes[k] = env;
        }

        let mut out_l = in_l;
        let mut out_r = in_r;
        let mut dominant_offset_db = 0.0_f32;
        let half_knee = SOFT_KNEE_DB * 0.5;

        for k in 0..NUM_SPECTRAL_SUBBANDS {
            let prev_env = if k > 0 { self.envelopes[k - 1] } else { 0.0 };
            let next_env = if k + 1 < NUM_SPECTRAL_SUBBANDS {
                self.envelopes[k + 1]
            } else {
                0.0
            };
            let neighbor_max = prev_env.max(next_env);
            let eff_env = (self.envelopes[k] - LATERAL_INHIBIT_RATIO * neighbor_max).max(0.0);

            if eff_env < 1e-6 {
                self.subband_offsets_db[k] = 0.0;
                continue;
            }

            let level_db = 20.0 * eff_env.log10();
            let over_db = level_db - threshold_db;
            let excess_db = if over_db <= -half_knee {
                0.0
            } else if over_db >= half_knee {
                over_db
            } else {
                let shifted = over_db + half_knee;
                (shifted * shifted) / (2.0 * SOFT_KNEE_DB)
            };

            if excess_db <= 0.0 {
                self.subband_offsets_db[k] = 0.0;
                continue;
            }

            let ratio = (excess_db / SPECTRAL_WINDOW_DB).clamp(0.0, 1.0);
            let offset_db = ratio * range_db;
            self.subband_offsets_db[k] = offset_db;

            if offset_db.abs() > dominant_offset_db.abs() {
                dominant_offset_db = offset_db;
            }

            let g_k = 10.0_f32.powf(offset_db / 20.0) - 1.0;
            out_l += g_k * bp_l[k];
            out_r += g_k * bp_r[k];
        }

        self.last_gain_offset_db = dominant_offset_db;
        (out_l, out_r)
    }
}

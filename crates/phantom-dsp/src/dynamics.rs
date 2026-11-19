//! Dynamic EQ Processing Module (Pro-Q 4 Advanced Dynamic Mode)
//! Real-time envelope tracking with configurable attack/release ballistics,
//! adaptive auto-threshold, external sidechain routing, soft-knee transfer,
//! and live gain-reduction telemetry — 100% zero-allocation on the audio thread.

use crate::biquad::{BiquadCoeffs, FilterType, StereoBiquad};

const AUTO_REF_TAU_SEC: f32 = 1.5;
const AUTO_NOISE_FLOOR_DB: f32 = -60.0;
const AUTO_NOISE_FLOOR_SQ: f32 = 1e-6;
const AUTO_GATE_ENV: f32 = 1e-3; // -60 dBFS envelope gate
const SOFT_KNEE_DB: f32 = 6.0;
const DYNAMIC_WINDOW_DB: f32 = 18.0;

#[derive(Debug, Clone)]
pub struct DynamicDetector {
    envelope_l: f32,
    envelope_r: f32,
    attack_coeff: f32,
    release_coeff: f32,
    auto_ref_sq: f32,
    auto_ref_coeff: f32,
    last_gain_offset_db: f32,
    sidechain_filter: StereoBiquad,
    cached_sr_bits: u32,
    cached_att_bits: u32,
    cached_rel_bits: u32,
    cached_sc_sr_bits: u32,
    cached_sc_freq_bits: u32,
    cached_sc_q_bits: u32,
}

impl Default for DynamicDetector {
    fn default() -> Self {
        let mut det = Self {
            envelope_l: 0.0,
            envelope_r: 0.0,
            attack_coeff: 0.002,
            release_coeff: 0.0002,
            auto_ref_sq: AUTO_NOISE_FLOOR_SQ,
            auto_ref_coeff: 1.0 / (AUTO_REF_TAU_SEC * 48000.0),
            last_gain_offset_db: 0.0,
            sidechain_filter: StereoBiquad::new(),
            cached_sr_bits: 0,
            cached_att_bits: 0,
            cached_rel_bits: 0,
            cached_sc_sr_bits: 0,
            cached_sc_freq_bits: 0,
            cached_sc_q_bits: 0,
        };
        det.update_ballistics(48000.0, 10.0, 100.0);
        det
    }
}

impl DynamicDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets envelope followers, adaptive threshold memory, and sidechain filter state.
    pub fn reset(&mut self) {
        self.envelope_l = 0.0;
        self.envelope_r = 0.0;
        self.auto_ref_sq = AUTO_NOISE_FLOOR_SQ;
        self.last_gain_offset_db = 0.0;
        self.sidechain_filter.reset();
    }

    /// Updates attack (0.5..250 ms) and release (10..2000 ms) ballistics for the given sample rate.
    /// Exponential coefficients are cached and only recomputed when parameters change.
    pub fn update_ballistics(&mut self, sample_rate: f32, attack_ms: f32, release_ms: f32) {
        let sr = sample_rate.max(1000.0);
        let att = attack_ms.clamp(0.5, 250.0);
        let rel = release_ms.clamp(10.0, 2000.0);

        let sr_bits = sr.to_bits();
        let att_bits = att.to_bits();
        let rel_bits = rel.to_bits();
        if sr_bits == self.cached_sr_bits
            && att_bits == self.cached_att_bits
            && rel_bits == self.cached_rel_bits
        {
            return;
        }

        let att_samples = (att * 0.001) * sr;
        let rel_samples = (rel * 0.001) * sr;
        let auto_samples = AUTO_REF_TAU_SEC * sr;

        self.attack_coeff = 1.0 - (-1.0 / att_samples).exp();
        self.release_coeff = 1.0 - (-1.0 / rel_samples).exp();
        self.auto_ref_coeff = 1.0 - (-1.0 / auto_samples).exp();

        self.cached_sr_bits = sr_bits;
        self.cached_att_bits = att_bits;
        self.cached_rel_bits = rel_bits;
    }

    /// Updates sidechain bandpass filter to isolate energy at this band's frequency.
    /// Coefficients are cached and only recalculated when inputs change.
    pub fn update_sidechain(&mut self, sample_rate: f32, freq_hz: f32, q: f32) {
        let sr_bits = sample_rate.to_bits();
        let freq_bits = freq_hz.to_bits();
        let q_bits = q.to_bits();
        if sr_bits == self.cached_sc_sr_bits
            && freq_bits == self.cached_sc_freq_bits
            && q_bits == self.cached_sc_q_bits
        {
            return;
        }

        let coeffs = BiquadCoeffs::calculate(FilterType::BandPass, sample_rate, freq_hz, 0.0, q);
        self.sidechain_filter.set_coeffs(coeffs);
        self.cached_sc_sr_bits = sr_bits;
        self.cached_sc_freq_bits = freq_bits;
        self.cached_sc_q_bits = q_bits;
    }

    /// Returns the most recently computed dynamic gain offset in dB (for GUI gain-reduction rings).
    #[inline]
    pub fn last_gain_offset_db(&self) -> f32 {
        self.last_gain_offset_db
    }

    /// Returns the current slow adaptive RMS auto-threshold level in dBFS.
    #[inline]
    pub fn auto_threshold_db(&self) -> f32 {
        (10.0 * self.auto_ref_sq.max(AUTO_NOISE_FLOOR_SQ).log10()).max(AUTO_NOISE_FLOOR_DB)
    }

    /// Computes dynamic gain offset in dB with full Pro-Q 4 features:
    /// optional external sidechain, adaptive auto-threshold, and 6 dB C1-smooth soft knee.
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn compute_dynamic_gain_ext(
        &mut self,
        sig_l: f32,
        sig_r: f32,
        sc_l: f32,
        sc_r: f32,
        use_ext_sc: bool,
        threshold_db: f32,
        auto_threshold: bool,
        range_db: f32,
    ) -> f32 {
        if range_db.abs() < 0.01 {
            self.last_gain_offset_db = 0.0;
            return 0.0;
        }

        let (raw_l, raw_r) = if use_ext_sc {
            (sc_l, sc_r)
        } else {
            (sig_l, sig_r)
        };
        let in_l = if raw_l.is_finite() { raw_l } else { 0.0 };
        let in_r = if raw_r.is_finite() { raw_r } else { 0.0 };

        // Band-limit detector input around this band's frequency
        let (det_l, det_r) = self.sidechain_filter.process_stereo(in_l, in_r);
        let abs_l = det_l.abs();
        let abs_r = det_r.abs();

        // Stereo peak envelope followers with independent attack / release
        let coeff_l = if abs_l > self.envelope_l {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_l += coeff_l * (abs_l - self.envelope_l);

        let coeff_r = if abs_r > self.envelope_r {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_r += coeff_r * (abs_r - self.envelope_r);

        // Flush denormals
        if self.envelope_l < 1e-15 {
            self.envelope_l = 0.0;
        }
        if self.envelope_r < 1e-15 {
            self.envelope_r = 0.0;
        }

        let peak_env = self.envelope_l.max(self.envelope_r);

        // Update slow ~1.5s adaptive RMS reference whenever signal is above noise floor
        if peak_env > AUTO_GATE_ENV {
            let inst_sq = 0.5 * (det_l * det_l + det_r * det_r);
            self.auto_ref_sq += self.auto_ref_coeff * (inst_sq - self.auto_ref_sq);
            if self.auto_ref_sq < AUTO_NOISE_FLOOR_SQ {
                self.auto_ref_sq = AUTO_NOISE_FLOOR_SQ;
            }
        }

        if peak_env < 1e-6 {
            self.last_gain_offset_db = 0.0;
            return 0.0;
        }

        let level_db = 20.0 * peak_env.log10();
        let effective_thresh = if auto_threshold {
            self.auto_threshold_db()
        } else {
            threshold_db
        };

        // 6 dB C1-continuous soft-knee transfer curve
        let over_db = level_db - effective_thresh;
        let half_knee = SOFT_KNEE_DB * 0.5;
        let excess_db = if over_db <= -half_knee {
            0.0
        } else if over_db >= half_knee {
            over_db
        } else {
            let shifted = over_db + half_knee;
            (shifted * shifted) / (2.0 * SOFT_KNEE_DB)
        };

        let ratio = (excess_db / DYNAMIC_WINDOW_DB).clamp(0.0, 1.0);
        let offset = ratio * range_db;
        self.last_gain_offset_db = offset;
        offset
    }

    /// Backward-compatible internal-sidechain manual-threshold wrapper.
    #[inline(always)]
    pub fn compute_dynamic_gain(
        &mut self,
        in_l: f32,
        in_r: f32,
        threshold_db: f32,
        dynamic_range_db: f32,
    ) -> f32 {
        self.compute_dynamic_gain_ext(
            in_l,
            in_r,
            0.0,
            0.0,
            false,
            threshold_db,
            false,
            dynamic_range_db,
        )
    }
}

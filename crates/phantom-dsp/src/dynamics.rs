//! Dynamic EQ Processing Module (Pro-Q Dynamic Mode)
//! Provides real-time envelope tracking and frequency-dependent dynamic expansion/compression.

use crate::biquad::{BiquadCoeffs, FilterType, StereoBiquad};

#[derive(Debug, Clone)]
pub struct DynamicDetector {
    envelope_l: f32,
    envelope_r: f32,
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_filter: StereoBiquad,
}

impl Default for DynamicDetector {
    fn default() -> Self {
        Self {
            envelope_l: 0.0,
            envelope_r: 0.0,
            attack_coeff: 0.05,
            release_coeff: 0.005,
            sidechain_filter: StereoBiquad::new(),
        }
    }
}

impl DynamicDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.envelope_l = 0.0;
        self.envelope_r = 0.0;
        self.sidechain_filter.reset();
    }

    /// Update ballistics based on sample rate
    pub fn update_ballistics(&mut self, sample_rate: f32, attack_ms: f32, release_ms: f32) {
        let sr = sample_rate.max(1000.0);
        let att_sec = (attack_ms.max(0.1) * 0.001) * sr;
        let rel_sec = (release_ms.max(1.0) * 0.001) * sr;

        self.attack_coeff = 1.0 - (-1.0 / att_sec).exp();
        self.release_coeff = 1.0 - (-1.0 / rel_sec).exp();
    }

    /// Update sidechain bandpass filter to track energy at this band's frequency
    pub fn update_sidechain(&mut self, sample_rate: f32, freq_hz: f32, q: f32) {
        let coeffs = BiquadCoeffs::calculate(FilterType::BandPass, sample_rate, freq_hz, 0.0, q);
        self.sidechain_filter.set_coeffs(coeffs);
    }

    /// Process sample pair and compute dynamic gain offset in dB
    /// - `dynamic_range_db`: negative for dynamic compression (e.g. -6 dB), positive for dynamic expansion (+6 dB)
    /// - `threshold_db`: trigger level (e.g. -18 dB)
    #[inline(always)]
    pub fn compute_dynamic_gain(
        &mut self,
        in_l: f32,
        in_r: f32,
        threshold_db: f32,
        dynamic_range_db: f32,
    ) -> f32 {
        if dynamic_range_db.abs() < 0.01 {
            return 0.0;
        }

        // Filter signal into bandpass frequency region
        let (sc_l, sc_r) = self.sidechain_filter.process_stereo(in_l, in_r);

        let abs_sc_l = sc_l.abs();
        let abs_sc_r = sc_r.abs();

        // Left envelope
        let coeff_l = if abs_sc_l > self.envelope_l {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_l += coeff_l * (abs_sc_l - self.envelope_l);

        // Right envelope
        let coeff_r = if abs_sc_r > self.envelope_r {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope_r += coeff_r * (abs_sc_r - self.envelope_r);

        // Denormal flushing
        if self.envelope_l < 1e-15 {
            self.envelope_l = 0.0;
        }
        if self.envelope_r < 1e-15 {
            self.envelope_r = 0.0;
        }

        let peak_env = self.envelope_l.max(self.envelope_r);
        if peak_env < 1e-6 {
            return 0.0;
        }

        let level_db = 20.0 * peak_env.log10();
        let over_db = level_db - threshold_db;

        if over_db > 0.0 {
            // Soft knee compression / expansion ratio
            let ratio = (over_db / 18.0).min(1.0);
            ratio * dynamic_range_db
        } else {
            0.0
        }
    }
}

//! Automatic Loudness-Matched Gain Compensation (Pro-Q Auto-Gain)
//! Measures K-weighted RMS energy pre-EQ vs post-EQ with a -55 dBFS silence gate
//! and smoothly slews makeup gain within [-18, +18] dB (~300 ms time constant)
//! with zero pumping and zero heap allocations.

use crate::metering::KWeightFilter;

const GATE_THRESHOLD_SQ: f64 = 3.162_277_660_168_379_5e-6; // -55 dBFS power
const MIN_POST_POWER: f64 = 1e-12;
const MAX_MAKEUP_DB: f32 = 18.0;
const FAST_TAU_SEC: f64 = 0.020;
const SLOW_TAU_SEC: f64 = 0.250;
const SLEW_TAU_SEC: f32 = 0.300;
const BYPASS_TAU_SEC: f32 = 0.015;

pub struct AutoGainCompensator {
    pre_k_weight: KWeightFilter,
    post_k_weight: KWeightFilter,
    fast_pre_sq: f64,
    slow_pre_sq: f64,
    slow_post_sq: f64,
    fast_coeff: f64,
    slow_coeff: f64,
    slew_coeff: f32,
    bypass_coeff: f32,
    target_db: f32,
    makeup_db: f32,
    makeup_lin: f32,
    cached_sr_bits: u32,
}

impl Default for AutoGainCompensator {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoGainCompensator {
    pub fn new() -> Self {
        let mut comp = Self {
            pre_k_weight: KWeightFilter::new(48000.0),
            post_k_weight: KWeightFilter::new(48000.0),
            fast_pre_sq: 0.0,
            slow_pre_sq: 0.0,
            slow_post_sq: 0.0,
            fast_coeff: 0.001,
            slow_coeff: 0.0001,
            slew_coeff: 0.00007,
            bypass_coeff: 0.0014,
            target_db: 0.0,
            makeup_db: 0.0,
            makeup_lin: 1.0,
            cached_sr_bits: 0,
        };
        comp.set_sample_rate(48000.0);
        comp
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        let sr_clamped = sr.max(8000.0);
        let sr_bits = sr_clamped.to_bits();
        if sr_bits == self.cached_sr_bits {
            return;
        }
        self.pre_k_weight.set_sample_rate(sr_clamped);
        self.post_k_weight.set_sample_rate(sr_clamped);

        let sr64 = sr_clamped as f64;
        self.fast_coeff = 1.0 - (-1.0 / (FAST_TAU_SEC * sr64)).exp();
        self.slow_coeff = 1.0 - (-1.0 / (SLOW_TAU_SEC * sr64)).exp();
        self.slew_coeff = 1.0 - (-1.0 / (SLEW_TAU_SEC * sr_clamped)).exp();
        self.bypass_coeff = 1.0 - (-1.0 / (BYPASS_TAU_SEC * sr_clamped)).exp();
        self.cached_sr_bits = sr_bits;
    }

    pub fn reset(&mut self) {
        self.pre_k_weight.reset();
        self.post_k_weight.reset();
        self.fast_pre_sq = 0.0;
        self.slow_pre_sq = 0.0;
        self.slow_post_sq = 0.0;
        self.target_db = 0.0;
        self.makeup_db = 0.0;
        self.makeup_lin = 1.0;
    }

    /// Returns the currently applied makeup gain in dB (clamped to [-18, +18] dB).
    #[inline]
    pub fn current_makeup_db(&self) -> f32 {
        self.makeup_db
    }

    /// Compares pre-EQ and post-EQ stereo samples and returns loudness-compensated post-EQ output.
    /// When `enabled` is false, smoothly returns makeup gain to 0 dB with zero clicks.
    #[inline]
    pub fn process_stereo(
        &mut self,
        pre_l: f32,
        pre_r: f32,
        post_l: f32,
        post_r: f32,
        enabled: bool,
    ) -> (f32, f32) {
        let clean_post_l = if post_l.is_finite() { post_l } else { 0.0 };
        let clean_post_r = if post_r.is_finite() { post_r } else { 0.0 };

        if !enabled {
            if self.makeup_db.abs() < 1e-4 {
                self.makeup_db = 0.0;
                self.makeup_lin = 1.0;
                return (clean_post_l, clean_post_r);
            }
            self.makeup_db += self.bypass_coeff * (0.0 - self.makeup_db);
            self.makeup_lin = db_to_lin(self.makeup_db);
            return (
                clean_post_l * self.makeup_lin,
                clean_post_r * self.makeup_lin,
            );
        }

        // Measure K-weighted stereo energy pre-EQ vs post-EQ
        let (pre_kl, pre_kr) = self.pre_k_weight.process_stereo(pre_l, pre_r);
        let (post_kl, post_kr) = self
            .post_k_weight
            .process_stereo(clean_post_l, clean_post_r);

        let pre_sq = 0.5 * (pre_kl * pre_kl + pre_kr * pre_kr);
        let post_sq = 0.5 * (post_kl * post_kl + post_kr * post_kr);

        // Fast silence gate tracker on pre-EQ input
        self.fast_pre_sq += self.fast_coeff * (pre_sq - self.fast_pre_sq);
        if self.fast_pre_sq < 1e-25 {
            self.fast_pre_sq = 0.0;
        }

        // Only update slow loudness integrators when input exceeds -55 dBFS
        if self.fast_pre_sq > GATE_THRESHOLD_SQ {
            self.slow_pre_sq += self.slow_coeff * (pre_sq - self.slow_pre_sq);
            self.slow_post_sq += self.slow_coeff * (post_sq - self.slow_post_sq);

            if self.slow_pre_sq > MIN_POST_POWER {
                let ratio = self.slow_pre_sq / self.slow_post_sq.max(MIN_POST_POWER);
                let raw_db = (10.0 * ratio.log10()) as f32;
                self.target_db = raw_db.clamp(-MAX_MAKEUP_DB, MAX_MAKEUP_DB);
            }
        }

        // Smooth ~300 ms slew rate to prevent pumping
        self.makeup_db += self.slew_coeff * (self.target_db - self.makeup_db);
        self.makeup_db = self.makeup_db.clamp(-MAX_MAKEUP_DB, MAX_MAKEUP_DB);
        self.makeup_lin = db_to_lin(self.makeup_db);

        (
            clean_post_l * self.makeup_lin,
            clean_post_r * self.makeup_lin,
        )
    }
}

#[inline(always)]
fn db_to_lin(db: f32) -> f32 {
    (db * (std::f32::consts::LN_10 * 0.05)).exp()
}

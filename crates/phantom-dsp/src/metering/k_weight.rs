//! ITU-R BS.1770-4 / EBU R128 Two-Stage K-Weighting Filter
//! Exact bilinear-transformed biquad coefficients in `f64` across arbitrary sample rates:
//! - Stage 1: Acoustic head high-shelf (+4 dB @ ~1681.97 Hz)
//! - Stage 2: RLB weighting 2nd-order high-pass (@ ~38.14 Hz)

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
struct KWeightBiquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z: [[f64; 2]; 2], // [channel][z1, z2]
}

impl KWeightBiquad {
    const fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z: [[0.0; 2]; 2],
        }
    }

    fn reset(&mut self) {
        self.z = [[0.0; 2]; 2];
    }

    #[inline(always)]
    fn process_stereo(&mut self, in_l: f64, in_r: f64) -> (f64, f64) {
        (self.process_ch(0, in_l), self.process_ch(1, in_r))
    }

    #[inline(always)]
    fn process_ch(&mut self, ch: usize, x: f64) -> f64 {
        let y = self.b0 * x + self.z[ch][0];
        let z1 = self.b1 * x - self.a1 * y + self.z[ch][1];
        let z2 = self.b2 * x - self.a2 * y;
        self.z[ch][0] = if z1.abs() < 1e-30 { 0.0 } else { z1 };
        self.z[ch][1] = if z2.abs() < 1e-30 { 0.0 } else { z2 };
        y
    }
}

/// Stereo ITU-R BS.1770-4 K-weighting filter used by both `OutputMeter` and `AutoGainCompensator`.
#[derive(Debug, Clone)]
pub(crate) struct KWeightFilter {
    stage1: KWeightBiquad,
    stage2: KWeightBiquad,
    cached_sr_bits: u32,
}

impl KWeightFilter {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut filter = Self {
            stage1: KWeightBiquad::identity(),
            stage2: KWeightBiquad::identity(),
            cached_sr_bits: 0,
        };
        filter.set_sample_rate(sample_rate);
        filter
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8000.0);
        let sr_bits = sr.to_bits();
        if sr_bits == self.cached_sr_bits {
            return;
        }
        let fs = sr as f64;

        // Stage 1: Acoustic head shelf (+3.9998 dB @ 1681.974 Hz, Q = 0.70718)
        let k1 = (PI * 1_681.974_450_955_533_f64 / fs).tan();
        let q1 = 0.707_175_236_955_419_6_f64;
        let vh = 10.0_f64.powf(3.999_843_853_973_347_f64 / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0_1 = 1.0 + k1 / q1 + k1 * k1;
        self.stage1.b0 = (vh + vb * k1 / q1 + k1 * k1) / a0_1;
        self.stage1.b1 = 2.0 * (k1 * k1 - vh) / a0_1;
        self.stage1.b2 = (vh - vb * k1 / q1 + k1 * k1) / a0_1;
        self.stage1.a1 = 2.0 * (k1 * k1 - 1.0) / a0_1;
        self.stage1.a2 = (1.0 - k1 / q1 + k1 * k1) / a0_1;

        // Stage 2: RLB weighting high-pass (38.135 Hz, Q = 0.50033)
        let k2 = (PI * 38.135_470_876_024_44_f64 / fs).tan();
        let q2 = 0.500_327_037_323_877_3_f64;
        let a0_2 = 1.0 + k2 / q2 + k2 * k2;
        self.stage2.b0 = 1.0;
        self.stage2.b1 = -2.0;
        self.stage2.b2 = 1.0;
        self.stage2.a1 = 2.0 * (k2 * k2 - 1.0) / a0_2;
        self.stage2.a2 = (1.0 - k2 / q2 + k2 * k2) / a0_2;

        self.cached_sr_bits = sr_bits;
    }

    pub(crate) fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
    }

    #[inline(always)]
    pub(crate) fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f64, f64) {
        let l = if in_l.is_finite() { in_l as f64 } else { 0.0 };
        let r = if in_r.is_finite() { in_r as f64 } else { 0.0 };
        let (s1_l, s1_r) = self.stage1.process_stereo(l, r);
        self.stage2.process_stereo(s1_l, s1_r)
    }
}

//! Biquad Filter Implementation (Robert Bristow-Johnson Audio EQ Cookbook)
//! Transposed Direct Form II for superior numerical stability and zero denormal CPU penalties.

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Bell,
    LowCut,
    HighCut,
    LowShelf,
    HighShelf,
    Notch,
    BandPass,
}

impl Default for FilterType {
    fn default() -> Self {
        Self::Bell
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

impl BiquadCoeffs {
    /// Calculate RBJ Audio EQ coefficients
    pub fn calculate(
        filter_type: FilterType,
        sample_rate: f32,
        frequency_hz: f32,
        gain_db: f32,
        q: f32,
    ) -> Self {
        let sr = sample_rate.max(1.0);
        // Clamp frequency between 10 Hz and 0.49 * sample_rate (Nyquist limit)
        let freq = frequency_hz.clamp(10.0, sr * 0.495);
        let q = q.clamp(0.05, 50.0);

        let omega = 2.0 * PI * (freq / sr);
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);
        let a_amp = 10.0f32.powf(gain_db / 40.0);

        let (b0_un, b1_un, b2_un, a0_un, a1_un, a2_un) = match filter_type {
            FilterType::Bell => {
                let b0 = 1.0 + alpha * a_amp;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0 - alpha * a_amp;
                let a0 = 1.0 + alpha / a_amp;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha / a_amp;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowCut => {
                let b0 = (1.0 + cos_omega) * 0.5;
                let b1 = -(1.0 + cos_omega);
                let b2 = (1.0 + cos_omega) * 0.5;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighCut => {
                let b0 = (1.0 - cos_omega) * 0.5;
                let b1 = 1.0 - cos_omega;
                let b2 = (1.0 - cos_omega) * 0.5;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowShelf => {
                let sqrt_a = a_amp.sqrt();
                let b0 = a_amp * ((a_amp + 1.0) - (a_amp - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = 2.0 * a_amp * ((a_amp - 1.0) - (a_amp + 1.0) * cos_omega);
                let b2 = a_amp * ((a_amp + 1.0) - (a_amp - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a_amp + 1.0) + (a_amp - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = -2.0 * ((a_amp - 1.0) + (a_amp + 1.0) * cos_omega);
                let a2 = (a_amp + 1.0) + (a_amp - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighShelf => {
                let sqrt_a = a_amp.sqrt();
                let b0 = a_amp * ((a_amp + 1.0) + (a_amp - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = -2.0 * a_amp * ((a_amp - 1.0) + (a_amp + 1.0) * cos_omega);
                let b2 = a_amp * ((a_amp + 1.0) + (a_amp - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a_amp + 1.0) - (a_amp - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = 2.0 * ((a_amp - 1.0) - (a_amp + 1.0) * cos_omega);
                let a2 = (a_amp + 1.0) - (a_amp - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        if a0_un.abs() < 1e-15 {
            return Self::default();
        }

        let inv_a0 = 1.0 / a0_un;
        Self {
            b0: b0_un * inv_a0,
            b1: b1_un * inv_a0,
            b2: b2_un * inv_a0,
            a1: a1_un * inv_a0,
            a2: a2_un * inv_a0,
        }
    }

    /// Calculate the magnitude response in decibels (dB) at a given frequency
    pub fn magnitude_db_at(&self, freq: f32, sample_rate: f32) -> f32 {
        let sr = sample_rate.max(1.0);
        let omega = 2.0 * PI * (freq.clamp(1.0, sr * 0.499) / sr);
        let cos1 = omega.cos();
        let sin1 = omega.sin();
        let cos2 = (2.0 * omega).cos();
        let sin2 = (2.0 * omega).sin();

        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -self.b1 * sin1 - self.b2 * sin2;

        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -self.a1 * sin1 - self.a2 * sin2;

        let num_sq = num_re * num_re + num_im * num_im;
        let den_sq = den_re * den_re + den_im * den_im;

        if den_sq < 1e-15 || num_sq < 1e-15 {
            return -100.0;
        }

        10.0 * (num_sq / den_sq).log10()
    }
}

/// Stereo Biquad processor with Transposed Direct Form II
#[derive(Debug, Clone, Default)]
pub struct StereoBiquad {
    pub coeffs: BiquadCoeffs,
    // Left channel delay registers
    z1_l: f32,
    z2_l: f32,
    // Right channel delay registers
    z1_r: f32,
    z2_r: f32,
}

impl StereoBiquad {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.z1_l = 0.0;
        self.z2_l = 0.0;
        self.z1_r = 0.0;
        self.z2_r = 0.0;
    }

    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    /// Process a single stereo sample pair (Transposed Direct Form II)
    #[inline(always)]
    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        // Left Channel
        let out_l = self.coeffs.b0 * in_l + self.z1_l;
        self.z1_l = self.coeffs.b1 * in_l - self.coeffs.a1 * out_l + self.z2_l;
        self.z2_l = self.coeffs.b2 * in_l - self.coeffs.a2 * out_l;

        // Right Channel
        let out_r = self.coeffs.b0 * in_r + self.z1_r;
        self.z1_r = self.coeffs.b1 * in_r - self.coeffs.a1 * out_r + self.z2_r;
        self.z2_r = self.coeffs.b2 * in_r - self.coeffs.a2 * out_r;

        // Anti-denormal protection (zero allocation, negligible CPU overhead)
        if self.z1_l.abs() < 1e-25 {
            self.z1_l = 0.0;
        }
        if self.z2_l.abs() < 1e-25 {
            self.z2_l = 0.0;
        }
        if self.z1_r.abs() < 1e-25 {
            self.z1_r = 0.0;
        }
        if self.z2_r.abs() < 1e-25 {
            self.z2_r = 0.0;
        }

        (out_l, out_r)
    }
}

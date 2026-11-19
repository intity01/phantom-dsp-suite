//! Biquad Filter Implementation (RBJ Audio EQ Cookbook + First-Order + Vicanek Natural Phase)
//! Transposed Direct Form II with f64 state registers for mastering-grade stability and precision.

mod natural;

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterType {
    #[default]
    Bell,
    LowCut,
    HighCut,
    LowShelf,
    HighShelf,
    Notch,
    BandPass,
    TiltShelf,
    FlatTilt,
    AllPass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    #[inline]
    pub(crate) fn from_raw(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        if !a0.is_finite() || a0.abs() < 1e-15 {
            return Self::default();
        }
        let inv = 1.0 / a0;
        let out = Self {
            b0: (b0 * inv) as f32,
            b1: (b1 * inv) as f32,
            b2: (b2 * inv) as f32,
            a1: (a1 * inv) as f32,
            a2: (a2 * inv) as f32,
        };
        if out.b0.is_finite()
            && out.b1.is_finite()
            && out.b2.is_finite()
            && out.a1.is_finite()
            && out.a2.is_finite()
        {
            out
        } else {
            Self::default()
        }
    }

    /// Scale the numerator gain linearly (used for pivot normalization and passband alignment).
    #[inline]
    pub(crate) fn scale_gain(self, gain: f32) -> Self {
        Self {
            b0: self.b0 * gain,
            b1: self.b1 * gain,
            b2: self.b2 * gain,
            ..self
        }
    }

    /// Calculate standard 2nd-order RBJ / bilinear filter coefficients for all 10 Pro-Q shapes.
    pub fn calculate(
        filter_type: FilterType,
        sample_rate: f32,
        frequency_hz: f32,
        gain_db: f32,
        q: f32,
    ) -> Self {
        let sr = (sample_rate as f64).max(1.0);
        let freq = (frequency_hz as f64).clamp(2.0, sr * 0.495);
        let q_val = (q as f64).clamp(0.05, 50.0);
        let omega = 2.0 * PI * (freq / sr);
        let (sin_w, cos_w) = omega.sin_cos();
        let alpha = sin_w / (2.0 * q_val);
        let a_amp = 10.0_f64.powf(gain_db as f64 / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::Bell => (
                1.0 + alpha * a_amp,
                -2.0 * cos_w,
                1.0 - alpha * a_amp,
                1.0 + alpha / a_amp,
                -2.0 * cos_w,
                1.0 - alpha / a_amp,
            ),
            FilterType::LowCut => (
                (1.0 + cos_w) * 0.5,
                -(1.0 + cos_w),
                (1.0 + cos_w) * 0.5,
                1.0 + alpha,
                -2.0 * cos_w,
                1.0 - alpha,
            ),
            FilterType::HighCut => (
                (1.0 - cos_w) * 0.5,
                1.0 - cos_w,
                (1.0 - cos_w) * 0.5,
                1.0 + alpha,
                -2.0 * cos_w,
                1.0 - alpha,
            ),
            FilterType::LowShelf => rbj_shelf(a_amp, cos_w, alpha, -1.0, a_amp),
            FilterType::HighShelf => rbj_shelf(a_amp, cos_w, alpha, 1.0, a_amp),
            FilterType::Notch => (
                1.0,
                -2.0 * cos_w,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w,
                1.0 - alpha,
            ),
            FilterType::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha),
            FilterType::TiltShelf => rbj_shelf(a_amp, cos_w, alpha, 1.0, 1.0),
            FilterType::FlatTilt => rbj_shelf(a_amp, cos_w, sin_w * 2.0, 1.0, 1.0),
            FilterType::AllPass => (
                1.0 - alpha,
                -2.0 * cos_w,
                1.0 + alpha,
                1.0 + alpha,
                -2.0 * cos_w,
                1.0 - alpha,
            ),
        };
        Self::from_raw(b0, b1, b2, a0, a1, a2)
    }

    /// Calculate exact first-order (6 dB/oct) bilinear filter coefficients (`b2 = a2 = 0`).
    pub fn calculate_first_order(
        filter_type: FilterType,
        sample_rate: f32,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Self {
        let sr = (sample_rate as f64).max(1.0);
        let freq = (frequency_hz as f64).clamp(2.0, sr * 0.495);
        let k = (PI * freq / sr).tan();
        let sg = 10.0_f64.powf(gain_db as f64 / 40.0);
        match filter_type {
            FilterType::LowCut => Self::from_raw(1.0, -1.0, 0.0, 1.0 + k, k - 1.0, 0.0),
            FilterType::HighCut => Self::from_raw(k, k, 0.0, 1.0 + k, k - 1.0, 0.0),
            FilterType::LowShelf => Self::from_raw(
                1.0 + k * sg,
                k * sg - 1.0,
                0.0,
                1.0 + k / sg,
                k / sg - 1.0,
                0.0,
            ),
            FilterType::HighShelf => Self::from_raw(
                sg * (sg + k),
                sg * (k - sg),
                0.0,
                1.0 + k * sg,
                k * sg - 1.0,
                0.0,
            ),
            FilterType::TiltShelf | FilterType::FlatTilt => {
                Self::from_raw(sg + k, k - sg, 0.0, 1.0 + k * sg, k * sg - 1.0, 0.0)
            }
            FilterType::AllPass => Self::from_raw(k - 1.0, k + 1.0, 0.0, k + 1.0, k - 1.0, 0.0),
            FilterType::Bell | FilterType::Notch | FilterType::BandPass => Self::calculate(
                filter_type,
                sample_rate,
                frequency_hz,
                gain_db,
                std::f32::consts::FRAC_1_SQRT_2,
            ),
        }
    }

    /// Calculate exact magnitude response in dB at a given frequency (f64 precision).
    pub fn magnitude_db_at(&self, freq: f32, sample_rate: f32) -> f32 {
        let sr = (sample_rate as f64).max(1.0);
        let omega = 2.0 * PI * ((freq as f64).clamp(1.0, sr * 0.499) / sr);
        let (sin1, cos1) = omega.sin_cos();
        let (sin2, cos2) = (2.0 * omega).sin_cos();
        let (b0, b1, b2) = (self.b0 as f64, self.b1 as f64, self.b2 as f64);
        let (a1, a2) = (self.a1 as f64, self.a2 as f64);

        let num_re = b0 + b1 * cos1 + b2 * cos2;
        let num_im = -b1 * sin1 - b2 * sin2;
        let den_re = 1.0 + a1 * cos1 + a2 * cos2;
        let den_im = -a1 * sin1 - a2 * sin2;

        let num_sq = num_re * num_re + num_im * num_im;
        let den_sq = den_re * den_re + den_im * den_im;
        if den_sq < 1e-24 || num_sq < 1e-24 {
            return -120.0;
        }
        (10.0 * (num_sq / den_sq).log10()) as f32
    }
}

/// Stereo Biquad processor using Transposed Direct Form II with f64 internal precision.
#[derive(Debug, Clone)]
pub struct StereoBiquad {
    pub coeffs: BiquadCoeffs,
    c: [f64; 5], // [b0, b1, b2, a1, a2] cached in f64
    z: [f64; 4], // [z1_l, z2_l, z1_r, z2_r]
}

impl Default for StereoBiquad {
    fn default() -> Self {
        Self::new()
    }
}

impl StereoBiquad {
    pub fn new() -> Self {
        Self {
            coeffs: BiquadCoeffs::default(),
            c: [1.0, 0.0, 0.0, 0.0, 0.0],
            z: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.z = [0.0; 4];
    }

    #[inline]
    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
        self.c = [
            coeffs.b0 as f64,
            coeffs.b1 as f64,
            coeffs.b2 as f64,
            coeffs.a1 as f64,
            coeffs.a2 as f64,
        ];
    }

    #[inline(always)]
    pub fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let [b0, b1, b2, a1, a2] = self.c;
        let xl = in_l as f64;
        let out_l = b0 * xl + self.z[0];
        self.z[0] = b1 * xl - a1 * out_l + self.z[1];
        self.z[1] = b2 * xl - a2 * out_l + 1e-30;

        let xr = in_r as f64;
        let out_r = b0 * xr + self.z[2];
        self.z[2] = b1 * xr - a1 * out_r + self.z[3];
        self.z[3] = b2 * xr - a2 * out_r + 1e-30;

        (out_l as f32, out_r as f32)
    }
}

#[inline]
fn rbj_shelf(
    a_amp: f64,
    cos_w: f64,
    alpha: f64,
    sign: f64,
    num_scale: f64,
) -> (f64, f64, f64, f64, f64, f64) {
    let s = 2.0 * a_amp.sqrt() * alpha;
    let (ap, am) = (a_amp + 1.0, a_amp - 1.0);
    (
        num_scale * (ap + sign * am * cos_w + s),
        -2.0 * sign * num_scale * (am + sign * ap * cos_w),
        num_scale * (ap + sign * am * cos_w - s),
        ap - sign * am * cos_w + s,
        2.0 * sign * (am - sign * ap * cos_w),
        ap - sign * am * cos_w - s,
    )
}

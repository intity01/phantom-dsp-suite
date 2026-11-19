//! Cramp-Free Analog-Matched Natural Phase Biquads (Vicanek Matched-Z Design)
//! Matches continuous-time analog magnitude & natural phase near Nyquist with zero pre-ringing.

use super::{BiquadCoeffs, FilterType};
use std::f64::consts::PI;

impl BiquadCoeffs {
    /// Calculate cramp-free Analog-Matched Natural Phase biquad coefficients.
    /// Matches analog response at DC, corner frequency, and Nyquist with zero pre-ringing.
    pub fn calculate_natural(
        filter_type: FilterType,
        sample_rate: f32,
        frequency_hz: f32,
        gain_db: f32,
        q: f32,
    ) -> Self {
        let sr = (sample_rate as f64).max(1.0);
        let freq = (frequency_hz as f64).clamp(2.0, sr * 0.49);
        let q_val = (q as f64).clamp(0.05, 50.0);
        let w0 = 2.0 * PI * (freq / sr);
        if w0 < 0.08 || matches!(filter_type, FilterType::AllPass | FilterType::FlatTilt) {
            return Self::calculate(filter_type, sample_rate, frequency_hz, gain_db, q);
        }

        let g_abs = 10.0_f64.powf(gain_db.abs() as f64 / 20.0);
        let a_abs = g_abs.sqrt();
        let x = PI / w0;
        let invert = |c: Self| -> Self {
            if c.b0.abs() < 1e-6 || (c.b2 / c.b0).abs() >= 0.9999 {
                Self::calculate(filter_type, sample_rate, frequency_hz, gain_db, q)
            } else {
                Self::from_raw(
                    1.0,
                    c.a1 as f64,
                    c.a2 as f64,
                    c.b0 as f64,
                    c.b1 as f64,
                    c.b2 as f64,
                )
            }
        };

        match filter_type {
            FilterType::Bell => {
                let (a1, a2) = matched_poles(w0, q_val * a_abs);
                let u = x - 1.0 / x;
                let gpi_sq =
                    (u * u + g_abs / (q_val * q_val)) / (u * u + 1.0 / (g_abs * q_val * q_val));
                let boost = vicanek_solve(a1, a2, w0, 1.0, g_abs * g_abs, gpi_sq);
                if gain_db < 0.0 {
                    invert(boost)
                } else {
                    boost
                }
            }
            FilterType::HighShelf | FilterType::TiltShelf => {
                // Match cut prototype (poles at w0/sqrt(A) < pi always), invert for boost
                let (a1, a2) = matched_poles(w0 / a_abs.sqrt(), q_val);
                let im2 = (a_abs.sqrt() * x / q_val).powi(2);
                let gpi_boost = a_abs * a_abs * ((1.0 - a_abs * x * x).powi(2) + im2)
                    / ((a_abs - x * x).powi(2) + im2);
                let cut = vicanek_solve(a1, a2, w0, 1.0, 1.0 / g_abs, 1.0 / gpi_boost);
                let shelf = if gain_db >= 0.0 { invert(cut) } else { cut };
                if filter_type == FilterType::TiltShelf {
                    shelf.scale_gain(10.0_f32.powf(-gain_db / 40.0))
                } else {
                    shelf
                }
            }
            FilterType::LowShelf => {
                let (a1, a2) = matched_poles(w0 / a_abs.sqrt(), q_val);
                let im2 = (a_abs.sqrt() * x / q_val).powi(2);
                let gpi_sq = a_abs * a_abs * ((a_abs - x * x).powi(2) + im2)
                    / ((1.0 - a_abs * x * x).powi(2) + im2);
                let boost = vicanek_solve(a1, a2, w0, g_abs * g_abs, g_abs, gpi_sq);
                if gain_db < 0.0 {
                    invert(boost)
                } else {
                    boost
                }
            }
            FilterType::HighCut => {
                let (a1, a2) = matched_poles(w0, q_val);
                let gpi_sq = 1.0 / ((1.0 - x * x).powi(2) + (x / q_val).powi(2));
                vicanek_solve(a1, a2, w0, 1.0, q_val * q_val, gpi_sq)
            }
            FilterType::LowCut => {
                let (a1, a2) = matched_poles(w0, q_val);
                let (_, phi1, _, aw0) = vicanek_basis(a1, a2, w0);
                let b0 = q_val * aw0.max(0.0).sqrt() / (4.0 * phi1.max(1e-12));
                Self::from_raw(b0, -2.0 * b0, b0, 1.0, a1, a2)
            }
            FilterType::BandPass => {
                let (a1, a2) = matched_poles(w0, q_val);
                let (_, _, _, aw0) = vicanek_basis(a1, a2, w0);
                let b0 = aw0.max(0.0).sqrt() / (2.0 * w0.sin().abs().max(1e-6));
                Self::from_raw(b0, 0.0, -b0, 1.0, a1, a2)
            }
            FilterType::Notch => {
                let (a1, a2) = matched_poles(w0, q_val);
                let cos_w = w0.cos();
                let scale = (1.0 + a1 + a2) / (2.0 * (1.0 - cos_w).max(1e-12));
                Self::from_raw(scale, -2.0 * cos_w * scale, scale, 1.0, a1, a2)
            }
            FilterType::AllPass | FilterType::FlatTilt => unreachable!(),
        }
    }
}

#[inline]
fn matched_poles(w0: f64, q: f64) -> (f64, f64) {
    let inv_2q = 0.5 / q;
    let r = (-w0 * inv_2q).exp();
    let a2 = r * r;
    let a1 = if inv_2q <= 1.0 {
        -2.0 * r * (w0 * (1.0 - inv_2q * inv_2q).sqrt()).cos()
    } else {
        -2.0 * r * (w0 * (inv_2q * inv_2q - 1.0).sqrt()).cosh()
    };
    (a1, a2)
}

#[inline]
fn vicanek_basis(a1: f64, a2: f64, w0: f64) -> (f64, f64, f64, f64) {
    let (a0_v, a1_v, a2_v) = ((1.0 + a1 + a2).powi(2), (1.0 - a1 + a2).powi(2), -4.0 * a2);
    let s = (0.5 * w0).sin();
    let phi1 = s * s;
    let phi0 = 1.0 - phi1;
    let phi2 = 4.0 * phi0 * phi1;
    let aw0 = a0_v * phi0 + a1_v * phi1 + a2_v * phi2;
    (phi0, phi1, phi2, aw0)
}

#[inline]
fn vicanek_solve(a1: f64, a2: f64, w0: f64, g0_sq: f64, gw0_sq: f64, gpi_sq: f64) -> BiquadCoeffs {
    let (a0_v, a1_v) = ((1.0 + a1 + a2).powi(2), (1.0 - a1 + a2).powi(2));
    let (phi0, phi1, phi2, aw0) = vicanek_basis(a1, a2, w0);
    let b0_v = a0_v * g0_sq;
    let b1_v = a1_v * gpi_sq;
    let b2_v = (aw0 * gw0_sq - b0_v * phi0 - b1_v * phi1) / phi2.max(1e-12);
    let (s0, s1) = (b0_v.max(0.0).sqrt(), b1_v.max(0.0).sqrt());
    let w = 0.5 * (s0 + s1);
    let b1 = 0.5 * (s0 - s1);
    let disc = (w * w + b2_v).max(0.0).sqrt();
    BiquadCoeffs::from_raw(0.5 * (w + disc), b1, 0.5 * (w - disc), 1.0, a1, a2)
}

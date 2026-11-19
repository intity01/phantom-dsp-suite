//! Cascaded Stage Builders for Shelf, Flat-Top Bell, Flat Tilt, AllPass, Notch & BandPass.

use super::{butterworth_q, single_biquad, CascadedCoeffs, FilterSlope};
use crate::biquad::{BiquadCoeffs, FilterType};
use std::f32::consts::FRAC_1_SQRT_2;
use std::f64::consts::PI;

#[allow(clippy::too_many_arguments)]
pub(super) fn calc_shelf(
    out: &mut CascadedCoeffs,
    ftype: FilterType,
    slope: FilterSlope,
    sr: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    natural: bool,
) {
    let q_scale = (q / FRAC_1_SQRT_2).clamp(0.15, 20.0);
    match slope {
        FilterSlope::Db6 => {
            out.num_stages = 1;
            out.stages[0] = BiquadCoeffs::calculate_first_order(ftype, sr, freq, gain_db);
        }
        FilterSlope::Db12 => {
            out.num_stages = 1;
            out.stages[0] = single_biquad(ftype, sr, freq, gain_db, q, natural);
        }
        FilterSlope::Db18 => {
            out.num_stages = 2;
            out.stages[0] = BiquadCoeffs::calculate_first_order(ftype, sr, freq, gain_db / 3.0);
            out.stages[1] = single_biquad(ftype, sr, freq, gain_db * (2.0 / 3.0), q_scale, natural);
        }
        _ => {
            let m = match slope {
                FilterSlope::Db24 => 2,
                FilterSlope::Db36 => 3,
                FilterSlope::Db48 => 4,
                _ => 8,
            };
            let order = m * 2;
            let stage_gain = gain_db / m as f32;
            out.num_stages = m;
            for (k, slot) in out.stages[..m].iter_mut().enumerate() {
                let qk = butterworth_q(order, k);
                let q_eff = if k == 0 {
                    (qk * q_scale).clamp(0.15, 25.0)
                } else {
                    qk
                };
                *slot = single_biquad(ftype, sr, freq, stage_gain, q_eff, natural);
            }
        }
    }
}

pub(super) fn calc_bell(
    out: &mut CascadedCoeffs,
    slope: FilterSlope,
    sr: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    natural: bool,
) {
    let m = match slope {
        FilterSlope::Db6 | FilterSlope::Db12 => {
            let eff_q = if slope == FilterSlope::Db6 {
                (q * FRAC_1_SQRT_2).max(0.05)
            } else {
                q
            };
            out.num_stages = 1;
            out.stages[0] = single_biquad(FilterType::Bell, sr, freq, gain_db, eff_q, natural);
            return;
        }
        FilterSlope::Db18 | FilterSlope::Db24 => 2,
        FilterSlope::Db36 => 3,
        FilterSlope::Db48 => 4,
        FilterSlope::Db96 => 6,
        FilterSlope::Brickwall => 8,
    };
    if gain_db.abs() < 1e-5 {
        out.num_stages = 1;
        out.stages[0] = BiquadCoeffs::default();
        return;
    }

    // Exact Holters-Zolzer Butterworth flat-top peaking cascade of order 2*M
    let sr64 = (sr as f64).max(1.0);
    let f64_hz = (freq as f64).clamp(2.0, sr64 * 0.49);
    let q64 = (q as f64).clamp(0.1, 30.0);
    let k_warp = (PI * f64_hz / sr64).tan();
    let k2 = k_warp * k_warp;
    let g_pow = 10.0_f64.powf(gain_db as f64 / (40.0 * m as f64));
    let (q_num, q_den) = (q64 / g_pow, q64 * g_pow);

    let mut idx = 0;
    for k in 0..(m / 2) {
        let phi = PI * (2 * k + 1) as f64 / (2 * m) as f64;
        let (sr_lp, si_lp) = (-phi.sin(), phi.cos());
        let (rn1, rn2) = quad_roots(sr_lp / (2.0 * q_num), si_lp / (2.0 * q_num));
        let (mut rd1, mut rd2) = quad_roots(sr_lp / (2.0 * q_den), si_lp / (2.0 * q_den));
        if (norm_sq(rn1) > 1.0) != (norm_sq(rd1) > 1.0) {
            std::mem::swap(&mut rd1, &mut rd2);
        }
        for (rn, rd) in [(rn1, rd1), (rn2, rd2)] {
            out.stages[idx] = bilinear_s_biquad(
                -2.0 * rn.0,
                norm_sq(rn),
                -2.0 * rd.0,
                norm_sq(rd),
                k_warp,
                k2,
            );
            idx += 1;
        }
    }
    if m % 2 == 1 {
        out.stages[idx] = bilinear_s_biquad(1.0 / q_num, 1.0, 1.0 / q_den, 1.0, k_warp, k2);
        idx += 1;
    }
    out.num_stages = idx;
}

/// 8 cascaded first-order shelves packed into 4 biquads for dead-flat linear dB/oct tilt.
pub(super) fn calc_flat_tilt(out: &mut CascadedCoeffs, sr: f32, freq: f32, gain_db: f32) {
    if gain_db.abs() < 1e-5 {
        out.num_stages = 1;
        out.stages[0] = BiquadCoeffs::default();
        return;
    }
    const SHELFS: [f32; 8] = [25.0, 63.0, 158.0, 398.0, 1000.0, 2512.0, 6310.0, 15849.0];
    let stage_gain = gain_db / 8.0;
    out.num_stages = 4;
    for pair in 0..4 {
        let s0 = BiquadCoeffs::calculate_first_order(
            FilterType::TiltShelf,
            sr,
            SHELFS[pair * 2],
            stage_gain,
        );
        let s1 = BiquadCoeffs::calculate_first_order(
            FilterType::TiltShelf,
            sr,
            SHELFS[pair * 2 + 1],
            stage_gain,
        );
        out.stages[pair] = BiquadCoeffs {
            b0: s0.b0 * s1.b0,
            b1: s0.b0 * s1.b1 + s0.b1 * s1.b0,
            b2: s0.b1 * s1.b1,
            a1: s0.a1 + s1.a1,
            a2: s0.a1 * s1.a1,
        };
    }
    let pivot_db = out.magnitude_db_at(freq, sr);
    out.stages[0] = out.stages[0].scale_gain(10.0_f32.powf(-pivot_db / 20.0));
}

pub(super) fn calc_allpass(
    out: &mut CascadedCoeffs,
    slope: FilterSlope,
    sr: f32,
    freq: f32,
    q: f32,
) {
    match slope {
        FilterSlope::Db6 => {
            out.num_stages = 1;
            out.stages[0] = BiquadCoeffs::calculate_first_order(FilterType::AllPass, sr, freq, 0.0);
        }
        FilterSlope::Db18 => {
            out.num_stages = 2;
            out.stages[0] = BiquadCoeffs::calculate_first_order(FilterType::AllPass, sr, freq, 0.0);
            out.stages[1] = BiquadCoeffs::calculate(FilterType::AllPass, sr, freq, 0.0, q);
        }
        _ => {
            let m = match slope {
                FilterSlope::Db12 => 1,
                FilterSlope::Db24 => 2,
                FilterSlope::Db36 => 3,
                FilterSlope::Db48 => 4,
                _ => 8,
            };
            let stage = BiquadCoeffs::calculate(FilterType::AllPass, sr, freq, 0.0, q);
            out.num_stages = m;
            out.stages[..m].fill(stage);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn calc_repeated(
    out: &mut CascadedCoeffs,
    ftype: FilterType,
    slope: FilterSlope,
    sr: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    natural: bool,
) {
    let m = match slope {
        FilterSlope::Db6 | FilterSlope::Db12 => 1,
        FilterSlope::Db18 | FilterSlope::Db24 => 2,
        FilterSlope::Db36 => 3,
        FilterSlope::Db48 => 4,
        FilterSlope::Db96 => 6,
        FilterSlope::Brickwall => 8,
    };
    let stage = single_biquad(ftype, sr, freq, gain_db, q, natural);
    out.num_stages = m;
    out.stages[..m].fill(stage);
}

#[inline]
fn norm_sq(z: (f64, f64)) -> f64 {
    z.0 * z.0 + z.1 * z.1
}

/// Roots of r^2 - 2*(hr + j*hi)*r + 1 = 0 -> (hr + j*hi) ± sqrt((hr + j*hi)^2 - 1).
#[inline]
fn quad_roots(hr: f64, hi: f64) -> ((f64, f64), (f64, f64)) {
    let (dr, di) = (hr * hr - hi * hi - 1.0, 2.0 * hr * hi);
    let mag = dr.hypot(di);
    let sr = (0.5 * (mag + dr)).max(0.0).sqrt();
    let si = (0.5 * (mag - dr)).max(0.0).sqrt().copysign(di);
    ((hr + sr, hi + si), (hr - sr, hi - si))
}

#[inline]
fn bilinear_s_biquad(c1: f64, c0: f64, d1: f64, d0: f64, k: f64, k2: f64) -> BiquadCoeffs {
    BiquadCoeffs::from_raw(
        1.0 + c1 * k + c0 * k2,
        2.0 * (c0 * k2 - 1.0),
        1.0 - c1 * k + c0 * k2,
        1.0 + d1 * k + d0 * k2,
        2.0 * (d0 * k2 - 1.0),
        1.0 - d1 * k + d0 * k2,
    )
}

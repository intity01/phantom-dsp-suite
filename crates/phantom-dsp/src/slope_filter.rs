//! Variable-Slope Cascaded Filter Engine (6 dB/oct to 96 dB/oct + Brickwall)
//! Zero-allocation up-to-16th-order stereo cascades for all 10 Pro-Q 4 filter shapes.

mod shapes;

use crate::biquad::{BiquadCoeffs, FilterType, StereoBiquad};
use shapes::{calc_allpass, calc_bell, calc_flat_tilt, calc_repeated, calc_shelf};
use std::f32::consts::FRAC_1_SQRT_2;
use std::f64::consts::PI;

pub const MAX_CASCADE_STAGES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterSlope {
    Db6,
    #[default]
    Db12,
    Db18,
    Db24,
    Db36,
    Db48,
    Db96,
    Brickwall,
}

#[derive(Debug, Clone, Copy)]
pub struct CascadedCoeffs {
    pub stages: [BiquadCoeffs; MAX_CASCADE_STAGES],
    pub num_stages: usize,
}

impl Default for CascadedCoeffs {
    fn default() -> Self {
        Self {
            stages: [BiquadCoeffs::default(); MAX_CASCADE_STAGES],
            num_stages: 1,
        }
    }
}

impl CascadedCoeffs {
    /// Compute cascaded biquad stages for any filter shape, slope, and Natural Phase setting.
    pub fn calculate(
        ftype: FilterType,
        slope: FilterSlope,
        sample_rate: f32,
        freq: f32,
        gain_db: f32,
        q: f32,
        natural_phase: bool,
    ) -> Self {
        let mut out = Self::default();
        match ftype {
            FilterType::LowCut | FilterType::HighCut => {
                calc_cut(&mut out, ftype, slope, sample_rate, freq, q, natural_phase);
            }
            FilterType::LowShelf | FilterType::HighShelf | FilterType::TiltShelf => {
                calc_shelf(
                    &mut out,
                    ftype,
                    slope,
                    sample_rate,
                    freq,
                    gain_db,
                    q,
                    natural_phase,
                );
            }
            FilterType::Bell => {
                calc_bell(
                    &mut out,
                    slope,
                    sample_rate,
                    freq,
                    gain_db,
                    q,
                    natural_phase,
                );
            }
            FilterType::FlatTilt => calc_flat_tilt(&mut out, sample_rate, freq, gain_db),
            FilterType::AllPass => calc_allpass(&mut out, slope, sample_rate, freq, q),
            FilterType::Notch | FilterType::BandPass => {
                calc_repeated(
                    &mut out,
                    ftype,
                    slope,
                    sample_rate,
                    freq,
                    gain_db,
                    q,
                    natural_phase,
                );
            }
        }
        out
    }

    /// Exact cascaded magnitude response in dB at `freq` (sum of all active stage dB responses).
    pub fn magnitude_db_at(&self, freq: f32, sample_rate: f32) -> f32 {
        let count = self.num_stages.min(MAX_CASCADE_STAGES);
        if count == 0 {
            return 0.0;
        }
        let mut total = 0.0f32;
        for stage in &self.stages[..count] {
            total += stage.magnitude_db_at(freq, sample_rate);
        }
        total
    }
}

#[inline]
pub(super) fn single_biquad(
    ftype: FilterType,
    sr: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    natural_phase: bool,
) -> BiquadCoeffs {
    if natural_phase {
        BiquadCoeffs::calculate_natural(ftype, sr, freq, gain_db, q)
    } else {
        BiquadCoeffs::calculate(ftype, sr, freq, gain_db, q)
    }
}

#[inline]
pub(super) fn butterworth_q(order: usize, stage_idx: usize) -> f32 {
    let theta = PI * (2 * stage_idx + 1) as f64 / (2 * order) as f64;
    (1.0 / (2.0 * theta.sin())) as f32
}

fn calc_cut(
    out: &mut CascadedCoeffs,
    ftype: FilterType,
    slope: FilterSlope,
    sr: f32,
    freq: f32,
    q: f32,
    natural: bool,
) {
    let q_scale = (q / FRAC_1_SQRT_2).clamp(0.1, 40.0);
    match slope {
        FilterSlope::Db6 => {
            out.num_stages = 1;
            out.stages[0] = BiquadCoeffs::calculate_first_order(ftype, sr, freq, 0.0);
        }
        FilterSlope::Db12 => {
            out.num_stages = 1;
            out.stages[0] = single_biquad(ftype, sr, freq, 0.0, q, natural);
        }
        FilterSlope::Db18 => {
            out.num_stages = 2;
            out.stages[0] = BiquadCoeffs::calculate_first_order(ftype, sr, freq, 0.0);
            out.stages[1] = single_biquad(ftype, sr, freq, 0.0, q_scale, natural);
        }
        FilterSlope::Db24 | FilterSlope::Db36 | FilterSlope::Db48 | FilterSlope::Db96 => {
            let m = match slope {
                FilterSlope::Db24 => 2,
                FilterSlope::Db36 => 3,
                FilterSlope::Db48 => 4,
                _ => 8,
            };
            let order = m * 2;
            out.num_stages = m;
            for (k, slot) in out.stages[..m].iter_mut().enumerate() {
                let qk = butterworth_q(order, k);
                let q_eff = if k == 0 {
                    (qk * q_scale).clamp(0.1, 40.0)
                } else {
                    qk
                };
                *slot = single_biquad(ftype, sr, freq, 0.0, q_eff, natural);
            }
        }
        FilterSlope::Brickwall => {
            // 16th-order Chebyshev Type I (0.02 dB passband ripple <= 0.5 dB, ultra-steep skirt)
            let sr64 = (sr as f64).max(1.0);
            let wc = (PI * (freq as f64).clamp(2.0, sr64 * 0.49) / sr64).tan();
            let eps = (10.0_f64.powf(0.02 / 10.0) - 1.0).sqrt();
            let v0 = (1.0 / eps).asinh() / 16.0;
            let (sh, ch) = (v0.sinh(), v0.cosh());
            out.num_stages = 8;
            for (k, slot) in out.stages.iter_mut().enumerate() {
                let theta = PI * (2 * k + 1) as f64 / 32.0;
                let sigma = -sh * theta.sin();
                let omega = ch * theta.cos();
                let rk = sigma.hypot(omega);
                let mut qk = (rk / (-2.0 * sigma)) as f32;
                if k == 0 {
                    qk = (qk * q_scale.clamp(0.5, 1.5)).clamp(0.5, 35.0);
                }
                let wk = if ftype == FilterType::HighCut {
                    wc * rk
                } else {
                    wc / rk
                };
                let fk = ((sr64 / PI) * wk.atan()) as f32;
                *slot = single_biquad(ftype, sr, fk, 0.0, qk, natural);
            }
            let dc_norm = (1.0 / (1.0 + eps * eps).sqrt()) as f32;
            out.stages[0] = out.stages[0].scale_gain(dc_norm);
        }
    }
}

/// Zero-allocation stereo cascaded filter supporting up to 8 biquad stages (16th order).
#[derive(Debug, Clone)]
pub struct CascadedStereoFilter {
    stages: [StereoBiquad; MAX_CASCADE_STAGES],
    num_stages: usize,
}

impl Default for CascadedStereoFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl CascadedStereoFilter {
    pub fn new() -> Self {
        Self {
            stages: std::array::from_fn(|_| StereoBiquad::new()),
            num_stages: 1,
        }
    }

    pub fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }

    #[inline]
    pub fn set_coeffs(&mut self, coeffs: &CascadedCoeffs) {
        let next = coeffs.num_stages.min(MAX_CASCADE_STAGES);
        if next > self.num_stages {
            for stage in &mut self.stages[self.num_stages..next] {
                stage.reset();
            }
        }
        self.num_stages = next;
        for i in 0..next {
            self.stages[i].set_coeffs(coeffs.stages[i]);
        }
    }

    #[inline(always)]
    pub fn process_stereo(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        for stage in &mut self.stages[..self.num_stages] {
            (l, r) = stage.process_stereo(l, r);
        }
        (l, r)
    }
}

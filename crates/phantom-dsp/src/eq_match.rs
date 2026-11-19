//! Pro-Q 4 Reference Spectrum Matching & Iterative Parametric Biquad Fitting Engine
//! Accumulates long-term reference and current track spectrums, computes a 1/3-octave
//! smoothed zero-mean delta curve, and iteratively fits up to 12 parametric EQ bands.

use crate::biquad::{BiquadCoeffs, FilterType};
use crate::spectrum::{display_bin_freq, NUM_DISPLAY_BINS, SPECTRUM_FLOOR_DB};

pub const MAX_MATCHED_BANDS: usize = 12;
const MATCH_SAMPLE_RATE: f32 = 48000.0;
const MAX_MATCH_GAIN_DB: f32 = 18.0;
const MIN_RESIDUAL_PEAK_DB: f32 = 0.35;

/// Parametric band specification produced by [`EqMatchEngine::compute_matched_bands`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchedBandSpec {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub filter_type: FilterType,
}

impl Default for MatchedBandSpec {
    fn default() -> Self {
        Self {
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 1.0,
            filter_type: FilterType::Bell,
        }
    }
}

/// Long-term spectrum accumulator and iterative parametric curve matcher.
#[derive(Debug, Clone)]
pub struct EqMatchEngine {
    ref_sum_db: [f64; NUM_DISPLAY_BINS],
    cur_sum_db: [f64; NUM_DISPLAY_BINS],
    ref_bins_db: [f32; NUM_DISPLAY_BINS],
    cur_bins_db: [f32; NUM_DISPLAY_BINS],
    delta_bins_db: [f32; NUM_DISPLAY_BINS],
    ref_frames: u32,
    cur_frames: u32,
}

impl Default for EqMatchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EqMatchEngine {
    pub fn new() -> Self {
        Self {
            ref_sum_db: [0.0; NUM_DISPLAY_BINS],
            cur_sum_db: [0.0; NUM_DISPLAY_BINS],
            ref_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            cur_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            delta_bins_db: [0.0; NUM_DISPLAY_BINS],
            ref_frames: 0,
            cur_frames: 0,
        }
    }

    pub fn record_reference_frame(&mut self, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        self.apply_frame(true, false, bins_db);
    }

    pub fn record_current_frame(&mut self, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        self.apply_frame(false, false, bins_db);
    }

    pub fn set_reference_snapshot(&mut self, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        self.apply_frame(true, true, bins_db);
    }

    pub fn set_current_snapshot(&mut self, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        self.apply_frame(false, true, bins_db);
    }

    pub fn clear_reference(&mut self) {
        self.ref_sum_db = [0.0; NUM_DISPLAY_BINS];
        self.ref_bins_db = [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS];
        self.ref_frames = 0;
        self.delta_bins_db = [0.0; NUM_DISPLAY_BINS];
    }

    pub fn clear_current(&mut self) {
        self.cur_sum_db = [0.0; NUM_DISPLAY_BINS];
        self.cur_bins_db = [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS];
        self.cur_frames = 0;
        self.delta_bins_db = [0.0; NUM_DISPLAY_BINS];
    }

    #[inline]
    pub fn reference_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.ref_bins_db
    }

    #[inline]
    pub fn current_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.cur_bins_db
    }

    #[inline]
    pub fn delta_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.delta_bins_db
    }

    #[inline]
    pub fn has_reference(&self) -> bool {
        self.ref_frames > 0
    }

    #[inline]
    pub fn has_current(&self) -> bool {
        self.cur_frames > 0
    }

    #[inline]
    pub fn reference_frame_count(&self) -> u32 {
        self.ref_frames
    }

    #[inline]
    pub fn current_frame_count(&self) -> u32 {
        self.cur_frames
    }

    fn apply_frame(&mut self, is_ref: bool, reset: bool, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        let (sum, avg, frames) = if is_ref {
            (
                &mut self.ref_sum_db,
                &mut self.ref_bins_db,
                &mut self.ref_frames,
            )
        } else {
            (
                &mut self.cur_sum_db,
                &mut self.cur_bins_db,
                &mut self.cur_frames,
            )
        };
        *frames = if reset { 1 } else { frames.saturating_add(1) };
        let inv = 1.0 / (*frames as f64);
        for i in 0..NUM_DISPLAY_BINS {
            let v = bins_db[i];
            let val = if v.is_finite() {
                v.clamp(SPECTRUM_FLOOR_DB, 60.0)
            } else {
                SPECTRUM_FLOOR_DB
            };
            sum[i] = if reset {
                val as f64
            } else {
                sum[i] + val as f64
            };
            avg[i] = (sum[i] * inv) as f32;
        }
        self.recompute_delta();
    }

    fn recompute_delta(&mut self) {
        if self.ref_frames == 0 || self.cur_frames == 0 {
            self.delta_bins_db = [0.0; NUM_DISPLAY_BINS];
            return;
        }
        let mut raw = [0.0_f32; NUM_DISPLAY_BINS];
        let mut mid_sum = 0.0_f64;
        let mut mid_count = 0usize;
        for (i, dst) in raw.iter_mut().enumerate() {
            let diff = self.ref_bins_db[i] - self.cur_bins_db[i];
            *dst = diff;
            let freq = display_bin_freq(i, MATCH_SAMPLE_RATE);
            if (100.0..=10000.0).contains(&freq) {
                mid_sum += diff as f64;
                mid_count += 1;
            }
        }
        let mean_offset = if mid_count > 0 {
            (mid_sum / (mid_count as f64)) as f32
        } else {
            0.0
        };
        for v in &mut raw {
            *v -= mean_offset;
        }
        const RADIUS: isize = 4;
        for (i, dst) in self.delta_bins_db.iter_mut().enumerate() {
            let (mut weighted_sum, mut weight_total) = (0.0_f32, 0.0_f32);
            for d in -RADIUS..=RADIUS {
                let idx = (i as isize + d).clamp(0, (NUM_DISPLAY_BINS - 1) as isize) as usize;
                let w = (RADIUS + 1 - d.abs()) as f32;
                weighted_sum += raw[idx] * w;
                weight_total += w;
            }
            *dst = (weighted_sum / weight_total).clamp(-MAX_MATCH_GAIN_DB, MAX_MATCH_GAIN_DB);
        }
    }

    pub fn compute_matched_bands(
        &mut self,
        max_bands: usize,
        amount_pct: f32,
    ) -> ([MatchedBandSpec; MAX_MATCHED_BANDS], usize) {
        self.recompute_delta();
        let mut out = [MatchedBandSpec::default(); MAX_MATCHED_BANDS];
        if self.ref_frames == 0 || self.cur_frames == 0 {
            return (out, 0);
        }
        let target_count = max_bands.clamp(1, MAX_MATCHED_BANDS);
        let amount_scale = (amount_pct / 100.0).clamp(0.0, 2.0);
        let mut residual = self.delta_bins_db;
        let bin_freqs: [f32; NUM_DISPLAY_BINS] =
            std::array::from_fn(|i| display_bin_freq(i, MATCH_SAMPLE_RATE));

        let mut count = 0usize;
        for _ in 0..target_count {
            let (mut best_idx, mut best_abs) = (0usize, 0.0_f32);
            for (i, &val) in residual.iter().enumerate() {
                if val.abs() > best_abs {
                    best_abs = val.abs();
                    best_idx = i;
                }
            }
            if best_abs < MIN_RESIDUAL_PEAK_DB {
                break;
            }
            let raw_gain_db = residual[best_idx].clamp(-MAX_MATCH_GAIN_DB, MAX_MATCH_GAIN_DB);
            let freq_hz = bin_freqs[best_idx];
            let (filter_type, fit_freq, q) = if freq_hz < 80.0 {
                (FilterType::LowShelf, freq_hz.clamp(35.0, 80.0), 0.707)
            } else if freq_hz > 10000.0 {
                (
                    FilterType::HighShelf,
                    freq_hz.clamp(10000.0, 16000.0),
                    0.707,
                )
            } else {
                (
                    FilterType::Bell,
                    freq_hz,
                    estimate_bell_q(&residual, &bin_freqs, best_idx),
                )
            };
            let proto =
                BiquadCoeffs::calculate(filter_type, MATCH_SAMPLE_RATE, fit_freq, raw_gain_db, q);
            for i in 0..NUM_DISPLAY_BINS {
                residual[i] -= proto.magnitude_db_at(bin_freqs[i], MATCH_SAMPLE_RATE);
            }
            let scaled_gain =
                (raw_gain_db * amount_scale).clamp(-MAX_MATCH_GAIN_DB, MAX_MATCH_GAIN_DB);
            if scaled_gain.abs() >= 0.05 {
                out[count] = MatchedBandSpec {
                    freq_hz: fit_freq,
                    gain_db: scaled_gain,
                    q,
                    filter_type,
                };
                count += 1;
            }
        }
        (out, count)
    }
}

fn estimate_bell_q(
    residual: &[f32; NUM_DISPLAY_BINS],
    freqs: &[f32; NUM_DISPLAY_BINS],
    peak_idx: usize,
) -> f32 {
    let peak_val = residual[peak_idx];
    let (sign, half_abs) = (peak_val.signum(), peak_val.abs() * 0.5);
    let mut left_idx = peak_idx;
    while left_idx > 0 && residual[left_idx] * sign > half_abs {
        left_idx -= 1;
    }
    let mut right_idx = peak_idx;
    while right_idx + 1 < NUM_DISPLAY_BINS && residual[right_idx] * sign > half_abs {
        right_idx += 1;
    }
    let f_left = freqs[left_idx].max(20.0);
    let f_right = freqs[right_idx].max(f_left * 1.05);
    let bw_oct = (f_right / f_left).log2().clamp(0.18, 2.8);
    (1.0 / (2.0 * (0.5 * std::f32::consts::LN_2 * bw_oct).sinh())).clamp(0.5, 6.0)
}

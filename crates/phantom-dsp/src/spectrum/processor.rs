//! GUI-thread FFT processor for `SharedSpectrum`.
//! Uses preallocated `realfft` scratch via `process_with_scratch`, supports Pro-Q tilt slope
//! (default 4.5 dB/oct pivoted around 1 kHz), and Peak Hold / Freeze mode.

use super::{
    SharedSpectrum, StageChannel, DEFAULT_TILT_DB_PER_OCT, FFT_SIZE, NUM_DISPLAY_BINS,
    SPECTRUM_FLOOR_DB,
};
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

/// GUI-thread FFT worker with preallocated scratch buffer, configurable tilt, and peak freeze.
pub struct SpectrumProcessor {
    fft_plan: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    post_bins_db: [f32; NUM_DISPLAY_BINS],
    pre_bins_db: [f32; NUM_DISPLAY_BINS],
    side_bins_db: [f32; NUM_DISPLAY_BINS],
    pub peak_hold_bins_db: [f32; NUM_DISPLAY_BINS],
    pub frozen: bool,
    tilt_db_per_oct: f32,
}

impl Default for SpectrumProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumProcessor {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_plan = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / FFT_SIZE as f32).cos()))
            .collect();
        let fft_in = fft_plan.make_input_vec();
        let fft_out = fft_plan.make_output_vec();
        let fft_scratch = fft_plan.make_scratch_vec();

        Self {
            fft_plan,
            window,
            fft_in,
            fft_out,
            fft_scratch,
            post_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            pre_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            side_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            peak_hold_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            frozen: false,
            tilt_db_per_oct: DEFAULT_TILT_DB_PER_OCT,
        }
    }

    pub fn set_tilt_db_per_oct(&mut self, tilt_db_per_oct: f32) {
        self.tilt_db_per_oct = tilt_db_per_oct.clamp(0.0, 9.0);
    }

    pub fn tilt_db_per_oct(&self) -> f32 {
        self.tilt_db_per_oct
    }

    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn reset_peaks(&mut self) {
        self.peak_hold_bins_db = [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS];
    }

    pub fn post_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.post_bins_db
    }

    pub fn pre_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.pre_bins_db
    }

    pub fn side_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.side_bins_db
    }

    pub fn peak_hold_bins_db(&self) -> &[f32; NUM_DISPLAY_BINS] {
        &self.peak_hold_bins_db
    }

    /// Processes pending Pre/Post/Side FFT frames and updates `shared.display_data`.
    pub fn update(&mut self, shared: &SharedSpectrum, sample_rate: f32) {
        if self.frozen {
            shared.pre.drain();
            shared.post.drain();
            shared.side.drain();
        } else {
            let sr = sample_rate.max(1000.0);
            let mut post = self.post_bins_db;
            self.process_stage(&shared.post, sr, &mut post);
            self.post_bins_db = post;

            let mut pre = self.pre_bins_db;
            self.process_stage(&shared.pre, sr, &mut pre);
            self.pre_bins_db = pre;

            let mut side = self.side_bins_db;
            self.process_stage(&shared.side, sr, &mut side);
            self.side_bins_db = side;

            for (peak, &post) in self
                .peak_hold_bins_db
                .iter_mut()
                .zip(self.post_bins_db.iter())
            {
                *peak = peak.max(post);
            }
        }

        if let Ok(mut disp) = shared.display_data.try_lock() {
            disp.bins_db.copy_from_slice(&self.post_bins_db);
            disp.post_bins_db.copy_from_slice(&self.post_bins_db);
            disp.pre_bins_db.copy_from_slice(&self.pre_bins_db);
            disp.side_bins_db.copy_from_slice(&self.side_bins_db);
            disp.peak_bins_db.copy_from_slice(&self.peak_hold_bins_db);
        }
    }

    fn process_stage(
        &mut self,
        ch: &StageChannel,
        sample_rate: f32,
        target: &mut [f32; NUM_DISPLAY_BINS],
    ) {
        if !ch.take_windowed(&self.window, &mut self.fft_in) {
            for bin in target.iter_mut() {
                *bin = (*bin - 1.2).max(SPECTRUM_FLOOR_DB);
            }
            return;
        }
        let _ = self.fft_plan.process_with_scratch(
            &mut self.fft_in,
            &mut self.fft_out,
            &mut self.fft_scratch,
        );

        let min_f = 20.0_f32;
        let max_f = 20000.0_f32.min(sample_rate * 0.499);
        let log_min = min_f.ln();
        let log_span = max_f.ln() - log_min;
        let n_bins = self.fft_out.len();
        let bin_hz = sample_rate / FFT_SIZE as f32;
        let scale = 1.0 / (FFT_SIZE as f32 * 0.25);
        let last = (NUM_DISPLAY_BINS - 1) as f32;

        for (i, out_db) in target.iter_mut().enumerate() {
            let fi = i as f32;
            let freq = (log_min + (fi / last) * log_span).exp();
            let lo = (((log_min + (fi - 0.5).max(0.0) / last * log_span).exp() / bin_hz).floor()
                as usize)
                .clamp(1, n_bins - 1);
            let hi = (((log_min + (fi + 0.5).min(last) / last * log_span).exp() / bin_hz).ceil()
                as usize)
                .clamp(lo, n_bins - 1);

            let mag = if hi > lo + 1 {
                (lo..=hi)
                    .map(|k| self.fft_out[k].norm())
                    .fold(0.0_f32, f32::max)
                    * scale
            } else {
                let exact = (freq / bin_hz).clamp(1.0, (n_bins - 1) as f32);
                let i0 = exact.floor() as usize;
                let i1 = (i0 + 1).min(n_bins - 1);
                let frac = exact - i0 as f32;
                let m0 = self.fft_out[i0].norm();
                let m1 = self.fft_out[i1].norm();
                (m0 + frac * (m1 - m0)) * scale
            };

            let raw_db = if mag > 1e-6 {
                20.0 * mag.log10()
            } else {
                SPECTRUM_FLOOR_DB
            };
            let db =
                (raw_db + self.tilt_db_per_oct * (freq / 1000.0).log2()).max(SPECTRUM_FLOOR_DB);
            *out_db = if db > *out_db {
                db
            } else {
                (*out_db - 1.8).max(SPECTRUM_FLOOR_DB)
            };
        }
    }
}

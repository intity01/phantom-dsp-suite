//! Mastering-Grade Linear Phase FIR Filter Engine
//!
//! Uses zero-padded Overlap-Save FFT convolution with an odd-length Type-I symmetric
//! Kaiser-windowed FIR kernel (`KERNEL_LEN = 4095`, `FFT_SIZE = 8192`) and a fixed-hop
//! framing buffer (`HOP_SIZE = 1024`) for arbitrary DAW buffer sizes with zero circular
//! aliasing, zero heap allocations during processing or kernel updates, full Stereo /
//! Left / Right / Mid / Side 2x2 transfer matrix support, and raised-cosine click-free
//! kernel crossfading.
//!
//! Total end-to-end latency:
//! `LATENCY_SAMPLES = FIR_GROUP_DELAY + HOP_SIZE = 2047 + 1024 = 3071` samples
//! (~64.0 ms at 48 kHz, invariant across all DAW buffer sizes).

mod convolve;
mod kernel;

pub use kernel::{
    StereoCurveDb, FFT_SIZE, FIR_GROUP_DELAY, FIR_SIZE, HOP_SIZE, KERNEL_LEN, LATENCY_SAMPLES,
    NUM_BINS,
};

use convolve::{
    convolve_hop_crossfade, convolve_hop_steady, sample_gain_matrix, synthesize_transfer_matrix,
};
use kernel::{
    build_crossfade_table, build_kaiser_window, EngineScratch, TransferMatrix, VALID_START,
};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct LinearPhaseEngine {
    fft_plan: Arc<dyn RealToComplex<f32>>,
    ifft_plan: Arc<dyn ComplexToReal<f32>>,
    window: Vec<f32>,
    crossfade_table: Vec<f32>,
    /// Double-buffered 2x2 frequency-domain transfer matrices for click-free crossfades
    matrices: [TransferMatrix; 2],
    rendered_idx: usize,
    target_idx: usize,
    dirty: bool,
    has_processed_audio: bool,
    /// Overlap-Save stereo input histories (length `FFT_SIZE`)
    input_history_l: Vec<f32>,
    input_history_r: Vec<f32>,
    /// Fixed-hop framing buffers (length `HOP_SIZE`)
    in_buf_l: Vec<f32>,
    in_buf_r: Vec<f32>,
    out_buf_l: Vec<f32>,
    out_buf_r: Vec<f32>,
    hop_pos: usize,
    /// Preallocated FFT/IFFT/synthesis scratch memory (zero runtime heap allocations)
    scratch: EngineScratch,
}

impl Default for LinearPhaseEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearPhaseEngine {
    /// Allocate and plan a new Linear Phase FIR engine initialized to flat 0 dB identity.
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_plan = planner.plan_fft_forward(FFT_SIZE);
        let ifft_plan = planner.plan_fft_inverse(FFT_SIZE);

        let window = build_kaiser_window();
        let crossfade_table = build_crossfade_table();
        let mut scratch = EngineScratch::new(&fft_plan, &ifft_plan);

        let m0 = TransferMatrix::new_identity(
            &fft_plan,
            &mut scratch.fft_in,
            &mut scratch.fft_scratch,
        );
        let m1 = TransferMatrix::new_identity(
            &fft_plan,
            &mut scratch.fft_in,
            &mut scratch.fft_scratch,
        );

        Self {
            fft_plan,
            ifft_plan,
            window,
            crossfade_table,
            matrices: [m0, m1],
            rendered_idx: 0,
            target_idx: 0,
            dirty: false,
            has_processed_audio: false,
            input_history_l: vec![0.0; FFT_SIZE],
            input_history_r: vec![0.0; FFT_SIZE],
            in_buf_l: vec![0.0; HOP_SIZE],
            in_buf_r: vec![0.0; HOP_SIZE],
            out_buf_l: vec![0.0; HOP_SIZE],
            out_buf_r: vec![0.0; HOP_SIZE],
            hop_pos: 0,
            scratch,
        }
    }

    /// Total end-to-end processing latency in samples (`FIR_GROUP_DELAY + HOP_SIZE`).
    #[inline]
    pub const fn latency_samples(&self) -> usize {
        LATENCY_SAMPLES
    }

    /// Active primary (Left-to-Left) time-domain Type-I symmetric FIR kernel (`KERNEL_LEN`).
    #[inline]
    pub fn kernel_left(&self) -> &[f32] {
        &self.matrices[self.target_idx].kernel_ll
    }

    /// Active secondary (Right-to-Right) time-domain Type-I symmetric FIR kernel (`KERNEL_LEN`).
    #[inline]
    pub fn kernel_right(&self) -> &[f32] {
        &self.matrices[self.target_idx].kernel_rr
    }

    /// Clear all delay lines and framing buffers while preserving the current EQ curve.
    pub fn reset(&mut self) {
        self.input_history_l.fill(0.0);
        self.input_history_r.fill(0.0);
        self.in_buf_l.fill(0.0);
        self.in_buf_r.fill(0.0);
        self.out_buf_l.fill(0.0);
        self.out_buf_r.fill(0.0);
        self.hop_pos = 0;
        self.has_processed_audio = false;
        self.rendered_idx = self.target_idx;
        self.dirty = false;
    }

    /// Re-synthesize a linked Stereo linear-phase FIR kernel from a dB magnitude curve.
    pub fn update_from_magnitude_curve<F>(&mut self, sample_rate: f32, mut magnitude_db_fn: F)
    where
        F: FnMut(f32) -> f32,
    {
        self.update_stereo_curves(sample_rate, move |freq| {
            let db = magnitude_db_fn(freq);
            StereoCurveDb { left: db, right: db, mid: 0.0, side: 0.0 }
        });
    }

    /// Re-synthesize independent two-channel linear-phase FIR curves in L/R or M/S mode.
    ///
    /// - `is_mid_side == false`: `ch0_db_fn` shapes Left, `ch1_db_fn` shapes Right.
    /// - `is_mid_side == true`: `ch0_db_fn` shapes Mid, `ch1_db_fn` shapes Side.
    pub fn update_curves<F1, F2>(
        &mut self,
        sample_rate: f32,
        mut ch0_db_fn: F1,
        mut ch1_db_fn: F2,
        is_mid_side: bool,
    ) where
        F1: FnMut(f32) -> f32,
        F2: FnMut(f32) -> f32,
    {
        self.update_stereo_curves(sample_rate, move |freq| {
            let d0 = ch0_db_fn(freq);
            let d1 = ch1_db_fn(freq);
            if is_mid_side {
                StereoCurveDb { left: 0.0, right: 0.0, mid: d0, side: d1 }
            } else {
                StereoCurveDb { left: d0, right: d1, mid: 0.0, side: 0.0 }
            }
        });
    }

    /// Re-synthesize simultaneous Stereo + Left/Right + Mid/Side linear-phase transfer curves.
    ///
    /// Zero heap allocations. Schedules a C1-smooth raised-cosine crossfade on the next hop
    /// if audio has already been processed, or applies immediately before playback starts.
    pub fn update_stereo_curves<F>(&mut self, sample_rate: f32, curve_db_fn: F)
    where
        F: FnMut(f32) -> StereoCurveDb,
    {
        let (has_cross, lr_equal, cross_equal) =
            sample_gain_matrix(&mut self.scratch, sample_rate, curve_db_fn);

        let dest_idx = if !self.has_processed_audio {
            self.rendered_idx = 0;
            self.target_idx = 0;
            self.dirty = false;
            0
        } else if self.dirty {
            self.target_idx
        } else {
            let next = 1 - self.rendered_idx;
            self.target_idx = next;
            self.dirty = true;
            next
        };

        synthesize_transfer_matrix(
            &mut self.matrices[dest_idx],
            &mut self.scratch,
            &self.window,
            &self.fft_plan,
            &self.ifft_plan,
            has_cross,
            lr_equal,
            cross_equal,
        );
    }

    /// Process an arbitrary-length stereo buffer in-place with zero heap allocations.
    pub fn process_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let block_len = left.len().min(right.len());
        let mut offset = 0;

        while offset < block_len {
            let count = (block_len - offset).min(HOP_SIZE - self.hop_pos);
            let pos = self.hop_pos;

            for i in 0..count {
                let l_in = left[offset + i];
                let r_in = right[offset + i];
                self.in_buf_l[pos + i] = if l_in.is_finite() { l_in } else { 0.0 };
                self.in_buf_r[pos + i] = if r_in.is_finite() { r_in } else { 0.0 };
                left[offset + i] = self.out_buf_l[pos + i];
                right[offset + i] = self.out_buf_r[pos + i];
            }

            self.hop_pos += count;
            offset += count;

            if self.hop_pos == HOP_SIZE {
                self.process_hop();
                self.hop_pos = 0;
            }
        }
    }

    fn process_hop(&mut self) {
        self.has_processed_audio = true;

        self.input_history_l.copy_within(HOP_SIZE..FFT_SIZE, 0);
        self.input_history_l[VALID_START..FFT_SIZE].copy_from_slice(&self.in_buf_l);
        self.input_history_r.copy_within(HOP_SIZE..FFT_SIZE, 0);
        self.input_history_r[VALID_START..FFT_SIZE].copy_from_slice(&self.in_buf_r);

        let s = &mut self.scratch;
        s.fft_in.copy_from_slice(&self.input_history_l);
        let res_l = self
            .fft_plan
            .process_with_scratch(&mut s.fft_in, &mut s.spec_in_l, &mut s.fft_scratch);
        debug_assert!(res_l.is_ok(), "realfft forward FFT failed (L): {:?}", res_l);

        s.fft_in.copy_from_slice(&self.input_history_r);
        let res_r = self
            .fft_plan
            .process_with_scratch(&mut s.fft_in, &mut s.spec_in_r, &mut s.fft_scratch);
        debug_assert!(res_r.is_ok(), "realfft forward FFT failed (R): {:?}", res_r);

        if !self.dirty {
            convolve_hop_steady(
                s,
                &self.matrices[self.target_idx],
                &self.ifft_plan,
                &mut self.out_buf_l,
                &mut self.out_buf_r,
            );
        } else {
            convolve_hop_crossfade(
                s,
                &self.matrices[self.rendered_idx],
                &self.matrices[self.target_idx],
                &self.crossfade_table,
                &self.ifft_plan,
                &mut self.out_buf_l,
                &mut self.out_buf_r,
            );
            self.rendered_idx = self.target_idx;
            self.dirty = false;
        }
    }
}

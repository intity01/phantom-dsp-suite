//! Type-I Linear Phase FIR Constants, Transfer Matrix, and Window Generators
//! Zero heap allocations after initialization.

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

/// FFT length for zero-padded Overlap-Save convolution (`>= KERNEL_LEN + HOP_SIZE - 1`).
pub const FFT_SIZE: usize = 8192;
/// Number of unique complex frequency bins (`FFT_SIZE / 2 + 1`).
pub const NUM_BINS: usize = FFT_SIZE / 2 + 1;
/// Odd Type-I symmetric FIR kernel length (`2 * FIR_GROUP_DELAY + 1`).
pub const KERNEL_LEN: usize = 4095;
/// Exact group delay of the symmetric FIR kernel in samples.
pub const FIR_GROUP_DELAY: usize = (KERNEL_LEN - 1) / 2;
/// Fixed internal framing hop size (avoids per-small-DAW-block FFT overhead).
pub const HOP_SIZE: usize = 1024;
/// Total end-to-end linear-phase engine latency in samples (`FIR_GROUP_DELAY + HOP_SIZE`).
pub const LATENCY_SAMPLES: usize = FIR_GROUP_DELAY + HOP_SIZE;
/// Backwards-compatible alias for FFT transform size.
pub const FIR_SIZE: usize = FFT_SIZE;

/// Start index of the valid, non-aliased Overlap-Save output slice (`FFT_SIZE - HOP_SIZE`).
pub(super) const VALID_START: usize = FFT_SIZE - HOP_SIZE;
/// Kaiser window beta (~-76 dB sidelobes with sharp ~30 Hz main lobe at 48 kHz).
const KAISER_BETA: f64 = 7.5;

/// Per-frequency target gains in decibels for Stereo / Left / Right / Mid / Side EQ curves.
///
/// For Stereo bands, add the band's dB gain to both `left` and `right`.
/// `mid` and `side` apply exact linear-phase M/S EQ simultaneously with `left`/`right`
/// via a unified 2x2 frequency-domain transfer matrix (zero history glitches on mode change).
#[derive(Debug, Clone, Copy, Default)]
pub struct StereoCurveDb {
    pub left: f32,
    pub right: f32,
    pub mid: f32,
    pub side: f32,
}

/// Frequency-domain 2x2 linear-phase transfer matrix (pre-scaled by `1 / FFT_SIZE`).
pub(super) struct TransferMatrix {
    pub ll: Vec<Complex<f32>>,
    pub lr: Vec<Complex<f32>>,
    pub rl: Vec<Complex<f32>>,
    pub rr: Vec<Complex<f32>>,
    pub has_cross: bool,
    pub kernel_ll: Vec<f32>,
    pub kernel_rr: Vec<f32>,
}

impl TransferMatrix {
    pub fn new_identity(
        fft_plan: &Arc<dyn RealToComplex<f32>>,
        fft_in: &mut [f32],
        fft_scratch: &mut [Complex<f32>],
    ) -> Self {
        let mut kernel_ll = vec![0.0f32; KERNEL_LEN];
        kernel_ll[FIR_GROUP_DELAY] = 1.0;
        let kernel_rr = kernel_ll.clone();

        fft_in.fill(0.0);
        fft_in[FIR_GROUP_DELAY] = 1.0 / FFT_SIZE as f32;

        let mut ll = vec![Complex::new(0.0, 0.0); NUM_BINS];
        let res = fft_plan.process_with_scratch(fft_in, &mut ll, fft_scratch);
        debug_assert!(res.is_ok(), "identity FFT failed: {:?}", res);
        enforce_real_dc_nyquist(&mut ll);

        let rr = ll.clone();
        let lr = vec![Complex::new(0.0, 0.0); NUM_BINS];
        let rl = vec![Complex::new(0.0, 0.0); NUM_BINS];

        Self {
            ll,
            lr,
            rl,
            rr,
            has_cross: false,
            kernel_ll,
            kernel_rr,
        }
    }
}

/// Preallocated scratch buffers used during kernel synthesis and overlap-save convolution.
pub(super) struct EngineScratch {
    pub fft_in: Vec<f32>,
    pub ifft_out: Vec<f32>,
    pub spec_in_l: Vec<Complex<f32>>,
    pub spec_in_r: Vec<Complex<f32>>,
    pub spec_work: Vec<Complex<f32>>,
    pub fft_scratch: Vec<Complex<f32>>,
    pub ifft_scratch: Vec<Complex<f32>>,
    pub gain_ll: Vec<f32>,
    pub gain_lr: Vec<f32>,
    pub gain_rl: Vec<f32>,
    pub gain_rr: Vec<f32>,
}

impl EngineScratch {
    pub fn new(
        fft_plan: &Arc<dyn RealToComplex<f32>>,
        ifft_plan: &Arc<dyn ComplexToReal<f32>>,
    ) -> Self {
        Self {
            fft_in: fft_plan.make_input_vec(),
            ifft_out: ifft_plan.make_output_vec(),
            spec_in_l: fft_plan.make_output_vec(),
            spec_in_r: fft_plan.make_output_vec(),
            spec_work: ifft_plan.make_input_vec(),
            fft_scratch: fft_plan.make_scratch_vec(),
            ifft_scratch: ifft_plan.make_scratch_vec(),
            gain_ll: vec![1.0; NUM_BINS],
            gain_lr: vec![0.0; NUM_BINS],
            gain_rl: vec![0.0; NUM_BINS],
            gain_rr: vec![1.0; NUM_BINS],
        }
    }
}

/// Enforce `im == 0.0` at DC (`k = 0`) and Nyquist (`k = NUM_BINS - 1`), required by `realfft`.
#[inline(always)]
pub(super) fn enforce_real_dc_nyquist(spectrum: &mut [Complex<f32>]) {
    spectrum[0].im = 0.0;
    spectrum[NUM_BINS - 1].im = 0.0;
}

/// Convert decibels to clamped linear amplitude, immune to NaN/Inf inputs.
#[inline(always)]
pub(super) fn db_to_linear_clamped(db: f32) -> f32 {
    if !db.is_finite() {
        return 1.0;
    }
    10.0f32.powf(db.clamp(-120.0, 48.0) * 0.05)
}

/// Zero-order modified Bessel function `I0(x)` for Kaiser window synthesis.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half_x_sq = 0.25 * x * x;
    for k in 1..32 {
        term *= half_x_sq / ((k * k) as f64);
        sum += term;
        if term < sum * 1e-16 {
            break;
        }
    }
    sum
}

/// Build an odd-length bit-exact symmetric Kaiser window (`w[M] == 1.0`, `w[M-d] == w[M+d]`).
pub(super) fn build_kaiser_window() -> Vec<f32> {
    let mut window = vec![0.0f32; KERNEL_LEN];
    let inv_i0_beta = 1.0 / bessel_i0(KAISER_BETA);
    let m = FIR_GROUP_DELAY;
    let m_f64 = m as f64;

    window[m] = 1.0;
    for d in 1..=m {
        let ratio = d as f64 / m_f64;
        let arg = KAISER_BETA * (1.0 - ratio * ratio).max(0.0).sqrt();
        let w = (bessel_i0(arg) * inv_i0_beta) as f32;
        window[m - d] = w;
        window[m + d] = w;
    }
    window
}

/// Precompute raised-cosine crossfade ramp (`0 -> 1` over `HOP_SIZE` samples, C1-smooth).
pub(super) fn build_crossfade_table() -> Vec<f32> {
    let mut table = Vec::with_capacity(HOP_SIZE);
    let denom = HOP_SIZE as f32;
    for i in 0..HOP_SIZE {
        let phase = PI * (i as f32 + 0.5) / denom;
        table.push(0.5 - 0.5 * phase.cos());
    }
    table
}

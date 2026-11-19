//! Zero-Allocation FIR Kernel Synthesis & Overlap-Save Stereo Convolution Helpers

use super::kernel::{
    db_to_linear_clamped, enforce_real_dc_nyquist, EngineScratch, StereoCurveDb, TransferMatrix,
    FFT_SIZE, FIR_GROUP_DELAY, HOP_SIZE, KERNEL_LEN, NUM_BINS, VALID_START,
};
use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealToComplex};
use std::sync::Arc;

/// Sample the user's dB transfer curves into `scratch.gain_*` and return `(has_cross, lr_equal, cross_equal)`.
pub(super) fn sample_gain_matrix<F>(
    scratch: &mut EngineScratch,
    sample_rate: f32,
    mut curve_db_fn: F,
) -> (bool, bool, bool)
where
    F: FnMut(f32) -> StereoCurveDb,
{
    let bin_hz = sample_rate.max(1.0) / FFT_SIZE as f32;
    let mut has_cross = false;
    let mut lr_equal = true;
    let mut cross_equal = true;

    for k in 0..NUM_BINS {
        let c = curve_db_fn(k as f32 * bin_hz);
        let gl = db_to_linear_clamped(c.left);
        let gr = db_to_linear_clamped(c.right);
        let gm = db_to_linear_clamped(c.mid);
        let gs = db_to_linear_clamped(c.side);

        let sum_ms = 0.5 * (gm + gs);
        let diff_ms = 0.5 * (gm - gs);
        let (m_ll, m_lr, m_rl, m_rr) = (sum_ms * gl, diff_ms * gr, diff_ms * gl, sum_ms * gr);

        scratch.gain_ll[k] = m_ll;
        scratch.gain_lr[k] = m_lr;
        scratch.gain_rl[k] = m_rl;
        scratch.gain_rr[k] = m_rr;

        if m_lr.abs() > 1e-7 || m_rl.abs() > 1e-7 {
            has_cross = true;
        }
        if (m_ll - m_rr).abs() > 1e-7 {
            lr_equal = false;
        }
        if (m_lr - m_rl).abs() > 1e-7 {
            cross_equal = false;
        }
    }

    (has_cross, lr_equal, cross_equal)
}

/// Synthesize a single Type-I symmetric FIR channel from real zero-phase spectrum `target_re`.
#[allow(clippy::too_many_arguments)]
fn synthesize_fir_channel(
    target_re: &[f32],
    window: &[f32],
    fft_plan: &Arc<dyn RealToComplex<f32>>,
    ifft_plan: &Arc<dyn ComplexToReal<f32>>,
    spec_work: &mut [Complex<f32>],
    fft_in: &mut [f32],
    ifft_out: &mut [f32],
    fft_scratch: &mut [Complex<f32>],
    ifft_scratch: &mut [Complex<f32>],
    out_kernel: Option<&mut [f32]>,
    out_spectrum: &mut [Complex<f32>],
) {
    for k in 0..NUM_BINS {
        spec_work[k] = Complex::new(target_re[k], 0.0);
    }
    enforce_real_dc_nyquist(spec_work);

    let inv_res = ifft_plan.process_with_scratch(spec_work, ifft_out, ifft_scratch);
    debug_assert!(inv_res.is_ok(), "realfft IFFT failed in FIR synthesis: {:?}", inv_res);

    let inv_n = 1.0 / FFT_SIZE as f32;
    let m = FIR_GROUP_DELAY;
    fft_in[m] = ifft_out[0] * inv_n * window[m];
    for d in 1..=m {
        fft_in[m - d] = ifft_out[FFT_SIZE - d] * inv_n * window[m - d];
        fft_in[m + d] = ifft_out[d] * inv_n * window[m + d];
    }

    if let Some(k_out) = out_kernel {
        k_out.copy_from_slice(&fft_in[..KERNEL_LEN]);
    }

    for sample in &mut fft_in[..KERNEL_LEN] {
        *sample *= inv_n;
    }
    fft_in[KERNEL_LEN..FFT_SIZE].fill(0.0);

    let fwd_res = fft_plan.process_with_scratch(fft_in, out_spectrum, fft_scratch);
    debug_assert!(fwd_res.is_ok(), "realfft FFT failed in FIR synthesis: {:?}", fwd_res);
    enforce_real_dc_nyquist(out_spectrum);
}

/// Synthesize a full 2x2 `TransferMatrix` from `scratch.gain_*`, skipping symmetric duplicates.
pub(super) fn synthesize_transfer_matrix(
    dest: &mut TransferMatrix,
    scratch: &mut EngineScratch,
    window: &[f32],
    fft_plan: &Arc<dyn RealToComplex<f32>>,
    ifft_plan: &Arc<dyn ComplexToReal<f32>>,
    has_cross: bool,
    lr_equal: bool,
    cross_equal: bool,
) {
    synthesize_fir_channel(
        &scratch.gain_ll,
        window,
        fft_plan,
        ifft_plan,
        &mut scratch.spec_work,
        &mut scratch.fft_in,
        &mut scratch.ifft_out,
        &mut scratch.fft_scratch,
        &mut scratch.ifft_scratch,
        Some(&mut dest.kernel_ll),
        &mut dest.ll,
    );

    if lr_equal {
        dest.rr.copy_from_slice(&dest.ll);
        dest.kernel_rr.copy_from_slice(&dest.kernel_ll);
    } else {
        synthesize_fir_channel(
            &scratch.gain_rr,
            window,
            fft_plan,
            ifft_plan,
            &mut scratch.spec_work,
            &mut scratch.fft_in,
            &mut scratch.ifft_out,
            &mut scratch.fft_scratch,
            &mut scratch.ifft_scratch,
            Some(&mut dest.kernel_rr),
            &mut dest.rr,
        );
    }

    dest.has_cross = has_cross;
    if has_cross {
        synthesize_fir_channel(
            &scratch.gain_lr,
            window,
            fft_plan,
            ifft_plan,
            &mut scratch.spec_work,
            &mut scratch.fft_in,
            &mut scratch.ifft_out,
            &mut scratch.fft_scratch,
            &mut scratch.ifft_scratch,
            None,
            &mut dest.lr,
        );
        if cross_equal {
            dest.rl.copy_from_slice(&dest.lr);
        } else {
            synthesize_fir_channel(
                &scratch.gain_rl,
                window,
                fft_plan,
                ifft_plan,
                &mut scratch.spec_work,
                &mut scratch.fft_in,
                &mut scratch.ifft_out,
                &mut scratch.fft_scratch,
                &mut scratch.ifft_scratch,
                None,
                &mut dest.rl,
            );
        }
    }
}

/// Apply one channel row of `TransferMatrix` to stereo spectra and inverse-FFT into `ifft_out`.
fn convolve_channel_ifft(
    scratch: &mut EngineScratch,
    matrix: &TransferMatrix,
    is_right: bool,
    ifft_plan: &Arc<dyn ComplexToReal<f32>>,
) {
    let (diag, cross) = if is_right {
        (&matrix.rr, &matrix.rl)
    } else {
        (&matrix.ll, &matrix.lr)
    };

    if matrix.has_cross {
        if is_right {
            for k in 0..NUM_BINS {
                scratch.spec_work[k] =
                    scratch.spec_in_l[k] * cross[k] + scratch.spec_in_r[k] * diag[k];
            }
        } else {
            for k in 0..NUM_BINS {
                scratch.spec_work[k] =
                    scratch.spec_in_l[k] * diag[k] + scratch.spec_in_r[k] * cross[k];
            }
        }
    } else {
        let primary = if is_right {
            &scratch.spec_in_r
        } else {
            &scratch.spec_in_l
        };
        for k in 0..NUM_BINS {
            scratch.spec_work[k] = primary[k] * diag[k];
        }
    }

    enforce_real_dc_nyquist(&mut scratch.spec_work);
    let res = ifft_plan.process_with_scratch(
        &mut scratch.spec_work,
        &mut scratch.ifft_out,
        &mut scratch.ifft_scratch,
    );
    debug_assert!(res.is_ok(), "realfft IFFT failed in audio convolution: {:?}", res);
}

/// Steady-state stereo frequency-domain convolution for one `HOP_SIZE` block.
pub(super) fn convolve_hop_steady(
    scratch: &mut EngineScratch,
    matrix: &TransferMatrix,
    ifft_plan: &Arc<dyn ComplexToReal<f32>>,
    out_buf_l: &mut [f32],
    out_buf_r: &mut [f32],
) {
    convolve_channel_ifft(scratch, matrix, false, ifft_plan);
    out_buf_l.copy_from_slice(&scratch.ifft_out[VALID_START..FFT_SIZE]);

    convolve_channel_ifft(scratch, matrix, true, ifft_plan);
    out_buf_r.copy_from_slice(&scratch.ifft_out[VALID_START..FFT_SIZE]);
}

/// Raised-cosine click-free crossfade between `old_matrix` and `new_matrix` over `HOP_SIZE`.
pub(super) fn convolve_hop_crossfade(
    scratch: &mut EngineScratch,
    old_matrix: &TransferMatrix,
    new_matrix: &TransferMatrix,
    crossfade_table: &[f32],
    ifft_plan: &Arc<dyn ComplexToReal<f32>>,
    out_buf_l: &mut [f32],
    out_buf_r: &mut [f32],
) {
    // Left channel: old -> new
    convolve_channel_ifft(scratch, old_matrix, false, ifft_plan);
    out_buf_l.copy_from_slice(&scratch.ifft_out[VALID_START..FFT_SIZE]);
    convolve_channel_ifft(scratch, new_matrix, false, ifft_plan);
    for i in 0..HOP_SIZE {
        let fade = crossfade_table[i];
        out_buf_l[i] = out_buf_l[i] * (1.0 - fade) + scratch.ifft_out[VALID_START + i] * fade;
    }

    // Right channel: old -> new
    convolve_channel_ifft(scratch, old_matrix, true, ifft_plan);
    out_buf_r.copy_from_slice(&scratch.ifft_out[VALID_START..FFT_SIZE]);
    convolve_channel_ifft(scratch, new_matrix, true, ifft_plan);
    for i in 0..HOP_SIZE {
        let fade = crossfade_table[i];
        out_buf_r[i] = out_buf_r[i] * (1.0 - fade) + scratch.ifft_out[VALID_START + i] * fade;
    }
}

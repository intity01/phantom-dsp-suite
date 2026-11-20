//! Cubic Hermite interpolation and FFT spectral-envelope formant preservation/shifting.

use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

/// 4-point cubic Hermite (Catmull-Rom) fractional sample reader.
#[inline]
pub(crate) fn sample_cubic(buf: &[f32], pos: f32) -> f32 {
    if buf.is_empty() || !pos.is_finite() {
        return 0.0;
    }
    let idx = pos.floor() as isize;
    let t = pos - idx as f32;
    let get = |k: isize| -> f32 {
        if k < 0 || (k as usize) >= buf.len() {
            0.0
        } else {
            buf[k as usize]
        }
    };
    let (y0, y1, y2, y3) = (get(idx - 1), get(idx), get(idx + 1), get(idx + 2));
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

/// Reusable FFT spectral-envelope processor for formant preservation and shifting.
pub(crate) struct FormantProcessor {
    fft_len: usize,
    smooth_radius: usize,
    rfft: Arc<dyn RealToComplex<f32>>,
    irfft: Arc<dyn ComplexToReal<f32>>,
    buf_a: Vec<f32>,
    buf_b: Vec<f32>,
    spec_a: Vec<Complex<f32>>,
    spec_b: Vec<Complex<f32>>,
    fwd_scratch: Vec<Complex<f32>>,
    inv_scratch: Vec<Complex<f32>>,
    env_a: Vec<f32>,
    env_b: Vec<f32>,
    env_tmp: Vec<f32>,
}

impl FormantProcessor {
    pub fn new(grain_len: usize, sample_rate: f32) -> Self {
        let fft_len = grain_len.next_power_of_two().max(256);
        let bin_hz = sample_rate / fft_len as f32;
        let smooth_radius = ((190.0 / bin_hz).round() as usize).clamp(3, 24);
        let mut planner = RealFftPlanner::<f32>::new();
        let rfft = planner.plan_fft_forward(fft_len);
        let irfft = planner.plan_fft_inverse(fft_len);
        let n_bins = fft_len / 2 + 1;
        Self {
            fft_len,
            smooth_radius,
            spec_a: rfft.make_output_vec(),
            spec_b: rfft.make_output_vec(),
            fwd_scratch: rfft.make_scratch_vec(),
            inv_scratch: irfft.make_scratch_vec(),
            rfft,
            irfft,
            buf_a: vec![0.0; fft_len],
            buf_b: vec![0.0; fft_len],
            env_a: vec![0.0; n_bins],
            env_b: vec![0.0; n_bins],
            env_tmp: vec![0.0; n_bins],
        }
    }

    /// Restores the smoothed spectral envelope of `orig` onto `shifted` (preserving formants).
    pub fn preserve_formants(&mut self, orig: &[f32], shifted: &mut [f32]) {
        let g = orig.len().min(shifted.len()).min(self.fft_len);
        self.buf_a.fill(0.0);
        self.buf_b.fill(0.0);
        self.buf_a[..g].copy_from_slice(&orig[..g]);
        self.buf_b[..g].copy_from_slice(&shifted[..g]);

        let _ = self.rfft.process_with_scratch(
            &mut self.buf_a,
            &mut self.spec_a,
            &mut self.fwd_scratch,
        );
        let _ = self.rfft.process_with_scratch(
            &mut self.buf_b,
            &mut self.spec_b,
            &mut self.fwd_scratch,
        );

        smooth_log_env(
            &self.spec_a,
            &mut self.env_a,
            &mut self.env_tmp,
            self.smooth_radius,
        );
        smooth_log_env(
            &self.spec_b,
            &mut self.env_b,
            &mut self.env_tmp,
            self.smooth_radius,
        );

        for k in 0..self.spec_b.len() {
            let gain = (self.env_a[k] - self.env_b[k]).clamp(-1.5, 1.5).exp();
            self.spec_b[k] *= gain;
        }
        zero_dc_nyquist_imag(&mut self.spec_b);

        let _ = self.irfft.process_with_scratch(
            &mut self.spec_b,
            &mut self.buf_b,
            &mut self.inv_scratch,
        );
        let inv_n = 1.0 / self.fft_len as f32;
        for (dst, &src) in shifted.iter_mut().take(g).zip(self.buf_b.iter()) {
            *dst = src * inv_n;
        }
    }

    /// Warps the spectral formant envelope of `grain` by `formant_st` semitones in-place.
    pub fn shift_formants(&mut self, grain: &mut [f32], formant_st: f32) {
        if formant_st.abs() < 0.05 {
            return;
        }
        let g = grain.len().min(self.fft_len);
        self.buf_a.fill(0.0);
        self.buf_a[..g].copy_from_slice(&grain[..g]);
        let _ = self.rfft.process_with_scratch(
            &mut self.buf_a,
            &mut self.spec_a,
            &mut self.fwd_scratch,
        );
        smooth_log_env(
            &self.spec_a,
            &mut self.env_a,
            &mut self.env_tmp,
            self.smooth_radius,
        );

        let scale = 2.0_f32.powf(formant_st.clamp(-6.0, 6.0) / 12.0);
        let n_bins = self.spec_a.len();
        for k in 0..n_bins {
            let src_k = (k as f32 / scale).clamp(0.0, (n_bins - 1) as f32);
            let i0 = src_k.floor() as usize;
            let i1 = (i0 + 1).min(n_bins - 1);
            let f = src_k - i0 as f32;
            let target_env = self.env_a[i0] * (1.0 - f) + self.env_a[i1] * f;
            let gain = (target_env - self.env_a[k]).clamp(-1.5, 1.5).exp();
            self.spec_a[k] *= gain;
        }
        zero_dc_nyquist_imag(&mut self.spec_a);
        let _ = self.irfft.process_with_scratch(
            &mut self.spec_a,
            &mut self.buf_a,
            &mut self.inv_scratch,
        );
        let inv_n = 1.0 / self.fft_len as f32;
        for (dst, &src) in grain.iter_mut().take(g).zip(self.buf_a.iter()) {
            *dst = src * inv_n;
        }
    }
}

fn zero_dc_nyquist_imag(spec: &mut [Complex<f32>]) {
    if let Some(first) = spec.first_mut() {
        first.im = 0.0;
    }
    if let Some(last) = spec.last_mut() {
        last.im = 0.0;
    }
}

fn smooth_log_env(spec: &[Complex<f32>], out: &mut [f32], tmp: &mut [f32], radius: usize) {
    let n = spec.len();
    for k in 0..n {
        out[k] = (spec[k].norm() + 1e-5).ln();
    }
    box_smooth(out, tmp, radius);
    box_smooth(tmp, out, radius);
}

fn box_smooth(src: &[f32], dst: &mut [f32], radius: usize) {
    let n = src.len();
    for k in 0..n {
        let lo = k.saturating_sub(radius);
        let hi = (k + radius).min(n - 1);
        let mut sum = 0.0_f32;
        for &v in &src[lo..=hi] {
            sum += v;
        }
        dst[k] = sum / (hi - lo + 1) as f32;
    }
}

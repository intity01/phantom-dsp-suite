//! STFT acoustic feature extraction (RMS, spectral flux, ZCR, sibilance, 20-band Mel & 13 MFCCs).

use std::f32::consts::PI;

use realfft::RealFftPlanner;
use serde::{Deserialize, Serialize};

pub const FRAME_LEN: usize = 1024;
pub const HOP_SEC: f32 = 0.010;
pub const NUM_MEL_BANDS: usize = 20;
pub const NUM_MFCC: usize = 13;

/// Per-frame acoustic descriptors computed on a 10 ms hop grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeatureFrame {
    pub time_sec: f32,
    pub rms_db: f32,
    pub flux: f32,
    pub zcr: f32,
    pub sibilance_ratio: f32,
    pub log_mel: [f32; NUM_MEL_BANDS],
    pub mfcc: [f32; NUM_MFCC],
}

/// Time-ordered sequence of `FeatureFrame`s for a mono audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTrack {
    pub sample_rate: u32,
    pub hop_samples: usize,
    pub frame_len: usize,
    pub frames: Vec<FeatureFrame>,
}

/// Computes the 10 ms hop size in samples, neatly clamped for edge sample rates.
pub fn hop_samples_for_sr(sample_rate: u32) -> usize {
    let sr = sample_rate.max(8000) as f32;
    ((sr * HOP_SEC).round() as usize).clamp(64, 2048)
}

/// Extracts centered STFT features (`FRAME_LEN = 1024`, ~10 ms hop) from mono audio.
pub fn extract_features(mono: &[f32], sample_rate: u32) -> FeatureTrack {
    let sr = sample_rate.max(8000);
    let hop = hop_samples_for_sr(sr);
    if mono.is_empty() {
        return FeatureTrack {
            sample_rate: sr,
            hop_samples: hop,
            frame_len: FRAME_LEN,
            frames: Vec::new(),
        };
    }

    let num_frames = (mono.len() + hop - 1) / hop;
    let n_bins = FRAME_LEN / 2 + 1;
    let hann = build_hann(FRAME_LEN);
    let mel_bank = build_mel_filterbank(sr as f32, FRAME_LEN);
    let dct = build_dct_matrix();

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FRAME_LEN);
    let mut time_buf = vec![0.0_f32; FRAME_LEN];
    let mut spec_buf = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut prev_log_mag = vec![0.0_f32; n_bins];
    let mut frames = Vec::with_capacity(num_frames);

    let bin_hz = sr as f32 / FRAME_LEN as f32;
    let low_bin = ((80.0 / bin_hz).floor() as usize).clamp(1, n_bins - 1);
    let sib_lo = ((4000.0 / bin_hz).floor() as usize).clamp(1, n_bins - 1);
    let sib_hi = ((11000.0 / bin_hz).ceil() as usize).clamp(sib_lo, n_bins - 1);

    let half = (FRAME_LEN / 2) as isize;
    for i in 0..num_frames {
        let center = (i * hop) as isize;
        let mut sq_sum = 0.0_f32;
        let mut zcr_count = 0usize;
        let mut prev_s = 0.0_f32;

        for n in 0..FRAME_LEN {
            let idx = center - half + n as isize;
            let raw = if idx >= 0 && (idx as usize) < mono.len() {
                mono[idx as usize]
            } else {
                0.0
            };
            sq_sum += raw * raw;
            if n > 0 && ((raw >= 0.0 && prev_s < 0.0) || (raw < 0.0 && prev_s >= 0.0)) {
                zcr_count += 1;
            }
            prev_s = raw;
            time_buf[n] = raw * hann[n];
        }

        let rms = (sq_sum / FRAME_LEN as f32).sqrt();
        let rms_db = if rms > 1e-6 {
            20.0 * rms.log10()
        } else {
            -120.0
        };
        let zcr = zcr_count as f32 / (FRAME_LEN - 1) as f32;

        let _ = fft.process_with_scratch(&mut time_buf, &mut spec_buf, &mut scratch);

        let mut flux_acc = 0.0_f32;
        let mut tot_e = 0.0_f32;
        let mut sib_e = 0.0_f32;
        let mut power = [0.0_f32; FRAME_LEN / 2 + 1];

        for k in 0..n_bins {
            let re = spec_buf[k].re;
            let im = spec_buf[k].im;
            let p = re * re + im * im;
            power[k] = p;
            let mag = p.sqrt();
            let log_m = mag.ln_1p();
            let diff = log_m - prev_log_mag[k];
            if i > 0 && diff > 0.0 {
                flux_acc += diff;
            }
            prev_log_mag[k] = log_m;
            if k >= low_bin && k <= sib_hi {
                tot_e += p;
            }
            if k >= sib_lo && k <= sib_hi {
                sib_e += p;
            }
        }

        let sibilance_ratio = if tot_e > 1e-12 {
            (sib_e / tot_e).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let flux = flux_acc / n_bins as f32;

        let mut log_mel = [0.0_f32; NUM_MEL_BANDS];
        for (m, band) in mel_bank.iter().enumerate() {
            let mut e = 0.0_f32;
            for &(bin, w) in band {
                e += power[bin] * w;
            }
            log_mel[m] = (e + 1e-8).ln();
        }

        let mut mfcc = [0.0_f32; NUM_MFCC];
        for k in 0..NUM_MFCC {
            let mut sum = 0.0_f32;
            for m in 0..NUM_MEL_BANDS {
                sum += log_mel[m] * dct[k][m];
            }
            mfcc[k] = sum;
        }

        frames.push(FeatureFrame {
            time_sec: (i * hop) as f32 / sr as f32,
            rms_db,
            flux,
            zcr,
            sibilance_ratio,
            log_mel,
            mfcc,
        });
    }

    FeatureTrack {
        sample_rate: sr,
        hop_samples: hop,
        frame_len: FRAME_LEN,
        frames,
    }
}

fn build_hann(len: usize) -> Vec<f32> {
    (0..len)
        .map(|n| 0.5 * (1.0 - (2.0 * PI * n as f32 / len as f32).cos()))
        .collect()
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

fn build_mel_filterbank(sr: f32, n_fft: usize) -> Vec<Vec<(usize, f32)>> {
    let n_bins = n_fft / 2 + 1;
    let f_min = 80.0_f32;
    let f_max = (sr * 0.45).min(11000.0).max(f_min + 200.0);
    let m_min = hz_to_mel(f_min);
    let m_max = hz_to_mel(f_max);

    let mut hz_pts = [0.0_f32; NUM_MEL_BANDS + 2];
    for (i, slot) in hz_pts.iter_mut().enumerate() {
        let t = i as f32 / (NUM_MEL_BANDS + 1) as f32;
        *slot = mel_to_hz(m_min + t * (m_max - m_min));
    }

    let mut bands = Vec::with_capacity(NUM_MEL_BANDS);
    for m in 0..NUM_MEL_BANDS {
        let f_lo = hz_pts[m];
        let f_mid = hz_pts[m + 1];
        let f_hi = hz_pts[m + 2];
        let mut weights = Vec::new();
        for k in 0..n_bins {
            let f = k as f32 * sr / n_fft as f32;
            let w = if f >= f_lo && f <= f_mid && f_mid > f_lo {
                (f - f_lo) / (f_mid - f_lo)
            } else if f > f_mid && f <= f_hi && f_hi > f_mid {
                (f_hi - f) / (f_hi - f_mid)
            } else {
                0.0
            };
            if w > 0.0 {
                weights.push((k, w));
            }
        }
        if weights.is_empty() {
            let center_bin = ((f_mid * n_fft as f32 / sr).round() as usize).clamp(1, n_bins - 1);
            weights.push((center_bin, 1.0));
        }
        bands.push(weights);
    }
    bands
}

fn build_dct_matrix() -> [[f32; NUM_MEL_BANDS]; NUM_MFCC] {
    let mut mat = [[0.0_f32; NUM_MEL_BANDS]; NUM_MFCC];
    let norm = (2.0 / NUM_MEL_BANDS as f32).sqrt();
    for (k, row) in mat.iter_mut().enumerate() {
        let scale = if k == 0 {
            (1.0 / NUM_MEL_BANDS as f32).sqrt()
        } else {
            norm
        };
        for (m, cell) in row.iter_mut().enumerate() {
            *cell = scale * (PI * k as f32 * (m as f32 + 0.5) / NUM_MEL_BANDS as f32).cos();
        }
    }
    mat
}

//! Clean-room YIN F0 pitch tracker (`65..1100 Hz`) and harmony-aware pitch target calculator.

use realfft::RealFftPlanner;
use serde::{Deserialize, Serialize};

pub const MIN_PITCH_HZ: f32 = 65.0;
pub const MAX_PITCH_HZ: f32 = 1100.0;

/// Per-frame fundamental frequency estimate from YIN CMNDF tracking.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchFrame {
    pub f0_hz: f32,
    pub confidence: f32,
    pub voiced: bool,
}

/// Policy controlling how dub pitch responds to the reference Guide pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PitchMatchMode {
    Off,
    MatchAllToGuide,
    MatchUnisonOnly,
    MatchUnisonAndTuneHarmony,
}

/// Computes the multiplicative pitch shift ratio to apply to a dub frame.
///
/// - `Off`: always `1.0`.
/// - `MatchAllToGuide`: shifts dub F0 to match `guide_f0`.
/// - `MatchUnisonOnly`: pulls dub toward `guide_f0` only when within `±unison_cents`;
///   keeps harmonies (e.g., `+4 st` major 3rd) untouched (`1.0`).
/// - `MatchUnisonAndTuneHarmony`: pulls unisons to `guide_f0` and snaps harmonies cleanly
///   to their nearest semitone interval relative to `guide_f0`.
pub fn compute_target_pitch_ratio(
    guide_f0: f32,
    dub_f0: f32,
    mode: PitchMatchMode,
    unison_cents: f32,
) -> f32 {
    if mode == PitchMatchMode::Off || guide_f0 <= 1.0 || dub_f0 <= 1.0 {
        return 1.0;
    }
    let cents_diff = 1200.0 * (dub_f0 / guide_f0).log2();
    let ratio = match mode {
        PitchMatchMode::Off => 1.0,
        PitchMatchMode::MatchAllToGuide => guide_f0 / dub_f0,
        PitchMatchMode::MatchUnisonOnly => {
            if cents_diff.abs() <= unison_cents.max(1.0) {
                guide_f0 / dub_f0
            } else {
                1.0
            }
        }
        PitchMatchMode::MatchUnisonAndTuneHarmony => {
            if cents_diff.abs() <= unison_cents.max(1.0) {
                guide_f0 / dub_f0
            } else {
                let nearest_st = (cents_diff / 100.0).round();
                let target_hz = guide_f0 * 2.0_f32.powf(nearest_st / 12.0);
                target_hz / dub_f0
            }
        }
    };
    if ratio.is_finite() {
        ratio.clamp(0.5, 2.0)
    } else {
        1.0
    }
}

/// Tracks fundamental frequency across `mono` at `hop_samples` steps using FFT-accelerated YIN CMNDF,
/// parabolic sub-sample interpolation, and a 3-frame median octave-jump cleaner.
pub fn analyze_pitch(mono: &[f32], sample_rate: u32, hop_samples: usize) -> Vec<PitchFrame> {
    let sr = sample_rate.max(8000) as f32;
    let hop = hop_samples.max(1);
    if mono.is_empty() {
        return Vec::new();
    }

    let num_frames = (mono.len() + hop - 1) / hop;
    let tau_min = ((sr / MAX_PITCH_HZ).floor() as usize).max(2);
    let tau_max = ((sr / MIN_PITCH_HZ).ceil() as usize).max(tau_min + 2);
    let win_len = tau_max.max(512);
    let total_len = win_len + tau_max;
    let fft_len = (total_len + 1).next_power_of_two();

    let mut planner = RealFftPlanner::<f32>::new();
    let rfft = planner.plan_fft_forward(fft_len);
    let irfft = planner.plan_fft_inverse(fft_len);

    let mut buf_a = vec![0.0_f32; fft_len];
    let mut buf_b = vec![0.0_f32; fft_len];
    let mut spec_a = rfft.make_output_vec();
    let mut spec_b = rfft.make_output_vec();
    let mut corr_time = irfft.make_output_vec();
    let mut fwd_scratch = rfft.make_scratch_vec();
    let mut inv_scratch = irfft.make_scratch_vec();
    let mut prefix_sq = vec![0.0_f64; total_len + 1];
    let mut cmndf = vec![1.0_f32; tau_max + 1];

    let mut out = Vec::with_capacity(num_frames);
    let half_w = (total_len / 2) as isize;
    let inv_n = 1.0_f64 / fft_len as f64;

    for i in 0..num_frames {
        let center = (i * hop) as isize;
        buf_a.fill(0.0);
        buf_b.fill(0.0);
        prefix_sq[0] = 0.0;

        for n in 0..total_len {
            let idx = center - half_w + n as isize;
            let s = if idx >= 0 && (idx as usize) < mono.len() {
                mono[idx as usize]
            } else {
                0.0
            };
            if n < win_len {
                buf_a[n] = s;
            }
            buf_b[n] = s;
            prefix_sq[n + 1] = prefix_sq[n] + (s as f64) * (s as f64);
        }

        let e0 = prefix_sq[win_len];
        let rms = (e0 / win_len as f64).sqrt();
        if rms < 0.004 {
            out.push(PitchFrame {
                f0_hz: 0.0,
                confidence: 0.0,
                voiced: false,
            });
            continue;
        }

        let _ = rfft.process_with_scratch(&mut buf_a, &mut spec_a, &mut fwd_scratch);
        let _ = rfft.process_with_scratch(&mut buf_b, &mut spec_b, &mut fwd_scratch);
        for k in 0..spec_a.len() {
            spec_a[k] = spec_a[k].conj() * spec_b[k];
        }
        spec_a[0].im = 0.0;
        if let Some(last) = spec_a.last_mut() {
            last.im = 0.0;
        }
        let _ = irfft.process_with_scratch(&mut spec_a, &mut corr_time, &mut inv_scratch);

        cmndf[0] = 1.0;
        let mut running_sum = 0.0_f64;
        for tau in 1..=tau_max {
            let e_tau = prefix_sq[tau + win_len] - prefix_sq[tau];
            let cross = corr_time[tau] as f64 * inv_n;
            let d = (e0 + e_tau - 2.0 * cross).max(0.0);
            running_sum += d;
            cmndf[tau] = if running_sum > 1e-12 {
                (d * tau as f64 / running_sum) as f32
            } else {
                1.0
            };
        }

        out.push(pick_yin_pitch(&cmndf, tau_min, tau_max, sr));
    }

    clean_octave_jumps(&mut out);
    out
}

fn pick_yin_pitch(cmndf: &[f32], tau_min: usize, tau_max: usize, sr: f32) -> PitchFrame {
    const ABS_THRESHOLD: f32 = 0.15;
    let mut best_tau = None;
    let mut tau = tau_min;
    while tau < tau_max {
        if cmndf[tau] < ABS_THRESHOLD {
            while tau + 1 <= tau_max && cmndf[tau + 1] < cmndf[tau] {
                tau += 1;
            }
            best_tau = Some(tau);
            break;
        }
        tau += 1;
    }

    let chosen = match best_tau {
        Some(t) => t,
        None => {
            let mut min_t = tau_min;
            for t in (tau_min + 1)..=tau_max {
                if cmndf[t] < cmndf[min_t] {
                    min_t = t;
                }
            }
            min_t
        }
    };

    let val = cmndf[chosen];
    let tau_interp = if chosen > tau_min && chosen < tau_max {
        let ym = cmndf[chosen - 1];
        let y0 = cmndf[chosen];
        let yp = cmndf[chosen + 1];
        let denom = 2.0 * (ym - 2.0 * y0 + yp);
        if denom.abs() > 1e-12 {
            chosen as f32 + ((ym - yp) / denom).clamp(-0.5, 0.5)
        } else {
            chosen as f32
        }
    } else {
        chosen as f32
    };

    let f0 = if tau_interp > 0.0 {
        sr / tau_interp
    } else {
        0.0
    };
    let confidence = (1.0 - val).clamp(0.0, 1.0);
    let voiced = val < 0.28 && (MIN_PITCH_HZ..=MAX_PITCH_HZ).contains(&f0);
    PitchFrame {
        f0_hz: if voiced { f0 } else { 0.0 },
        confidence,
        voiced,
    }
}

/// 3-frame median cleaner eliminating isolated octave jumps on voiced trajectories.
fn clean_octave_jumps(frames: &mut [PitchFrame]) {
    if frames.len() < 3 {
        return;
    }
    let orig = frames.to_vec();
    for i in 1..(frames.len() - 1) {
        let (a, b, c) = (orig[i - 1], orig[i], orig[i + 1]);
        if !(a.voiced && b.voiced && c.voiced) {
            continue;
        }
        let mut vals = [a.f0_hz, b.f0_hz, c.f0_hz];
        vals.sort_by(|x, y| x.total_cmp(y));
        let med = vals[1];
        let ratio = (b.f0_hz / med).max(med / b.f0_hz);
        if ratio > 1.35 {
            frames[i].f0_hz = med;
        }
    }
}

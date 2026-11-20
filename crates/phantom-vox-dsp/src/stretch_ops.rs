//! Helper routines for WSOLA phase correlation, smoothed level matching, and ProtectedRegion bypass.

use crate::dtw_align::ProtectedRegion;
use crate::wav_io::AudioBuffer;

/// WSOLA normalized cross-correlation phase matcher around `ideal`.
pub(crate) fn wsola_best_pos(
    mono: &[f32],
    natural: f32,
    ideal: f32,
    overlap: usize,
    rad: usize,
) -> f32 {
    let n = mono.len() as isize;
    let nat_i = natural.round() as isize;
    let id_i = ideal.round() as isize;
    let half_ov = (overlap / 2) as isize;
    let cmp_len = overlap.min(512);

    let score = |cand: isize| -> f32 {
        let mut dot = 0.0_f32;
        let mut ea = 0.0_f32;
        let mut eb = 0.0_f32;
        for k in (0..cmp_len).step_by(2) {
            let ia = nat_i - half_ov + k as isize;
            let ib = cand - half_ov + k as isize;
            let a = if ia >= 0 && ia < n {
                mono[ia as usize]
            } else {
                0.0
            };
            let b = if ib >= 0 && ib < n {
                mono[ib as usize]
            } else {
                0.0
            };
            dot += a * b;
            ea += a * a;
            eb += b * b;
        }
        let denom = (ea * eb).sqrt();
        if denom > 1e-8 {
            dot / denom
        } else {
            0.0
        }
    };

    let mut best_off = 0isize;
    let mut best_score = f32::NEG_INFINITY;
    let r = rad as isize;
    let mut off = -r;
    while off <= r {
        let sc = score(id_i + off);
        if sc > best_score {
            best_score = sc;
            best_off = off;
        }
        off += 2;
    }
    for delta in [-1isize, 1isize] {
        let cand_off = (best_off + delta).clamp(-r, r);
        let sc = score(id_i + cand_off);
        if sc > best_score {
            best_score = sc;
            best_off = cand_off;
        }
    }
    (ideal + best_off as f32).clamp(0.0, (mono.len().saturating_sub(1)) as f32)
}

/// Applies smooth vocal-rider envelope level matching from `guide` onto rendered output.
pub(crate) fn apply_smoothed_level_match(
    guide: &AudioBuffer,
    out_l: &mut [f32],
    out_r: &mut [f32],
    stereo: bool,
    sr: f32,
    strength: f32,
) {
    let g_mono = guide.to_mono();
    let n = out_l.len();
    let env_coeff = (-1.0 / (0.025 * sr)).exp();
    let gain_coeff = (-1.0 / (0.060 * sr)).exp();
    let mut g_sq = 0.0_f32;
    let mut d_sq = 0.0_f32;
    let mut smooth_gain = 1.0_f32;

    for i in 0..n {
        let gs = *g_mono.get(i).unwrap_or(&0.0);
        let ds = if stereo {
            0.5 * (out_l[i] + out_r[i])
        } else {
            out_l[i]
        };
        g_sq = env_coeff * g_sq + (1.0 - env_coeff) * gs * gs;
        d_sq = env_coeff * d_sq + (1.0 - env_coeff) * ds * ds;

        let target = if g_sq > 6e-5 && d_sq > 6e-5 {
            (g_sq / d_sq).sqrt().clamp(0.25, 4.0).powf(strength)
        } else {
            1.0
        };
        smooth_gain = gain_coeff * smooth_gain + (1.0 - gain_coeff) * target;
        out_l[i] *= smooth_gain;
        if stereo {
            out_r[i] *= smooth_gain;
        }
    }
}

/// Preserves exact bit-for-bit original `dub` samples inside every `ProtectedRegion`
/// with tiny exterior 3 ms boundary crossfades to prevent boundary clicks.
pub(crate) fn apply_protected_regions(
    dub: &AudioBuffer,
    out_l: &mut [f32],
    out_r: &mut [f32],
    stereo: bool,
    sr: f32,
    protected: &[ProtectedRegion],
) {
    let n = out_l.len().min(dub.len());
    let xf = ((0.003 * sr).round() as usize).max(1);

    for p in protected {
        if p.end_sec <= p.start_sec {
            continue;
        }
        let s = ((p.start_sec.max(0.0) * sr).round() as usize).min(n);
        let e = ((p.end_sec.max(0.0) * sr).round() as usize).min(n);
        if e <= s {
            continue;
        }
        let pre_start = s.saturating_sub(xf);
        let pre_len = s - pre_start;
        for k in 0..pre_len {
            let i = pre_start + k;
            let t = (k + 1) as f32 / (pre_len + 1) as f32;
            out_l[i] = out_l[i] * (1.0 - t) + dub.samples_l[i] * t;
            if stereo {
                out_r[i] = out_r[i] * (1.0 - t) + dub.samples_r[i] * t;
            }
        }
        out_l[s..e].copy_from_slice(&dub.samples_l[s..e]);
        if stereo {
            out_r[s..e].copy_from_slice(&dub.samples_r[s..e]);
        }
        let post_end = (e + xf).min(n);
        let post_len = post_end - e;
        for k in 0..post_len {
            let i = e + k;
            let t = (k + 1) as f32 / (post_len + 1) as f32;
            out_l[i] = dub.samples_l[i] * (1.0 - t) + out_l[i] * t;
            if stereo {
                out_r[i] = dub.samples_r[i] * (1.0 - t) + out_r[i] * t;
            }
        }
    }
}

//! Slope-constrained piecewise DTW path solver, smoothing, and monotonic rate enforcement.

use crate::features::FeatureFrame;
use crate::segmentation::FrameClass;

/// Per-frame normalized acoustic summary used for pitch-invariant DTW matching.
#[derive(Clone, Copy)]
pub(crate) struct NormFrame {
    pub activity: f32,
    pub flux: f32,
    pub sibilance: f32,
    pub mfcc_unit: [f32; 12],
    pub class: FrameClass,
}

pub(crate) fn build_norm_frames(frames: &[FeatureFrame], classes: &[FrameClass]) -> Vec<NormFrame> {
    let peak_db = frames.iter().map(|f| f.rms_db).fold(-120.0_f32, f32::max);
    let floor_db = (peak_db - 40.0).clamp(-60.0, -32.0);
    let span = (peak_db - floor_db).max(6.0);

    frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let activity = ((f.rms_db - floor_db) / span).clamp(0.0, 1.0);
            let mut mfcc_unit = [0.0_f32; 12];
            let mut norm_sq = 0.0_f32;
            for k in 0..12 {
                let v = f.mfcc[k + 1];
                mfcc_unit[k] = v;
                norm_sq += v * v;
            }
            let inv = if norm_sq > 1e-10 {
                norm_sq.sqrt().recip()
            } else {
                0.0
            };
            for v in &mut mfcc_unit {
                *v *= inv;
            }
            NormFrame {
                activity,
                flux: f.flux.min(2.0),
                sibilance: f.sibilance_ratio,
                mfcc_unit,
                class: classes.get(i).copied().unwrap_or(FrameClass::Silence),
            }
        })
        .collect()
}

/// Cosine similarity in `[-1.0, 1.0]` over L2-normalized MFCC[1..13].
#[inline]
pub(crate) fn mfcc_cosine(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let mut dot = 0.0_f32;
    for k in 0..12 {
        dot += a[k] * b[k];
    }
    dot.clamp(-1.0, 1.0)
}

/// Pitch-invariant local frame dissimilarity cost between guide frame `g` and dub frame `d`.
#[inline]
pub(crate) fn frame_cost(g: &NormFrame, d: &NormFrame) -> f32 {
    let g_sil = g.class == FrameClass::Silence || g.activity < 0.06;
    let d_sil = d.class == FrameClass::Silence || d.activity < 0.06;
    if g_sil && d_sil {
        return 0.02;
    }
    if g_sil != d_sil {
        let act = g.activity.max(d.activity);
        return 1.0 + 1.4 * act;
    }
    let mfcc_dist = 1.0 - mfcc_cosine(&g.mfcc_unit, &d.mfcc_unit);
    let env_dist = (g.activity - d.activity).abs();
    let flux_dist = (g.flux - d.flux).abs().min(1.5);
    let sib_dist = (g.sibilance - d.sibilance).abs();
    let cls_pen = if g.class == d.class {
        0.0
    } else if matches!(g.class, FrameClass::Sibilance) != matches!(d.class, FrameClass::Sibilance) {
        0.45
    } else {
        0.12
    };
    0.45 * mfcc_dist + 0.35 * env_dist + 0.30 * flux_dist + 0.25 * sib_dist + 0.15 * cls_pen
}

/// Solves a slope-constrained `[0.5, 2.0]` DTW segment on guide frames `g0..=g1`.
/// Returns a vector of length `g1 - g0 + 1` mapping each guide frame to a dub frame index.
pub(crate) fn solve_dtw_segment(
    g: &[NormFrame],
    d: &[NormFrame],
    g0: usize,
    g1: usize,
    d0: usize,
    d1: usize,
    pin_start: bool,
    pin_end: bool,
) -> Vec<f32> {
    let len = g1 - g0 + 1;
    let m = d.len();
    if len == 0 || m == 0 {
        return Vec::new();
    }
    if len == 1 {
        return vec![d0.min(m - 1) as f32];
    }

    let d0 = d0.min(m - 1);
    let d1 = d1.min(m - 1);
    let band = 80usize.max((len / 3).min(140));
    let inf = f32::INFINITY;
    let mut dp = vec![inf; len * m];
    let mut back = vec![0u8; len * m]; // 1=(1,1), 2=(1,2), 3=(2,1), 4=(1,0 boundary)

    let idx = |r: usize, c: usize| r * m + c;

    if pin_start {
        dp[idx(0, d0)] = frame_cost(&g[g0], &d[d0]);
    } else {
        let lo = d0.saturating_sub(band);
        let hi = (d0 + band).min(m - 1);
        for j in lo..=hi {
            dp[idx(0, j)] = frame_cost(&g[g0], &d[j]) + 0.004 * (j as f32 - d0 as f32).abs();
        }
    }

    for r in 1..len {
        let gi = g0 + r;
        let j_ref = if pin_start && pin_end {
            let t = r as f32 / (len - 1) as f32;
            (d0 as f32 + t * (d1 as f32 - d0 as f32)).round() as usize
        } else if pin_start {
            (d0 + r).min(m - 1)
        } else if pin_end {
            d1.saturating_sub((len - 1) - r)
        } else {
            (d0 + r).min(m - 1)
        };
        let lo = j_ref.saturating_sub(band);
        let hi = (j_ref + band).min(m - 1);

        for j in lo..=hi {
            let c_ij = frame_cost(&g[gi], &d[j]);
            let mut best = inf;
            let mut step = 0u8;

            // (1, 1): slope 1.0
            if j >= 1 {
                let cand = dp[idx(r - 1, j - 1)] + c_ij;
                if cand < best {
                    best = cand;
                    step = 1;
                }
            }
            // (1, 2): slope 2.0 (compression of dub)
            if j >= 2 {
                let prev = dp[idx(r - 1, j - 2)];
                if prev.is_finite() {
                    let cand = prev + 0.5 * (frame_cost(&g[gi], &d[j - 1]) + c_ij) + 0.075;
                    if cand < best {
                        best = cand;
                        step = 2;
                    }
                }
            }
            // (2, 1): slope 0.5 (stretch of dub)
            if r >= 2 && j >= 1 {
                let prev = dp[idx(r - 2, j - 1)];
                if prev.is_finite() {
                    let cand = prev + frame_cost(&g[gi - 1], &d[j]) + c_ij + 0.075;
                    if cand < best {
                        best = cand;
                        step = 3;
                    }
                }
            }
            // Boundary clamp at dub start (0) or dub end (m - 1)
            if j == 0 || j == m - 1 {
                let prev = dp[idx(r - 1, j)];
                if prev.is_finite() {
                    let cand = prev + c_ij + 0.05;
                    if cand < best {
                        best = cand;
                        step = 4;
                    }
                }
            }

            dp[idx(r, j)] = best;
            back[idx(r, j)] = step;
        }
    }

    let end_j = if pin_end {
        if dp[idx(len - 1, d1)].is_finite() {
            Some(d1)
        } else {
            None
        }
    } else {
        let j_ref = (d0 + len - 1).min(m - 1);
        let lo = j_ref.saturating_sub(band);
        let hi = (j_ref + band).min(m - 1);
        let mut best_j = None;
        let mut best_cost = inf;
        for j in lo..=hi {
            let v = dp[idx(len - 1, j)] + 0.004 * (j as f32 - j_ref as f32).abs();
            if v < best_cost {
                best_cost = v;
                best_j = Some(j);
            }
        }
        best_j
    };

    let Some(mut cur_j) = end_j else {
        // Fallback linear interpolation if extreme anchors exceed [0.5, 2.0] slope feasibility.
        return (0..len)
            .map(|r| {
                let t = r as f32 / (len - 1) as f32;
                d0 as f32 + t * (d1 as f32 - d0 as f32)
            })
            .collect();
    };

    let mut path = vec![0.0_f32; len];
    let mut cur_r = len - 1;
    path[cur_r] = cur_j as f32;

    while cur_r > 0 {
        let step = back[idx(cur_r, cur_j)];
        match step {
            1 => {
                cur_r -= 1;
                cur_j = cur_j.saturating_sub(1);
                path[cur_r] = cur_j as f32;
            }
            2 => {
                cur_r -= 1;
                cur_j = cur_j.saturating_sub(2);
                path[cur_r] = cur_j as f32;
            }
            3 => {
                let mid = (cur_j as f32 - 0.5).max(0.0);
                path[cur_r - 1] = mid;
                cur_r -= 2;
                cur_j = cur_j.saturating_sub(1);
                path[cur_r] = cur_j as f32;
            }
            4 => {
                cur_r -= 1;
                path[cur_r] = cur_j as f32;
            }
            _ => {
                cur_r -= 1;
                path[cur_r] = cur_j as f32;
            }
        }
    }
    path
}

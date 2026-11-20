//! Anchor & ProtectedRegion constraint pinning, piecewise DTW dispatch, and path smoothing.

use crate::dtw_align::{ProtectedRegion, SyncAnchor};
use crate::dtw_core::{solve_dtw_segment, NormFrame};

pub(crate) fn build_protected_mask(
    protected: &[ProtectedRegion],
    n: usize,
    hop_sec: f32,
    max_g_sec: f32,
) -> (Vec<bool>, Vec<(usize, usize)>) {
    let mut mask = vec![false; n];
    let mut spans = Vec::new();
    for p in protected {
        if p.end_sec <= p.start_sec {
            continue;
        }
        let s = ((p.start_sec.max(0.0) / hop_sec).floor() as usize).min(n - 1);
        let e = ((p.end_sec.min(max_g_sec) / hop_sec).ceil() as usize).min(n - 1);
        if e > s {
            mask[s..=e].fill(true);
            spans.push((s, e));
        }
    }
    (mask, spans)
}

pub(crate) fn collect_pins(
    anchors: &[SyncAnchor],
    clean_prot: &[(usize, usize)],
    is_protected: &[bool],
    n: usize,
    hop_sec: f32,
    max_g_sec: f32,
    max_d_sec: f32,
) -> (Vec<SyncAnchor>, Vec<(usize, f32)>) {
    let mut pins: Vec<(usize, f32)> = Vec::new();
    for &(s, e) in clean_prot {
        pins.push((s, (s as f32 * hop_sec).min(max_d_sec)));
        pins.push((e, (e as f32 * hop_sec).min(max_d_sec)));
    }
    let mut sorted: Vec<SyncAnchor> = anchors
        .iter()
        .copied()
        .filter(|a| a.guide_sec.is_finite() && a.dub_sec.is_finite())
        .collect();
    sorted.sort_by(|a, b| a.guide_sec.total_cmp(&b.guide_sec));
    for a in &sorted {
        let gf = ((a.guide_sec.clamp(0.0, max_g_sec) / hop_sec).round() as usize).min(n - 1);
        if !is_protected[gf] {
            pins.push((gf, a.dub_sec.clamp(0.0, max_d_sec)));
        }
    }
    pins.sort_by_key(|p| p.0);
    let mut mono: Vec<(usize, f32)> = Vec::new();
    for (gf, dsec) in pins {
        if let Some(last) = mono.last_mut() {
            if last.0 == gf {
                last.1 = dsec;
                continue;
            }
            if dsec < last.1 {
                continue;
            }
        }
        mono.push((gf, dsec));
    }
    (sorted, mono)
}

pub(crate) fn solve_piecewise(
    g_norm: &[NormFrame],
    d_norm: &[NormFrame],
    pins: &[(usize, f32)],
    is_protected: &[bool],
    map_sec: &mut [f32],
    hop_sec: f32,
    max_d_sec: f32,
) {
    let (n, m) = (g_norm.len(), d_norm.len());
    let (fg, fd) = pins[0];
    let fdf = ((fd / hop_sec).round() as usize).min(m - 1);
    if fg > 0 {
        for (k, &df) in solve_dtw_segment(g_norm, d_norm, 0, fg, 0, fdf, false, true)
            .iter()
            .enumerate()
        {
            map_sec[k] = df * hop_sec;
        }
    }
    map_sec[fg] = fd;

    for w in pins.windows(2) {
        let ((g0, d0), (g1, d1)) = (w[0], w[1]);
        if (g0..=g1).all(|idx| is_protected[idx]) {
            for (i, slot) in map_sec.iter_mut().enumerate().take(g1 + 1).skip(g0) {
                *slot = (i as f32 * hop_sec).min(max_d_sec);
            }
        } else {
            let df0 = ((d0 / hop_sec).round() as usize).min(m - 1);
            let df1 = ((d1 / hop_sec).round() as usize).min(m - 1);
            for (k, &df) in solve_dtw_segment(g_norm, d_norm, g0, g1, df0, df1, true, true)
                .iter()
                .enumerate()
            {
                map_sec[g0 + k] = df * hop_sec;
            }
            map_sec[g0] = d0;
            map_sec[g1] = d1;
        }
    }

    let (lg, ld) = *pins.last().unwrap();
    let ldf = ((ld / hop_sec).round() as usize).min(m - 1);
    if lg + 1 < n {
        for (k, &df) in solve_dtw_segment(g_norm, d_norm, lg, n - 1, ldf, m - 1, true, false)
            .iter()
            .enumerate()
        {
            map_sec[lg + k] = df * hop_sec;
        }
    }
    map_sec[lg] = ld;
}

/// Applies light 2-pass smoothing on non-pinned frames and enforces bounded slope `[0.5, 2.0]`.
pub(crate) fn smooth_and_constrain_map(
    map: &mut [f32],
    pinned: &[bool],
    hop_sec: f32,
    max_dub_sec: f32,
) {
    let n = map.len();
    if n < 3 {
        return;
    }
    for _ in 0..2 {
        let prev = map.to_vec();
        for i in 1..(n - 1) {
            if !pinned[i] {
                map[i] = 0.25 * prev[i - 1] + 0.50 * prev[i] + 0.25 * prev[i + 1];
            }
        }
    }

    let min_step = 0.5 * hop_sec;
    let max_step = 2.0 * hop_sec;
    for i in 1..n {
        if !pinned[i] {
            let lo = map[i - 1] + min_step;
            let hi = (map[i - 1] + max_step).min(max_dub_sec);
            map[i] = map[i].clamp(lo.min(hi), hi);
        }
    }
    for i in (0..(n - 1)).rev() {
        if !pinned[i] {
            let lo = (map[i + 1] - max_step).max(0.0);
            let hi = (map[i + 1] - min_step).max(0.0);
            map[i] = map[i].clamp(lo.min(hi), hi);
        }
    }
}

pub(crate) fn enforce_exact_pins(
    map_sec: &mut [f32],
    is_protected: &[bool],
    anchors: &[SyncAnchor],
    hop_sec: f32,
    max_g_sec: f32,
    max_d_sec: f32,
) {
    let n = map_sec.len();
    for i in 0..n {
        if is_protected[i] {
            map_sec[i] = (i as f32 * hop_sec).min(max_d_sec);
        }
    }
    for a in anchors {
        let pos = a.guide_sec.clamp(0.0, max_g_sec) / hop_sec;
        let i0 = (pos.floor() as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        if !is_protected[i0] && !is_protected[i1] {
            let frac = pos - i0 as f32;
            let cur = map_sec[i0] * (1.0 - frac) + map_sec[i1] * frac;
            let delta = a.dub_sec.clamp(0.0, max_d_sec) - cur;
            map_sec[i0] = (map_sec[i0] + delta).clamp(0.0, max_d_sec);
            if i1 != i0 {
                map_sec[i1] = (map_sec[i1] + delta).clamp(0.0, max_d_sec);
            }
        }
    }
}

//! Frame and phrase-level alignment confidence scoring and diagnostic warnings.

use crate::dtw_align::{ConfidenceLevel, PhraseReport, TimeMap};
use crate::dtw_core::{mfcc_cosine, NormFrame};
use crate::segmentation::{FrameClass, PhraseSpan};

pub(crate) fn evaluate_confidence(
    g: &[NormFrame],
    d: &[NormFrame],
    time_map: &TimeMap,
    spans: &[PhraseSpan],
) -> (Vec<f32>, Vec<PhraseReport>, f32) {
    let n = g.len();
    let m = d.len();
    let mut conf = vec![0.85_f32; n];
    let mut mfcc_sims = vec![1.0_f32; n];
    let mut dub_silent = vec![false; n];

    for i in 0..n {
        let g_sec = i as f32 * time_map.hop_sec;
        let d_sec = time_map.guide_to_dub_sec[i];
        let j = ((d_sec / time_map.hop_sec).round() as usize).min(m - 1);
        let gf = &g[i];
        let df = &d[j];
        let g_sil = gf.class == FrameClass::Silence || gf.activity < 0.08;
        let d_sil = df.class == FrameClass::Silence || df.activity < 0.08;
        dub_silent[i] = d_sil;

        if g_sil {
            conf[i] = 0.85;
            continue;
        }
        if d_sil {
            conf[i] = 0.05;
            mfcc_sims[i] = 0.0;
            continue;
        }
        let cos_sim = (mfcc_cosine(&gf.mfcc_unit, &df.mfcc_unit) + 1.0) * 0.5;
        mfcc_sims[i] = cos_sim;
        let env_sim = 1.0 - (gf.activity - df.activity).abs();
        let cls_sim = if gf.class == df.class { 1.0 } else { 0.65 };
        let rate = time_map.local_rate_at_guide(g_sec);
        let rate_pen = if !(0.60..=1.65).contains(&rate) {
            0.72
        } else {
            1.0
        };
        conf[i] = ((0.45 * cos_sim + 0.35 * env_sim + 0.20 * cls_sim) * rate_pen).clamp(0.0, 1.0);
    }

    let mut reports = Vec::with_capacity(spans.len());
    let mut weighted_sum = 0.0_f32;
    let mut weight_tot = 0.0_f32;

    for sp in spans {
        let s = sp.start_frame.min(n);
        let e = sp.end_frame.min(n);
        if e <= s {
            continue;
        }
        let report = score_phrase(g, time_map, &conf, &mfcc_sims, &dub_silent, sp, s, e);
        let count = (e - s) as f32;
        weighted_sum += report.confidence * count;
        weight_tot += count;
        reports.push(report);
    }

    let overall = if weight_tot > 0.0 {
        weighted_sum / weight_tot
    } else if !conf.is_empty() {
        conf.iter().sum::<f32>() / conf.len() as f32
    } else {
        0.0
    };
    (conf, reports, overall)
}

#[allow(clippy::too_many_arguments)]
fn score_phrase(
    g: &[NormFrame],
    time_map: &TimeMap,
    conf: &[f32],
    mfcc_sims: &[f32],
    dub_silent: &[bool],
    sp: &PhraseSpan,
    s: usize,
    e: usize,
) -> PhraseReport {
    let count = (e - s) as f32;
    let mut c_sum = 0.0_f32;
    let mut active_cnt = 0.0_f32;
    let mut missing_cnt = 0.0_f32;
    let mut sim_sum = 0.0_f32;
    let mut extreme_cnt = 0.0_f32;

    for i in s..e {
        c_sum += conf[i];
        if g[i].class != FrameClass::Silence && g[i].activity >= 0.08 {
            active_cnt += 1.0;
            if dub_silent[i] {
                missing_cnt += 1.0;
            } else {
                sim_sum += mfcc_sims[i];
            }
        }
        let rate = time_map.local_rate_at_guide(i as f32 * time_map.hop_sec);
        if !(0.60..=1.65).contains(&rate) {
            extreme_cnt += 1.0;
        }
    }

    let mut phrase_conf = c_sum / count;
    let miss_ratio = if active_cnt > 0.0 {
        missing_cnt / active_cnt
    } else {
        0.0
    };
    let present_cnt = (active_cnt - missing_cnt).max(1.0);
    let mean_sim = sim_sum / present_cnt;
    let stretch_ratio = extreme_cnt / count;

    let mut warning = None;
    if miss_ratio > 0.35 {
        phrase_conf = phrase_conf.min(0.25);
        warning = Some("Missing or silent vocal phrase in dub track".to_string());
    } else if mean_sim < 0.50 {
        phrase_conf = phrase_conf.min(0.32);
        warning = Some("Mismatched vocal timbre or lyrics between guide and dub".to_string());
    } else if stretch_ratio > 0.25 {
        phrase_conf = phrase_conf.min(0.55);
        warning = Some("Extreme time-stretch detected — add a manual sync anchor".to_string());
    }

    let level = if phrase_conf >= 0.70 {
        ConfidenceLevel::High
    } else if phrase_conf >= 0.42 {
        ConfidenceLevel::Medium
    } else {
        if warning.is_none() {
            warning = Some("Low alignment confidence in this phrase".to_string());
        }
        ConfidenceLevel::Low
    };

    PhraseReport {
        start_sec: sp.start_sec,
        end_sec: sp.end_sec,
        confidence: phrase_conf,
        level,
        warning,
    }
}

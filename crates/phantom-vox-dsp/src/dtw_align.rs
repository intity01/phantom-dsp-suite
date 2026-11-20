//! Piecewise DTW vocal alignment with manual SyncAnchors, ProtectedRegions, and Phrase diagnostics.

use serde::{Deserialize, Serialize};

use crate::confidence::evaluate_confidence;
use crate::dtw_core::{build_norm_frames, solve_dtw_segment};
use crate::dtw_pins::{
    build_protected_mask, collect_pins, enforce_exact_pins, smooth_and_constrain_map,
    solve_piecewise,
};
use crate::features::{extract_features, hop_samples_for_sr};
use crate::pitch::{analyze_pitch, PitchFrame};
use crate::segmentation::{classify_frames, extract_phrases, FrameClass};
use crate::wav_io::AudioBuffer;

/// User-defined manual sync anchor pinning a Guide timestamp to a Dub timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SyncAnchor {
    pub guide_sec: f32,
    pub dub_sec: f32,
}

/// Time span where the dub track remains untouched (`map(t) == t`, exact sample preservation).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProtectedRegion {
    pub start_sec: f32,
    pub end_sec: f32,
}

/// Traffic-light alignment confidence bucket for UI display and warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

/// Per-phrase diagnostic report indicating alignment confidence and actionable warnings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhraseReport {
    pub start_sec: f32,
    pub end_sec: f32,
    pub confidence: f32,
    pub level: ConfidenceLevel,
    pub warning: Option<String>,
}

/// Monotonic time-warping map from Guide time (seconds) to Dub time (seconds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeMap {
    pub hop_sec: f32,
    pub guide_to_dub_sec: Vec<f32>,
}

impl TimeMap {
    /// Returns the mapped Dub timestamp (seconds) corresponding to `guide_sec`.
    pub fn dub_sec_at_guide(&self, guide_sec: f32) -> f32 {
        if self.guide_to_dub_sec.is_empty() || self.hop_sec <= 0.0 {
            return guide_sec.max(0.0);
        }
        let n = self.guide_to_dub_sec.len();
        if guide_sec <= 0.0 {
            return (self.guide_to_dub_sec[0] + guide_sec).max(0.0);
        }
        let pos = guide_sec / self.hop_sec;
        let idx = pos.floor() as usize;
        if idx + 1 >= n {
            let last_g = (n - 1) as f32 * self.hop_sec;
            return (self.guide_to_dub_sec[n - 1] + (guide_sec - last_g)).max(0.0);
        }
        let frac = pos - idx as f32;
        self.guide_to_dub_sec[idx] * (1.0 - frac) + self.guide_to_dub_sec[idx + 1] * frac
    }

    /// Returns smoothed local stretch slope `d(dub)/d(guide)` over a ~60 ms window around `guide_sec`.
    pub fn local_rate_at_guide(&self, guide_sec: f32) -> f32 {
        let half = (self.hop_sec * 3.0).max(0.025);
        let t0 = (guide_sec - half).max(0.0);
        let t1 = guide_sec + half;
        let dt = (t1 - t0).max(0.005);
        let a = self.dub_sec_at_guide(t0);
        let b = self.dub_sec_at_guide(t1);
        ((b - a) / dt).clamp(0.25, 4.0)
    }

    /// Returns signed timing offset `(dub_sec - guide_sec) * 1000.0` in milliseconds.
    pub fn offset_ms_at_guide(&self, guide_sec: f32) -> f32 {
        (self.dub_sec_at_guide(guide_sec) - guide_sec) * 1000.0
    }
}

/// Complete analysis bundle returned by `analyze_alignment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentResult {
    pub time_map: TimeMap,
    pub frame_confidences: Vec<f32>,
    pub phrases: Vec<PhraseReport>,
    pub guide_pitch: Vec<PitchFrame>,
    pub dub_pitch: Vec<PitchFrame>,
    pub guide_classes: Vec<FrameClass>,
    pub dub_classes: Vec<FrameClass>,
    pub overall_confidence: f32,
}

/// Analyzes `guide` and `dub` tracks to produce a monotonic, anchor-pinned, protected-region-safe `AlignmentResult`.
pub fn analyze_alignment(
    guide: &AudioBuffer,
    dub: &AudioBuffer,
    anchors: &[SyncAnchor],
    protected: &[ProtectedRegion],
) -> AlignmentResult {
    let sr = guide.sample_rate.max(8000);
    let hop = hop_samples_for_sr(sr);
    let hop_sec = hop as f32 / sr as f32;

    let guide_mono = guide.to_mono();
    let dub_mono = dub.to_mono();
    let g_feat = extract_features(&guide_mono, sr);
    let d_feat = extract_features(&dub_mono, dub.sample_rate.max(8000));
    let guide_pitch = analyze_pitch(&guide_mono, sr, hop);
    let dub_pitch = analyze_pitch(&dub_mono, dub.sample_rate.max(8000), hop);
    let guide_classes = classify_frames(&g_feat, &guide_pitch);
    let dub_classes = classify_frames(&d_feat, &dub_pitch);

    let (n, m) = (g_feat.frames.len(), d_feat.frames.len());
    if n == 0 || m == 0 {
        return AlignmentResult {
            time_map: TimeMap {
                hop_sec,
                guide_to_dub_sec: Vec::new(),
            },
            frame_confidences: Vec::new(),
            phrases: Vec::new(),
            guide_pitch,
            dub_pitch,
            guide_classes,
            dub_classes,
            overall_confidence: 0.0,
        };
    }

    let g_norm = build_norm_frames(&g_feat.frames, &guide_classes);
    let d_norm = build_norm_frames(&d_feat.frames, &dub_classes);
    let max_g_sec = (n - 1) as f32 * hop_sec;
    let max_d_sec = (m - 1) as f32 * hop_sec;

    let (is_protected, clean_prot) = build_protected_mask(protected, n, hop_sec, max_g_sec);
    let (sorted_anchors, mono_pins) = collect_pins(
        anchors,
        &clean_prot,
        &is_protected,
        n,
        hop_sec,
        max_g_sec,
        max_d_sec,
    );

    let mut map_sec = vec![0.0_f32; n];
    let mut pinned = is_protected.clone();
    for &(gf, _) in &mono_pins {
        pinned[gf] = true;
    }

    if mono_pins.is_empty() {
        let path = solve_dtw_segment(&g_norm, &d_norm, 0, n - 1, 0, m - 1, false, false);
        for (i, &df) in path.iter().enumerate() {
            map_sec[i] = df * hop_sec;
        }
    } else {
        solve_piecewise(
            &g_norm,
            &d_norm,
            &mono_pins,
            &is_protected,
            &mut map_sec,
            hop_sec,
            max_d_sec,
        );
    }

    smooth_and_constrain_map(&mut map_sec, &pinned, hop_sec, max_d_sec);
    enforce_exact_pins(
        &mut map_sec,
        &is_protected,
        &sorted_anchors,
        hop_sec,
        max_g_sec,
        max_d_sec,
    );

    let time_map = TimeMap {
        hop_sec,
        guide_to_dub_sec: map_sec,
    };
    let spans = extract_phrases(&g_feat, &guide_classes);
    let (frame_confidences, phrases, overall_confidence) =
        evaluate_confidence(&g_norm, &d_norm, &time_map, &spans);

    AlignmentResult {
        time_map,
        frame_confidences,
        phrases,
        guide_pitch,
        dub_pitch,
        guide_classes,
        dub_classes,
        overall_confidence,
    }
}

//! Vocal frame segmentation (Silence, Voiced, Unvoiced, Sibilance, Breath, Transient) and phrase extraction.

use serde::{Deserialize, Serialize};

use crate::features::FeatureTrack;
use crate::pitch::PitchFrame;

/// Acoustic class assigned to each 10 ms vocal analysis frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameClass {
    Silence,
    Voiced,
    UnvoicedConsonant,
    Sibilance,
    Breath,
    Transient,
}

/// Contiguous vocal phrase span in frame and second coordinates (`end_frame` is exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhraseSpan {
    pub start_frame: usize,
    pub end_frame: usize,
    pub start_sec: f32,
    pub end_sec: f32,
}

/// Classifies every frame in `features` using RMS energy, spectral flux, ZCR, sibilance ratio, and YIN pitch.
pub fn classify_frames(features: &FeatureTrack, pitch: &[PitchFrame]) -> Vec<FrameClass> {
    let n = features.frames.len();
    if n == 0 {
        return Vec::new();
    }

    let peak_db = features
        .frames
        .iter()
        .map(|f| f.rms_db)
        .fold(-120.0_f32, f32::max);
    let silence_db = (peak_db - 38.0).clamp(-56.0, -30.0);

    let mut classes = Vec::with_capacity(n);
    for i in 0..n {
        let f = &features.frames[i];
        let p = pitch.get(i).copied().unwrap_or(PitchFrame {
            f0_hz: 0.0,
            confidence: 0.0,
            voiced: false,
        });
        let prev_db = if i > 0 {
            features.frames[i - 1].rms_db
        } else {
            -120.0
        };

        let cls = if f.rms_db < silence_db {
            FrameClass::Silence
        } else if f.sibilance_ratio > 0.40 && (f.zcr > 0.14 || !p.voiced) {
            FrameClass::Sibilance
        } else if f.flux > 0.32 && (f.rms_db - prev_db) > 6.5 && (!p.voiced || p.confidence < 0.7) {
            FrameClass::Transient
        } else if p.voiced && p.confidence >= 0.50 {
            FrameClass::Voiced
        } else if f.rms_db < peak_db - 18.0 && f.sibilance_ratio > 0.10 && f.zcr > 0.06 {
            FrameClass::Breath
        } else {
            FrameClass::UnvoicedConsonant
        };
        classes.push(cls);
    }

    // Smooth isolated 1-frame spikes surrounded by identical neighbors.
    if classes.len() >= 3 {
        let orig = classes.clone();
        for i in 1..(classes.len() - 1) {
            if orig[i - 1] == orig[i + 1] && orig[i] != orig[i - 1] {
                classes[i] = orig[i - 1];
            }
        }
    }
    classes
}

/// Groups active vocal frames into phrase spans, bridging short gaps (`< 120 ms`)
/// and ignoring tiny blips (`< 60 ms`).
pub fn extract_phrases(features: &FeatureTrack, classes: &[FrameClass]) -> Vec<PhraseSpan> {
    let n = features.frames.len().min(classes.len());
    if n == 0 || features.sample_rate == 0 {
        return Vec::new();
    }

    let hop_sec = features.hop_samples as f32 / features.sample_rate as f32;
    let max_gap_frames = ((0.120 / hop_sec).round() as usize).max(1);
    let min_phrase_frames = ((0.060 / hop_sec).round() as usize).max(2);

    let mut raw: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;

    for (i, &cls) in classes.iter().take(n).enumerate() {
        let active = cls != FrameClass::Silence;
        match (start, active) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                raw.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        raw.push((s, n));
    }

    // Merge consecutive active runs separated by less than `max_gap_frames`.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in raw {
        if let Some(last) = merged.last_mut() {
            if s.saturating_sub(last.1) <= max_gap_frames {
                last.1 = e;
                continue;
            }
        }
        merged.push((s, e));
    }

    merged
        .into_iter()
        .filter(|&(s, e)| e.saturating_sub(s) >= min_phrase_frames)
        .map(|(s, e)| PhraseSpan {
            start_frame: s,
            end_frame: e,
            start_sec: s as f32 * hop_sec,
            end_sec: e as f32 * hop_sec,
        })
        .collect()
}

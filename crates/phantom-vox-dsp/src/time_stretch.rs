//! WSOLA time-warping with sibilance/breath/transient protection, harmony-aware pitch shifting,
//! formant preservation, smoothed level matching, and exact ProtectedRegion bypass.

use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::dtw_align::{AlignmentResult, ProtectedRegion};
use crate::formant::{sample_cubic, FormantProcessor};
use crate::pitch::{compute_target_pitch_ratio, PitchFrame, PitchMatchMode};
use crate::segmentation::FrameClass;
use crate::stretch_ops::{apply_protected_regions, apply_smoothed_level_match, wsola_best_pos};
use crate::wav_io::AudioBuffer;

pub use crate::demo::{create_artificial_double, generate_demo_vocal_stack};

/// Rendering parameters for `render_aligned_track`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AlignConfig {
    pub timing_strength: f32,
    pub pitch_strength: f32,
    pub level_strength: f32,
    pub pitch_mode: PitchMatchMode,
    pub unison_tolerance_cents: f32,
    pub protect_sibilance: bool,
    pub formant_preserve: bool,
}

impl Default for AlignConfig {
    fn default() -> Self {
        Self {
            timing_strength: 1.0,
            pitch_strength: 0.75,
            level_strength: 0.35,
            pitch_mode: PitchMatchMode::MatchUnisonOnly,
            unison_tolerance_cents: 120.0,
            protect_sibilance: true,
            formant_preserve: true,
        }
    }
}

/// Renders an aligned version of `dub` synchronized to `guide` according to `alignment` and `cfg`.
///
/// Inside any `ProtectedRegion`, original `dub` samples are copied bit-for-bit (`max_diff == 0.0`).
pub fn render_aligned_track(
    guide: &AudioBuffer,
    dub: &AudioBuffer,
    alignment: &AlignmentResult,
    cfg: &AlignConfig,
    protected: &[ProtectedRegion],
) -> AudioBuffer {
    let sr = dub.sample_rate.max(8000);
    let sr_f = sr as f32;
    let out_len = guide.len().max(dub.len());
    if out_len == 0 || dub.is_empty() {
        return dub.clone();
    }

    let stereo = dub.channels >= 2 && !dub.samples_r.is_empty();
    let dub_mono = dub.to_mono();
    let g_len = ((0.032 * sr_f).round() as usize).clamp(256, 4096) & !1;
    let hop = g_len / 2;
    let search_rad = (hop / 2).clamp(16, 512);
    let hann: Vec<f32> = (0..g_len)
        .map(|k| 0.5 * (1.0 - (TAU * k as f32 / g_len as f32).cos()))
        .collect();

    let timing_str = cfg.timing_strength.clamp(0.0, 1.0);
    let pitch_str = cfg.pitch_strength.clamp(0.0, 1.0);
    let hop_sec = alignment.time_map.hop_sec.max(0.005);

    let mut out_l = vec![0.0_f32; out_len];
    let mut out_r = if stereo {
        vec![0.0_f32; out_len]
    } else {
        Vec::new()
    };
    let mut norm = vec![0.0_f32; out_len];

    let mut formant = FormantProcessor::new(g_len, sr_f);
    let mut grain_l = vec![0.0_f32; g_len];
    let mut orig_l = vec![0.0_f32; g_len];
    let mut grain_r = if stereo {
        vec![0.0_f32; g_len]
    } else {
        Vec::new()
    };
    let mut orig_r = if stereo {
        vec![0.0_f32; g_len]
    } else {
        Vec::new()
    };

    let mut prev_in_pos = 0.0_f32;
    let mut first_grain = true;
    let mut out_pos = 0usize;

    while out_pos < out_len {
        let t_out = out_pos as f32 / sr_f;
        let mapped_sec = alignment.time_map.dub_sec_at_guide(t_out);
        let target_sec = t_out + timing_str * (mapped_sec - t_out);
        let ideal_in_pos = (target_sec * sr_f).clamp(0.0, (dub.len().saturating_sub(1)) as f32);

        let d_frame = ((ideal_in_pos / sr_f) / hop_sec).round() as usize;
        let d_class = alignment
            .dub_classes
            .get(d_frame)
            .copied()
            .unwrap_or(FrameClass::Silence);
        let protect_grain = cfg.protect_sibilance
            && matches!(
                d_class,
                FrameClass::Sibilance | FrameClass::Breath | FrameClass::Transient
            );

        let actual_in_pos = if first_grain {
            first_grain = false;
            ideal_in_pos
        } else if protect_grain {
            let natural = prev_in_pos + hop as f32;
            let max_drift = (0.012 * sr_f).max(hop as f32 * 0.5);
            natural.clamp(ideal_in_pos - max_drift, ideal_in_pos + max_drift)
        } else {
            let natural = prev_in_pos + hop as f32;
            if (natural - ideal_in_pos).abs() < 1.5 {
                ideal_in_pos
            } else {
                wsola_best_pos(&dub_mono, natural, ideal_in_pos, hop, search_rad)
            }
        };
        prev_in_pos = actual_in_pos;

        let pitch_ratio =
            if protect_grain || pitch_str <= 1e-4 || cfg.pitch_mode == PitchMatchMode::Off {
                1.0
            } else {
                let gf = alignment
                    .guide_pitch
                    .get((t_out / hop_sec).round() as usize)
                    .copied();
                let df = alignment
                    .dub_pitch
                    .get(((actual_in_pos / sr_f) / hop_sec).round() as usize)
                    .copied();
                match (gf, df) {
                    (
                        Some(PitchFrame {
                            voiced: true,
                            f0_hz: g_hz,
                            ..
                        }),
                        Some(PitchFrame {
                            voiced: true,
                            f0_hz: d_hz,
                            ..
                        }),
                    ) => compute_target_pitch_ratio(
                        g_hz,
                        d_hz,
                        cfg.pitch_mode,
                        cfg.unison_tolerance_cents,
                    )
                    .powf(pitch_str),
                    _ => 1.0,
                }
            };

        let do_pitch = (pitch_ratio - 1.0).abs() > 0.002;
        let half = (g_len / 2) as f32;
        for k in 0..g_len {
            let off = k as f32 - half;
            let w = hann[k];
            orig_l[k] = sample_cubic(&dub.samples_l, actual_in_pos + off) * w;
            grain_l[k] = if do_pitch {
                sample_cubic(&dub.samples_l, actual_in_pos + off * pitch_ratio) * w
            } else {
                orig_l[k]
            };
            if stereo {
                orig_r[k] = sample_cubic(&dub.samples_r, actual_in_pos + off) * w;
                grain_r[k] = if do_pitch {
                    sample_cubic(&dub.samples_r, actual_in_pos + off * pitch_ratio) * w
                } else {
                    orig_r[k]
                };
            }
        }

        if do_pitch && cfg.formant_preserve {
            formant.preserve_formants(&orig_l, &mut grain_l);
            if stereo {
                formant.preserve_formants(&orig_r, &mut grain_r);
            }
        }

        for k in 0..g_len {
            let dst = out_pos as isize - (g_len / 2) as isize + k as isize;
            if dst >= 0 && (dst as usize) < out_len {
                let d = dst as usize;
                out_l[d] += grain_l[k];
                if stereo {
                    out_r[d] += grain_r[k];
                }
                norm[d] += hann[k];
            }
        }
        out_pos += hop;
    }

    for i in 0..out_len {
        if norm[i] > 1e-4 {
            let inv = norm[i].recip();
            out_l[i] *= inv;
            if stereo {
                out_r[i] *= inv;
            }
        }
    }

    let level_str = cfg.level_strength.clamp(0.0, 1.0);
    if level_str > 1e-4 && !guide.is_empty() {
        apply_smoothed_level_match(guide, &mut out_l, &mut out_r, stereo, sr_f, level_str);
    }
    apply_protected_regions(dub, &mut out_l, &mut out_r, stereo, sr_f, protected);

    if stereo {
        AudioBuffer::new_stereo(sr, out_l, out_r)
    } else {
        AudioBuffer::new_mono(sr, out_l)
    }
}

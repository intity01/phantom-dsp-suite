//! Offline artificial vocal doubler and 4-track synthetic harmonic vocal stack demo generator.

use std::f32::consts::{PI, TAU};

use crate::formant::{sample_cubic, FormantProcessor};
use crate::wav_io::AudioBuffer;

/// Offline artificial vocal double generator with organic timing offset, pitch detune,
/// optional spectral formant warp, and natural vibrato.
pub fn create_artificial_double(
    source: &AudioBuffer,
    timing_offset_ms: f32,
    detune_cents: f32,
    formant_st: f32,
    vibrato_depth_cents: f32,
) -> AudioBuffer {
    let sr = source.sample_rate.max(8000) as f32;
    let n = source.len();
    if n == 0 {
        return source.clone();
    }
    let g_len = ((0.032 * sr).round() as usize).clamp(256, 4096) & !1;
    let hop = g_len / 2;
    let hann: Vec<f32> = (0..g_len)
        .map(|k| 0.5 * (1.0 - (TAU * k as f32 / g_len as f32).cos()))
        .collect();
    let mut formant = FormantProcessor::new(g_len, sr);
    let mut process_ch = |src: &[f32]| -> Vec<f32> {
        let mut out = vec![0.0_f32; n];
        let mut norm = vec![0.0_f32; n];
        let mut grain = vec![0.0_f32; g_len];
        let mut orig_g = vec![0.0_f32; g_len];
        let base_shift = (timing_offset_ms * 0.001) * sr;

        let mut out_pos = 0usize;
        while out_pos < n {
            let t = out_pos as f32 / sr;
            let wander = 0.0025 * sr * (TAU * 0.45 * t).sin();
            let center_in = out_pos as f32 - base_shift + wander;
            let vib_ct = vibrato_depth_cents * (TAU * 4.8 * t).sin();
            let ratio = 2.0_f32.powf((detune_cents + vib_ct).clamp(-400.0, 400.0) / 1200.0);
            let half = (g_len / 2) as f32;

            for k in 0..g_len {
                let off = k as f32 - half;
                orig_g[k] = sample_cubic(src, center_in + off) * hann[k];
                grain[k] = sample_cubic(src, center_in + off * ratio) * hann[k];
            }
            if (ratio - 1.0).abs() > 0.005 {
                formant.preserve_formants(&orig_g, &mut grain);
            }
            if formant_st.abs() >= 0.05 {
                formant.shift_formants(&mut grain, formant_st);
            }
            for k in 0..g_len {
                let dst = out_pos as isize - (g_len / 2) as isize + k as isize;
                if dst >= 0 && (dst as usize) < n {
                    out[dst as usize] += grain[k];
                    norm[dst as usize] += hann[k];
                }
            }
            out_pos += hop;
        }
        for i in 0..n {
            if norm[i] > 1e-4 {
                out[i] /= norm[i];
            }
        }
        out
    };

    let l = process_ch(&source.samples_l);
    if source.channels >= 2 && !source.samples_r.is_empty() {
        let r = process_ch(&source.samples_r);
        AudioBuffer::new_stereo(source.sample_rate, l, r)
    } else {
        AudioBuffer::new_mono(source.sample_rate, l)
    }
}

/// Generates a realistic 4-track harmonic vocal demo stack `(Guide, Dub_L, Dub_R, Harmony_3rd)`
/// with formant vowels, breaths, and `"s"` sibilance so Studio & tests work out-of-the-box.
pub fn generate_demo_vocal_stack(
    sample_rate: u32,
) -> (AudioBuffer, AudioBuffer, AudioBuffer, AudioBuffer) {
    let sr = sample_rate.max(16000);
    let guide = synth_vocal_take(sr, 0.0, 0.0, 0.0, 1.0, 11);
    let dub_l = synth_vocal_take(sr, 0.042, 0.012, -18.0, 0.80, 29);
    let dub_r = synth_vocal_take(sr, -0.034, 0.016, 14.0, 0.84, 53);
    let harmony = synth_vocal_take(sr, 0.052, 0.014, 400.0, 0.78, 79);
    (guide, dub_l, dub_r, harmony)
}

#[derive(Clone, Copy)]
struct Resonator {
    b0: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Resonator {
    fn new() -> Self {
        Self {
            b0: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }
    fn set(&mut self, freq_hz: f32, bw_hz: f32, sr: f32) {
        let r = (-PI * bw_hz / sr).exp();
        let w = TAU * (freq_hz.clamp(80.0, sr * 0.42)) / sr;
        self.a1 = -2.0 * r * w.cos();
        self.a2 = r * r;
        self.b0 = (1.0 - r) * 0.75;
    }
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x - self.a1 * self.z1 - self.a2 * self.z2;
        self.z2 = self.z1;
        self.z1 = y;
        y
    }
}

fn synth_vocal_take(
    sample_rate: u32,
    base_shift_sec: f32,
    drift_sec: f32,
    pitch_offset_cents: f32,
    gain: f32,
    seed_init: u32,
) -> AudioBuffer {
    let sr = sample_rate as f32;
    let total_samples = (4.0 * sr).round() as usize;
    let mut out = vec![0.0_f32; total_samples];
    let pitch_mul = 2.0_f32.powf(pitch_offset_cents / 1200.0);

    let mut f1 = Resonator::new();
    let mut f2 = Resonator::new();
    let mut f3 = Resonator::new();
    let mut sib_bp = Resonator::new();
    sib_bp.set(6800.0, 1400.0, sr);
    let mut phase = 0.0_f32;
    let mut rng = seed_init
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);

    for (n, slot) in out.iter_mut().enumerate() {
        let t_out = n as f32 / sr;
        let t = t_out - base_shift_sec - drift_sec * (TAU * 0.65 * t_out).sin();
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = ((rng >> 9) as f32 / (1u32 << 23) as f32) * 2.0 - 1.0;

        let (env, f0, formants, sib_env, breath_env) = vocal_score(t);
        if n % 32 == 0 {
            f1.set(formants.0, 90.0, sr);
            f2.set(formants.1, 130.0, sr);
            f3.set(formants.2, 180.0, sr);
        }

        let vib = 1.0 + 0.007 * (TAU * 5.2 * t_out).sin();
        phase = (phase + TAU * (f0 * pitch_mul * vib) / sr) % TAU;

        let mut glottal = 0.0_f32;
        if env > 1e-4 {
            for h in 1..=10 {
                let hf = h as f32;
                glottal += (hf * phase).sin() / hf.powf(0.85);
            }
            glottal *= 0.28 * env;
        }
        let voiced = f1.tick(glottal) + 0.65 * f2.tick(glottal) + 0.35 * f3.tick(glottal);
        let sib = sib_bp.tick(noise) * sib_env * 0.42;
        let breath = noise * breath_env * 0.045;
        *slot = (voiced + sib + breath) * gain;
    }

    let peak = out.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    if peak > 0.0 {
        let norm = (0.75 * gain) / peak;
        for s in &mut out {
            *s *= norm;
        }
    }
    AudioBuffer::new_mono(sample_rate, out)
}

fn vocal_score(t: f32) -> (f32, f32, (f32, f32, f32), f32, f32) {
    let win = |s: f32, e: f32, fade: f32| -> f32 {
        if t < s || t > e {
            return 0.0;
        }
        ((t - s) / fade).min((e - t) / fade).clamp(0.0, 1.0)
    };
    let breath = win(0.16, 0.24, 0.025) + win(1.36, 1.44, 0.025);
    let sib = win(1.46, 1.56, 0.020) + win(3.56, 3.66, 0.020);
    if (0.26..=1.18).contains(&t) {
        let e = win(0.26, 1.18, 0.045);
        let (f0, fm) = if t < 0.60 {
            (261.63, (730.0, 1090.0, 2440.0))
        } else if t < 0.90 {
            (293.66, (530.0, 1840.0, 2480.0))
        } else {
            (329.63, (570.0, 840.0, 2410.0))
        };
        (e, f0, fm, sib, breath)
    } else if (1.56..=2.48).contains(&t) {
        let e = win(1.56, 2.48, 0.045);
        let (f0, fm) = if t < 1.92 {
            (329.63, (290.0, 2250.0, 3000.0))
        } else if t < 2.22 {
            (392.00, (730.0, 1100.0, 2450.0))
        } else {
            (349.23, (550.0, 1780.0, 2490.0))
        };
        (e, f0, fm, sib, breath)
    } else if (2.76..=3.56).contains(&t) {
        let e = win(2.76, 3.56, 0.045);
        let (f0, fm) = if t < 3.16 {
            (293.66, (570.0, 860.0, 2400.0))
        } else {
            (261.63, (720.0, 1120.0, 2440.0))
        };
        (e, f0, fm, sib, breath)
    } else {
        (0.0, 261.63, (700.0, 1100.0, 2450.0), sib, breath)
    }
}

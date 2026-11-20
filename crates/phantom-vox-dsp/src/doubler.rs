//! Zero-allocation real-time 2/4-voice vocal doubler engine with sibilance-safe crossover,
//! formant tilt, stereo width, mono compatibility, and rolling phase correlation metering.

use std::f32::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::doubler_dsp::{read_delay_cubic, FormantBiquad, OnePole, DELAY_LEN, DELAY_MASK};

pub use crate::live_align::{LiveSidechainAligner, LIVE_ALIGN_LATENCY_SAMPLES};

const NUM_VOICES: usize = 4;

/// Real-time parameter snapshot for `VocalDoublerEngine`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DoublerParams {
    pub timing_ms: f32,
    pub detune_cents: f32,
    pub formant_st: f32,
    pub width: f32,
    pub voices_4: bool,
    pub sibilance_safe: bool,
    pub mono_check: bool,
    pub mix: f32,
}

impl Default for DoublerParams {
    fn default() -> Self {
        Self {
            timing_ms: 12.0,
            detune_cents: 10.0,
            formant_st: 0.0,
            width: 1.0,
            voices_4: false,
            sibilance_safe: true,
            mono_check: false,
            mix: 0.55,
        }
    }
}

#[derive(Clone, Copy)]
struct VoiceState {
    lfo_phase: f32,
    lfo_hz: f32,
    delay_scale: f32,
    delay_offset_ms: f32,
    pan: f32,
    cur_delay_samples: f32,
    tilt_lo: FormantBiquad,
    tilt_hi: FormantBiquad,
}

impl VoiceState {
    const fn new(phase: f32, hz: f32, scale: f32, offset_ms: f32, pan: f32, init_d: f32) -> Self {
        Self {
            lfo_phase: phase,
            lfo_hz: hz,
            delay_scale: scale,
            delay_offset_ms: offset_ms,
            pan,
            cur_delay_samples: init_d,
            tilt_lo: FormantBiquad::identity(),
            tilt_hi: FormantBiquad::identity(),
        }
    }
}

/// Strictly zero-allocation 2/4-voice real-time stereo vocal doubler.
pub struct VocalDoublerEngine {
    sample_rate: f32,
    delay_buf: Vec<f32>,
    write_pos: usize,
    xover_a: f32,
    lp1_l: OnePole,
    lp2_l: OnePole,
    lp1_r: OnePole,
    lp2_r: OnePole,
    voices: [VoiceState; NUM_VOICES],
    last_formant_st: f32,
    corr_a: f32,
    avg_lr: f32,
    avg_ll: f32,
    avg_rr: f32,
}

impl VocalDoublerEngine {
    /// Pre-allocates all delay memory once (safe for sample rates up to 192 kHz).
    pub fn new(sample_rate: f32) -> Self {
        let voices = [
            VoiceState::new(0.0, 0.43, 0.85, 4.5, -0.82, 600.0),
            VoiceState::new(0.5 * PI, 0.59, 1.15, 7.5, 0.82, 850.0),
            VoiceState::new(PI, 0.73, 1.50, 11.0, -1.0, 1200.0),
            VoiceState::new(1.5 * PI, 0.89, 1.85, 14.5, 1.0, 1500.0),
        ];
        let mut engine = Self {
            sample_rate: sample_rate.max(8000.0),
            delay_buf: vec![0.0_f32; DELAY_LEN],
            write_pos: 0,
            xover_a: 0.5,
            lp1_l: OnePole::new(),
            lp2_l: OnePole::new(),
            lp1_r: OnePole::new(),
            lp2_r: OnePole::new(),
            voices,
            last_formant_st: f32::NAN,
            corr_a: 0.99,
            avg_lr: 0.0,
            avg_ll: 0.0,
            avg_rr: 0.0,
        };
        engine.set_sample_rate(sample_rate);
        engine
    }

    /// Updates sample rate and filter coefficients without heap allocations.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(8000.0);
        self.xover_a = (-TAU * 4500.0 / self.sample_rate).exp();
        self.corr_a = (-1.0 / (0.150 * self.sample_rate)).exp();
        self.last_formant_st = f32::NAN;
    }

    /// Clears delay line, filters, and meter state without heap allocations.
    pub fn reset(&mut self) {
        self.delay_buf.fill(0.0);
        self.write_pos = 0;
        self.lp1_l = OnePole::new();
        self.lp2_l = OnePole::new();
        self.lp1_r = OnePole::new();
        self.lp2_r = OnePole::new();
        for (idx, v) in self.voices.iter_mut().enumerate() {
            v.lfo_phase = idx as f32 * 0.5 * PI;
            v.tilt_lo.reset();
            v.tilt_hi.reset();
        }
        self.avg_lr = 0.0;
        self.avg_ll = 0.0;
        self.avg_rr = 0.0;
    }

    /// Returns current rolling stereo phase correlation in `-1.0..=1.0`.
    #[inline]
    pub fn correlation(&self) -> f32 {
        let denom = (self.avg_ll * self.avg_rr).sqrt();
        if denom < 1e-12 {
            1.0
        } else {
            (self.avg_lr / denom).clamp(-1.0, 1.0)
        }
    }

    /// Zero-allocation stereo-to-stereo vocal doubler processing.
    pub fn process_stereo(
        &mut self,
        in_l: &[f32],
        in_r: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
        params: &DoublerParams,
    ) {
        let n = in_l.len().min(in_r.len()).min(out_l.len()).min(out_r.len());
        self.sync_formant_filters(params.formant_st);
        for i in 0..n {
            let (l, r) = self.process_sample_stereo(in_l[i], in_r[i], params);
            out_l[i] = l;
            out_r[i] = r;
        }
    }

    /// Zero-allocation mono-to-stereo vocal doubler processing.
    pub fn process_mono_to_stereo(
        &mut self,
        input: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
        params: &DoublerParams,
    ) {
        let n = input.len().min(out_l.len()).min(out_r.len());
        self.sync_formant_filters(params.formant_st);
        for i in 0..n {
            let s = input[i];
            let (l, r) = self.process_sample_stereo(s, s, params);
            out_l[i] = l;
            out_r[i] = r;
        }
    }

    #[inline]
    fn sync_formant_filters(&mut self, formant_st: f32) {
        let f_st = formant_st.clamp(-4.0, 4.0);
        if (f_st - self.last_formant_st).abs() < 0.02 {
            return;
        }
        self.last_formant_st = f_st;
        let sr = self.sample_rate;
        let gain_lo = -f_st * 1.4;
        let gain_hi = f_st * 1.6;
        for (idx, v) in self.voices.iter_mut().enumerate() {
            let spread = 1.0 + (idx as f32 - 1.5) * 0.06;
            v.tilt_lo.set_peaking(820.0 * spread, 0.9, gain_lo, sr);
            v.tilt_hi.set_peaking(2650.0 * spread, 0.9, gain_hi, sr);
        }
    }

    #[inline]
    fn process_sample_stereo(&mut self, raw_l: f32, raw_r: f32, p: &DoublerParams) -> (f32, f32) {
        let in_l = if raw_l.is_finite() { raw_l } else { 0.0 };
        let in_r = if raw_r.is_finite() { raw_r } else { 0.0 };

        let lp_l = self
            .lp2_l
            .lp(self.lp1_l.lp(in_l, self.xover_a), self.xover_a);
        let lp_r = self
            .lp2_r
            .lp(self.lp1_r.lp(in_r, self.xover_a), self.xover_a);
        let (hp_l, hp_r) = (in_l - lp_l, in_r - lp_r);

        let feed = if p.sibilance_safe {
            0.5 * (lp_l + lp_r)
        } else {
            0.5 * (in_l + in_r)
        };
        self.delay_buf[self.write_pos] = feed;

        let sr = self.sample_rate;
        let smooth_a = (-1.0 / (0.015 * sr)).exp();
        let active_voices = if p.voices_4 { 4usize } else { 2usize };
        let timing_ms = p.timing_ms.clamp(0.0, 35.0);
        let detune_ratio = 2.0_f32.powf(p.detune_cents.clamp(0.0, 35.0) / 1200.0) - 1.0;
        let max_delay = (DELAY_LEN - 8) as f32;
        let voice_gain = if p.voices_4 { 0.48 } else { 0.68 };

        let (mut wet_l, mut wet_r) = (0.0_f32, 0.0_f32);
        for v in self.voices.iter_mut().take(active_voices) {
            v.lfo_phase = (v.lfo_phase + TAU * v.lfo_hz / sr) % TAU;
            let mod_sec = (detune_ratio / (TAU * v.lfo_hz)).min(0.0055);
            let base_ms = (v.delay_offset_ms + timing_ms * v.delay_scale).clamp(1.5, 72.0);
            let target_s =
                ((base_ms * 0.001 + mod_sec * v.lfo_phase.sin()) * sr).clamp(4.0, max_delay);
            v.cur_delay_samples = smooth_a * v.cur_delay_samples + (1.0 - smooth_a) * target_s;

            let s = v.tilt_hi.tick(v.tilt_lo.tick(read_delay_cubic(
                &self.delay_buf,
                self.write_pos,
                v.cur_delay_samples,
            ))) * voice_gain;
            let pan = v.pan.clamp(-1.0, 1.0);
            wet_l += s * (0.5 * (1.0 - pan * 0.78) + 0.12);
            wet_r += s * (0.5 * (1.0 + pan * 0.78) + 0.12);
        }

        let width = p.width.clamp(0.0, 1.5);
        let mid = 0.5 * (wet_l + wet_r);
        let side = 0.5 * (wet_l - wet_r) * width;
        (wet_l, wet_r) = (mid + side, mid - side);

        let mix = p.mix.clamp(0.0, 1.0);
        let dry_gain = 1.0 - 0.55 * mix;
        let hf_l = if p.sibilance_safe {
            hp_l * (0.55 * mix)
        } else {
            0.0
        };
        let hf_r = if p.sibilance_safe {
            hp_r * (0.55 * mix)
        } else {
            0.0
        };

        let mut out_l = in_l * dry_gain + wet_l * mix + hf_l;
        let mut out_r = in_r * dry_gain + wet_r * mix + hf_r;
        if p.mono_check {
            let m = 0.5 * (out_l + out_r);
            (out_l, out_r) = (m, m);
        }

        let ca = self.corr_a;
        self.avg_lr = ca * self.avg_lr + (1.0 - ca) * (out_l * out_r);
        self.avg_ll = ca * self.avg_ll + (1.0 - ca) * (out_l * out_l);
        self.avg_rr = ca * self.avg_rr + (1.0 - ca) * (out_r * out_r);
        self.write_pos = (self.write_pos + 1) & DELAY_MASK;
        (out_l, out_r)
    }
}

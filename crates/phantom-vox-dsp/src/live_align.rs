//! Zero-allocation real-time sidechain vocal aligner (`LiveSidechainAligner`) with fixed PDC lookahead.

use crate::doubler_dsp::read_ring_cubic_masked;

/// Fixed lookahead latency reported to the DAW for symmetric early/late live timing compensation.
pub const LIVE_ALIGN_LATENCY_SAMPLES: u32 = 2048;

const RING_LEN: usize = 8192;
const RING_MASK: usize = RING_LEN - 1;
const ENV_LEN: usize = 256;
const ENV_MASK: usize = ENV_LEN - 1;
const MAX_LAG_MS: isize = 42;
const CORR_WIN_MS: usize = 140;

#[derive(Clone, Copy)]
struct SmoothCoeffs {
    fast_a: f32,
    slow_a: f32,
    rms_a: f32,
    delay_a: f32,
    gain_a: f32,
    max_comp: f32,
}

/// Real-time zero-allocation sidechain vocal aligner.
pub struct LiveSidechainAligner {
    sample_rate: f32,
    decim_step: usize,
    decim_ctr: usize,
    eval_ctr: usize,
    ring_l: Vec<f32>,
    ring_r: Vec<f32>,
    write_pos: usize,
    env_guide: [f32; ENV_LEN],
    env_dub: [f32; ENV_LEN],
    env_write: usize,
    g_fast: f32,
    g_slow: f32,
    d_fast: f32,
    d_slow: f32,
    g_rms_sq: f32,
    d_rms_sq: f32,
    offset_ms: f32,
    confidence: f32,
    sidechain_active: bool,
    cur_delay_samples: f32,
    cur_level_gain: f32,
}

impl LiveSidechainAligner {
    /// Allocates fixed internal ring buffers once for sample rates up to 192 kHz.
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate: sample_rate.max(8000.0),
            decim_step: 48,
            decim_ctr: 0,
            eval_ctr: 0,
            ring_l: vec![0.0_f32; RING_LEN],
            ring_r: vec![0.0_f32; RING_LEN],
            write_pos: 0,
            env_guide: [0.0; ENV_LEN],
            env_dub: [0.0; ENV_LEN],
            env_write: 0,
            g_fast: 0.0,
            g_slow: 0.0,
            d_fast: 0.0,
            d_slow: 0.0,
            g_rms_sq: 0.0,
            d_rms_sq: 0.0,
            offset_ms: 0.0,
            confidence: 0.0,
            sidechain_active: false,
            cur_delay_samples: LIVE_ALIGN_LATENCY_SAMPLES as f32,
            cur_level_gain: 1.0,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    /// Updates sample rate without heap allocations.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(8000.0);
        self.decim_step = ((self.sample_rate * 0.001).round() as usize).clamp(8, 256);
    }

    /// Resets all delay lines and tracking state without heap allocations.
    pub fn reset(&mut self) {
        self.ring_l.fill(0.0);
        self.ring_r.fill(0.0);
        self.write_pos = 0;
        self.env_guide.fill(0.0);
        self.env_dub.fill(0.0);
        self.env_write = 0;
        self.decim_ctr = 0;
        self.eval_ctr = 0;
        (self.g_fast, self.g_slow, self.d_fast, self.d_slow) = (0.0, 0.0, 0.0, 0.0);
        (self.g_rms_sq, self.d_rms_sq) = (0.0, 0.0);
        (self.offset_ms, self.confidence, self.sidechain_active) = (0.0, 0.0, false);
        self.cur_delay_samples = LIVE_ALIGN_LATENCY_SAMPLES as f32;
        self.cur_level_gain = 1.0;
    }

    #[inline]
    pub fn latency_samples(&self) -> u32 {
        LIVE_ALIGN_LATENCY_SAMPLES
    }

    /// Current smoothed timing offset in ms (`> 0` means dub is late relative to guide).
    #[inline]
    pub fn offset_ms(&self) -> f32 {
        self.offset_ms
    }

    /// Current alignment confidence in `0.0..=1.0`.
    #[inline]
    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    /// Returns `true` when audible signal is detected on the Guide sidechain input.
    #[inline]
    pub fn sidechain_active(&self) -> bool {
        self.sidechain_active
    }

    fn coeffs(&self) -> SmoothCoeffs {
        let sr = self.sample_rate;
        SmoothCoeffs {
            fast_a: (-1.0 / (0.004 * sr)).exp(),
            slow_a: (-1.0 / (0.040 * sr)).exp(),
            rms_a: (-1.0 / (0.050 * sr)).exp(),
            delay_a: (-1.0 / (0.080 * sr)).exp(),
            gain_a: (-1.0 / (0.070 * sr)).exp(),
            max_comp: (LIVE_ALIGN_LATENCY_SAMPLES as f32 - 32.0).max(0.0),
        }
    }

    /// Zero-allocation stereo sidechain alignment.
    #[allow(clippy::too_many_arguments)]
    pub fn process_stereo(
        &mut self,
        dub_l: &[f32],
        dub_r: &[f32],
        guide_l: &[f32],
        guide_r: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
        tightness: f32,
        level_match: f32,
    ) {
        let n = dub_l
            .len()
            .min(dub_r.len())
            .min(guide_l.len())
            .min(guide_r.len())
            .min(out_l.len())
            .min(out_r.len());
        let (tight, lvl, c) = (
            tightness.clamp(0.0, 1.0),
            level_match.clamp(0.0, 1.0),
            self.coeffs(),
        );
        for i in 0..n {
            let (dl, dr) = (sanitize(dub_l[i]), sanitize(dub_r[i]));
            let (gl, gr) = (sanitize(guide_l[i]), sanitize(guide_r[i]));
            self.ring_l[self.write_pos] = dl;
            self.ring_r[self.write_pos] = dr;
            self.step_tracker(0.5 * (dl + dr), 0.5 * (gl + gr), tight, lvl, &c);
            out_l[i] = read_ring_cubic(&self.ring_l, self.write_pos, self.cur_delay_samples)
                * self.cur_level_gain;
            out_r[i] = read_ring_cubic(&self.ring_r, self.write_pos, self.cur_delay_samples)
                * self.cur_level_gain;
            self.write_pos = (self.write_pos + 1) & RING_MASK;
        }
    }

    /// Zero-allocation mono sidechain alignment.
    pub fn process_mono(
        &mut self,
        dub: &[f32],
        guide: &[f32],
        out: &mut [f32],
        tightness: f32,
        level_match: f32,
    ) {
        let n = dub.len().min(guide.len()).min(out.len());
        let (tight, lvl, c) = (
            tightness.clamp(0.0, 1.0),
            level_match.clamp(0.0, 1.0),
            self.coeffs(),
        );
        for i in 0..n {
            let (dm, gm) = (sanitize(dub[i]), sanitize(guide[i]));
            self.ring_l[self.write_pos] = dm;
            self.step_tracker(dm, gm, tight, lvl, &c);
            out[i] = read_ring_cubic(&self.ring_l, self.write_pos, self.cur_delay_samples)
                * self.cur_level_gain;
            self.write_pos = (self.write_pos + 1) & RING_MASK;
        }
    }

    #[inline]
    fn step_tracker(&mut self, dub_m: f32, guide_m: f32, tight: f32, lvl: f32, c: &SmoothCoeffs) {
        let (da, ga) = (dub_m.abs(), guide_m.abs());
        self.d_fast = c.fast_a * self.d_fast + (1.0 - c.fast_a) * da;
        self.d_slow = c.slow_a * self.d_slow + (1.0 - c.slow_a) * da;
        self.g_fast = c.fast_a * self.g_fast + (1.0 - c.fast_a) * ga;
        self.g_slow = c.slow_a * self.g_slow + (1.0 - c.slow_a) * ga;
        self.d_rms_sq = c.rms_a * self.d_rms_sq + (1.0 - c.rms_a) * dub_m * dub_m;
        self.g_rms_sq = c.rms_a * self.g_rms_sq + (1.0 - c.rms_a) * guide_m * guide_m;

        self.decim_ctr += 1;
        if self.decim_ctr >= self.decim_step {
            self.decim_ctr = 0;
            self.env_guide[self.env_write] =
                self.g_fast + 0.6 * (self.g_fast - self.g_slow).max(0.0);
            self.env_dub[self.env_write] = self.d_fast + 0.6 * (self.d_fast - self.d_slow).max(0.0);
            self.env_write = (self.env_write + 1) & ENV_MASK;
            self.eval_ctr += 1;
            if self.eval_ctr >= 16 {
                self.eval_ctr = 0;
                self.estimate_offset();
            }
        }

        let comp = (self.offset_ms * 0.001 * self.sample_rate * tight * self.confidence)
            .clamp(-c.max_comp, c.max_comp);
        let target_d =
            (LIVE_ALIGN_LATENCY_SAMPLES as f32 - comp).clamp(16.0, (RING_LEN - 16) as f32);
        self.cur_delay_samples = c.delay_a * self.cur_delay_samples + (1.0 - c.delay_a) * target_d;

        let target_g = if lvl > 1e-4
            && self.sidechain_active
            && self.g_rms_sq > 1e-5
            && self.d_rms_sq > 1e-5
        {
            (self.g_rms_sq / self.d_rms_sq)
                .sqrt()
                .clamp(0.25, 4.0)
                .powf(lvl * self.confidence.max(0.25))
        } else {
            1.0
        };
        self.cur_level_gain = c.gain_a * self.cur_level_gain + (1.0 - c.gain_a) * target_g;
    }

    fn estimate_offset(&mut self) {
        self.sidechain_active = self.g_rms_sq > 1e-6;
        if !self.sidechain_active || self.d_rms_sq < 1e-6 {
            self.confidence *= 0.85;
            self.offset_ms *= 0.92;
            return;
        }
        let base = self.env_write.wrapping_sub(1 + MAX_LAG_MS as usize);
        let (mut best_lag, mut best_corr) = (0isize, -1.0_f32);
        for lag in -MAX_LAG_MS..=MAX_LAG_MS {
            let (mut dot, mut eg, mut ed) = (0.0_f32, 0.0_f32, 0.0_f32);
            for k in 0..CORR_WIN_MS {
                let gi = base.wrapping_sub(k) & ENV_MASK;
                let di = base.wrapping_sub(k).wrapping_add(lag as usize) & ENV_MASK;
                let (gv, dv) = (self.env_guide[gi], self.env_dub[di]);
                dot += gv * dv;
                eg += gv * gv;
                ed += dv * dv;
            }
            let denom = (eg * ed).sqrt();
            let corr = if denom > 1e-8 { dot / denom } else { 0.0 };
            if corr > best_corr {
                (best_corr, best_lag) = (corr, lag);
            }
        }
        let conf = ((best_corr - 0.35) / 0.55).clamp(0.0, 1.0);
        self.confidence = 0.70 * self.confidence + (1.0 - 0.70) * conf;
        if conf > 0.20 {
            self.offset_ms = 0.72 * self.offset_ms + 0.28 * best_lag as f32;
        }
    }
}

#[inline]
fn sanitize(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

#[inline]
fn read_ring_cubic(ring: &[f32], write_pos: usize, delay: f32) -> f32 {
    read_ring_cubic_masked(ring, RING_MASK, write_pos, delay)
}

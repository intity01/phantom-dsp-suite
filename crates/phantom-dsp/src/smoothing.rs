//! Zero-Allocation Parameter Smoothing, Click-Free Bypass Crossfade & PDC Delay Line.

use crate::linear_phase::LATENCY_SAMPLES;

/// Sub-block stride (in samples) for smoothed EQ coefficient updates.
pub const SMOOTH_SUBBLOCK: usize = 16;

const DEFAULT_SR: f32 = 48_000.0;
const DEFAULT_SMOOTH_MS: f32 = 15.0;
const BYPASS_RAMP_SEC: f64 = 0.010;
const LOG_FLOOR: f64 = 1e-6;
const SNAP_EPS: f64 = 1e-9;

fn finite_or(v: f32, fallback: f32) -> f64 {
    (if v.is_finite() { v } else { fallback }) as f64
}

fn pos_finite(v: f32, fallback: f32) -> f32 {
    finite_or(v, fallback).max(LOG_FLOOR) as f32
}

/// Exponential one-pole smoother supporting linear (`next`) and log-ratio (`next_log`) ramps.
#[derive(Debug, Clone, Copy)]
pub struct OnePoleSmoother {
    current: f64,
    target: f64,
    inv_tau: f64,
    coeff: f64,
    cached_sr_bits: u32,
    cached_ms_bits: u32,
}

impl OnePoleSmoother {
    pub fn new(initial: f32) -> Self {
        let v = finite_or(initial, 0.0);
        let inv_tau = -1.0 / ((DEFAULT_SMOOTH_MS as f64 * 0.001) * DEFAULT_SR as f64);
        Self {
            current: v,
            target: v,
            inv_tau,
            coeff: -inv_tau.exp_m1(),
            cached_sr_bits: DEFAULT_SR.to_bits(),
            cached_ms_bits: DEFAULT_SMOOTH_MS.to_bits(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32, time_ms: f32) {
        let sr = (finite_or(sample_rate, DEFAULT_SR) as f32).max(1.0);
        let ms = (finite_or(time_ms, 0.0) as f32).max(0.0);
        let key = (sr.to_bits(), ms.to_bits());
        if key == (self.cached_sr_bits, self.cached_ms_bits) {
            return;
        }
        (self.inv_tau, self.coeff) = if ms <= 0.0 {
            (0.0, 1.0)
        } else {
            let inv = -1.0 / ((ms as f64 * 0.001) * sr as f64);
            (inv, -inv.exp_m1())
        };
        (self.cached_sr_bits, self.cached_ms_bits) = key;
    }

    pub fn reset(&mut self, value: f32) {
        let v = finite_or(value, 0.0);
        (self.current, self.target) = (v, v);
    }

    pub fn set_target(&mut self, target: f32) {
        if target.is_finite() {
            self.target = target as f64;
        }
    }

    fn alpha_for(&self, samples: usize) -> f64 {
        if self.coeff >= 1.0 {
            1.0
        } else {
            -(self.inv_tau * samples as f64).exp_m1()
        }
    }

    fn step_lin(&mut self, alpha: f64) -> f32 {
        let eps = SNAP_EPS * self.target.abs().max(1.0);
        let next = if alpha >= 1.0 {
            self.target
        } else {
            self.current + alpha * (self.target - self.current)
        };
        self.current = if (self.target - next).abs() <= eps {
            self.target
        } else {
            next
        };
        self.current as f32
    }

    fn step_log(&mut self, alpha: f64) -> f32 {
        let (cur, tgt) = (self.current.max(LOG_FLOOR), self.target.max(LOG_FLOOR));
        let eps = SNAP_EPS * tgt.max(1.0);
        if (tgt - cur).abs() <= eps || alpha >= 1.0 {
            self.current = tgt;
        } else {
            let next = (cur.ln() + alpha * (tgt.ln() - cur.ln())).exp();
            self.current = if (tgt - next).abs() <= eps { tgt } else { next };
        }
        self.current as f32
    }

    #[allow(clippy::should_implement_trait)]
    #[inline(always)]
    pub fn next(&mut self) -> f32 {
        self.step_lin(self.coeff)
    }

    #[inline(always)]
    pub fn next_log(&mut self) -> f32 {
        self.step_log(self.coeff)
    }

    pub fn skip(&mut self, samples: usize) {
        if samples > 0 && self.current != self.target {
            self.step_lin(self.alpha_for(samples));
        }
    }

    pub fn skip_log(&mut self, samples: usize) {
        if samples > 0 && (self.current != self.target || self.current < LOG_FLOOR) {
            self.step_log(self.alpha_for(samples));
        }
    }

    pub fn current(&self) -> f32 {
        self.current as f32
    }

    pub fn is_settled(&self, eps: f32) -> bool {
        (self.target - self.current).abs() <= eps.max(0.0) as f64
    }
}

/// Per-band EQ smoother: log `freq_hz`, linear `gain_db`, and log `q` (~15 ms default).
#[derive(Debug, Clone, Copy)]
pub struct BandParamSmoother {
    freq: OnePoleSmoother,
    gain: OnePoleSmoother,
    q: OnePoleSmoother,
}

impl BandParamSmoother {
    pub fn new(freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self {
            freq: OnePoleSmoother::new(pos_finite(freq_hz, 1000.0)),
            gain: OnePoleSmoother::new(finite_or(gain_db, 0.0) as f32),
            q: OnePoleSmoother::new(pos_finite(q, 1.0)),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32, time_ms: f32) {
        self.freq.set_sample_rate(sample_rate, time_ms);
        self.gain.set_sample_rate(sample_rate, time_ms);
        self.q.set_sample_rate(sample_rate, time_ms);
    }

    pub fn reset(&mut self, freq_hz: f32, gain_db: f32, q: f32) {
        self.freq.reset(pos_finite(freq_hz, 1000.0));
        self.gain.reset(finite_or(gain_db, 0.0) as f32);
        self.q.reset(pos_finite(q, 1.0));
    }

    pub fn set_targets(&mut self, freq_hz: f32, gain_db: f32, q: f32) {
        self.freq
            .set_target(pos_finite(freq_hz, self.freq.target as f32));
        self.gain.set_target(gain_db);
        self.q.set_target(pos_finite(q, self.q.target as f32));
    }

    pub fn advance_subblock(&mut self, samples: usize) -> (f32, f32, f32) {
        if samples > 0 {
            if !self.is_settled() {
                let alpha = self.freq.alpha_for(samples);
                self.freq.step_log(alpha);
                self.gain.step_lin(alpha);
                self.q.step_log(alpha);
            }
            if self.is_settled() {
                (self.freq.current, self.gain.current, self.q.current) =
                    (self.freq.target, self.gain.target, self.q.target);
            }
        }
        self.current()
    }

    pub fn current(&self) -> (f32, f32, f32) {
        (self.freq.current(), self.gain.current(), self.q.current())
    }

    pub fn is_settled(&self) -> bool {
        self.freq.is_settled(1e-3) && self.gain.is_settled(1e-4) && self.q.is_settled(1e-4)
    }
}

/// Linear 10.0 ms wet/dry bypass crossfade (`0.0` = bypassed, `1.0` = active).
#[derive(Debug, Clone, Copy)]
pub struct BypassCrossfade {
    wet: f64,
    target_wet: f64,
    step: f64,
    cached_sr_bits: u32,
}

impl BypassCrossfade {
    pub fn new(bypassed: bool) -> Self {
        let v = if bypassed { 0.0 } else { 1.0 };
        Self {
            wet: v,
            target_wet: v,
            step: 1.0 / (BYPASS_RAMP_SEC * DEFAULT_SR as f64),
            cached_sr_bits: DEFAULT_SR.to_bits(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = (finite_or(sample_rate, DEFAULT_SR) as f32).max(1.0);
        if sr.to_bits() == self.cached_sr_bits {
            return;
        }
        self.step = 1.0 / (BYPASS_RAMP_SEC * sr as f64).max(1.0);
        self.cached_sr_bits = sr.to_bits();
    }

    pub fn set_bypassed(&mut self, bypassed: bool) {
        self.target_wet = if bypassed { 0.0 } else { 1.0 };
    }

    pub fn reset(&mut self, bypassed: bool) {
        let v = if bypassed { 0.0 } else { 1.0 };
        (self.wet, self.target_wet) = (v, v);
    }

    #[inline(always)]
    pub fn next_wet(&mut self) -> f32 {
        let diff = self.target_wet - self.wet;
        self.wet = if diff.abs() <= self.step + SNAP_EPS {
            self.target_wet
        } else {
            self.wet + self.step.copysign(diff)
        };
        self.wet as f32
    }

    pub fn is_fully_bypassed(&self) -> bool {
        self.wet <= 0.0 && self.target_wet <= 0.0
    }

    pub fn is_fully_active(&self) -> bool {
        self.wet >= 1.0 && self.target_wet >= 1.0
    }
}

/// Fixed circular stereo delay line matching `LinearPhaseEngine`'s `LATENCY_SAMPLES` (`3071`).
/// Allocates once in `new()`; strictly zero allocations in `process_sample` and `reset`.
#[derive(Debug, Clone)]
pub struct LatencyBypassDelay {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
}

impl Default for LatencyBypassDelay {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyBypassDelay {
    pub fn new() -> Self {
        Self {
            buf_l: vec![0.0; LATENCY_SAMPLES],
            buf_r: vec![0.0; LATENCY_SAMPLES],
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.buf_l.fill(0.0);
        self.buf_r.fill(0.0);
        self.pos = 0;
    }

    #[inline(always)]
    pub fn process_sample(&mut self, l: f32, r: f32) -> (f32, f32) {
        let i = self.pos;
        let out = (self.buf_l[i], self.buf_r[i]);
        self.buf_l[i] = if l.is_finite() { l } else { 0.0 };
        self.buf_r[i] = if r.is_finite() { r } else { 0.0 };
        self.pos = if i + 1 < LATENCY_SAMPLES { i + 1 } else { 0 };
        out
    }
}

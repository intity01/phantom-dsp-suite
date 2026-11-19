//! Analog Character Saturation Processor (Clean, Subtle Analog, Warm Tube)
//! Zero-allocation stereo harmonic coloration with a 6.5 Hz DC-blocking filter
//! so asymmetric even-harmonic waveshaping introduces zero DC offset.

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterMode {
    #[default]
    Clean,
    SubtleAnalog,
    WarmTube,
}

#[derive(Debug, Clone)]
pub struct CharacterProcessor {
    dc_r: f64,
    dc_gain: f64,
    prev_in_l: f64,
    prev_out_l: f64,
    prev_in_r: f64,
    prev_out_r: f64,
}

impl Default for CharacterProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterProcessor {
    pub fn new() -> Self {
        let mut proc = Self {
            dc_r: 0.999,
            dc_gain: 0.9995,
            prev_in_l: 0.0,
            prev_out_l: 0.0,
            prev_in_r: 0.0,
            prev_out_r: 0.0,
        };
        proc.set_sample_rate(44100.0);
        proc
    }

    /// Update the ~6.5 Hz DC-blocking high-pass filter pole for the current sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = (sample_rate as f64).max(1000.0);
        let r = (-2.0 * PI * 6.5 / sr).exp().clamp(0.9, 0.99995);
        self.dc_r = r;
        self.dc_gain = 0.5 * (1.0 + r);
    }

    pub fn reset(&mut self) {
        self.prev_in_l = 0.0;
        self.prev_out_l = 0.0;
        self.prev_in_r = 0.0;
        self.prev_out_r = 0.0;
    }

    /// Process a stereo sample pair with the selected analog character mode.
    /// `Clean` is a bit-for-bit exact passthrough; nonlinear modes are strictly bounded and DC-free.
    #[inline(always)]
    pub fn process_stereo(&mut self, l: f32, r: f32, mode: CharacterMode) -> (f32, f32) {
        match mode {
            CharacterMode::Clean => (l, r),
            CharacterMode::SubtleAnalog => {
                let sl = shape_subtle_analog(l);
                let sr = shape_subtle_analog(r);
                self.dc_block_stereo(sl, sr)
            }
            CharacterMode::WarmTube => {
                let sl = shape_warm_tube(l);
                let sr = shape_warm_tube(r);
                self.dc_block_stereo(sl, sr)
            }
        }
    }

    #[inline(always)]
    fn dc_block_stereo(&mut self, xl: f64, xr: f64) -> (f32, f32) {
        let yl = self.dc_gain * (xl - self.prev_in_l) + self.dc_r * self.prev_out_l;
        self.prev_in_l = xl;
        self.prev_out_l = if yl.abs() < 1e-25 { 0.0 } else { yl };

        let yr = self.dc_gain * (xr - self.prev_in_r) + self.dc_r * self.prev_out_r;
        self.prev_in_r = xr;
        self.prev_out_r = if yr.abs() < 1e-25 { 0.0 } else { yr };

        (yl as f32, yr as f32)
    }
}

/// Subtle transformer saturation: unity small-signal gain, ~-60 dB gentle 2nd/3rd harmonics,
/// smooth soft clipping on overload.
#[inline(always)]
fn shape_subtle_analog(sample: f32) -> f64 {
    if !sample.is_finite() {
        return 0.0;
    }
    const DRIVE: f64 = 0.28;
    const BIAS: f64 = 0.015;
    let x = (sample as f64).clamp(-32.0, 32.0);
    let t0 = BIAS.tanh();
    let norm = 1.0 / (DRIVE * (1.0 - t0 * t0));
    ((DRIVE * x + BIAS).tanh() - t0) * norm
}

/// Warm triode tube saturation: unity small-signal gain, rich asymmetric 2nd-order even harmonics
/// plus smooth soft saturation. Strictly bounded for any input magnitude.
#[inline(always)]
fn shape_warm_tube(sample: f32) -> f64 {
    if !sample.is_finite() {
        return 0.0;
    }
    const DRIVE: f64 = 0.65;
    const BIAS: f64 = 0.22;
    let x = (sample as f64).clamp(-32.0, 32.0);
    let t0 = BIAS.tanh();
    let norm = 1.0 / (DRIVE * (1.0 - t0 * t0));
    ((DRIVE * x + BIAS).tanh() - t0) * norm
}

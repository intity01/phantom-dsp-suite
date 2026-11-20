//! Focus band for the doubled voices: a 12 dB/oct high-pass and low-pass applied to the
//! doubled (wet) voices only, so widening can skip the lows and the harsh top end.
//! At the full range (20 Hz to 20 kHz) both filters are bypassed and the sound is unchanged.

use std::f32::consts::TAU;

/// Lowest focus frequency. At or below this the high-pass is off.
pub const FOCUS_MIN_HZ: f32 = 20.0;
/// Highest focus frequency. At or above this the low-pass is off.
pub const FOCUS_MAX_HZ: f32 = 20_000.0;

#[derive(Clone, Copy)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [[f32; 2]; 2],
    on: bool,
}

impl Biquad {
    const fn off() -> Self {
        Self {
            b: [1.0, 0.0, 0.0],
            a: [0.0, 0.0],
            z: [[0.0; 2]; 2],
            on: false,
        }
    }

    /// RBJ Butterworth high-pass (`high_pass = true`) or low-pass, Q = 1/sqrt(2).
    fn set(&mut self, high_pass: bool, freq_hz: f32, sr: f32) {
        let w0 = TAU * freq_hz.clamp(10.0, sr * 0.45) / sr;
        let (sin_w, cos_w) = w0.sin_cos();
        let alpha = sin_w * std::f32::consts::FRAC_1_SQRT_2;
        let a0 = 1.0 + alpha;
        let (b0, b1) = if high_pass {
            ((1.0 + cos_w) * 0.5, -(1.0 + cos_w))
        } else {
            ((1.0 - cos_w) * 0.5, 1.0 - cos_w)
        };
        self.b = [b0 / a0, b1 / a0, b0 / a0];
        self.a = [(-2.0 * cos_w) / a0, (1.0 - alpha) / a0];
        if !self.on {
            self.z = [[0.0; 2]; 2];
        }
        self.on = true;
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f32) -> f32 {
        if !self.on {
            return x;
        }
        let z = &mut self.z[ch];
        let y = self.b[0] * x + z[0];
        z[0] = self.b[1] * x - self.a[0] * y + z[1];
        z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Stereo high-pass + low-pass pair for the doubled voices (zero allocations).
#[derive(Clone, Copy)]
pub struct FocusBand {
    hp: Biquad,
    lp: Biquad,
    last: (f32, f32, f32),
}

impl Default for FocusBand {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusBand {
    pub const fn new() -> Self {
        Self {
            hp: Biquad::off(),
            lp: Biquad::off(),
            last: (f32::NAN, f32::NAN, f32::NAN),
        }
    }

    pub fn reset(&mut self) {
        self.hp.z = [[0.0; 2]; 2];
        self.lp.z = [[0.0; 2]; 2];
    }

    /// Updates the filters when the range or sample rate changed.
    pub fn sync(&mut self, low_hz: f32, high_hz: f32, sr: f32) {
        let low = low_hz.clamp(FOCUS_MIN_HZ, FOCUS_MAX_HZ);
        let high = high_hz.clamp(FOCUS_MIN_HZ, FOCUS_MAX_HZ).max(low);
        if (low, high, sr) == self.last {
            return;
        }
        self.last = (low, high, sr);
        if low <= FOCUS_MIN_HZ + 0.5 {
            self.hp.on = false;
        } else {
            self.hp.set(true, low, sr);
        }
        if high >= FOCUS_MAX_HZ - 50.0 || high >= sr * 0.45 {
            self.lp.on = false;
        } else {
            self.lp.set(false, high, sr);
        }
    }

    /// True when both filters are off (full range).
    pub fn is_bypassed(&self) -> bool {
        !self.hp.on && !self.lp.on
    }

    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let l = self.lp.tick(0, self.hp.tick(0, l));
        let r = self.lp.tick(1, self.hp.tick(1, r));
        (l, r)
    }
}

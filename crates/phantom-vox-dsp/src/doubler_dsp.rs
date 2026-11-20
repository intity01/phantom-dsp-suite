//! Zero-allocation filter and fractional delay primitives for `VocalDoublerEngine`.

use std::f32::consts::TAU;

pub(crate) const DELAY_LEN: usize = 16384;
pub(crate) const DELAY_MASK: usize = DELAY_LEN - 1;

#[derive(Clone, Copy)]
pub(crate) struct OnePole {
    z: f32,
}

impl OnePole {
    pub(crate) const fn new() -> Self {
        Self { z: 0.0 }
    }

    #[inline]
    pub(crate) fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z = (1.0 - a) * x + a * self.z;
        self.z
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FormantBiquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl FormantBiquad {
    pub(crate) const fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    pub(crate) fn set_peaking(&mut self, freq_hz: f32, q: f32, gain_db: f32, sr: f32) {
        if gain_db.abs() < 0.05 {
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
            return;
        }
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = TAU * freq_hz.clamp(120.0, sr * 0.42) / sr;
        let alpha = w0.sin() / (2.0 * q.max(0.3));
        let cos_w = w0.cos();
        let a0 = 1.0 + alpha / a;
        self.b0 = (1.0 + alpha * a) / a0;
        self.b1 = (-2.0 * cos_w) / a0;
        self.b2 = (1.0 - alpha * a) / a0;
        self.a1 = (-2.0 * cos_w) / a0;
        self.a2 = (1.0 - alpha / a) / a0;
    }

    #[inline]
    pub(crate) fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// 4-point cubic Hermite fractional read from any power-of-two ring buffer.
#[inline]
pub(crate) fn read_ring_cubic_masked(
    buf: &[f32],
    mask: usize,
    write_pos: usize,
    delay: f32,
) -> f32 {
    let d_int = delay.floor() as usize;
    let frac = delay - d_int as f32;
    let i1 = write_pos.wrapping_sub(d_int) & mask;
    let i0 = i1.wrapping_add(1) & mask;
    let i2 = i1.wrapping_sub(1) & mask;
    let i3 = i1.wrapping_sub(2) & mask;
    let (y0, y1, y2, y3) = (buf[i0], buf[i1], buf[i2], buf[i3]);
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * frac + c2) * frac + c1) * frac + c0
}

#[inline]
pub(crate) fn read_delay_cubic(buf: &[f32], write_pos: usize, delay: f32) -> f32 {
    read_ring_cubic_masked(buf, DELAY_MASK, write_pos, delay)
}

//! Channel Target Routing & Mid/Side Processing
//! Implements pristine M/S encoding, decoding, and per-band stereo placement.

use std::f32::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTarget {
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}

impl Default for ChannelTarget {
    fn default() -> Self {
        Self::Stereo
    }
}

/// Convert Left/Right stereo to Mid/Side representation
/// Mid = (L + R) / sqrt(2)  [Center energy]
/// Side = (L - R) / sqrt(2) [Stereo width energy]
#[inline(always)]
pub fn encode_mid_side(left: f32, right: f32) -> (f32, f32) {
    let mid = (left + right) * FRAC_1_SQRT_2;
    let side = (left - right) * FRAC_1_SQRT_2;
    (mid, side)
}

/// Convert Mid/Side back to Left/Right stereo
/// Left = (Mid + Side) / sqrt(2)
/// Right = (Mid - Side) / sqrt(2)
#[inline(always)]
pub fn decode_mid_side(mid: f32, side: f32) -> (f32, f32) {
    let left = (mid + side) * FRAC_1_SQRT_2;
    let right = (mid - side) * FRAC_1_SQRT_2;
    (left, right)
}

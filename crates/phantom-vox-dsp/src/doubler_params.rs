//! Real-time parameter snapshot for `VocalDoublerEngine`.

use crate::focus_band::{FOCUS_MAX_HZ, FOCUS_MIN_HZ};
use serde::{Deserialize, Serialize};

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
    /// Doubled voices are high-passed here (20 Hz = off).
    #[serde(default = "focus_min")]
    pub focus_low_hz: f32,
    /// Doubled voices are low-passed here (20 kHz = off).
    #[serde(default = "focus_max")]
    pub focus_high_hz: f32,
}

fn focus_min() -> f32 {
    FOCUS_MIN_HZ
}

fn focus_max() -> f32 {
    FOCUS_MAX_HZ
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
            focus_low_hz: FOCUS_MIN_HZ,
            focus_high_hz: FOCUS_MAX_HZ,
        }
    }
}

//! Click-Free 10 ms Crossfaded Per-Band Solo Audition State
//! Smoothly ramps between normal EQ output and isolated band monitoring with zero clicks.

use crate::dsp_state::{process_solo_sample, BandSnapshot};
use crate::editor::NO_SOLO;
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{ChannelTarget, StereoBiquad};

const SOLO_RAMP_SEC: f32 = 0.010;

pub struct SoloCrossfade {
    filter: StereoBiquad,
    active_band: u8,
    mix: f32,
    ramp_step: f32,
}

impl Default for SoloCrossfade {
    fn default() -> Self {
        Self::new()
    }
}

impl SoloCrossfade {
    pub fn new() -> Self {
        Self {
            filter: StereoBiquad::new(),
            active_band: NO_SOLO,
            mix: 0.0,
            ramp_step: 1.0 / (SOLO_RAMP_SEC * 44100.0),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.ramp_step = 1.0 / (SOLO_RAMP_SEC * sample_rate.max(8000.0)).max(1.0);
    }

    pub fn reset(&mut self) {
        self.filter.reset();
        self.active_band = NO_SOLO;
        self.mix = 0.0;
    }

    /// Updates the audition filter and returns `(target_mix, placement)` for the current block.
    pub fn prepare(
        &mut self,
        params: &PhantomEqParams,
        requested: u8,
        sample_rate: f32,
    ) -> (f32, ChannelTarget) {
        let valid_req =
            (requested as usize) < NUM_BANDS && params.bands[requested as usize].used.value();
        let target = if valid_req {
            if requested != self.active_band {
                self.filter.reset();
                self.active_band = requested;
            }
            1.0
        } else {
            0.0
        };

        if (self.active_band as usize) < NUM_BANDS {
            let snap = BandSnapshot::from_band(&params.bands[self.active_band as usize]);
            self.filter.set_coeffs(snap.solo_coeffs(sample_rate));
            (target, snap.placement)
        } else {
            (0.0, ChannelTarget::Stereo)
        }
    }

    #[inline(always)]
    pub fn is_steady_full_solo(&self, target: f32) -> bool {
        self.mix >= 1.0 && target >= 1.0
    }

    /// Advances the 10 ms linear ramp and crossfades `eq` with the isolated band audition signal.
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn apply_sample(
        &mut self,
        pre_l: f32,
        pre_r: f32,
        eq_l: f32,
        eq_r: f32,
        target: f32,
        placement: ChannelTarget,
    ) -> (f32, f32) {
        if target > 0.5 {
            self.mix = (self.mix + self.ramp_step).min(1.0);
        } else if self.mix > 0.0 {
            self.mix = (self.mix - self.ramp_step).max(0.0);
        }

        if self.mix > 0.0 && (self.active_band as usize) < NUM_BANDS {
            let (sl, sr) = process_solo_sample(&mut self.filter, placement, pre_l, pre_r);
            let m = self.mix;
            let inv = 1.0 - m;
            (eq_l * inv + sl * m, eq_r * inv + sr * m)
        } else {
            (eq_l, eq_r)
        }
    }

    pub fn finish_block(&mut self, target: f32) {
        if self.mix <= 0.0 && target <= 0.0 {
            self.active_band = NO_SOLO;
        }
    }
}

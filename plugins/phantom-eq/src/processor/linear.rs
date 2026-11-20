//! Linear Phase FIR Synchronization, PDC-Aligned Bypass Crossfade & Zero-Alloc Chunk Processing
//! Synchronizes the 2x2 Stereo/L/R/M/S transfer matrix across all 24 cascaded bands and keeps
//! `LatencyBypassDelay` updated on every sample for exact `LATENCY_SAMPLES` (3071) PDC alignment.

use super::EqProcessor;
use crate::dsp_state::{capture_band_snapshots_with, scale_band_gain, BandSnapshot};
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{CascadedCoeffs, ChannelTarget, CharacterMode, LinearPhaseEngine, StereoCurveDb};

pub const LP_CHUNK_SIZE: usize = 256;

/// Tracks cached band snapshots and updates `LinearPhaseEngine` only when parameters change.
pub struct LinearPhaseTracker {
    last_snapshots: [BandSnapshot; NUM_BANDS],
    last_sr_bits: u32,
    dirty: bool,
}

impl Default for LinearPhaseTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearPhaseTracker {
    pub fn new() -> Self {
        Self {
            last_snapshots: [BandSnapshot::default(); NUM_BANDS],
            last_sr_bits: 0,
            dirty: true,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Synchronizes the FIR 2x2 transfer matrix if any used/enabled band, gain scale, or SR changed.
    /// Strictly zero heap allocations (uses fixed stack arrays).
    pub fn sync(
        &mut self,
        engine: &mut LinearPhaseEngine,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        sample_rate: f32,
        gain_scale: f32,
    ) {
        let mut snapshots = capture_band_snapshots_with(&params.bands, live);
        for snap in &mut snapshots {
            snap.gain = scale_band_gain(snap.filter_type, snap.gain, gain_scale);
        }
        let sr_bits = sample_rate.to_bits();
        if !self.dirty && sr_bits == self.last_sr_bits && snapshots == self.last_snapshots {
            return;
        }

        let coeffs: [CascadedCoeffs; NUM_BANDS] = std::array::from_fn(|i| {
            if snapshots[i].is_active() {
                snapshots[i].cascaded_coeffs(sample_rate, false)
            } else {
                CascadedCoeffs::default()
            }
        });

        engine.update_stereo_curves(sample_rate, |freq| {
            let mut curve = StereoCurveDb::default();
            for (snap, c) in snapshots.iter().zip(coeffs.iter()) {
                if !snap.is_active() {
                    continue;
                }
                let db = c.magnitude_db_at(freq, sample_rate);
                match snap.placement {
                    ChannelTarget::Stereo => {
                        curve.left += db;
                        curve.right += db;
                    }
                    ChannelTarget::Left => curve.left += db,
                    ChannelTarget::Right => curve.right += db,
                    ChannelTarget::Mid => curve.mid += db,
                    ChannelTarget::Side => curve.side += db,
                }
            }
            curve
        });

        self.last_snapshots = snapshots;
        self.last_sr_bits = sr_bits;
        self.dirty = false;
    }
}

impl EqProcessor {
    /// Processes Linear Phase FIR chunks while ALWAYS advancing `LatencyBypassDelay` per sample so
    /// toggling Bypass crossfades cleanly over 10 ms with exact `LATENCY_SAMPLES` (3071) alignment.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn process_linear_phase_chunks(
        &mut self,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        gain_scale: f32,
        left: &mut [f32],
        right: &mut [f32],
        char_mode: CharacterMode,
        auto_gain_en: bool,
        solo_target: f32,
        solo_placement: ChannelTarget,
    ) {
        self.lp_tracker.sync(
            &mut self.linear_phase,
            params,
            live,
            self.sample_rate,
            gain_scale,
        );
        let mut pre_l_buf = [0.0_f32; LP_CHUNK_SIZE];
        let mut pre_r_buf = [0.0_f32; LP_CHUNK_SIZE];
        let total = left.len();
        let mut offset = 0;

        while offset < total {
            let n = (total - offset).min(LP_CHUNK_SIZE);
            pre_l_buf[..n].copy_from_slice(&left[offset..offset + n]);
            pre_r_buf[..n].copy_from_slice(&right[offset..offset + n]);

            self.linear_phase.process_block(
                &mut left[offset..offset + n],
                &mut right[offset..offset + n],
            );

            for k in 0..n {
                let (dry_l, dry_r) = self
                    .latency_bypass_delay
                    .process_sample(pre_l_buf[k], pre_r_buf[k]);
                let wet_mix = self.bypass_xfade.next_wet();
                let out_gain = self.output_gain_smoother.next();

                let (out_l, out_r) = if wet_mix <= 0.0 {
                    (dry_l, dry_r)
                } else {
                    let (cl, cr) = self.character.process_stereo(
                        left[offset + k],
                        right[offset + k],
                        char_mode,
                    );
                    let (ag_l, ag_r) =
                        self.auto_gain
                            .process_stereo(dry_l, dry_r, cl, cr, auto_gain_en);
                    let (ml, mr) = self.solo.apply_sample(
                        dry_l,
                        dry_r,
                        ag_l,
                        ag_r,
                        solo_target,
                        solo_placement,
                    );
                    let inv = 1.0 - wet_mix;
                    (
                        dry_l * inv + (ml * out_gain) * wet_mix,
                        dry_r * inv + (mr * out_gain) * wet_mix,
                    )
                };

                self.output_meter.process_sample(out_l, out_r);
                left[offset + k] = out_l;
                right[offset + k] = out_r;
            }
            offset += n;
        }
    }
}

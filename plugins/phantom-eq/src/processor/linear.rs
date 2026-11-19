//! Linear Phase FIR Synchronization & Zero-Allocation Chunk Processing
//! Synchronizes the 2x2 Stereo/L/R/M/S transfer matrix across all 24 cascaded bands.

use crate::dsp_state::{capture_band_snapshots_with, BandSnapshot};
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{CascadedCoeffs, ChannelTarget, LinearPhaseEngine, StereoCurveDb};

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

    /// Synchronizes the FIR 2x2 transfer matrix if any used/enabled band or sample rate changed.
    /// Strictly zero heap allocations (uses fixed stack arrays).
    pub fn sync(
        &mut self,
        engine: &mut LinearPhaseEngine,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        sample_rate: f32,
    ) {
        let snapshots = capture_band_snapshots_with(&params.bands, live);
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

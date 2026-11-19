//! Lock-Free In-Process Multi-Instance Spectrum Collision Detector
//! Shares live post-EQ spectrum curves across up to 16 active plugin instances
//! in the same DAW process using purely atomic registers (zero locks, zero I/O, zero network).

use crate::spectrum::{NUM_DISPLAY_BINS, SPECTRUM_FLOOR_DB};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub const MAX_INSTANCES: usize = 16;
pub const COLLISION_FLOOR_DB: f32 = -45.0;
pub const COLLISION_PROXIMITY_DB: f32 = 6.0;

const FLOOR_BITS: u32 = SPECTRUM_FLOOR_DB.to_bits();

static SLOT_ACTIVE: [AtomicBool; MAX_INSTANCES] = [const { AtomicBool::new(false) }; MAX_INSTANCES];
static SLOT_HEARTBEAT: [AtomicU64; MAX_INSTANCES] = [const { AtomicU64::new(0) }; MAX_INSTANCES];
static SLOT_BINS_BITS: [[AtomicU32; NUM_DISPLAY_BINS]; MAX_INSTANCES] =
    [const { [const { AtomicU32::new(FLOOR_BITS) }; NUM_DISPLAY_BINS] }; MAX_INSTANCES];
static GLOBAL_HEARTBEAT: AtomicU64 = AtomicU64::new(1);
static FALLBACK_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// Zero-lock static registry for multi-instance frequency collision detection.
#[derive(Debug, Clone, Copy, Default)]
pub struct CollisionRegistry;

impl CollisionRegistry {
    /// Claims an available instance slot (`0..MAX_INSTANCES`), or wraps safely if all 16 are active.
    pub fn register_instance() -> usize {
        for (slot, active) in SLOT_ACTIVE.iter().enumerate() {
            if active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                Self::clear_slot_bins(slot);
                let tick = GLOBAL_HEARTBEAT.fetch_add(1, Ordering::Relaxed);
                SLOT_HEARTBEAT[slot].store(tick, Ordering::Release);
                return slot;
            }
        }

        let fallback = FALLBACK_CURSOR.fetch_add(1, Ordering::Relaxed) % MAX_INSTANCES;
        SLOT_ACTIVE[fallback].store(true, Ordering::Release);
        Self::clear_slot_bins(fallback);
        let tick = GLOBAL_HEARTBEAT.fetch_add(1, Ordering::Relaxed);
        SLOT_HEARTBEAT[fallback].store(tick, Ordering::Release);
        fallback
    }

    /// Releases an instance slot and resets its stored spectrum bins to floor.
    pub fn unregister_instance(slot: usize) {
        if slot >= MAX_INSTANCES {
            return;
        }
        SLOT_ACTIVE[slot].store(false, Ordering::Release);
        SLOT_HEARTBEAT[slot].store(0, Ordering::Release);
        Self::clear_slot_bins(slot);
    }

    /// Publishes a 300-bin display spectrum (in dB) for `slot` and bumps its atomic heartbeat.
    #[inline]
    pub fn publish_spectrum(slot: usize, bins_db: &[f32; NUM_DISPLAY_BINS]) {
        if slot >= MAX_INSTANCES {
            return;
        }
        SLOT_ACTIVE[slot].store(true, Ordering::Release);
        let dst_row = &SLOT_BINS_BITS[slot];
        for (cell, &v) in dst_row.iter().zip(bins_db.iter()) {
            let clean = if v.is_finite() { v } else { SPECTRUM_FLOOR_DB };
            cell.store(clean.to_bits(), Ordering::Relaxed);
        }
        let tick = GLOBAL_HEARTBEAT.fetch_add(1, Ordering::Relaxed);
        SLOT_HEARTBEAT[slot].store(tick, Ordering::Release);
    }

    /// Compares `my_bins_db` against all other active instance slots.
    /// Wherever both `my_bins_db[i] > -45.0 dB` and `other_bins_db[i] > -45.0 dB` within 6 dB
    /// of each other, writes a smooth `0.0..1.0` collision intensity into `out_collision_0_to_1[i]`.
    /// Returns the number of other active instances detected.
    pub fn compute_collision_bins(
        my_slot: usize,
        my_bins_db: &[f32; NUM_DISPLAY_BINS],
        out_collision_0_to_1: &mut [f32; NUM_DISPLAY_BINS],
    ) -> usize {
        out_collision_0_to_1.fill(0.0);
        let mut other_active_count = 0usize;

        for slot in 0..MAX_INSTANCES {
            if slot == my_slot
                || !SLOT_ACTIVE[slot].load(Ordering::Acquire)
                || SLOT_HEARTBEAT[slot].load(Ordering::Acquire) == 0
            {
                continue;
            }

            other_active_count += 1;
            let other_row = &SLOT_BINS_BITS[slot];

            for i in 0..NUM_DISPLAY_BINS {
                let my_db = my_bins_db[i];
                if !my_db.is_finite() || my_db <= COLLISION_FLOOR_DB {
                    continue;
                }

                let other_db = f32::from_bits(other_row[i].load(Ordering::Relaxed));
                if !other_db.is_finite() || other_db <= COLLISION_FLOOR_DB {
                    continue;
                }

                let diff_db = (my_db - other_db).abs();
                if diff_db >= COLLISION_PROXIMITY_DB {
                    continue;
                }

                let proximity = 1.0 - (diff_db / COLLISION_PROXIMITY_DB);
                let min_db = my_db.min(other_db);
                let energy = ((min_db - COLLISION_FLOOR_DB) / 12.0)
                    .clamp(0.0, 1.0)
                    .sqrt();
                let intensity = (proximity * energy).clamp(0.0, 1.0);
                if intensity > out_collision_0_to_1[i] {
                    out_collision_0_to_1[i] = intensity;
                }
            }
        }

        other_active_count
    }

    /// Returns whether `slot` is currently registered and active.
    #[inline]
    pub fn is_slot_active(slot: usize) -> bool {
        slot < MAX_INSTANCES && SLOT_ACTIVE[slot].load(Ordering::Acquire)
    }

    /// Returns the latest heartbeat counter for `slot` (`0` if inactive).
    #[inline]
    pub fn slot_heartbeat(slot: usize) -> u64 {
        if slot < MAX_INSTANCES {
            SLOT_HEARTBEAT[slot].load(Ordering::Acquire)
        } else {
            0
        }
    }

    /// Returns the total count of currently active instance slots.
    #[inline]
    pub fn active_instance_count() -> usize {
        SLOT_ACTIVE
            .iter()
            .filter(|a| a.load(Ordering::Acquire))
            .count()
    }

    #[inline]
    fn clear_slot_bins(slot: usize) {
        for cell in &SLOT_BINS_BITS[slot] {
            cell.store(FLOOR_BITS, Ordering::Relaxed);
        }
    }
}

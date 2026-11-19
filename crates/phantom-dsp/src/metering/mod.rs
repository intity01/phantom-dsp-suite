//! Output Stereo Peak & ITU-R BS.1770-4 / EBU R128 LUFS Metering
//! Fixed 100 ms sub-block ring buffer (400 ms Momentary, 3.0 s Short-Term),
//! smooth stereo Peak + Peak Hold, and lock-free `SharedMeters` atomics — zero allocations.

mod k_weight;

pub(crate) use k_weight::KWeightFilter;

use std::sync::atomic::{AtomicU32, Ordering};

pub const MOMENTARY_BLOCKS: usize = 4; // 4 x 100 ms = 400 ms
pub const SHORT_TERM_BLOCKS: usize = 30; // 30 x 100 ms = 3.0 s
pub const PEAK_FLOOR_DB: f32 = -100.0;
pub const LUFS_FLOOR: f32 = -70.0;

/// Immutable stereo peak & loudness snapshot published to the GUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSnapshot {
    pub peak_l_db: f32,
    pub peak_r_db: f32,
    pub hold_l_db: f32,
    pub hold_r_db: f32,
    pub momentary_lufs: f32,
    pub short_term_lufs: f32,
}

impl Default for MeterSnapshot {
    fn default() -> Self {
        Self {
            peak_l_db: PEAK_FLOOR_DB,
            peak_r_db: PEAK_FLOOR_DB,
            hold_l_db: PEAK_FLOOR_DB,
            hold_r_db: PEAK_FLOOR_DB,
            momentary_lufs: LUFS_FLOOR,
            short_term_lufs: LUFS_FLOOR,
        }
    }
}

/// Lock-free atomic meter bus between audio thread (`Relaxed` stores) and 60 fps GUI.
pub struct SharedMeters {
    // [peak_l, peak_r, hold_l, hold_r, momentary_lufs, short_term_lufs, makeup_db]
    slots: [AtomicU32; 7],
}

impl Default for SharedMeters {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedMeters {
    pub fn new() -> Self {
        let d = MeterSnapshot::default();
        let vals = [
            d.peak_l_db,
            d.peak_r_db,
            d.hold_l_db,
            d.hold_r_db,
            d.momentary_lufs,
            d.short_term_lufs,
            0.0,
        ];
        Self {
            slots: vals.map(|v| AtomicU32::new(v.to_bits())),
        }
    }

    #[inline]
    pub fn store(&self, s: MeterSnapshot) {
        let vals = [
            s.peak_l_db,
            s.peak_r_db,
            s.hold_l_db,
            s.hold_r_db,
            s.momentary_lufs,
            s.short_term_lufs,
        ];
        for (slot, val) in self.slots[..6].iter().zip(vals) {
            slot.store(val.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn load(&self) -> MeterSnapshot {
        let f = |i: usize| f32::from_bits(self.slots[i].load(Ordering::Relaxed));
        MeterSnapshot {
            peak_l_db: f(0),
            peak_r_db: f(1),
            hold_l_db: f(2),
            hold_r_db: f(3),
            momentary_lufs: f(4),
            short_term_lufs: f(5),
        }
    }

    #[inline]
    pub fn store_makeup_db(&self, db: f32) {
        self.slots[6].store(db.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn load_makeup_db(&self) -> f32 {
        f32::from_bits(self.slots[6].load(Ordering::Relaxed))
    }

    pub fn reset(&self) {
        self.store(MeterSnapshot::default());
        self.store_makeup_db(0.0);
    }
}

/// Real-time stereo peak + ITU-R BS.1770-4 / EBU R128 loudness meter (zero heap allocations).
pub struct OutputMeter {
    k_weight: KWeightFilter,
    block_ring: [f32; SHORT_TERM_BLOCKS],
    ring_write_idx: usize,
    filled_blocks: usize,
    block_sum_sq: f64,
    block_samples: usize,
    samples_per_block: usize,
    momentary_lufs: f32,
    short_term_lufs: f32,
    peak_lin: [f32; 2],
    hold_lin: [f32; 2],
    hold_timer: [usize; 2],
    peak_decay: f32,
    hold_decay: f32,
    hold_duration_samples: usize,
}

impl Default for OutputMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputMeter {
    pub fn new() -> Self {
        let mut meter = Self {
            k_weight: KWeightFilter::new(48000.0),
            block_ring: [0.0; SHORT_TERM_BLOCKS],
            ring_write_idx: 0,
            filled_blocks: 0,
            block_sum_sq: 0.0,
            block_samples: 0,
            samples_per_block: 4800,
            momentary_lufs: LUFS_FLOOR,
            short_term_lufs: LUFS_FLOOR,
            peak_lin: [0.0; 2],
            hold_lin: [0.0; 2],
            hold_timer: [0; 2],
            peak_decay: 0.9999,
            hold_decay: 0.99995,
            hold_duration_samples: 72000,
        };
        meter.set_sample_rate(48000.0);
        meter
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8000.0);
        self.k_weight.set_sample_rate(sr);
        self.samples_per_block = (sr * 0.1).round().max(1.0) as usize;
        self.peak_decay = (-1.0 / (0.300 * sr)).exp();
        self.hold_decay = (-1.0 / (0.600 * sr)).exp();
        self.hold_duration_samples = (sr * 1.5).round() as usize;
    }

    pub fn reset(&mut self) {
        self.k_weight.reset();
        self.block_ring = [0.0; SHORT_TERM_BLOCKS];
        self.ring_write_idx = 0;
        self.filled_blocks = 0;
        self.block_sum_sq = 0.0;
        self.block_samples = 0;
        self.momentary_lufs = LUFS_FLOOR;
        self.short_term_lufs = LUFS_FLOOR;
        self.peak_lin = [0.0; 2];
        self.reset_peak_hold();
    }

    pub fn reset_peak_hold(&mut self) {
        self.hold_lin = [0.0; 2];
        self.hold_timer = [0; 2];
    }

    #[inline]
    pub fn process_sample(&mut self, in_l: f32, in_r: f32) {
        let abs = [
            if in_l.is_finite() { in_l.abs() } else { 0.0 },
            if in_r.is_finite() { in_r.abs() } else { 0.0 },
        ];
        for ch in 0..2 {
            self.peak_lin[ch] = abs[ch].max(self.peak_lin[ch] * self.peak_decay);
            if abs[ch] >= self.hold_lin[ch] {
                self.hold_lin[ch] = abs[ch];
                self.hold_timer[ch] = self.hold_duration_samples;
            } else if self.hold_timer[ch] > 0 {
                self.hold_timer[ch] -= 1;
            } else {
                self.hold_lin[ch] *= self.hold_decay;
            }
        }

        // K-weighted stereo power sum (G_L = G_R = 1.0 per ITU-R BS.1770-4)
        let (kl, kr) = self.k_weight.process_stereo(in_l, in_r);
        self.block_sum_sq += kl * kl + kr * kr;
        self.block_samples += 1;
        if self.block_samples >= self.samples_per_block {
            let mean_sq = (self.block_sum_sq / self.samples_per_block as f64) as f32;
            self.block_ring[self.ring_write_idx] = mean_sq;
            self.ring_write_idx = (self.ring_write_idx + 1) % SHORT_TERM_BLOCKS;
            self.filled_blocks = (self.filled_blocks + 1).min(SHORT_TERM_BLOCKS);
            self.block_sum_sq = 0.0;
            self.block_samples = 0;
            self.momentary_lufs = self.window_lufs(MOMENTARY_BLOCKS);
            self.short_term_lufs = self.window_lufs(SHORT_TERM_BLOCKS);
        }
    }

    pub fn process_block(&mut self, left: &[f32], right: &[f32]) {
        for (&l, &r) in left.iter().zip(right.iter()) {
            self.process_sample(l, r);
        }
    }

    #[inline]
    pub fn snapshot(&self) -> MeterSnapshot {
        MeterSnapshot {
            peak_l_db: lin_to_peak_db(self.peak_lin[0]),
            peak_r_db: lin_to_peak_db(self.peak_lin[1]),
            hold_l_db: lin_to_peak_db(self.hold_lin[0]),
            hold_r_db: lin_to_peak_db(self.hold_lin[1]),
            momentary_lufs: self.momentary_lufs,
            short_term_lufs: self.short_term_lufs,
        }
    }

    #[inline]
    pub fn publish(&self, shared: &SharedMeters) {
        shared.store(self.snapshot());
    }

    fn window_lufs(&self, max_blocks: usize) -> f32 {
        let count = self.filled_blocks.min(max_blocks);
        if count == 0 {
            return LUFS_FLOOR;
        }
        let mut sum = 0.0_f64;
        for k in 0..count {
            let idx = (self.ring_write_idx + SHORT_TERM_BLOCKS - 1 - k) % SHORT_TERM_BLOCKS;
            sum += self.block_ring[idx] as f64;
        }
        let mean_power = sum / count as f64;
        if mean_power <= 1e-10 {
            LUFS_FLOOR
        } else {
            (-0.691 + 10.0 * mean_power.log10()).max(LUFS_FLOOR as f64) as f32
        }
    }
}

#[inline]
fn lin_to_peak_db(lin: f32) -> f32 {
    if lin <= 1e-5 {
        PEAK_FLOOR_DB
    } else {
        (20.0 * lin.log10()).clamp(PEAK_FLOOR_DB, 24.0)
    }
}

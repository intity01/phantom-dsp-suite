//! Lock-Free Real-Time Audio -> GUI Telemetry Bridge for Phantom Vox
//!
//! Zero allocations and zero mutexes: publishes live phase correlation (`-1..+1`),
//! alignment timing offset (`±ms`), confidence (`0..1`), sidechain status, L/R input/output
//! peaks, PDC latency, and a scrolling Guide/Dub waveform ring buffer to the 60 fps GUI.

use phantom_dsp::PEAK_FLOOR_DB;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// Number of decimated envelope bins in the scrolling Guide/Dub waveform strip (~1 s window).
pub const WAVE_BINS: usize = 96;

/// Immutable telemetry snapshot consumed once per GUI frame.
#[derive(Debug, Clone, Copy)]
pub struct VoxTelemetrySnapshot {
    pub phase_corr: f32,
    pub offset_ms: f32,
    pub confidence: f32,
    pub sidechain_active: bool,
    pub in_peak_l_db: f32,
    pub in_peak_r_db: f32,
    pub out_peak_l_db: f32,
    pub out_peak_r_db: f32,
    pub latency_samples: u32,
    pub sample_rate: f32,
}

impl Default for VoxTelemetrySnapshot {
    fn default() -> Self {
        Self {
            phase_corr: 1.0,
            offset_ms: 0.0,
            confidence: 0.0,
            sidechain_active: false,
            in_peak_l_db: PEAK_FLOOR_DB,
            in_peak_r_db: PEAK_FLOOR_DB,
            out_peak_l_db: PEAK_FLOOR_DB,
            out_peak_r_db: PEAK_FLOOR_DB,
            latency_samples: 0,
            sample_rate: 48_000.0,
        }
    }
}

/// Lock-free shared telemetry bus owned via `Arc` by audio DSP and editor.
pub struct SharedVoxTelemetry {
    phase_corr: AtomicU32,
    offset_ms: AtomicU32,
    confidence: AtomicU32,
    sidechain_active: AtomicBool,
    in_peak_l_db: AtomicU32,
    in_peak_r_db: AtomicU32,
    out_peak_l_db: AtomicU32,
    out_peak_r_db: AtomicU32,
    latency_samples: AtomicU32,
    sample_rate: AtomicU32,
    guide_wave: [AtomicU32; WAVE_BINS],
    dub_wave: [AtomicU32; WAVE_BINS],
    wave_write: AtomicUsize,
}

impl Default for SharedVoxTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedVoxTelemetry {
    pub fn new() -> Self {
        let d = VoxTelemetrySnapshot::default();
        Self {
            phase_corr: AtomicU32::new(d.phase_corr.to_bits()),
            offset_ms: AtomicU32::new(d.offset_ms.to_bits()),
            confidence: AtomicU32::new(d.confidence.to_bits()),
            sidechain_active: AtomicBool::new(false),
            in_peak_l_db: AtomicU32::new(d.in_peak_l_db.to_bits()),
            in_peak_r_db: AtomicU32::new(d.in_peak_r_db.to_bits()),
            out_peak_l_db: AtomicU32::new(d.out_peak_l_db.to_bits()),
            out_peak_r_db: AtomicU32::new(d.out_peak_r_db.to_bits()),
            latency_samples: AtomicU32::new(0),
            sample_rate: AtomicU32::new(d.sample_rate.to_bits()),
            guide_wave: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            dub_wave: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            wave_write: AtomicUsize::new(0),
        }
    }

    pub fn reset(&self) {
        let d = VoxTelemetrySnapshot::default();
        self.phase_corr
            .store(d.phase_corr.to_bits(), Ordering::Relaxed);
        self.offset_ms
            .store(d.offset_ms.to_bits(), Ordering::Relaxed);
        self.confidence
            .store(d.confidence.to_bits(), Ordering::Relaxed);
        self.sidechain_active.store(false, Ordering::Relaxed);
        self.in_peak_l_db
            .store(d.in_peak_l_db.to_bits(), Ordering::Relaxed);
        self.in_peak_r_db
            .store(d.in_peak_r_db.to_bits(), Ordering::Relaxed);
        self.out_peak_l_db
            .store(d.out_peak_l_db.to_bits(), Ordering::Relaxed);
        self.out_peak_r_db
            .store(d.out_peak_r_db.to_bits(), Ordering::Relaxed);
        for bin in &self.guide_wave {
            bin.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        for bin in &self.dub_wave {
            bin.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        self.wave_write.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn store_sample_rate(&self, sr: f32) {
        if sr.is_finite() && sr > 0.0 {
            self.sample_rate.store(sr.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn store_latency(&self, samples: u32) {
        self.latency_samples.store(samples, Ordering::Relaxed);
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn store_metrics(
        &self,
        phase_corr: f32,
        offset_ms: f32,
        confidence: f32,
        sidechain_active: bool,
        in_peak_l_db: f32,
        in_peak_r_db: f32,
        out_peak_l_db: f32,
        out_peak_r_db: f32,
    ) {
        self.phase_corr
            .store(phase_corr.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed);
        self.offset_ms.store(offset_ms.to_bits(), Ordering::Relaxed);
        self.confidence
            .store(confidence.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        self.sidechain_active
            .store(sidechain_active, Ordering::Relaxed);
        self.in_peak_l_db
            .store(in_peak_l_db.to_bits(), Ordering::Relaxed);
        self.in_peak_r_db
            .store(in_peak_r_db.to_bits(), Ordering::Relaxed);
        self.out_peak_l_db
            .store(out_peak_l_db.to_bits(), Ordering::Relaxed);
        self.out_peak_r_db
            .store(out_peak_r_db.to_bits(), Ordering::Relaxed);
    }

    /// Appends one decimated Guide & Dub amplitude bin into the scrolling ring buffer.
    #[inline]
    pub fn push_wave_bin(&self, guide_peak: f32, dub_peak: f32) {
        let idx = self.wave_write.load(Ordering::Relaxed) % WAVE_BINS;
        self.guide_wave[idx].store(guide_peak.clamp(0.0, 1.5).to_bits(), Ordering::Relaxed);
        self.dub_wave[idx].store(dub_peak.clamp(0.0, 1.5).to_bits(), Ordering::Relaxed);
        self.wave_write
            .store((idx + 1) % WAVE_BINS, Ordering::Release);
    }

    /// Reads an atomic snapshot of all scalar metrics.
    #[inline]
    pub fn load(&self) -> VoxTelemetrySnapshot {
        let load_f = |a: &AtomicU32| f32::from_bits(a.load(Ordering::Relaxed));
        VoxTelemetrySnapshot {
            phase_corr: load_f(&self.phase_corr).clamp(-1.0, 1.0),
            offset_ms: load_f(&self.offset_ms),
            confidence: load_f(&self.confidence).clamp(0.0, 1.0),
            sidechain_active: self.sidechain_active.load(Ordering::Relaxed),
            in_peak_l_db: load_f(&self.in_peak_l_db),
            in_peak_r_db: load_f(&self.in_peak_r_db),
            out_peak_l_db: load_f(&self.out_peak_l_db),
            out_peak_r_db: load_f(&self.out_peak_r_db),
            latency_samples: self.latency_samples.load(Ordering::Relaxed),
            sample_rate: load_f(&self.sample_rate).max(8000.0),
        }
    }

    /// Copies the scrolling waveform history oldest -> newest into caller-owned stack arrays.
    pub fn load_waveforms(&self, guide_out: &mut [f32; WAVE_BINS], dub_out: &mut [f32; WAVE_BINS]) {
        let head = self.wave_write.load(Ordering::Acquire) % WAVE_BINS;
        for i in 0..WAVE_BINS {
            let slot = (head + i) % WAVE_BINS;
            guide_out[i] = f32::from_bits(self.guide_wave[slot].load(Ordering::Relaxed));
            dub_out[i] = f32::from_bits(self.dub_wave[slot].load(Ordering::Relaxed));
        }
    }
}

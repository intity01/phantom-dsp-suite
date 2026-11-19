//! Multi-Stage Real-Time FFT Spectrum Analyzer (Pre-EQ, Post-EQ, Side + Peak Hold/Freeze)
//! 100% zero-allocation on both audio thread (fixed preallocated collectors) and GUI thread
//! (preallocated `realfft` scratch via `process_with_scratch`). Supports Pro-Q 4.5 dB/oct tilt.

mod processor;

pub use processor::SpectrumProcessor;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const FFT_SIZE: usize = 2048;
pub const NUM_DISPLAY_BINS: usize = 300;
pub const DEFAULT_TILT_DB_PER_OCT: f32 = 4.5;
pub const SPECTRUM_FLOOR_DB: f32 = -120.0;

#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Post-EQ spectrum bins in dB (synced alias for backward compatibility)
    pub bins_db: [f32; NUM_DISPLAY_BINS],
    /// Post-EQ spectrum bins in dB
    pub post_bins_db: [f32; NUM_DISPLAY_BINS],
    /// Pre-EQ spectrum bins in dB
    pub pre_bins_db: [f32; NUM_DISPLAY_BINS],
    /// Side (L-R) spectrum bins in dB
    pub side_bins_db: [f32; NUM_DISPLAY_BINS],
    /// Peak-hold envelope of Post-EQ spectrum in dB
    pub peak_bins_db: [f32; NUM_DISPLAY_BINS],
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            post_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            pre_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            side_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
            peak_bins_db: [SPECTRUM_FLOOR_DB; NUM_DISPLAY_BINS],
        }
    }
}

pub(crate) struct StageChannel {
    buf: Mutex<(Box<[f32; FFT_SIZE]>, usize)>,
    ready: AtomicBool,
}

impl StageChannel {
    fn new() -> Self {
        let arr: Box<[f32; FFT_SIZE]> = vec![0.0; FFT_SIZE].into_boxed_slice().try_into().unwrap();
        Self {
            buf: Mutex::new((arr, 0)),
            ready: AtomicBool::new(false),
        }
    }

    #[inline]
    fn push_slice(&self, samples: &[f32]) {
        if let Ok(mut guard) = self.buf.try_lock() {
            let (buf, len) = &mut *guard;
            if *len < FFT_SIZE {
                let take = (FFT_SIZE - *len).min(samples.len());
                buf[*len..*len + take].copy_from_slice(&samples[..take]);
                *len += take;
                if *len >= FFT_SIZE {
                    self.ready.store(true, Ordering::Release);
                }
            }
        }
    }

    #[inline]
    fn push_stereo(&self, left: &[f32], right: &[f32], side: bool) {
        if let Ok(mut guard) = self.buf.try_lock() {
            let (buf, len) = &mut *guard;
            let count = left.len().min(right.len());
            if *len < FFT_SIZE {
                let take = (FFT_SIZE - *len).min(count);
                let dst = &mut buf[*len..*len + take];
                for (d, (&l, &r)) in dst.iter_mut().zip(left.iter().zip(right.iter())) {
                    *d = if side { (l - r) * 0.5 } else { (l + r) * 0.5 };
                }
                *len += take;
                if *len >= FFT_SIZE {
                    self.ready.store(true, Ordering::Release);
                }
            }
        }
    }

    pub(crate) fn take_windowed(&self, window: &[f32], dst: &mut [f32]) -> bool {
        if !self.ready.load(Ordering::Acquire) {
            return false;
        }
        if let Ok(mut guard) = self.buf.try_lock() {
            let (buf, len) = &mut *guard;
            if *len >= FFT_SIZE {
                for ((d, &b), &w) in dst.iter_mut().zip(buf.iter()).zip(window.iter()) {
                    *d = b * w;
                }
                *len = 0;
                self.ready.store(false, Ordering::Release);
                return true;
            }
        }
        false
    }

    pub(crate) fn drain(&self) {
        if self.ready.load(Ordering::Acquire) {
            if let Ok(mut guard) = self.buf.try_lock() {
                guard.1 = 0;
                self.ready.store(false, Ordering::Release);
            }
        }
    }
}

/// Shared multi-stage spectrum bridge between Audio thread and GUI thread.
pub struct SharedSpectrum {
    pub(crate) pre: StageChannel,
    pub(crate) post: StageChannel,
    pub(crate) side: StageChannel,
    pub display_data: Mutex<SpectrumData>,
}

impl Default for SharedSpectrum {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedSpectrum {
    pub fn new() -> Self {
        Self {
            pre: StageChannel::new(),
            post: StageChannel::new(),
            side: StageChannel::new(),
            display_data: Mutex::new(SpectrumData::default()),
        }
    }

    #[inline]
    pub fn push_pre_samples(&self, samples: &[f32]) {
        self.pre.push_slice(samples);
    }

    #[inline]
    pub fn push_post_samples(&self, samples: &[f32]) {
        self.post.push_slice(samples);
    }

    #[inline]
    pub fn push_side_samples(&self, samples: &[f32]) {
        self.side.push_slice(samples);
    }

    /// Backward-compatible mono push (routes to Post-EQ spectrum).
    #[inline]
    pub fn push_samples(&self, samples: &[f32]) {
        self.post.push_slice(samples);
    }

    /// Sums stereo Pre-EQ `(L+R)/2` directly into the fixed collector with zero allocations.
    #[inline]
    pub fn push_stereo_pre(&self, left: &[f32], right: &[f32]) {
        self.pre.push_stereo(left, right, false);
    }

    /// Pushes stereo Post-EQ `(L+R)/2` and Side `(L-R)/2` directly with zero allocations.
    #[inline]
    pub fn push_stereo_post_and_side(&self, left: &[f32], right: &[f32]) {
        self.post.push_stereo(left, right, false);
        self.side.push_stereo(left, right, true);
    }
}

/// Returns the logarithmic center frequency in Hz for a display bin index `0..NUM_DISPLAY_BINS`.
#[inline]
pub fn display_bin_freq(bin_idx: usize, sample_rate: f32) -> f32 {
    let min_f = 20.0_f32;
    let max_f = 20000.0_f32.min(sample_rate.max(1000.0) * 0.499);
    let norm = bin_idx.min(NUM_DISPLAY_BINS - 1) as f32 / (NUM_DISPLAY_BINS - 1) as f32;
    (min_f.ln() + norm * (max_f.ln() - min_f.ln())).exp()
}

//! Real-time FFT Audio Spectrum Analyzer
//! High-performance, zero-allocation during audio thread processing.
//! Provides log-scaled frequency spectrum with smooth peak decay.

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const FFT_SIZE: usize = 2048;
pub const NUM_DISPLAY_BINS: usize = 300;

pub struct SpectrumData {
    /// Log-spaced frequency amplitudes in dB for GUI rendering (-100 dB to +12 dB)
    pub bins_db: [f32; NUM_DISPLAY_BINS],
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            bins_db: [-100.0; NUM_DISPLAY_BINS],
        }
    }
}

/// Shared spectrum handle between Audio thread and GUI thread
pub struct SharedSpectrum {
    /// Double buffer for audio samples
    input_buffer: Mutex<Vec<f32>>,
    new_data_available: AtomicBool,
    /// Processed dB spectrum bins for GUI
    pub display_data: Mutex<SpectrumData>,
}

impl SharedSpectrum {
    pub fn new() -> Self {
        Self {
            input_buffer: Mutex::new(Vec::with_capacity(FFT_SIZE)),
            new_data_available: AtomicBool::new(false),
            display_data: Mutex::new(SpectrumData::default()),
        }
    }

    /// Audio thread: pushes audio samples into collector
    #[inline]
    pub fn push_samples(&self, samples: &[f32]) {
        if let Ok(mut buf) = self.input_buffer.try_lock() {
            if buf.len() < FFT_SIZE {
                let to_take = (FFT_SIZE - buf.len()).min(samples.len());
                buf.extend_from_slice(&samples[..to_take]);
                if buf.len() >= FFT_SIZE {
                    self.new_data_available.store(true, Ordering::Release);
                }
            }
        }
    }
}

/// GUI Thread worker to process FFT and maintain decay
pub struct SpectrumProcessor {
    fft_plan: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    decay_bins_db: [f32; NUM_DISPLAY_BINS],
}

impl SpectrumProcessor {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_plan = planner.plan_fft_forward(FFT_SIZE);

        // Pre-compute Hann window
        let mut window = Vec::with_capacity(FFT_SIZE);
        for i in 0..FFT_SIZE {
            let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / FFT_SIZE as f32).cos());
            window.push(w);
        }

        let fft_in = fft_plan.make_input_vec();
        let fft_out = fft_plan.make_output_vec();

        Self {
            fft_plan,
            window,
            fft_in,
            fft_out,
            decay_bins_db: [-100.0; NUM_DISPLAY_BINS],
        }
    }

    /// Updates spectrum calculations from shared audio buffer.
    /// Call this once per GUI frame (e.g. 60Hz).
    pub fn update(&mut self, shared: &SharedSpectrum, sample_rate: f32) {
        let mut has_new_fft = false;

        if shared.new_data_available.load(Ordering::Acquire) {
            if let Ok(mut buf) = shared.input_buffer.try_lock() {
                if buf.len() >= FFT_SIZE {
                    // Apply Hann window
                    for i in 0..FFT_SIZE {
                        self.fft_in[i] = buf[i] * self.window[i];
                    }
                    buf.clear();
                    shared.new_data_available.store(false, Ordering::Release);
                    has_new_fft = true;
                }
            }
        }

        if has_new_fft {
            let _ = self.fft_plan.process(&mut self.fft_in, &mut self.fft_out);

            // Map linear FFT bins to logarithmic display bins (20 Hz to 20,000 Hz)
            let min_freq = 20.0f32;
            let max_freq = 20000.0f32.min(sample_rate * 0.499);
            let log_min = min_freq.ln();
            let log_max = max_freq.ln();
            let num_fft_bins = self.fft_out.len(); // FFT_SIZE / 2 + 1
            let bin_hz = sample_rate / FFT_SIZE as f32;

            for i in 0..NUM_DISPLAY_BINS {
                let norm = i as f32 / (NUM_DISPLAY_BINS - 1) as f32;
                let freq = (log_min + norm * (log_max - log_min)).exp();
                let bin_idx = (freq / bin_hz).round() as usize;
                let bin_idx = bin_idx.clamp(1, num_fft_bins - 1);

                let mag = self.fft_out[bin_idx].norm() / (FFT_SIZE as f32 * 0.5);
                let db = if mag > 1e-6 {
                    20.0 * mag.log10()
                } else {
                    -120.0
                };

                // Fast attack, smooth decay
                if db > self.decay_bins_db[i] {
                    self.decay_bins_db[i] = db;
                } else {
                    // Smooth decay (~2.5 dB per frame at 60fps)
                    self.decay_bins_db[i] = (self.decay_bins_db[i] - 1.8).max(-120.0);
                }
            }
        } else {
            // Natural falloff when audio pauses
            for i in 0..NUM_DISPLAY_BINS {
                self.decay_bins_db[i] = (self.decay_bins_db[i] - 1.2).max(-120.0);
            }
        }

        // Copy to shared display data
        if let Ok(mut disp) = shared.display_data.try_lock() {
            disp.bins_db.copy_from_slice(&self.decay_bins_db);
        }
    }
}

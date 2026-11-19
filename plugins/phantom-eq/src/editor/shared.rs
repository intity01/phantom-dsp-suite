//! Lock-free Audio <-> GUI Shared State Bundle for the Phantom EQ Editor
//! Spectrum/meter/GR telemetry, Solo Audition bus, sample rate, latency, collision slot and the
//! authoritative `LiveParamMirror` (instant GUI parameter edits even while Cubase is stopped).

use crate::dsp_state::SharedBandGr;
use crate::live_mirror::LiveParamMirror;
use crate::params::NUM_BANDS;
use phantom_dsp::{SharedMeters, SharedSpectrum};
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

pub const NO_SOLO: u8 = u8::MAX;

pub struct EditorShared {
    pub spectrum: Arc<SharedSpectrum>,
    pub meters: Arc<SharedMeters>,
    pub band_gr: Arc<SharedBandGr>,
    pub solo_band: Arc<AtomicU8>,
    pub sample_rate_bits: Arc<AtomicU32>,
    pub latency_samples: Arc<AtomicU32>,
    pub live: Arc<LiveParamMirror>,
    pub collision_slot: usize,
}

impl EditorShared {
    pub fn sample_rate(&self) -> f32 {
        let sr = f32::from_bits(self.sample_rate_bits.load(Ordering::Relaxed));
        if sr.is_finite() && sr >= 8000.0 {
            sr
        } else {
            44100.0
        }
    }

    pub fn solo_band(&self) -> Option<usize> {
        let v = self.solo_band.load(Ordering::Relaxed) as usize;
        (v < NUM_BANDS).then_some(v)
    }

    pub fn set_solo(&self, band: Option<usize>) {
        let raw = band
            .filter(|&i| i < NUM_BANDS)
            .map(|i| i as u8)
            .unwrap_or(NO_SOLO);
        self.solo_band.store(raw, Ordering::Relaxed);
    }

    pub fn clear_solo(&self) {
        self.solo_band.store(NO_SOLO, Ordering::Relaxed);
    }

    pub fn latency_samples(&self) -> u32 {
        self.latency_samples.load(Ordering::Relaxed)
    }
}

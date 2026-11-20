//! Zero-dependency stereo playback engine (`winmm.dll` double-buffered on Windows,
//! smooth playhead timer stub on Linux/macOS) with instant A/B toggle, Loop, Solo/Mute, and Mono Check.

use std::sync::atomic::Ordering;
use std::sync::Arc;

pub use crate::playback_mix::{PlaybackMixData, PlaybackTrack, TransportShared};

pub struct AudioEngine {
    shared: Arc<TransportShared>,
    #[cfg(not(windows))]
    last_tick: std::time::Instant,
    #[cfg(windows)]
    _thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn new(sample_rate: u32) -> Self {
        let shared = Arc::new(TransportShared::new(sample_rate));
        #[cfg(windows)]
        let thread = Some(crate::audio_out_win32::spawn_waveout_thread(Arc::clone(
            &shared,
        )));
        Self {
            shared,
            #[cfg(not(windows))]
            last_tick: std::time::Instant::now(),
            #[cfg(windows)]
            _thread: thread,
        }
    }

    pub fn update_mix(&self, data: PlaybackMixData) {
        self.shared
            .sample_rate
            .store(data.sample_rate.max(8000), Ordering::Relaxed);
        if let Ok(mut guard) = self.shared.mix.lock() {
            *guard = Arc::new(data);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    pub fn set_playing(&mut self, play: bool) {
        #[cfg(not(windows))]
        {
            self.last_tick = std::time::Instant::now();
        }
        self.shared.playing.store(play, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.shared.playing.store(false, Ordering::Relaxed);
        let start = self.shared.loop_start.load(Ordering::Relaxed);
        self.shared.playhead.store(start, Ordering::Relaxed);
    }

    pub fn audition_b(&self) -> bool {
        self.shared.audition_b.load(Ordering::Relaxed)
    }

    pub fn set_audition_b(&self, b: bool) {
        self.shared.audition_b.store(b, Ordering::Relaxed);
    }

    pub fn mono_check(&self) -> bool {
        self.shared.mono_check.load(Ordering::Relaxed)
    }

    pub fn set_mono_check(&self, mono: bool) {
        self.shared.mono_check.store(mono, Ordering::Relaxed);
    }

    pub fn loop_enabled(&self) -> bool {
        self.shared.loop_enabled.load(Ordering::Relaxed)
    }

    pub fn set_loop_enabled(&self, on: bool) {
        self.shared.loop_enabled.store(on, Ordering::Relaxed);
    }

    pub fn set_loop_range_sec(&self, start_sec: f32, end_sec: f32) {
        let sr = self.shared.sample_rate.load(Ordering::Relaxed) as f32;
        self.shared
            .loop_start
            .store((start_sec.max(0.0) * sr) as usize, Ordering::Relaxed);
        self.shared
            .loop_end
            .store((end_sec.max(0.0) * sr) as usize, Ordering::Relaxed);
    }

    pub fn playhead_sec(&self) -> f32 {
        let sr = self.shared.sample_rate.load(Ordering::Relaxed).max(8000) as f32;
        self.shared.playhead.load(Ordering::Relaxed) as f32 / sr
    }

    pub fn seek_sec(&mut self, sec: f32) {
        let sr = self.shared.sample_rate.load(Ordering::Relaxed).max(8000) as f32;
        self.shared
            .playhead
            .store((sec.max(0.0) * sr) as usize, Ordering::Relaxed);
        #[cfg(not(windows))]
        {
            self.last_tick = std::time::Instant::now();
        }
    }

    /// Advances the playhead timer cleanly on non-Windows targets (no-op on Windows where winmm drives it).
    pub fn tick_ui(&mut self) {
        #[cfg(not(windows))]
        {
            let now = std::time::Instant::now();
            let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.1);
            self.last_tick = now;
            if !self.is_playing() {
                return;
            }
            let total = self.shared.mix.lock().map(|m| m.total_samples).unwrap_or(0);
            if total == 0 {
                self.shared.playing.store(false, Ordering::Relaxed);
                return;
            }
            let sr = self.shared.sample_rate.load(Ordering::Relaxed) as f32;
            let step = (dt * sr).round() as usize;
            let l_start = self
                .shared
                .loop_start
                .load(Ordering::Relaxed)
                .min(total.saturating_sub(1));
            let raw_end = self.shared.loop_end.load(Ordering::Relaxed);
            let l_end = if raw_end > l_start + 64 {
                raw_end.min(total)
            } else {
                total
            };
            let mut pos = self.shared.playhead.load(Ordering::Relaxed) + step;
            if pos >= l_end {
                if self.loop_enabled() && l_end > l_start {
                    pos = l_start + (pos - l_end) % (l_end - l_start);
                } else {
                    pos = l_end;
                    self.shared.playing.store(false, Ordering::Relaxed);
                }
            }
            self.shared.playhead.store(pos, Ordering::Relaxed);
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.shared.stop_flag.store(true, Ordering::Relaxed);
    }
}

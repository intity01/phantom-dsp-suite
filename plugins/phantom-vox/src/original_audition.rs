//! "Hold to hear the original": swaps the output to the unprocessed input, turned up or down
//! to the same loudness as the processed sound, so the comparison is about tone and width
//! rather than "louder sounds better". Click-free (10 ms crossfade) and zero-allocation.

use phantom_dsp::BypassCrossfade;

/// Loudness is averaged over this window before matching.
const LOUDNESS_WINDOW_S: f32 = 0.4;
/// The original is never turned up or down by more than 12 dB.
const MAX_MATCH_GAIN: f32 = 4.0;
/// Below this mean square the input counts as silence and the match gain is held.
const SILENCE_MS: f32 = 1e-8;

pub struct OriginalAudition {
    xfade: BypassCrossfade,
    avg_a: f32,
    ms_original: f32,
    ms_processed: f32,
    match_gain: f32,
}

impl Default for OriginalAudition {
    fn default() -> Self {
        Self::new()
    }
}

impl OriginalAudition {
    pub fn new() -> Self {
        let mut s = Self {
            xfade: BypassCrossfade::new(false),
            avg_a: 0.0,
            ms_original: 0.0,
            ms_processed: 0.0,
            match_gain: 1.0,
        };
        s.set_sample_rate(48_000.0);
        s
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.xfade.set_sample_rate(sr);
        self.avg_a = (-1.0 / (LOUDNESS_WINDOW_S * sr.max(8000.0))).exp();
    }

    pub fn reset(&mut self) {
        self.ms_original = 0.0;
        self.ms_processed = 0.0;
        self.match_gain = 1.0;
    }

    /// Call once per block: `on` while the user holds the button.
    pub fn set_active(&mut self, on: bool) {
        self.xfade.set_bypassed(on);
    }

    /// Gain currently applied to the original to match the processed loudness.
    pub fn match_gain(&self) -> f32 {
        self.match_gain
    }

    /// Feeds one stereo frame and returns what should be heard.
    #[inline]
    pub fn tick(&mut self, original: (f32, f32), processed: (f32, f32)) -> (f32, f32) {
        let a = self.avg_a;
        let ms_o = 0.5 * (original.0 * original.0 + original.1 * original.1);
        let ms_p = 0.5 * (processed.0 * processed.0 + processed.1 * processed.1);
        self.ms_original = a * self.ms_original + (1.0 - a) * ms_o;
        self.ms_processed = a * self.ms_processed + (1.0 - a) * ms_p;
        if self.ms_original > SILENCE_MS {
            self.match_gain = (self.ms_processed / self.ms_original)
                .sqrt()
                .clamp(1.0 / MAX_MATCH_GAIN, MAX_MATCH_GAIN);
        }
        let w = self.xfade.next_wet();
        if w >= 1.0 {
            return processed;
        }
        let g = self.match_gain * (1.0 - w);
        (
            original.0 * g + processed.0 * w,
            original.1 * g + processed.1 * w,
        )
    }
}

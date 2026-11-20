use std::f32::consts::FRAC_PI_4;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use phantom_vox_dsp::AudioBuffer;

#[allow(dead_code)]
#[derive(Clone)]
pub struct PlaybackTrack {
    pub original: Arc<AudioBuffer>,
    pub aligned: Option<Arc<AudioBuffer>>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub is_guide: bool,
}

#[derive(Clone)]
pub struct PlaybackMixData {
    pub sample_rate: u32,
    pub total_samples: usize,
    pub tracks: Vec<PlaybackTrack>,
}

impl Default for PlaybackMixData {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            total_samples: 0,
            tracks: Vec::new(),
        }
    }
}

pub struct TransportShared {
    pub playing: AtomicBool,
    pub audition_b: AtomicBool,
    pub mono_check: AtomicBool,
    pub loop_enabled: AtomicBool,
    pub loop_start: AtomicUsize,
    pub loop_end: AtomicUsize,
    pub playhead: AtomicUsize,
    pub sample_rate: AtomicU32,
    pub stop_flag: AtomicBool,
    pub mix: Mutex<Arc<PlaybackMixData>>,
}

impl TransportShared {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            playing: AtomicBool::new(false),
            audition_b: AtomicBool::new(true),
            mono_check: AtomicBool::new(false),
            loop_enabled: AtomicBool::new(true),
            loop_start: AtomicUsize::new(0),
            loop_end: AtomicUsize::new(0),
            playhead: AtomicUsize::new(0),
            sample_rate: AtomicU32::new(sample_rate.max(8000)),
            stop_flag: AtomicBool::new(false),
            mix: Mutex::new(Arc::new(PlaybackMixData::default())),
        }
    }
}

/// Mixes `out_i16.len() / 2` stereo frames into `out_i16` and advances `ctrl.playhead`.
#[allow(dead_code)]
pub fn render_mix_block(mix: &PlaybackMixData, ctrl: &TransportShared, out_i16: &mut [i16]) {
    let frames = out_i16.len() / 2;
    if !ctrl.playing.load(Ordering::Relaxed) || mix.total_samples == 0 || mix.tracks.is_empty() {
        out_i16.fill(0);
        return;
    }

    let use_b = ctrl.audition_b.load(Ordering::Relaxed);
    let mono = ctrl.mono_check.load(Ordering::Relaxed);
    let looping = ctrl.loop_enabled.load(Ordering::Relaxed);
    let total = mix.total_samples;
    let l_start = ctrl
        .loop_start
        .load(Ordering::Relaxed)
        .min(total.saturating_sub(1));
    let raw_end = ctrl.loop_end.load(Ordering::Relaxed);
    let l_end = if raw_end > l_start + 64 {
        raw_end.min(total)
    } else {
        total
    };

    let any_solo = mix.tracks.iter().any(|t| t.solo);
    let mut pos = ctrl.playhead.load(Ordering::Relaxed);

    for f in 0..frames {
        if pos >= l_end {
            if looping && l_end > l_start {
                pos = l_start;
            } else {
                ctrl.playing.store(false, Ordering::Relaxed);
                out_i16[f * 2..].fill(0);
                break;
            }
        }
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for tr in &mix.tracks {
            if tr.muted || (any_solo && !tr.solo) {
                continue;
            }
            let buf = if use_b && !tr.is_guide {
                tr.aligned.as_ref().unwrap_or(&tr.original)
            } else {
                &tr.original
            };
            if pos >= buf.samples_l.len() {
                continue;
            }
            let sl = buf.samples_l[pos];
            let sr = if buf.channels >= 2 && pos < buf.samples_r.len() {
                buf.samples_r[pos]
            } else {
                sl
            };
            let angle = (tr.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let gl = angle.cos() * tr.gain;
            let gr = angle.sin() * tr.gain;
            sum_l += sl * gl;
            sum_r += sr * gr;
        }
        if mono {
            let m = 0.5 * (sum_l + sum_r);
            sum_l = m;
            sum_r = m;
        }
        let cl = (sum_l * 0.85).tanh().clamp(-1.0, 1.0);
        let cr = (sum_r * 0.85).tanh().clamp(-1.0, 1.0);
        out_i16[f * 2] = (cl * 32767.0) as i16;
        out_i16[f * 2 + 1] = (cr * 32767.0) as i16;
        pos += 1;
    }
    ctrl.playhead.store(pos, Ordering::Relaxed);
}

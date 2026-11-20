//! Zero-Allocation Real-Time Audio Processor for Phantom Vox
//!
//! Routes Stereo (`2->2`), Mono-to-Stereo (`1->2`), and Mono (`1->1`) buffers through
//! `VocalDoublerEngine` and `LiveSidechainAligner` using fixed `[0.0_f32; 256]` stack scratch
//! buffers, click-free 10 ms `BypassCrossfade`, PDC-aligned dry delay, and smoothed mix/gain.

use crate::original_audition::OriginalAudition;
use crate::params::{VoicesParam, VoxModeParam};
use crate::pdc_meter::{PdcDelayLine, VoxMeterTracker};
use crate::telemetry::SharedVoxTelemetry;
use phantom_dsp::{BypassCrossfade, OnePoleSmoother, SharedMeters};
use phantom_vox_dsp::{LiveSidechainAligner, VocalDoublerEngine};

const SCRATCH_LEN: usize = 256;

/// Read-only DSP settings captured once per audio callback block.
#[derive(Debug, Clone, Copy)]
pub struct VoxBlockConfig {
    pub mode: VoxModeParam,
    pub voices: VoicesParam,
    pub bypassed: bool,
    pub timing_ms: f32,
    pub detune_cents: f32,
    pub formant_st: f32,
    pub width: f32,
    pub sibilance_safe: bool,
    pub align_tightness: f32,
    pub level_match: f32,
    pub mono_check: bool,
    pub mix: f32,
    pub output_gain_lin: f32,
    pub focus_low_hz: f32,
    pub focus_high_hz: f32,
    /// True while the user holds "hear original".
    pub audition: bool,
}

/// Real-time vocal doubler & live sidechain alignment processor (zero heap allocs in `process`).
pub struct VoxProcessor {
    doubler: VocalDoublerEngine,
    aligner: LiveSidechainAligner,
    bypass_xfade: BypassCrossfade,
    mix_smooth: OnePoleSmoother,
    gain_smooth: OnePoleSmoother,
    pdc: PdcDelayLine,
    tracker: VoxMeterTracker,
    audition: OriginalAudition,
}

impl Default for VoxProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxProcessor {
    pub fn new() -> Self {
        let mut proc = Self {
            doubler: VocalDoublerEngine::new(48_000.0),
            aligner: LiveSidechainAligner::new(48_000.0),
            bypass_xfade: BypassCrossfade::new(false),
            mix_smooth: OnePoleSmoother::new(0.55),
            gain_smooth: OnePoleSmoother::new(1.0),
            pdc: PdcDelayLine::new(),
            tracker: VoxMeterTracker::new(),
            audition: OriginalAudition::new(),
        };
        proc.initialize(48_000.0);
        proc
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        let sr = if sample_rate.is_finite() {
            sample_rate.max(8000.0)
        } else {
            48_000.0
        };
        self.doubler.set_sample_rate(sr);
        self.aligner.set_sample_rate(sr);
        self.bypass_xfade.set_sample_rate(sr);
        self.mix_smooth.set_sample_rate(sr, 15.0);
        self.gain_smooth.set_sample_rate(sr, 15.0);
        self.tracker.set_sample_rate(sr);
        self.audition.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.doubler.reset();
        self.aligner.reset();
        self.pdc.reset();
        self.tracker.reset();
        self.audition.reset();
    }

    #[inline]
    pub fn latency_for_mode(&self, mode: VoxModeParam) -> u32 {
        match mode {
            VoxModeParam::Doubler => 0,
            VoxModeParam::LiveAlign | VoxModeParam::AlignAndDouble => {
                self.aligner.latency_samples()
            }
        }
    }

    /// Processes a stereo (`2->2`) or mono-to-stereo (`1->2`) block with zero heap allocations.
    #[allow(clippy::too_many_arguments)]
    pub fn process_stereo_or_m2s(
        &mut self,
        cfg: &VoxBlockConfig,
        mono_input: bool,
        left: &mut [f32],
        right: &mut [f32],
        guide_l: &[f32],
        guide_r: &[f32],
        meters: &SharedMeters,
        telemetry: &SharedVoxTelemetry,
    ) {
        let total = left.len().min(right.len());
        self.prepare_smoothers(cfg);
        let (mut dry_l, mut dry_r) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut pdc_l, mut pdc_r) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut g_l, mut g_r) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut aln_l, mut aln_r) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut wet_l, mut wet_r) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let use_pdc = cfg.mode != VoxModeParam::Doubler;

        let mut offset = 0;
        while offset < total {
            let n = (total - offset).min(SCRATCH_LEN);
            for i in 0..n {
                let idx = offset + i;
                dry_l[i] = sanitize(left[idx]);
                dry_r[i] = if mono_input {
                    dry_l[i]
                } else {
                    sanitize(right[idx])
                };
                g_l[i] = guide_l.get(idx).copied().map(sanitize).unwrap_or(0.0);
                g_r[i] = if mono_input {
                    g_l[i]
                } else {
                    guide_r.get(idx).copied().map(sanitize).unwrap_or(g_l[i])
                };
                (pdc_l[i], pdc_r[i]) = self.pdc.tick(dry_l[i], dry_r[i]);
                self.tracker
                    .push_input(dry_l[i], dry_r[i], g_l[i], g_r[i], telemetry);
            }
            crate::processor_modes::run_mode_chunk(
                cfg,
                mono_input,
                n,
                &dry_l[..n],
                &dry_r[..n],
                &g_l[..n],
                &g_r[..n],
                &mut aln_l[..n],
                &mut aln_r[..n],
                &mut wet_l[..n],
                &mut wet_r[..n],
                self.tracker.has_sidechain_signal(),
                &mut self.aligner,
                &mut self.doubler,
                &mut self.mix_smooth,
            );
            for i in 0..n {
                let g = self.gain_smooth.next();
                let fade = self.bypass_xfade.next_wet();
                let (rl, rr) = if use_pdc {
                    (pdc_l[i], pdc_r[i])
                } else {
                    (dry_l[i], dry_r[i])
                };
                let processed = (
                    rl * (1.0 - fade) + (wet_l[i] * g) * fade,
                    rr * (1.0 - fade) + (wet_r[i] * g) * fade,
                );
                let (ol, or) = self.audition.tick((rl, rr), processed);
                let (ol, or) = (sanitize(ol), sanitize(or));
                left[offset + i] = ol;
                right[offset + i] = or;
                self.tracker.push_output(ol, or);
            }
            offset += n;
        }
        self.tracker.publish(
            self.aligner.sidechain_active(),
            self.aligner.offset_ms(),
            self.aligner.confidence(),
            meters,
            telemetry,
        );
    }

    /// Processes a mono (`1->1`) block with zero heap allocations.
    pub fn process_mono(
        &mut self,
        cfg: &VoxBlockConfig,
        mono: &mut [f32],
        guide: &[f32],
        meters: &SharedMeters,
        telemetry: &SharedVoxTelemetry,
    ) {
        let total = mono.len();
        self.prepare_smoothers(cfg);
        let (mut dry, mut pdc) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut g_buf, mut aln_l) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let (mut aln_r, mut wet_l) = ([0.0_f32; SCRATCH_LEN], [0.0_f32; SCRATCH_LEN]);
        let mut wet_r = [0.0_f32; SCRATCH_LEN];
        let use_pdc = cfg.mode != VoxModeParam::Doubler;

        let mut offset = 0;
        while offset < total {
            let n = (total - offset).min(SCRATCH_LEN);
            for i in 0..n {
                let idx = offset + i;
                dry[i] = sanitize(mono[idx]);
                g_buf[i] = guide.get(idx).copied().map(sanitize).unwrap_or(0.0);
                pdc[i] = self.pdc.tick(dry[i], dry[i]).0;
                self.tracker
                    .push_input(dry[i], dry[i], g_buf[i], g_buf[i], telemetry);
            }
            crate::processor_modes::run_mode_chunk(
                cfg,
                true,
                n,
                &dry[..n],
                &dry[..n],
                &g_buf[..n],
                &g_buf[..n],
                &mut aln_l[..n],
                &mut aln_r[..n],
                &mut wet_l[..n],
                &mut wet_r[..n],
                self.tracker.has_sidechain_signal(),
                &mut self.aligner,
                &mut self.doubler,
                &mut self.mix_smooth,
            );
            for i in 0..n {
                let g = self.gain_smooth.next();
                let fade = self.bypass_xfade.next_wet();
                let rm = if use_pdc { pdc[i] } else { dry[i] };
                let wm = 0.5 * (wet_l[i] + wet_r[i]);
                let pm = rm * (1.0 - fade) + (wm * g) * fade;
                let om = sanitize(self.audition.tick((rm, rm), (pm, pm)).0);
                mono[offset + i] = om;
                self.tracker.push_output(om, om);
            }
            offset += n;
        }
        self.tracker.publish(
            self.aligner.sidechain_active(),
            self.aligner.offset_ms(),
            self.aligner.confidence(),
            meters,
            telemetry,
        );
    }

    fn prepare_smoothers(&mut self, cfg: &VoxBlockConfig) {
        self.bypass_xfade.set_bypassed(cfg.bypassed);
        self.audition.set_active(cfg.audition);
        self.mix_smooth.set_target(cfg.mix.clamp(0.0, 1.0));
        self.gain_smooth
            .set_target(cfg.output_gain_lin.clamp(0.0, 16.0));
    }
}

#[inline(always)]
fn sanitize(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

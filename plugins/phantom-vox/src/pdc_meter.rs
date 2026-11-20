//! PDC Delay Line & Real-Time Metering Tracker for Phantom Vox (Zero Allocations)

use crate::telemetry::SharedVoxTelemetry;
use phantom_dsp::{OutputMeter, SharedMeters, PEAK_FLOOR_DB};
use phantom_vox_dsp::LIVE_ALIGN_LATENCY_SAMPLES;

const PDC_LEN: usize = LIVE_ALIGN_LATENCY_SAMPLES as usize;

/// Fixed 2048-sample stereo delay matching `LiveSidechainAligner`'s lookahead latency.
/// Allocates once in `new()`; strictly zero allocations during `tick` and `reset`.
pub struct PdcDelayLine {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
}

impl Default for PdcDelayLine {
    fn default() -> Self {
        Self::new()
    }
}

impl PdcDelayLine {
    pub fn new() -> Self {
        Self {
            buf_l: vec![0.0_f32; PDC_LEN],
            buf_r: vec![0.0_f32; PDC_LEN],
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.buf_l.fill(0.0);
        self.buf_r.fill(0.0);
        self.pos = 0;
    }

    #[inline(always)]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let i = self.pos;
        let out = (self.buf_l[i], self.buf_r[i]);
        self.buf_l[i] = l;
        self.buf_r[i] = r;
        self.pos = if i + 1 < PDC_LEN { i + 1 } else { 0 };
        out
    }
}

/// Real-time correlation, peak, sidechain-presence, and waveform-strip telemetry tracker.
pub struct VoxMeterTracker {
    output_meter: OutputMeter,
    corr_a: f32,
    avg_lr: f32,
    avg_ll: f32,
    avg_rr: f32,
    peak_decay: f32,
    in_peak: [f32; 2],
    sc_rms_sq: f32,
    sc_rms_a: f32,
    wave_step: usize,
    wave_ctr: usize,
    wave_g_peak: f32,
    wave_d_peak: f32,
}

impl Default for VoxMeterTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxMeterTracker {
    pub fn new() -> Self {
        let mut s = Self {
            output_meter: OutputMeter::new(),
            corr_a: 0.99,
            avg_lr: 0.0,
            avg_ll: 0.0,
            avg_rr: 0.0,
            peak_decay: 0.9999,
            in_peak: [0.0; 2],
            sc_rms_sq: 0.0,
            sc_rms_a: 0.999,
            wave_step: 480,
            wave_ctr: 0,
            wave_g_peak: 0.0,
            wave_d_peak: 0.0,
        };
        s.set_sample_rate(48_000.0);
        s
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.output_meter.set_sample_rate(sr);
        self.corr_a = (-1.0 / (0.150 * sr)).exp();
        self.peak_decay = (-1.0 / (0.300 * sr)).exp();
        self.sc_rms_a = (-1.0 / (0.050 * sr)).exp();
        self.wave_step = ((sr * 0.012).round() as usize).clamp(64, 4096);
    }

    pub fn reset(&mut self) {
        self.output_meter.reset();
        (self.avg_lr, self.avg_ll, self.avg_rr) = (0.0, 0.0, 0.0);
        self.in_peak = [0.0; 2];
        self.sc_rms_sq = 0.0;
        self.wave_ctr = 0;
        (self.wave_g_peak, self.wave_d_peak) = (0.0, 0.0);
    }

    #[inline(always)]
    pub fn has_sidechain_signal(&self) -> bool {
        self.sc_rms_sq > 1e-6
    }

    #[inline(always)]
    pub fn push_input(
        &mut self,
        dl: f32,
        dr: f32,
        gl: f32,
        gr: f32,
        telemetry: &SharedVoxTelemetry,
    ) {
        self.in_peak[0] = dl.abs().max(self.in_peak[0] * self.peak_decay);
        self.in_peak[1] = dr.abs().max(self.in_peak[1] * self.peak_decay);
        let gm = 0.5 * (gl + gr);
        let dm = 0.5 * (dl + dr);
        self.sc_rms_sq = self.sc_rms_a * self.sc_rms_sq + (1.0 - self.sc_rms_a) * gm * gm;
        self.wave_g_peak = self.wave_g_peak.max(gm.abs());
        self.wave_d_peak = self.wave_d_peak.max(dm.abs());
        self.wave_ctr += 1;
        if self.wave_ctr >= self.wave_step {
            self.wave_ctr = 0;
            telemetry.push_wave_bin(self.wave_g_peak, self.wave_d_peak);
            self.wave_g_peak = 0.0;
            self.wave_d_peak = 0.0;
        }
    }

    #[inline(always)]
    pub fn push_output(&mut self, out_l: f32, out_r: f32) {
        self.output_meter.process_sample(out_l, out_r);
        let ca = self.corr_a;
        self.avg_lr = ca * self.avg_lr + (1.0 - ca) * (out_l * out_r);
        self.avg_ll = ca * self.avg_ll + (1.0 - ca) * (out_l * out_l);
        self.avg_rr = ca * self.avg_rr + (1.0 - ca) * (out_r * out_r);
    }

    pub fn publish(
        &self,
        aligner_sc_active: bool,
        aligner_offset_ms: f32,
        aligner_confidence: f32,
        meters: &SharedMeters,
        telemetry: &SharedVoxTelemetry,
    ) {
        self.output_meter.publish(meters);
        let snap = self.output_meter.snapshot();
        let denom = (self.avg_ll * self.avg_rr).sqrt();
        let corr = if denom < 1e-12 {
            1.0
        } else {
            (self.avg_lr / denom).clamp(-1.0, 1.0)
        };
        let sc_active = self.has_sidechain_signal() || aligner_sc_active;
        let (off_ms, conf) = if sc_active {
            (aligner_offset_ms, aligner_confidence)
        } else {
            (0.0, 0.0)
        };
        telemetry.store_metrics(
            corr,
            off_ms,
            conf,
            sc_active,
            lin_to_db(self.in_peak[0]),
            lin_to_db(self.in_peak[1]),
            snap.peak_l_db,
            snap.peak_r_db,
        );
    }
}

#[inline(always)]
fn lin_to_db(lin: f32) -> f32 {
    if lin <= 1e-5 {
        PEAK_FLOOR_DB
    } else {
        (20.0 * lin.log10()).clamp(PEAK_FLOOR_DB, 24.0)
    }
}

//! Audio Processing Engine for Phantom EQ
//! Coordinates 24-band smoothed Cascaded IIR (Zero Latency / Natural Phase + 1x/2x/4x Polyphase OS
//! + Dynamic & Spectral EQ), Linear Phase FIR, 10 ms click-free PDC Bypass Crossfade, Character,
//! Auto-Gain, Output Metering, Mono + Stereo blocks, and Solo Audition — zero heap allocations.

mod iir;
mod iir_run;
mod linear;
mod mono;
mod solo;

use crate::dsp_state::SharedBandGr;
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use linear::LinearPhaseTracker;
use phantom_dsp::{
    AutoGainCompensator, BandParamSmoother, BypassCrossfade, CascadedStereoFilter, CharacterMode,
    CharacterProcessor, DynamicDetector, LatencyBypassDelay, LinearPhaseEngine, OnePoleSmoother,
    OutputMeter, Oversampler2x, Oversampler4x, SharedMeters, SpectralBandProcessor,
};
use solo::SoloCrossfade;

const PARAM_SMOOTH_MS: f32 = 15.0;

pub struct EqProcessor {
    pub(super) sample_rate: f32,
    pub(super) filters: [CascadedStereoFilter; NUM_BANDS],
    pub(super) dynamic_detectors: [DynamicDetector; NUM_BANDS],
    pub(super) spectral_processors: [SpectralBandProcessor; NUM_BANDS],
    pub(super) band_smoothers: [BandParamSmoother; NUM_BANDS],
    pub(super) band_was_active: [bool; NUM_BANDS],
    pub(super) output_gain_smoother: OnePoleSmoother,
    pub(super) gain_scale_smoother: OnePoleSmoother,
    pub(super) bypass_xfade: BypassCrossfade,
    pub(super) latency_bypass_delay: LatencyBypassDelay,
    pub(super) solo: SoloCrossfade,
    pub(super) oversampler_2x: Oversampler2x,
    pub(super) oversampler_4x: Oversampler4x,
    pub(super) linear_phase: LinearPhaseEngine,
    pub(super) lp_tracker: LinearPhaseTracker,
    pub(super) character: CharacterProcessor,
    pub(super) auto_gain: AutoGainCompensator,
    pub(super) output_meter: OutputMeter,
    was_fully_bypassed_iir: bool,
    primed: bool,
}

impl Default for EqProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl EqProcessor {
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            filters: std::array::from_fn(|_| CascadedStereoFilter::new()),
            dynamic_detectors: std::array::from_fn(|_| DynamicDetector::new()),
            spectral_processors: std::array::from_fn(|_| SpectralBandProcessor::new()),
            band_smoothers: std::array::from_fn(|_| BandParamSmoother::new(1000.0, 0.0, 1.0)),
            band_was_active: [false; NUM_BANDS],
            output_gain_smoother: OnePoleSmoother::new(1.0),
            gain_scale_smoother: OnePoleSmoother::new(1.0),
            bypass_xfade: BypassCrossfade::new(false),
            latency_bypass_delay: LatencyBypassDelay::new(),
            solo: SoloCrossfade::new(),
            oversampler_2x: Oversampler2x::new(),
            oversampler_4x: Oversampler4x::new(),
            linear_phase: LinearPhaseEngine::new(),
            lp_tracker: LinearPhaseTracker::new(),
            character: CharacterProcessor::new(),
            auto_gain: AutoGainCompensator::new(),
            output_meter: OutputMeter::new(),
            was_fully_bypassed_iir: false,
            primed: false,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(8000.0);
        for sm in &mut self.band_smoothers {
            sm.set_sample_rate(self.sample_rate, PARAM_SMOOTH_MS);
        }
        self.output_gain_smoother
            .set_sample_rate(self.sample_rate, PARAM_SMOOTH_MS);
        self.gain_scale_smoother
            .set_sample_rate(self.sample_rate, PARAM_SMOOTH_MS);
        self.bypass_xfade.set_sample_rate(self.sample_rate);
        self.solo.set_sample_rate(self.sample_rate);
        self.character.set_sample_rate(self.sample_rate);
        self.auto_gain.set_sample_rate(self.sample_rate);
        self.output_meter.set_sample_rate(self.sample_rate);
        self.reset();
        for det in &mut self.dynamic_detectors {
            det.update_ballistics(self.sample_rate, 10.0, 100.0);
        }
        for spec in &mut self.spectral_processors {
            spec.set_ballistics(self.sample_rate, 10.0, 100.0);
        }
        self.lp_tracker.mark_dirty();
    }

    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
        for det in &mut self.dynamic_detectors {
            det.reset();
        }
        for spec in &mut self.spectral_processors {
            spec.reset();
        }
        self.band_was_active = [false; NUM_BANDS];
        self.was_fully_bypassed_iir = false;
        self.primed = false;
        self.solo.reset();
        self.oversampler_2x.reset();
        self.oversampler_4x.reset();
        self.linear_phase.reset();
        self.latency_bypass_delay.reset();
        self.character.reset();
        self.auto_gain.reset();
        self.output_meter.reset();
    }

    pub fn on_mode_switched(&mut self, mode: ProcessingModeParam) {
        self.oversampler_2x.reset();
        self.oversampler_4x.reset();
        if mode == ProcessingModeParam::LinearPhase {
            self.linear_phase.reset();
            self.latency_bypass_delay.reset();
            self.lp_tracker.mark_dirty();
        }
    }

    #[inline]
    pub fn latency_for_mode(&self, mode: ProcessingModeParam) -> u32 {
        match mode {
            ProcessingModeParam::ZeroLatency | ProcessingModeParam::NaturalPhase => 0,
            ProcessingModeParam::LinearPhase => self.linear_phase.latency_samples() as u32,
        }
    }

    pub fn sync_linear_phase(&mut self, params: &PhantomEqParams, live: &impl ParamRead) {
        let gain_scale = (live.read(&params.gain_scale) / 100.0).clamp(0.0, 2.0);
        self.lp_tracker.sync(
            &mut self.linear_phase,
            params,
            live,
            self.sample_rate,
            gain_scale,
        );
    }

    /// Processes a stereo audio block through smoothed EQ, Character, Auto-Gain, Solo, 10 ms PDC
    /// Bypass Crossfade, and Metering — strictly zero heap allocations.
    #[allow(clippy::too_many_arguments)]
    pub fn process_block(
        &mut self,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        mode: ProcessingModeParam,
        bypassed: bool,
        requested_solo: u8,
        left: &mut [f32],
        right: &mut [f32],
        sc_left: &[f32],
        sc_right: &[f32],
        final_gain: f32,
        shared_meters: &SharedMeters,
        shared_gr: &SharedBandGr,
    ) {
        let total = left.len().min(right.len());
        if total == 0 {
            return;
        }

        let gain_scale_target = (live.read(&params.gain_scale) / 100.0).clamp(0.0, 2.0);
        let snap_initial = !self.primed;
        if snap_initial {
            self.bypass_xfade.reset(bypassed);
            self.output_gain_smoother.reset(final_gain);
            self.gain_scale_smoother.reset(gain_scale_target);
            self.primed = true;
        } else {
            self.bypass_xfade.set_bypassed(bypassed);
            self.output_gain_smoother.set_target(final_gain);
            self.gain_scale_smoother.set_target(gain_scale_target);
        }

        let char_mode: CharacterMode = live.read(&params.character).into();
        let auto_gain_en = live.read(&params.auto_gain);
        let (solo_target, solo_placement) =
            self.solo
                .prepare(params, live, requested_solo, self.sample_rate);

        if mode == ProcessingModeParam::LinearPhase {
            shared_gr.clear_all();
            self.gain_scale_smoother.skip(total);
            self.process_linear_phase_chunks(
                params,
                live,
                gain_scale_target,
                &mut left[..total],
                &mut right[..total],
                char_mode,
                auto_gain_en,
                solo_target,
                solo_placement,
            );
        } else if self.bypass_xfade.is_fully_bypassed() {
            self.was_fully_bypassed_iir = true;
            shared_gr.clear_all();
            self.output_gain_smoother.skip(total);
            self.gain_scale_smoother.skip(total);
            self.output_meter
                .process_block(&left[..total], &right[..total]);
        } else {
            if self.was_fully_bypassed_iir {
                self.was_fully_bypassed_iir = false;
                for filter in &mut self.filters {
                    filter.reset();
                }
                self.oversampler_2x.reset();
                self.oversampler_4x.reset();
                self.band_was_active = [false; NUM_BANDS];
            }
            self.process_iir_block(
                params,
                live,
                mode == ProcessingModeParam::NaturalPhase,
                snap_initial,
                char_mode,
                auto_gain_en,
                solo_target,
                solo_placement,
                &mut left[..total],
                &mut right[..total],
                sc_left,
                sc_right,
                shared_gr,
            );
        }

        self.solo.finish_block(solo_target);
        self.output_meter.publish(shared_meters);
        let makeup = if self.bypass_xfade.is_fully_bypassed() {
            0.0
        } else {
            self.auto_gain.current_makeup_db()
        };
        shared_meters.store_makeup_db(makeup);
    }
}

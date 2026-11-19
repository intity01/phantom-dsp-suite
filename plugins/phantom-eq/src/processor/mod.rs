//! Audio Processing Engine for Phantom EQ
//! Coordinates 24-band Cascaded IIR (Zero Latency / Natural Phase + 2x/4x Polyphase OS +
//! Dynamic EQ + External Sidechain), Linear Phase FIR, Character, Auto-Gain, Output Metering,
//! and click-free 10 ms crossfaded Per-Band Solo Audition — strictly zero heap allocations.

mod iir;
mod linear;
mod solo;

use crate::dsp_state::SharedBandGr;
use crate::params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use iir::{prepare_iir_bands, process_iir_sample_os, publish_band_gr};
use linear::{LinearPhaseTracker, LP_CHUNK_SIZE};
use phantom_dsp::{
    AutoGainCompensator, CascadedStereoFilter, ChannelTarget, CharacterMode, CharacterProcessor,
    DynamicDetector, LinearPhaseEngine, OutputMeter, OversampleFactor, Oversampler2x,
    Oversampler4x, SharedMeters,
};
use solo::SoloCrossfade;

pub struct EqProcessor {
    sample_rate: f32,
    filters: [CascadedStereoFilter; NUM_BANDS],
    dynamic_detectors: [DynamicDetector; NUM_BANDS],
    solo: SoloCrossfade,
    oversampler_2x: Oversampler2x,
    oversampler_4x: Oversampler4x,
    linear_phase: LinearPhaseEngine,
    lp_tracker: LinearPhaseTracker,
    character: CharacterProcessor,
    auto_gain: AutoGainCompensator,
    output_meter: OutputMeter,
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
            solo: SoloCrossfade::new(),
            oversampler_2x: Oversampler2x::new(),
            oversampler_4x: Oversampler4x::new(),
            linear_phase: LinearPhaseEngine::new(),
            lp_tracker: LinearPhaseTracker::new(),
            character: CharacterProcessor::new(),
            auto_gain: AutoGainCompensator::new(),
            output_meter: OutputMeter::new(),
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(8000.0);
        self.solo.set_sample_rate(self.sample_rate);
        self.character.set_sample_rate(self.sample_rate);
        self.auto_gain.set_sample_rate(self.sample_rate);
        self.output_meter.set_sample_rate(self.sample_rate);
        self.reset();
        for det in &mut self.dynamic_detectors {
            det.update_ballistics(self.sample_rate, 10.0, 100.0);
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
        self.solo.reset();
        self.oversampler_2x.reset();
        self.oversampler_4x.reset();
        self.linear_phase.reset();
        self.character.reset();
        self.auto_gain.reset();
        self.output_meter.reset();
    }

    pub fn on_mode_switched(&mut self, mode: ProcessingModeParam) {
        self.oversampler_2x.reset();
        self.oversampler_4x.reset();
        if mode == ProcessingModeParam::LinearPhase {
            self.linear_phase.reset();
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

    pub fn sync_linear_phase(&mut self, params: &PhantomEqParams) {
        self.lp_tracker
            .sync(&mut self.linear_phase, params, self.sample_rate);
    }

    /// Processes a stereo audio block through EQ, Character, Auto-Gain, Solo ramp, and Metering.
    #[allow(clippy::too_many_arguments)]
    pub fn process_block(
        &mut self,
        params: &PhantomEqParams,
        mode: ProcessingModeParam,
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

        let char_mode: CharacterMode = params.character.value().into();
        let auto_gain_en = params.auto_gain.value();
        let (solo_target, solo_placement) =
            self.solo.prepare(params, requested_solo, self.sample_rate);

        if mode == ProcessingModeParam::LinearPhase {
            shared_gr.clear_all();
            self.process_linear_phase_chunks(
                params,
                &mut left[..total],
                &mut right[..total],
                char_mode,
                auto_gain_en,
                final_gain,
                solo_target,
                solo_placement,
            );
        } else {
            let natural_phase = mode == ProcessingModeParam::NaturalPhase;
            let os_factor: OversampleFactor = params.oversampling.value().into();
            let effective_sr = self.sample_rate * os_factor.multiplier() as f32;
            let (mut active, count) = prepare_iir_bands(
                params,
                &mut self.filters,
                &mut self.dynamic_detectors,
                shared_gr,
                effective_sr,
                natural_phase,
            );

            for i in 0..total {
                let pre_l = left[i];
                let pre_r = right[i];
                let sc_l = sc_left.get(i).copied().unwrap_or(0.0);
                let sc_r = sc_right.get(i).copied().unwrap_or(0.0);

                let (eq_l, eq_r) = if self.solo.is_steady_full_solo(solo_target) {
                    (pre_l, pre_r)
                } else {
                    let (fl, fr) = process_iir_sample_os(
                        &mut active[..count],
                        &mut self.filters,
                        &mut self.dynamic_detectors,
                        &mut self.oversampler_2x,
                        &mut self.oversampler_4x,
                        os_factor,
                        effective_sr,
                        natural_phase,
                        pre_l,
                        pre_r,
                        sc_l,
                        sc_r,
                    );
                    let (cl, cr) = self.character.process_stereo(fl, fr, char_mode);
                    self.auto_gain
                        .process_stereo(pre_l, pre_r, cl, cr, auto_gain_en)
                };

                let (ml, mr) =
                    self.solo
                        .apply_sample(pre_l, pre_r, eq_l, eq_r, solo_target, solo_placement);
                let out_l = ml * final_gain;
                let out_r = mr * final_gain;
                self.output_meter.process_sample(out_l, out_r);
                left[i] = out_l;
                right[i] = out_r;
            }

            publish_band_gr(&active[..count], &self.dynamic_detectors, shared_gr);
        }

        self.solo.finish_block(solo_target);
        self.output_meter.publish(shared_meters);
        shared_meters.store_makeup_db(self.auto_gain.current_makeup_db());
    }

    #[allow(clippy::too_many_arguments)]
    fn process_linear_phase_chunks(
        &mut self,
        params: &PhantomEqParams,
        left: &mut [f32],
        right: &mut [f32],
        char_mode: CharacterMode,
        auto_gain_en: bool,
        final_gain: f32,
        solo_target: f32,
        solo_placement: ChannelTarget,
    ) {
        self.sync_linear_phase(params);
        let mut pre_l_buf = [0.0_f32; LP_CHUNK_SIZE];
        let mut pre_r_buf = [0.0_f32; LP_CHUNK_SIZE];
        let total = left.len();
        let mut offset = 0;

        while offset < total {
            let n = (total - offset).min(LP_CHUNK_SIZE);
            pre_l_buf[..n].copy_from_slice(&left[offset..offset + n]);
            pre_r_buf[..n].copy_from_slice(&right[offset..offset + n]);

            self.linear_phase.process_block(
                &mut left[offset..offset + n],
                &mut right[offset..offset + n],
            );

            for k in 0..n {
                let pre_l = pre_l_buf[k];
                let pre_r = pre_r_buf[k];
                let (cl, cr) =
                    self.character
                        .process_stereo(left[offset + k], right[offset + k], char_mode);
                let (ag_l, ag_r) =
                    self.auto_gain
                        .process_stereo(pre_l, pre_r, cl, cr, auto_gain_en);
                let (ml, mr) =
                    self.solo
                        .apply_sample(pre_l, pre_r, ag_l, ag_r, solo_target, solo_placement);
                let out_l = ml * final_gain;
                let out_r = mr * final_gain;
                self.output_meter.process_sample(out_l, out_r);
                left[offset + k] = out_l;
                right[offset + k] = out_r;
            }
            offset += n;
        }
    }

    pub fn meter_bypass_block(
        &mut self,
        left: &[f32],
        right: &[f32],
        shared_meters: &SharedMeters,
    ) {
        self.output_meter.process_block(left, right);
        self.output_meter.publish(shared_meters);
        shared_meters.store_makeup_db(0.0);
    }
}

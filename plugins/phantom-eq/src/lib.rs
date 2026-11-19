//! Phantom EQ - High-Performance, Zero-Telemetry Digital Parametric Equalizer
//! Professional 8-band parametric EQ with M/S routing, Dynamic EQ, 2x Oversampling,
//! Linear Phase FIR mode, and Per-Band Solo Audition.

pub mod dsp_state;
pub mod editor;
pub mod params;
pub mod processor;

use dsp_state::feed_spectrum;
use editor::{draw_editor, EditorShared, EditorState, NO_SOLO};
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use phantom_dsp::SharedSpectrum;
use processor::EqProcessor;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

#[cfg(windows)]
#[link(name = "ole32")]
#[link(name = "oleaut32")]
extern "C" {}

pub struct PhantomEq {
    params: Arc<PhantomEqParams>,
    processor: EqProcessor,
    shared_spectrum: Arc<SharedSpectrum>,
    solo_band: Arc<AtomicU8>,
    shared_sample_rate: Arc<AtomicU32>,
    shared_latency: Arc<AtomicU32>,
    last_reported_latency: u32,
    last_mode: ProcessingModeParam,
    was_bypassed: bool,
}

impl Default for PhantomEq {
    fn default() -> Self {
        Self {
            params: Arc::new(PhantomEqParams::default()),
            processor: EqProcessor::new(),
            shared_spectrum: Arc::new(SharedSpectrum::new()),
            solo_band: Arc::new(AtomicU8::new(NO_SOLO)),
            shared_sample_rate: Arc::new(AtomicU32::new(44100.0_f32.to_bits())),
            shared_latency: Arc::new(AtomicU32::new(0)),
            last_reported_latency: 0,
            last_mode: ProcessingModeParam::ZeroLatency,
            was_bypassed: false,
        }
    }
}

impl Plugin for PhantomEq {
    const NAME: &'static str = "Phantom EQ";
    const VENDOR: &'static str = "Phantom DSP";
    const URL: &'static str = "https://local.audio";
    const EMAIL: &'static str = "phantom@local.audio";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let editor_state_handle = self.params.editor_state.clone();
        let shared = EditorShared {
            spectrum: self.shared_spectrum.clone(),
            solo_band: self.solo_band.clone(),
            sample_rate_bits: self.shared_sample_rate.clone(),
            latency_samples: self.shared_latency.clone(),
        };

        create_egui_editor(
            editor_state_handle,
            EditorState::default(),
            |_, _| {},
            move |ctx, setter, state| {
                draw_editor(ctx, setter, state, &params, &shared);
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        let sr = buffer_config.sample_rate;
        self.shared_sample_rate
            .store(sr.to_bits(), Ordering::Relaxed);
        self.processor.initialize(sr);

        let mode = self.params.mode.value();
        self.last_mode = mode;
        if mode == ProcessingModeParam::LinearPhase {
            self.processor.sync_linear_phase(&self.params);
        }
        let latency = self.processor.latency_for_mode(mode);
        context.set_latency_samples(latency);
        self.last_reported_latency = latency;
        self.shared_latency.store(latency, Ordering::Relaxed);
        true
    }

    fn reset(&mut self) {
        self.processor.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mode = self.params.mode.value();
        let desired_latency = self.processor.latency_for_mode(mode);
        if desired_latency != self.last_reported_latency {
            context.set_latency_samples(desired_latency);
            self.last_reported_latency = desired_latency;
            self.shared_latency
                .store(desired_latency, Ordering::Relaxed);
        }
        if mode != self.last_mode {
            self.last_mode = mode;
            self.processor.on_mode_switched(mode);
        }

        if self.params.bypass.value() {
            self.was_bypassed = true;
            return ProcessStatus::Normal;
        }
        if self.was_bypassed {
            self.was_bypassed = false;
            self.processor.reset();
        }

        let out_gain_linear = util::db_to_gain(self.params.output_gain.value());
        let phase_mult = if self.params.phase_invert.value() {
            -1.0
        } else {
            1.0
        };
        let final_gain = out_gain_linear * phase_mult;

        let slices = buffer.as_slice();
        if slices.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (left_slice, right_slice) = slices.split_at_mut(1);
        let left_channel = &mut left_slice[0];
        let right_channel = &mut right_slice[0];

        // Per-Band Solo Audition monitors the selected band cleanly
        let solo_raw = self.solo_band.load(Ordering::Relaxed);
        if (solo_raw as usize) < NUM_BANDS {
            self.processor.process_solo(
                &self.params,
                solo_raw,
                left_channel,
                right_channel,
                final_gain,
            );
            if self.params.editor_state.is_open() {
                feed_spectrum(&self.shared_spectrum, left_channel, right_channel);
            }
            return ProcessStatus::Normal;
        }
        self.processor.clear_solo_state();

        match mode {
            ProcessingModeParam::ZeroLatency => {
                self.processor.process_zero_latency(
                    &self.params,
                    left_channel,
                    right_channel,
                    final_gain,
                );
            }
            ProcessingModeParam::LinearPhase => {
                self.processor.process_linear_phase(
                    &self.params,
                    left_channel,
                    right_channel,
                    final_gain,
                );
            }
        }

        if self.params.editor_state.is_open() {
            feed_spectrum(&self.shared_spectrum, left_channel, right_channel);
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for PhantomEq {
    const CLAP_ID: &'static str = "com.phantom-dsp.phantom-eq";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("High-performance zero-telemetry parametric digital equalizer with M/S and Dynamics");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Equalizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for PhantomEq {
    const VST3_CLASS_ID: [u8; 16] = *b"PhantomEQ_Audio1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nih_export_clap!(PhantomEq);
nih_export_vst3!(PhantomEq);

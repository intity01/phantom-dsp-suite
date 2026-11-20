//! Phantom EQ — High-Performance, Zero-Telemetry Mastering Parametric Equalizer
//! 24 dynamic bands (clean empty Pro-Q 4 startup), 10 shapes, 8 slopes (6 dB..Brickwall),
//! Zero Latency / Natural Phase / Linear Phase FIR, 1x/2x/4x Polyphase OS, Mono & Stereo +
//! External Sidechain, zero-click smoothing, 10 ms PDC bypass crossfade, Gain Scale, Character,
//! Auto-Gain, EBU R128 LUFS metering, and Headphone Solo Audition.

pub mod dsp_state;
pub mod editor;
pub mod live_mirror;
pub mod params;
pub mod processor;
pub mod state;

#[cfg(test)]
mod v01_tests;

use dsp_state::SharedBandGr;
use editor::{draw_editor, EditorShared, EditorState, NO_SOLO};
use live_mirror::LiveParamMirror;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use params::{PhantomEqParams, ProcessingModeParam};
use phantom_dsp::{CollisionRegistry, SharedMeters, SharedSpectrum};
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
    shared_meters: Arc<SharedMeters>,
    shared_band_gr: Arc<SharedBandGr>,
    solo_band: Arc<AtomicU8>,
    shared_sample_rate: Arc<AtomicU32>,
    shared_latency: Arc<AtomicU32>,
    live_params: Arc<LiveParamMirror>,
    collision_slot: usize,
    last_reported_latency: u32,
    last_mode: ProcessingModeParam,
}

impl Default for PhantomEq {
    fn default() -> Self {
        let params = Arc::new(PhantomEqParams::default());
        Self {
            live_params: Arc::new(LiveParamMirror::new(params.clone())),
            params,
            processor: EqProcessor::new(),
            shared_spectrum: Arc::new(SharedSpectrum::new()),
            shared_meters: Arc::new(SharedMeters::new()),
            shared_band_gr: Arc::new(SharedBandGr::new()),
            solo_band: Arc::new(AtomicU8::new(NO_SOLO)),
            shared_sample_rate: Arc::new(AtomicU32::new(44100.0_f32.to_bits())),
            shared_latency: Arc::new(AtomicU32::new(0)),
            collision_slot: CollisionRegistry::register_instance(),
            last_reported_latency: 0,
            last_mode: ProcessingModeParam::ZeroLatency,
        }
    }
}

impl Drop for PhantomEq {
    fn drop(&mut self) {
        CollisionRegistry::unregister_instance(self.collision_slot);
    }
}

impl Plugin for PhantomEq {
    const NAME: &'static str = "Phantom EQ";
    const VENDOR: &'static str = "Phantom DSP";
    const URL: &'static str = "https://github.com/intity01/phantom-dsp-suite";
    const EMAIL: &'static str = "phantom@local.audio";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[new_nonzero_u32(2)],
            names: PortNames {
                aux_inputs: &["Sidechain"],
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            aux_input_ports: &[new_nonzero_u32(1)],
            names: PortNames {
                aux_inputs: &["Sidechain"],
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
    ];

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
            meters: self.shared_meters.clone(),
            band_gr: self.shared_band_gr.clone(),
            solo_band: self.solo_band.clone(),
            sample_rate_bits: self.shared_sample_rate.clone(),
            latency_samples: self.shared_latency.clone(),
            live: self.live_params.clone(),
            collision_slot: self.collision_slot,
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
        self.shared_meters.reset();
        self.shared_band_gr.clear_all();
        self.live_params.reconcile();

        let mode = self.live_params.read(&self.params.mode);
        self.last_mode = mode;
        if mode == ProcessingModeParam::LinearPhase {
            self.processor
                .sync_linear_phase(&self.params, self.live_params.as_ref());
        }
        let latency = self.processor.latency_for_mode(mode);
        context.set_latency_samples(latency);
        self.last_reported_latency = latency;
        self.shared_latency.store(latency, Ordering::Relaxed);
        true
    }

    fn reset(&mut self) {
        self.processor.reset();
        self.shared_meters.reset();
        self.shared_band_gr.clear_all();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.live_params.reconcile();
        let live = self.live_params.as_ref();

        let mode = live.read(&self.params.mode);
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

        let editor_open = self.params.editor_state.is_open();
        let bypassed = live.read(&self.params.bypass);
        let out_gain_linear = util::db_to_gain(live.read(&self.params.output_gain));
        let phase_mult = if live.read(&self.params.phase_invert) {
            -1.0
        } else {
            1.0
        };
        let final_gain = out_gain_linear * phase_mult;
        let requested_solo = self.solo_band.load(Ordering::Relaxed);

        let (sc_left, sc_right): (&[f32], &[f32]) = aux
            .inputs
            .first()
            .map(|b| b.as_slice_immutable())
            .map(|s| match s {
                [l, r, ..] => (&l[..], &r[..]),
                [l] => (&l[..], &l[..]),
                _ => (&[][..], &[][..]),
            })
            .unwrap_or((&[], &[]));

        let slices = buffer.as_slice();
        if slices.is_empty() {
            return ProcessStatus::KeepAlive;
        }
        if slices.len() == 1 {
            let sp = editor_open.then_some(self.shared_spectrum.as_ref());
            self.processor.process_mono_block(
                &self.params,
                live,
                mode,
                bypassed,
                requested_solo,
                &mut slices[0],
                sc_left,
                sc_right,
                final_gain,
                &self.shared_meters,
                &self.shared_band_gr,
                sp,
            );
            return ProcessStatus::KeepAlive;
        }

        let (left_slice, right_slice) = slices.split_at_mut(1);
        let left_channel = &mut left_slice[0];
        let right_channel = &mut right_slice[0];

        if editor_open {
            self.shared_spectrum
                .push_stereo_pre(left_channel, right_channel);
        }
        self.processor.process_block(
            &self.params,
            live,
            mode,
            bypassed,
            requested_solo,
            left_channel,
            right_channel,
            sc_left,
            sc_right,
            final_gain,
            &self.shared_meters,
            &self.shared_band_gr,
        );
        if editor_open {
            self.shared_spectrum
                .push_stereo_post_and_side(left_channel, right_channel);
        }

        // `KeepAlive` keeps Cubase calling `process()` even on silent/stopped tracks.
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for PhantomEq {
    const CLAP_ID: &'static str = "com.phantom-dsp.phantom-eq";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Mastering-grade 24-band dynamic parametric EQ with Natural/Linear Phase & LUFS");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Equalizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for PhantomEq {
    const VST3_CLASS_ID: [u8; 16] = *b"PhantomEQ_Audio1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nih_export_clap!(PhantomEq);
nih_export_vst3!(PhantomEq);

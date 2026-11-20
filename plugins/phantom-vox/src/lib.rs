//! Phantom Vox — Cubase 15 VST3/CLAP Vocal Doubler & Sidechain Alignment Plugin
//!
//! Zero-allocation real-time 2/4-voice vocal doubler & live sidechain guide aligner with
//! Stereo (`2->2`), Mono-to-Stereo (`1->2`), and Mono (`1->1`) layouts, `.make_bypass()`,
//! 10 ms PDC bypass crossfade, `LiveParamMirror`, and Dark Obsidian Pro GUI.

pub mod editor;
pub mod live_mirror;
pub mod params;
pub mod pdc_meter;
pub mod processor;
pub mod processor_modes;
pub mod telemetry;

use editor::{draw_editor, EditorShared, EditorState};
use live_mirror::LiveParamMirror;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use params::PhantomVoxParams;
use phantom_dsp::SharedMeters;
use processor::{VoxBlockConfig, VoxProcessor};
use std::sync::Arc;
use telemetry::SharedVoxTelemetry;

#[cfg(windows)]
#[link(name = "ole32")]
#[link(name = "oleaut32")]
extern "C" {}

pub struct PhantomVox {
    params: Arc<PhantomVoxParams>,
    processor: VoxProcessor,
    shared_meters: Arc<SharedMeters>,
    shared_telemetry: Arc<SharedVoxTelemetry>,
    live_params: Arc<LiveParamMirror>,
    mono_in_stereo_out: bool,
    last_reported_latency: u32,
}

impl Default for PhantomVox {
    fn default() -> Self {
        let params = Arc::new(PhantomVoxParams::default());
        Self {
            live_params: Arc::new(LiveParamMirror::new(params.clone())),
            params,
            processor: VoxProcessor::new(),
            shared_meters: Arc::new(SharedMeters::new()),
            shared_telemetry: Arc::new(SharedVoxTelemetry::new()),
            mono_in_stereo_out: false,
            last_reported_latency: 0,
        }
    }
}

impl PhantomVox {
    fn read_block_config(&self) -> VoxBlockConfig {
        let live = self.live_params.as_ref();
        VoxBlockConfig {
            mode: live.read(&self.params.mode),
            voices: live.read(&self.params.voices),
            bypassed: live.read(&self.params.bypass),
            timing_ms: live.read(&self.params.timing_ms),
            detune_cents: live.read(&self.params.detune_cents),
            formant_st: live.read(&self.params.formant_st),
            width: live.read(&self.params.width_pct) * 0.01,
            sibilance_safe: live.read(&self.params.sibilance_safe),
            align_tightness: live.read(&self.params.align_tightness) * 0.01,
            level_match: live.read(&self.params.level_match) * 0.01,
            mono_check: live.read(&self.params.mono_check),
            mix: live.read(&self.params.mix_pct) * 0.01,
            output_gain_lin: util::db_to_gain(live.read(&self.params.output_gain)),
        }
    }
}

const fn vox_layout(in_ch: u32, out_ch: u32, aux_ports: &'static [NonZeroU32]) -> AudioIOLayout {
    AudioIOLayout {
        main_input_channels: NonZeroU32::new(in_ch),
        main_output_channels: NonZeroU32::new(out_ch),
        aux_input_ports: aux_ports,
        names: PortNames {
            aux_inputs: &["Guide"],
            ..PortNames::const_default()
        },
        ..AudioIOLayout::const_default()
    }
}

impl Plugin for PhantomVox {
    const NAME: &'static str = "Phantom Vox";
    const VENDOR: &'static str = "Phantom DSP";
    const URL: &'static str = "https://github.com/intity01/phantom-dsp-suite";
    const EMAIL: &'static str = "phantom@local.audio";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        vox_layout(2, 2, &[new_nonzero_u32(2)]),
        vox_layout(1, 2, &[new_nonzero_u32(1)]),
        vox_layout(1, 1, &[new_nonzero_u32(1)]),
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
            live: self.live_params.clone(),
            meters: self.shared_meters.clone(),
            telemetry: self.shared_telemetry.clone(),
        };
        create_egui_editor(
            editor_state_handle,
            EditorState::default(),
            |_, _| {},
            move |ctx, setter, state| draw_editor(ctx, setter, state, &params, &shared),
        )
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        let sr = buffer_config.sample_rate;
        self.mono_in_stereo_out = audio_io_layout.main_input_channels == NonZeroU32::new(1)
            && audio_io_layout.main_output_channels == NonZeroU32::new(2);
        self.processor.initialize(sr);
        self.shared_meters.reset();
        self.shared_telemetry.reset();
        self.shared_telemetry.store_sample_rate(sr);
        self.live_params.reconcile();

        let mode = self.live_params.read(&self.params.mode);
        let latency = self.processor.latency_for_mode(mode);
        context.set_latency_samples(latency);
        self.last_reported_latency = latency;
        self.shared_telemetry.store_latency(latency);
        true
    }

    fn reset(&mut self) {
        self.processor.reset();
        self.shared_meters.reset();
        self.shared_telemetry.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.live_params.reconcile();
        let cfg = self.read_block_config();

        let desired_latency = self.processor.latency_for_mode(cfg.mode);
        if desired_latency != self.last_reported_latency {
            context.set_latency_samples(desired_latency);
            self.last_reported_latency = desired_latency;
            self.shared_telemetry.store_latency(desired_latency);
        }

        let (guide_l, guide_r): (&[f32], &[f32]) = aux
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
            self.processor.process_mono(
                &cfg,
                &mut slices[0],
                guide_l,
                &self.shared_meters,
                &self.shared_telemetry,
            );
            return ProcessStatus::KeepAlive;
        }
        let (left_slice, right_slice) = slices.split_at_mut(1);
        self.processor.process_stereo_or_m2s(
            &cfg,
            self.mono_in_stereo_out,
            &mut left_slice[0],
            &mut right_slice[0],
            guide_l,
            guide_r,
            &self.shared_meters,
            &self.shared_telemetry,
        );
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for PhantomVox {
    const CLAP_ID: &'static str = "com.phantom-dsp.phantom-vox";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation real-time vocal doubler & live sidechain timing aligner");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for PhantomVox {
    const VST3_CLASS_ID: [u8; 16] = *b"PhntmVoxAlgn0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Modulation,
        Vst3SubCategory::Spatial,
    ];
}

nih_export_clap!(PhantomVox);
nih_export_vst3!(PhantomVox);

#[cfg(test)]
mod tests;

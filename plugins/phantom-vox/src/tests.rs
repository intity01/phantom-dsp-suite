use std::f32::consts::TAU;
use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::*;
use phantom_dsp::SharedMeters;
use phantom_vox_dsp::LIVE_ALIGN_LATENCY_SAMPLES;

use crate::live_mirror::LiveParamMirror;
use crate::params::{PhantomVoxParams, VoicesParam, VoxModeParam};
use crate::processor::{VoxBlockConfig, VoxProcessor};
use crate::telemetry::SharedVoxTelemetry;
use crate::PhantomVox;

#[test]
fn bypass_flag_metadata_and_live_mirror() {
    let params = Arc::new(PhantomVoxParams::default());
    assert!(params.bypass.flags().contains(ParamFlags::BYPASS));
    assert_eq!(PhantomVox::NAME, "Phantom Vox");
    assert_eq!(PhantomVox::VENDOR, "Phantom DSP");
    assert_eq!(PhantomVox::CLAP_ID, "com.phantom-dsp.phantom-vox");
    assert_eq!(PhantomVox::VST3_CLASS_ID, *b"PhntmVoxAlgn0001");

    let mirror = LiveParamMirror::new(params.clone());
    mirror.write_normalized(params.bypass.as_ptr(), 1.0);
    mirror.reconcile();
    assert!(mirror.read(&params.bypass));
    assert!(!params.bypass.value());
}

#[test]
fn audio_io_layouts_cover_stereo_m2s_and_mono() {
    let layouts = PhantomVox::AUDIO_IO_LAYOUTS;
    assert_eq!(layouts.len(), 3);
    for (layout, (in_ch, out_ch, aux_ch)) in layouts.iter().zip([(2, 2, 2), (1, 2, 1), (1, 1, 1)]) {
        assert_eq!(layout.main_input_channels, NonZeroU32::new(in_ch));
        assert_eq!(layout.main_output_channels, NonZeroU32::new(out_ch));
        assert_eq!(layout.aux_input_ports, &[NonZeroU32::new(aux_ch).unwrap()]);
        assert_eq!(layout.names.aux_inputs, &["Guide"]);
    }
}

#[test]
fn stereo_m2s_and_mono_zero_nan_across_all_modes() {
    let (mut proc, meters, telemetry) = (
        VoxProcessor::new(),
        SharedMeters::new(),
        SharedVoxTelemetry::new(),
    );
    proc.initialize(48_000.0);
    assert_eq!(proc.latency_for_mode(VoxModeParam::Doubler), 0);
    assert_eq!(
        proc.latency_for_mode(VoxModeParam::LiveAlign),
        LIVE_ALIGN_LATENCY_SAMPLES
    );
    assert_eq!(
        proc.latency_for_mode(VoxModeParam::AlignAndDouble),
        LIVE_ALIGN_LATENCY_SAMPLES
    );

    let n = 4096;
    let guide: Vec<f32> = (0..n)
        .map(|i| 0.35 * (TAU * 220.0 * i as f32 / 48_000.0).sin())
        .collect();
    for mode in [
        VoxModeParam::Doubler,
        VoxModeParam::LiveAlign,
        VoxModeParam::AlignAndDouble,
    ] {
        let cfg = VoxBlockConfig {
            mode,
            voices: VoicesParam::FourVoices,
            bypassed: false,
            timing_ms: 14.0,
            detune_cents: 12.0,
            formant_st: 1.0,
            width: 1.1,
            sibilance_safe: true,
            align_tightness: 0.85,
            level_match: 0.35,
            mono_check: false,
            mix: 0.60,
            output_gain_lin: 1.0,
            focus_low_hz: 20.0,
            focus_high_hz: 20_000.0,
            audition: false,
        };
        for mono_in in [false, true] {
            proc.reset();
            let mut l: Vec<f32> = (0..n)
                .map(|i| 0.3 * (TAU * 220.0 * (i as f32 - 48.0) / 48_000.0).sin())
                .collect();
            let mut r = if mono_in { vec![0.0; n] } else { l.clone() };
            proc.process_stereo_or_m2s(
                &cfg, mono_in, &mut l, &mut r, &guide, &guide, &meters, &telemetry,
            );
            assert!(l.iter().chain(r.iter()).all(|s| s.is_finite()));
            assert!(r[2400..].iter().map(|s| s.abs()).fold(0.0_f32, f32::max) > 1e-3);
        }
        proc.reset();
        let mut mono = guide.clone();
        proc.process_mono(&cfg, &mut mono, &guide, &meters, &telemetry);
        assert!(mono.iter().all(|s| s.is_finite()));
    }
}

//! Phantom EQ v0.1.0 Release Candidate Unit & Integration Tests
//! Verifies default engine settings, Gain Scale parameter & LiveParamMirror sync, `.peq` save /
//! delete / legacy compatibility, Mono + Stereo AUDIO_IO_LAYOUTS, URL metadata, and PDC bypass.

use crate::dsp_state::{scale_band_dyn_range, scale_band_gain, SharedBandGr};
use crate::editor::NO_SOLO;
use crate::live_mirror::LiveParamMirror;
use crate::params::{OversamplingParam, PhantomEqParams, ProcessingModeParam};
use crate::processor::EqProcessor;
use crate::state::{
    delete_user_preset_in, load_user_presets_in, parse_peq_preset, sanitize_preset_stem,
    save_user_preset_in, serialize_peq_preset, EqPresetSnapshot, OversampleParam,
};
use crate::PhantomEq;
use nih_plug::prelude::{Param, Plugin};
use phantom_dsp::{FilterType, SharedMeters, LATENCY_SAMPLES};
use std::f32::consts::TAU;
use std::num::NonZeroU32;
use std::sync::Arc;

#[test]
fn oversampling_defaults_to_off_1x() {
    assert_eq!(OversamplingParam::default(), OversamplingParam::Off);
    let params = PhantomEqParams::default();
    assert_eq!(params.oversampling.value(), OversamplingParam::Off);
    assert_eq!(params.mode.value(), ProcessingModeParam::ZeroLatency);

    let snap = EqPresetSnapshot::default();
    assert_eq!(snap.oversampling, OversampleParam::Off);
    assert!((snap.gain_scale_pct - 100.0).abs() < 1e-4);
}

#[test]
fn gain_scale_param_range_and_live_mirror_sync() {
    let params = Arc::new(PhantomEqParams::default());
    assert!((params.gain_scale.value() - 100.0).abs() < 1e-4);
    assert!((params.gain_scale.preview_plain(0.0) - 0.0).abs() < 1e-4);
    assert!((params.gain_scale.preview_plain(1.0) - 200.0).abs() < 1e-4);
    assert_eq!(params.gain_scale.to_string(), "100%");

    let mirror = LiveParamMirror::new(params.clone());
    assert!((mirror.read(&params.gain_scale) - 100.0).abs() < 1e-4);

    let norm_65 = params.gain_scale.preview_normalized(65.0);
    mirror.write_normalized(params.gain_scale.as_ptr(), norm_65);
    mirror.reconcile();
    assert!((mirror.read(&params.gain_scale) - 65.0).abs() < 0.05);
    // Host param still holds default when audio thread is frozen; mirror stays authoritative.
    assert!((params.gain_scale.value() - 100.0).abs() < 1e-4);
}

#[test]
fn peq_save_delete_and_legacy_parse_defaults() {
    assert_eq!(sanitize_preset_stem("../../bad/name"), "bad_name");
    assert_eq!(sanitize_preset_stem("   ...  "), "preset");
    assert_eq!(sanitize_preset_stem("Vocal Shine 01"), "Vocal Shine 01");

    // Legacy .peq without `gain_scale_pct=` must cleanly default to 100.0%.
    let legacy = "format=PhantomEQ.v1\nname=Legacy Test\nmode=0\noversampling=1\ncharacter=0\nauto_gain=0\noutput_gain_db=-1.5000\nphase_invert=0\n";
    let parsed_legacy = parse_peq_preset(legacy).expect("legacy preset should parse");
    assert_eq!(parsed_legacy.name, "Legacy Test");
    assert_eq!(parsed_legacy.oversampling, OversampleParam::X2);
    assert!((parsed_legacy.gain_scale_pct - 100.0).abs() < 1e-4);
    assert!((parsed_legacy.output_gain_db - (-1.5)).abs() < 1e-3);

    // Save, load, verify gain_scale_pct roundtrip, and delete in an isolated temp dir.
    let tmp = std::env::temp_dir().join(format!("phantom_eq_v01_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let mut snap = EqPresetSnapshot::empty("Custom Mix / Clean!", "User");
    snap.gain_scale_pct = 65.0;
    snap.output_gain_db = -2.0;

    let text = serialize_peq_preset(&snap);
    assert!(text.contains("gain_scale_pct=65.00"));

    let saved_path = save_user_preset_in(&tmp, &snap).expect("save preset should succeed");
    assert!(saved_path.is_file());

    let loaded = load_user_presets_in(&tmp);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "Custom Mix / Clean!");
    assert!((loaded[0].gain_scale_pct - 65.0).abs() < 1e-2);

    delete_user_preset_in(&tmp, "Custom Mix / Clean!").expect("delete preset should succeed");
    assert!(load_user_presets_in(&tmp).is_empty());
    assert!(delete_user_preset_in(&tmp, "Custom Mix / Clean!").is_err());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn audio_io_layouts_support_stereo_and_mono_and_repo_url() {
    assert_eq!(
        PhantomEq::URL,
        "https://github.com/intity01/phantom-dsp-suite"
    );
    let layouts = PhantomEq::AUDIO_IO_LAYOUTS;
    assert_eq!(layouts.len(), 2);

    // Stereo 2->2 + 2-ch Sidechain
    assert_eq!(layouts[0].main_input_channels, NonZeroU32::new(2));
    assert_eq!(layouts[0].main_output_channels, NonZeroU32::new(2));
    assert_eq!(layouts[0].aux_input_ports, &[NonZeroU32::new(2).unwrap()]);
    assert_eq!(layouts[0].names.aux_inputs, &["Sidechain"]);

    // Mono 1->1 + 1-ch Sidechain
    assert_eq!(layouts[1].main_input_channels, NonZeroU32::new(1));
    assert_eq!(layouts[1].main_output_channels, NonZeroU32::new(1));
    assert_eq!(layouts[1].aux_input_ports, &[NonZeroU32::new(1).unwrap()]);
    assert_eq!(layouts[1].names.aux_inputs, &["Sidechain"]);
}

#[test]
fn gain_scale_shapes_mono_processing_and_linear_phase_pdc_bypass() {
    // Gain-bearing shapes scale; cuts/notch/all-pass remain unscaled.
    for ft in [
        FilterType::Bell,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::TiltShelf,
        FilterType::FlatTilt,
        FilterType::BandPass,
    ] {
        assert!((scale_band_gain(ft, 6.0, 0.5) - 3.0).abs() < 1e-4);
        assert!((scale_band_dyn_range(ft, -4.0, 0.5) - (-2.0)).abs() < 1e-4);
    }
    for ft in [
        FilterType::LowCut,
        FilterType::HighCut,
        FilterType::Notch,
        FilterType::AllPass,
    ] {
        assert!((scale_band_gain(ft, 6.0, 0.5) - 6.0).abs() < 1e-4);
        assert!((scale_band_dyn_range(ft, -4.0, 0.5) - (-4.0)).abs() < 1e-4);
    }

    // Mono block processing with +6 dB Bell at 1 kHz boosts 1 kHz sine cleanly.
    let params = Arc::new(PhantomEqParams::default());
    let live = LiveParamMirror::new(params.clone());
    live.write_normalized(params.bands[0].used.as_ptr(), 1.0);
    live.write_normalized(params.bands[0].enabled.as_ptr(), 1.0);
    live.write_normalized(
        params.bands[0].frequency.as_ptr(),
        params.bands[0].frequency.preview_normalized(1000.0),
    );
    live.write_normalized(
        params.bands[0].gain.as_ptr(),
        params.bands[0].gain.preview_normalized(6.0),
    );

    let mut proc = EqProcessor::new();
    proc.initialize(48_000.0);
    let meters = SharedMeters::new();
    let gr = SharedBandGr::new();

    let mut mono: Vec<f32> = (0..1024)
        .map(|i| 0.25 * (TAU * 1000.0 * i as f32 / 48_000.0).sin())
        .collect();
    proc.process_mono_block(
        &params,
        &live,
        ProcessingModeParam::ZeroLatency,
        false,
        NO_SOLO,
        &mut mono,
        &[],
        &[],
        1.0,
        &meters,
        &gr,
        None,
    );
    let peak_boosted = mono[512..].iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(
        peak_boosted > 0.42 && peak_boosted < 0.60,
        "expected ~+6 dB boosted mono peak (~0.50), got {peak_boosted}"
    );

    // Linear Phase bypassed unit impulse aligns to exact LATENCY_SAMPLES (3071).
    proc.reset();
    let len = LATENCY_SAMPLES + 128;
    let mut l = vec![0.0_f32; len];
    let mut r = vec![0.0_f32; len];
    l[0] = 1.0;
    r[0] = 1.0;
    proc.process_block(
        &params,
        &live,
        ProcessingModeParam::LinearPhase,
        true,
        NO_SOLO,
        &mut l,
        &mut r,
        &[],
        &[],
        1.0,
        &meters,
        &gr,
    );
    let peak_idx = l
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|(i, _)| i)
        .unwrap();
    assert_eq!(peak_idx, LATENCY_SAMPLES);
    assert!((l[LATENCY_SAMPLES] - 1.0).abs() < 1e-6);
}

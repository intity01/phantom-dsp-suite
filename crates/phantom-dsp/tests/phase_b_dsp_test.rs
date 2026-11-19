//! Phase B Intelligent DSP Integration Tests:
//! 1. `EqMatchEngine` (Reference spectrum accumulation, 1/3-octave delta, parametric band fitting)
//! 2. `SpectralBandProcessor` (Selective per-subband spectral compression & preservation)
//! 3. `CollisionRegistry` (Lock-free multi-instance spectrum collision detection)

use phantom_dsp::{
    display_bin_freq, BiquadCoeffs, CollisionRegistry, EqMatchEngine, FilterType,
    SpectralBandProcessor, NUM_DISPLAY_BINS,
};
use std::f32::consts::PI;

const SR: f32 = 48000.0;

#[test]
fn test_eq_match_engine_fits_1khz_bump_and_250hz_dip() {
    let mut engine = EqMatchEngine::new();
    assert!(!engine.has_reference());
    assert_eq!(engine.reference_frame_count(), 0);
    assert_eq!(engine.current_frame_count(), 0);

    // Current track: flat at -24.0 dB
    let cur_bins = [-24.0_f32; NUM_DISPLAY_BINS];

    // Reference track: -24.0 dB baseline + (+8 dB Bell @ 1 kHz, Q=1.4) + (-6 dB Bell @ 250 Hz, Q=1.6)
    let bell_1k = BiquadCoeffs::calculate(FilterType::Bell, SR, 1000.0, 8.0, 1.4);
    let bell_250 = BiquadCoeffs::calculate(FilterType::Bell, SR, 250.0, -6.0, 1.6);

    let mut ref_bins = [0.0_f32; NUM_DISPLAY_BINS];
    for (i, slot) in ref_bins.iter_mut().enumerate() {
        let f = display_bin_freq(i, SR);
        *slot = -24.0 + bell_1k.magnitude_db_at(f, SR) + bell_250.magnitude_db_at(f, SR);
    }

    // Accumulate multiple frames for both reference and current track
    for _ in 0..4 {
        engine.record_reference_frame(&ref_bins);
        engine.record_current_frame(&cur_bins);
    }

    assert!(engine.has_reference());
    assert!(engine.has_current());
    assert_eq!(engine.reference_frame_count(), 4);
    assert_eq!(engine.current_frame_count(), 4);

    let initial_delta = *engine.delta_bins_db();
    let initial_rms = rms_db(&initial_delta);
    assert!(
        initial_rms > 1.5,
        "Initial spectral delta RMS should be significant, got {initial_rms:.3} dB"
    );

    let (bands, count) = engine.compute_matched_bands(6, 100.0);
    assert!(count >= 2, "Expected at least 2 fitted bands, got {count}");

    // Verify presence of a positive-gain band near 1 kHz and negative-gain band near 250 Hz
    let has_1k_boost = bands[..count]
        .iter()
        .any(|b| (b.freq_hz - 1000.0).abs() < 180.0 && b.gain_db > 3.5);
    let has_250_cut = bands[..count]
        .iter()
        .any(|b| (b.freq_hz - 250.0).abs() < 60.0 && b.gain_db < -2.5);

    assert!(
        has_1k_boost,
        "Expected a boost band near 1 kHz in fitted bands: {:?}",
        &bands[..count]
    );
    assert!(
        has_250_cut,
        "Expected a cut band near 250 Hz in fitted bands: {:?}",
        &bands[..count]
    );

    // Compute residual spectral error after applying the fitted parametric bands
    let mut post_match_error = initial_delta;
    for b in &bands[..count] {
        let c = BiquadCoeffs::calculate(b.filter_type, SR, b.freq_hz, b.gain_db, b.q);
        for (i, err) in post_match_error.iter_mut().enumerate() {
            let f = display_bin_freq(i, SR);
            *err -= c.magnitude_db_at(f, SR);
        }
    }
    let final_rms = rms_db(&post_match_error);
    let reduction_ratio = 1.0 - (final_rms / initial_rms);

    println!(
        "EQ Match RMS error: initial={initial_rms:.3} dB -> final={final_rms:.3} dB (reduction={:.1}%)",
        reduction_ratio * 100.0
    );
    assert!(
        reduction_ratio > 0.50,
        "Expected > 50% RMS spectral error reduction, got {:.1}%",
        reduction_ratio * 100.0
    );

    // Verify 50% amount scaling halves the fitted gains
    let (half_bands, half_count) = engine.compute_matched_bands(6, 50.0);
    assert_eq!(half_count, count);
    assert!(
        (half_bands[0].gain_db - bands[0].gain_db * 0.5).abs() < 1e-4,
        "Expected 50% amount scaling to halve band gains"
    );

    engine.clear_reference();
    engine.clear_current();
    assert!(!engine.has_reference());
    assert_eq!(engine.reference_frame_count(), 0);
    assert_eq!(engine.current_frame_count(), 0);
}

#[test]
fn test_spectral_dynamics_selective_suppression() {
    let mut proc = SpectralBandProcessor::new();

    // Composite signal inside a wide band centered at 1000 Hz (Q = 0.8):
    // - Loud 1000 Hz resonant tone at amplitude 0.70 (-3.1 dBFS, well above -18 dB threshold)
    // - Quiet 1250 Hz neighbor tone at amplitude 0.05 (-26.0 dBFS, below -18 dB threshold)
    let f_loud = 1000.0_f32;
    let f_quiet = 1250.0_f32;
    let amp_loud = 0.70_f32;
    let amp_quiet = 0.05_f32;

    let total_samples = 9600usize; // 200 ms at 48 kHz
    let measure_start = 4800usize; // Measure steady-state on second 100 ms

    let mut in_loud_proj = 0.0_f64;
    let mut out_loud_proj_sin = 0.0_f64;
    let mut out_loud_proj_cos = 0.0_f64;

    let mut in_quiet_proj = 0.0_f64;
    let mut out_quiet_proj_sin = 0.0_f64;
    let mut out_quiet_proj_cos = 0.0_f64;

    for n in 0..total_samples {
        let t = n as f32 / SR;
        let s_loud = (2.0 * PI * f_loud * t).sin();
        let c_loud = (2.0 * PI * f_loud * t).cos();
        let s_quiet = (2.0 * PI * f_quiet * t).sin();
        let c_quiet = (2.0 * PI * f_quiet * t).cos();

        let x = amp_loud * s_loud + amp_quiet * s_quiet;
        let (yl, yr) = proc.process_stereo(x, x, SR, 1000.0, 0.8, -18.0, -12.0);
        assert!(yl.is_finite() && yr.is_finite(), "Output must be finite");
        assert!((yl - yr).abs() < 1e-6, "Symmetric stereo input must match");

        if n >= measure_start {
            in_loud_proj += (amp_loud * s_loud * s_loud) as f64;
            out_loud_proj_sin += (yl * s_loud) as f64;
            out_loud_proj_cos += (yl * c_loud) as f64;

            in_quiet_proj += (amp_quiet * s_quiet * s_quiet) as f64;
            out_quiet_proj_sin += (yl * s_quiet) as f64;
            out_quiet_proj_cos += (yl * c_quiet) as f64;
        }
    }

    let out_loud_mag =
        (out_loud_proj_sin * out_loud_proj_sin + out_loud_proj_cos * out_loud_proj_cos).sqrt();
    let out_quiet_mag =
        (out_quiet_proj_sin * out_quiet_proj_sin + out_quiet_proj_cos * out_quiet_proj_cos).sqrt();

    let gain_loud_db = (20.0 * (out_loud_mag / in_loud_proj).log10()) as f32;
    let gain_quiet_db = (20.0 * (out_quiet_mag / in_quiet_proj).log10()) as f32;

    println!(
        "SpectralBandProcessor selective response: 1000 Hz loud tone change = {gain_loud_db:.2} dB, 1250 Hz quiet tone change = {gain_quiet_db:.2} dB, telemetry = {:.2} dB",
        proc.last_gain_offset_db()
    );

    // 1 kHz loud tone must be strongly compressed (at least -6 dB), while 1.25 kHz quiet tone is preserved (within 1.2 dB)
    assert!(
        gain_loud_db < -6.0,
        "Expected 1 kHz loud tone to be compressed by > 6 dB, got {gain_loud_db:.2} dB"
    );
    assert!(
        gain_quiet_db.abs() < 1.2,
        "Expected 1.25 kHz quiet tone to be preserved within 1.2 dB, got {gain_quiet_db:.2} dB"
    );
    assert!(
        proc.last_gain_offset_db() < -6.0,
        "Expected negative gain offset telemetry, got {:.2} dB",
        proc.last_gain_offset_db()
    );

    // Non-finite input resilience check
    let (nl, nr) = proc.process_stereo(f32::NAN, f32::INFINITY, SR, 1000.0, 1.0, -18.0, -12.0);
    assert!(nl.is_finite() && nr.is_finite());

    proc.reset();
    assert_eq!(proc.last_gain_offset_db(), 0.0);
}

#[test]
fn test_collision_registry_multi_instance_500hz_overlap() {
    let slot_a = CollisionRegistry::register_instance();
    let slot_b = CollisionRegistry::register_instance();
    assert_ne!(
        slot_a, slot_b,
        "Two active instances must receive distinct slots"
    );
    assert!(CollisionRegistry::is_slot_active(slot_a));
    assert!(CollisionRegistry::is_slot_active(slot_b));

    let mut bins_a = [-75.0_f32; NUM_DISPLAY_BINS];
    let mut bins_b = [-75.0_f32; NUM_DISPLAY_BINS];

    // Find the bin closest to 500 Hz
    let mut idx_500 = 0usize;
    let mut min_dist = f32::MAX;
    for i in 0..NUM_DISPLAY_BINS {
        let d = (display_bin_freq(i, SR) - 500.0).abs();
        if d < min_dist {
            min_dist = d;
            idx_500 = i;
        }
    }

    // Both instances have an overlapping peak at 500 Hz (-18 dB and -20 dB, within 6 dB and > -45 dB)
    bins_a[idx_500] = -18.0;
    bins_b[idx_500] = -20.0;

    // Instance A also has an unshared peak at 4 kHz (Instance B is at -75 dB there)
    bins_a[220] = -15.0;

    CollisionRegistry::publish_spectrum(slot_a, &bins_a);
    CollisionRegistry::publish_spectrum(slot_b, &bins_b);

    let mut collisions = [0.0_f32; NUM_DISPLAY_BINS];
    let other_count = CollisionRegistry::compute_collision_bins(slot_a, &bins_a, &mut collisions);
    assert!(
        other_count >= 1,
        "Expected at least 1 other active instance, got {other_count}"
    );
    assert!(
        collisions[idx_500] > 0.5,
        "Expected collision > 0.5 at 500 Hz bin {idx_500}, got {:.3}",
        collisions[idx_500]
    );

    // Every other bin must report strictly 0.0 collision
    for (i, &c) in collisions.iter().enumerate() {
        if i != idx_500 {
            assert_eq!(
                c, 0.0,
                "Expected 0.0 collision at non-overlapping bin {i}, got {c}"
            );
        }
    }

    // Unregister both instances and verify cleanup
    CollisionRegistry::unregister_instance(slot_b);
    let other_after = CollisionRegistry::compute_collision_bins(slot_a, &bins_a, &mut collisions);
    assert_eq!(other_after, 0);
    assert_eq!(collisions[idx_500], 0.0);

    CollisionRegistry::unregister_instance(slot_a);
    assert!(!CollisionRegistry::is_slot_active(slot_a));
    assert!(!CollisionRegistry::is_slot_active(slot_b));
}

fn rms_db(curve: &[f32; NUM_DISPLAY_BINS]) -> f32 {
    let sum_sq: f64 = curve.iter().map(|&v| (v as f64) * (v as f64)).sum();
    (sum_sq / (NUM_DISPLAY_BINS as f64)).sqrt() as f32
}

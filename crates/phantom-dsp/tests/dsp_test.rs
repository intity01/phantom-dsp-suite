use phantom_dsp::{
    decode_mid_side, encode_mid_side, BiquadCoeffs, DynamicDetector, FilterType, StereoBiquad,
};
use std::time::Instant;

#[test]
fn test_dsp_stability_and_no_nans() {
    let filter_types = [
        FilterType::Bell,
        FilterType::LowCut,
        FilterType::HighCut,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::Notch,
        FilterType::BandPass,
    ];
    let sample_rates = [44100.0, 48000.0, 96000.0, 192000.0];

    for &sr in &sample_rates {
        for &ftype in &filter_types {
            let coeffs = BiquadCoeffs::calculate(ftype, sr, 1000.0, 6.0, 1.414);
            let mut filter = StereoBiquad::new();
            filter.set_coeffs(coeffs);

            let (out_l, out_r) = filter.process_stereo(1.0, 1.0);
            assert!(out_l.is_finite() && out_r.is_finite());

            let mag_db = coeffs.magnitude_db_at(1000.0, sr);
            assert!(mag_db.is_finite());

            for _ in 0..1000 {
                let (l, r) = filter.process_stereo(0.0, 0.0);
                assert!(l.is_finite() && r.is_finite());
            }
        }
    }
}

#[test]
fn test_mid_side_reconstruction() {
    let (mid, side) = encode_mid_side(0.75, -0.25);
    let (l_out, r_out) = decode_mid_side(mid, side);
    assert!((l_out - 0.75).abs() < 1e-6);
    assert!((r_out + 0.25).abs() < 1e-6);
}

#[test]
fn test_dynamic_detector() {
    let mut detector = DynamicDetector::new();
    detector.update_ballistics(48000.0, 10.0, 100.0);
    detector.update_sidechain(48000.0, 1000.0, 1.0);

    let offset = detector.compute_dynamic_gain(0.5, 0.5, -20.0, -6.0);
    assert!(offset.is_finite());
}

#[test]
fn test_dsp_throughput_benchmark() {
    let sr = 48000.0;
    let num_samples = 48000 * 20;

    let mut filters: Vec<StereoBiquad> = (0..8)
        .map(|i| {
            let mut f = StereoBiquad::new();
            f.set_coeffs(BiquadCoeffs::calculate(
                FilterType::Bell,
                sr,
                200.0 * (i + 1) as f32,
                3.0,
                1.0,
            ));
            f
        })
        .collect();

    // One timed pass is only ~50 ms, so a single scheduler hiccup on a shared CI runner can
    // halve the result. Warm up once, then keep the fastest of several passes.
    let mut run_pass = || {
        let start = Instant::now();
        let (mut acc_l, mut acc_r) = (0.5f32, -0.5f32);
        for _ in 0..num_samples {
            for f in &mut filters {
                (acc_l, acc_r) = f.process_stereo(acc_l, acc_r);
            }
        }
        std::hint::black_box((acc_l, acc_r));
        start.elapsed()
    };
    run_pass();
    let duration = (0..5).map(|_| run_pass()).min().expect("at least one pass");

    let samples_per_sec = (num_samples * 2) as f64 / duration.as_secs_f64();
    let realtime_ratio = samples_per_sec / (sr as f64 * 2.0);
    println!(
        "Phantom DSP Biquad Performance: {:.2} M samples/sec ({:.0}x realtime at 48kHz stereo)",
        samples_per_sec / 1_000_000.0,
        realtime_ratio
    );
    // A regression guard, not a speed target: 8 bands of stereo biquads must stay far above
    // realtime even on slow shared CI runners (desktop release builds run ~850x).
    let min_realtime = if cfg!(debug_assertions) { 20.0 } else { 100.0 };
    assert!(
        realtime_ratio > min_realtime,
        "{realtime_ratio:.0}x realtime is below the {min_realtime:.0}x floor"
    );
}

use phantom_dsp::{
    decode_mid_side, encode_mid_side, BiquadCoeffs, DynamicDetector, FilterType,
    LinearPhaseEngine, StereoBiquad, FIR_GROUP_DELAY, HOP_SIZE, KERNEL_LEN, LATENCY_SAMPLES,
};
use realfft::RealFftPlanner;
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
fn test_linear_phase_fir_exact_symmetry() {
    let sr = 48000.0;
    let bell = BiquadCoeffs::calculate(FilterType::Bell, sr, 1000.0, 6.0, 1.414);
    let lowcut = BiquadCoeffs::calculate(FilterType::LowCut, sr, 100.0, 0.0, 0.7071);

    let mut engine = LinearPhaseEngine::new();
    engine.update_from_magnitude_curve(sr, |f| {
        bell.magnitude_db_at(f, sr) + lowcut.magnitude_db_at(f, sr)
    });

    let h = engine.kernel_left();
    assert_eq!(h.len(), KERNEL_LEN);

    let mut max_asym = 0.0f32;
    for n in 0..KERNEL_LEN {
        assert!(h[n].is_finite(), "Non-finite FIR coefficient at {}", n);
        let diff = (h[n] - h[KERNEL_LEN - 1 - n]).abs();
        max_asym = max_asym.max(diff);
    }
    assert!(
        max_asym < 1e-6,
        "FIR symmetry error {:.3e} exceeded 1e-6",
        max_asym
    );
}

#[test]
fn test_linear_phase_flat_identity_varied_chunk_sizes() {
    let sr = 48000.0;
    let chunk_sizes = [32, 64, 128, 333, 1024, 2048];
    let total_len = LATENCY_SAMPLES + KERNEL_LEN + 2048;

    for &chunk_size in &chunk_sizes {
        let mut engine = LinearPhaseEngine::new();
        assert_eq!(engine.latency_samples(), LATENCY_SAMPLES);
        engine.update_from_magnitude_curve(sr, |_| 0.0);

        let mut left = vec![0.0f32; total_len];
        let mut right = vec![0.0f32; total_len];
        left[0] = 1.0;
        right[0] = -0.5;

        let mut offset = 0;
        while offset < total_len {
            let end = (offset + chunk_size).min(total_len);
            engine.process_block(&mut left[offset..end], &mut right[offset..end]);
            offset = end;
        }

        let mut max_err = 0.0f32;
        for (i, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
            assert!(l.is_finite() && r.is_finite());
            let expected_l = if i == LATENCY_SAMPLES { 1.0 } else { 0.0 };
            let expected_r = if i == LATENCY_SAMPLES { -0.5 } else { 0.0 };
            max_err = max_err.max((l - expected_l).abs()).max((r - expected_r).abs());
        }

        let err_db = 20.0 * max_err.max(1e-12).log10();
        assert!(
            err_db < -70.0,
            "Chunk {}: identity impulse error {:.2} dB (max_err={:.3e}) >= -70 dB",
            chunk_size,
            err_db,
            max_err
        );
    }
}

fn measure_impulse_response_db(engine: &mut LinearPhaseEngine, sr: f32, chunk_size: usize) -> Vec<f32> {
    let total_len = HOP_SIZE + KERNEL_LEN + 2048;
    let mut left = vec![0.0f32; total_len];
    let mut right = vec![0.0f32; total_len];
    left[0] = 1.0;
    right[0] = 1.0;

    let mut offset = 0;
    while offset < total_len {
        let end = (offset + chunk_size).min(total_len);
        engine.process_block(&mut left[offset..end], &mut right[offset..end]);
        offset = end;
    }

    // Verify zero wrap-around / pre-ringing leakage outside [HOP_SIZE .. HOP_SIZE + KERNEL_LEN]
    for i in 0..HOP_SIZE {
        assert!(left[i].abs() < 1e-6, "Pre-hop artifact at {}: {}", i, left[i]);
    }
    for i in (HOP_SIZE + KERNEL_LEN)..total_len {
        assert!(left[i].abs() < 1e-6, "Post-kernel wrap-around artifact at {}: {}", i, left[i]);
    }

    // Verify end-to-end impulse response symmetry around LATENCY_SAMPLES
    for d in 1..=FIR_GROUP_DELAY {
        let asym = (left[LATENCY_SAMPLES - d] - left[LATENCY_SAMPLES + d]).abs();
        assert!(asym < 1e-5, "Output impulse asymmetry {:.3e} at offset {}", asym, d);
    }

    let analysis_fft = 16384usize;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(analysis_fft);
    let mut time_buf = vec![0.0f32; analysis_fft];
    time_buf[..KERNEL_LEN].copy_from_slice(&left[HOP_SIZE..HOP_SIZE + KERNEL_LEN]);

    let mut spec = fft.make_output_vec();
    fft.process(&mut time_buf, &mut spec).unwrap();

    let _ = sr;
    spec.iter()
        .map(|c| 20.0 * c.norm().max(1e-12).log10())
        .collect()
}

#[test]
fn test_linear_phase_frequency_response_accuracy() {
    let sr = 48000.0;
    let analysis_fft = 16384.0f32;
    let bin_for = |hz: f32| -> usize { (hz * analysis_fft / sr).round() as usize };

    // 1. +6 dB Bell at 1 kHz (Q = 1.0)
    let bell = BiquadCoeffs::calculate(FilterType::Bell, sr, 1000.0, 6.0, 1.0);
    let mut engine = LinearPhaseEngine::new();
    engine.update_from_magnitude_curve(sr, |f| bell.magnitude_db_at(f, sr));
    let bell_db = measure_impulse_response_db(&mut engine, sr, 128);

    for &freq in &[100.0f32, 500.0, 1000.0, 2000.0, 10000.0] {
        let bin = bin_for(freq);
        let actual_freq = bin as f32 * sr / analysis_fft;
        let expected = bell.magnitude_db_at(actual_freq, sr);
        let measured = bell_db[bin];
        let diff = (measured - expected).abs();
        assert!(
            diff <= 0.5,
            "Bell @ {:.1} Hz: expected {:.2} dB, got {:.2} dB (err {:.2} dB)",
            actual_freq,
            expected,
            measured,
            diff
        );
    }

    // 2. Butterworth LowCut at 100 Hz (Q = 0.7071)
    let lowcut = BiquadCoeffs::calculate(FilterType::LowCut, sr, 100.0, 0.0, 0.70710678);
    engine.reset();
    engine.update_from_magnitude_curve(sr, |f| lowcut.magnitude_db_at(f, sr));
    let lc_db = measure_impulse_response_db(&mut engine, sr, 256);

    for &freq in &[50.0f32, 100.0, 200.0, 1000.0, 8000.0] {
        let bin = bin_for(freq);
        let actual_freq = bin as f32 * sr / analysis_fft;
        let expected = lowcut.magnitude_db_at(actual_freq, sr);
        let measured = lc_db[bin];
        let diff = (measured - expected).abs();
        assert!(
            diff <= 0.5,
            "LowCut @ {:.1} Hz: expected {:.2} dB, got {:.2} dB (err {:.2} dB)",
            actual_freq,
            expected,
            measured,
            diff
        );
    }
}

#[test]
fn test_linear_phase_nan_resilience_and_throughput() {
    let sr = 48000.0;
    let bell = BiquadCoeffs::calculate(FilterType::Bell, sr, 1000.0, 6.0, 1.0);
    let mut engine = LinearPhaseEngine::new();
    engine.update_from_magnitude_curve(sr, |f| {
        if f == 0.0 { f32::NAN } else { bell.magnitude_db_at(f, sr) }
    });

    // Inject NaN/Inf samples and verify output remains strictly finite
    let mut bad_l = vec![0.0f32; 2048];
    let mut bad_r = vec![0.0f32; 2048];
    bad_l[10] = f32::NAN;
    bad_r[20] = f32::INFINITY;
    engine.process_block(&mut bad_l, &mut bad_r);
    assert!(bad_l.iter().all(|s| s.is_finite()));
    assert!(bad_r.iter().all(|s| s.is_finite()));

    // Throughput benchmark: 10 seconds of 48 kHz stereo audio in 256-sample DAW blocks
    engine.reset();
    let total_frames = 48000 * 10;
    let mut buf_l = vec![0.25f32; 256];
    let mut buf_r = vec![-0.25f32; 256];

    let start = Instant::now();
    let mut processed = 0;
    while processed < total_frames {
        engine.process_block(&mut buf_l, &mut buf_r);
        processed += 256;
    }
    let dur = start.elapsed();
    let stereo_samples_per_sec = (total_frames * 2) as f64 / dur.as_secs_f64();
    let realtime_ratio = stereo_samples_per_sec / (sr as f64 * 2.0);
    println!(
        "LinearPhaseEngine Throughput: {:.2} M samples/sec ({:.0}x realtime at 48kHz stereo)",
        stereo_samples_per_sec / 1_000_000.0,
        realtime_ratio
    );
    let min_realtime = if cfg!(debug_assertions) { 1.0 } else { 50.0 };
    assert!(
        realtime_ratio > min_realtime,
        "LinearPhase throughput too slow: {:.1}x (min {:.1}x)",
        realtime_ratio,
        min_realtime
    );
}

#[test]
fn test_dsp_throughput_benchmark() {
    let sr = 48000.0;
    let num_samples = 48000 * 20;

    let mut filters: Vec<StereoBiquad> = (0..8)
        .map(|i| {
            let mut f = StereoBiquad::new();
            f.set_coeffs(BiquadCoeffs::calculate(FilterType::Bell, sr, 200.0 * (i + 1) as f32, 3.0, 1.0));
            f
        })
        .collect();

    let start = Instant::now();
    let (mut acc_l, mut acc_r) = (0.5f32, -0.5f32);
    for _ in 0..num_samples {
        for f in &mut filters {
            (acc_l, acc_r) = f.process_stereo(acc_l, acc_r);
        }
    }

    let duration = start.elapsed();
    let samples_per_sec = (num_samples * 2) as f64 / duration.as_secs_f64();
    let realtime_ratio = samples_per_sec / (sr as f64 * 2.0);
    println!(
        "Phantom DSP Biquad Performance: {:.2} M samples/sec ({:.0}x realtime at 48kHz stereo)",
        samples_per_sec / 1_000_000.0,
        realtime_ratio
    );
    let min_realtime = if cfg!(debug_assertions) { 20.0 } else { 500.0 };
    assert!(realtime_ratio > min_realtime);
}

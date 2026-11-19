use phantom_dsp::{
    BiquadCoeffs, CascadedCoeffs, CascadedStereoFilter, CharacterMode, CharacterProcessor,
    FilterSlope, FilterType, OversampleFactor, Oversampler2x, Oversampler4x,
};
use std::f32::consts::{FRAC_1_SQRT_2, PI};

const ALL_SHAPES: [FilterType; 10] = [
    FilterType::Bell,
    FilterType::LowCut,
    FilterType::HighCut,
    FilterType::LowShelf,
    FilterType::HighShelf,
    FilterType::Notch,
    FilterType::BandPass,
    FilterType::TiltShelf,
    FilterType::FlatTilt,
    FilterType::AllPass,
];

const ALL_SLOPES: [FilterSlope; 8] = [
    FilterSlope::Db6,
    FilterSlope::Db12,
    FilterSlope::Db18,
    FilterSlope::Db24,
    FilterSlope::Db36,
    FilterSlope::Db48,
    FilterSlope::Db96,
    FilterSlope::Brickwall,
];

fn calc(ft: FilterType, sl: FilterSlope, sr: f32, f: f32, g: f32, q: f32) -> CascadedCoeffs {
    CascadedCoeffs::calculate(ft, sl, sr, f, g, q, false)
}

#[test]
fn test_all_shapes_and_slopes_stability_no_nans() {
    for &sr in &[44100.0, 48000.0, 96000.0, 192000.0] {
        for &ft in &ALL_SHAPES {
            for &sl in &ALL_SLOPES {
                for &nat in &[false, true] {
                    for &f in &[80.0, 1000.0, 12000.0] {
                        let c = CascadedCoeffs::calculate(ft, sl, sr, f, 6.0, FRAC_1_SQRT_2, nat);
                        assert!((1..=8).contains(&c.num_stages));
                        assert!(c.magnitude_db_at(f, sr).is_finite());

                        let mut flt = CascadedStereoFilter::new();
                        flt.set_coeffs(&c);
                        let (l0, r0) = flt.process_stereo(1.0, -0.5);
                        assert!(l0.is_finite() && r0.is_finite());
                        for _ in 0..256 {
                            let (l, r) = flt.process_stereo(0.0, 0.0);
                            assert!(l.is_finite() && r.is_finite() && l.abs() < 1e4);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_cut_slopes_monotonic_and_brickwall_ripple() {
    let (sr, cutoff) = (48000.0, 1000.0);
    for &ft in &[FilterType::HighCut, FilterType::LowCut] {
        let stop_f = if ft == FilterType::HighCut {
            cutoff * 2.0
        } else {
            cutoff * 0.5
        };
        let mut prev_db = 0.0f32;
        for &sl in &ALL_SLOPES {
            let c = calc(ft, sl, sr, cutoff, 0.0, FRAC_1_SQRT_2);
            let stop_db = c.magnitude_db_at(stop_f, sr);
            assert!(
                stop_db < prev_db - 2.0,
                "{ft:?} {sl:?}: {stop_db:.2} vs {prev_db:.2}"
            );
            prev_db = stop_db;

            if sl != FilterSlope::Brickwall {
                let fc_db = c.magnitude_db_at(cutoff, sr);
                assert!((fc_db + 3.01).abs() < 0.25, "{ft:?} {sl:?}: {fc_db:.2} dB");
            } else {
                assert!(
                    stop_db < -100.0,
                    "{ft:?} Brickwall stopband {stop_db:.2} dB"
                );
                for step in 1..=20 {
                    let pf = if ft == FilterType::HighCut {
                        cutoff * (step as f32 / 22.0)
                    } else {
                        cutoff * (1.1 + step as f32 * 0.3)
                    };
                    let pb_db = c.magnitude_db_at(pf, sr);
                    assert!(
                        (-0.5..=0.05).contains(&pb_db),
                        "{ft:?} ripple @{pf} = {pb_db:.3}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_allpass_tilt_flattilt_bell_and_natural_phase() {
    let sr = 44100.0;
    for &sl in &ALL_SLOPES {
        let c = calc(FilterType::AllPass, sl, sr, 1200.0, 0.0, 1.5);
        for &f in &[20.0, 100.0, 500.0, 1200.0, 5000.0, 12000.0, 20000.0] {
            assert!(c.magnitude_db_at(f, sr).abs() < 0.05, "AllPass {sl:?} @{f}");
        }
    }

    let tilt = calc(
        FilterType::TiltShelf,
        FilterSlope::Db12,
        sr,
        1000.0,
        6.0,
        FRAC_1_SQRT_2,
    );
    assert!(tilt.magnitude_db_at(1000.0, sr).abs() < 0.05);
    assert!(tilt.magnitude_db_at(50.0, sr) < -2.5 && tilt.magnitude_db_at(15000.0, sr) > 2.5);

    let ft = calc(
        FilterType::FlatTilt,
        FilterSlope::Db12,
        sr,
        1000.0,
        10.0,
        FRAC_1_SQRT_2,
    );
    assert!(ft.magnitude_db_at(1000.0, sr).abs() < 0.05);
    let s1 = ft.magnitude_db_at(400.0, sr) - ft.magnitude_db_at(200.0, sr);
    let s2 = ft.magnitude_db_at(1600.0, sr) - ft.magnitude_db_at(800.0, sr);
    let s3 = ft.magnitude_db_at(6400.0, sr) - ft.magnitude_db_at(3200.0, sr);
    assert!(s1 > 0.5 && (s1 - s2).abs() < 0.25 && (s2 - s3).abs() < 0.25);

    let rbj = BiquadCoeffs::calculate(FilterType::Bell, sr, 15000.0, 12.0, 1.0);
    let nat = BiquadCoeffs::calculate_natural(FilterType::Bell, sr, 15000.0, 12.0, 1.0);
    let cut = BiquadCoeffs::calculate_natural(FilterType::Bell, sr, 15000.0, -12.0, 1.0);
    assert!((nat.magnitude_db_at(15000.0, sr) - 12.0).abs() < 0.1);
    assert!(nat.magnitude_db_at(21000.0, sr) > rbj.magnitude_db_at(21000.0, sr) + 3.0);
    assert!((nat.magnitude_db_at(18000.0, sr) + cut.magnitude_db_at(18000.0, sr)).abs() < 0.05);

    let b12 = calc(
        FilterType::Bell,
        FilterSlope::Db12,
        48000.0,
        1000.0,
        6.0,
        1.0,
    );
    let b48 = calc(
        FilterType::Bell,
        FilterSlope::Db48,
        48000.0,
        1000.0,
        6.0,
        1.0,
    );
    assert!((b48.magnitude_db_at(1000.0, 48000.0) - 6.0).abs() < 0.05);
    assert!(b48.magnitude_db_at(850.0, 48000.0) > b12.magnitude_db_at(850.0, 48000.0));
    assert!(b48.magnitude_db_at(500.0, 48000.0) < b12.magnitude_db_at(500.0, 48000.0) - 1.0);
}

#[test]
fn test_oversampler_2x_and_4x_flatness_and_stopband() {
    assert_eq!(OversampleFactor::Off.multiplier(), 1);
    assert_eq!(OversampleFactor::X2.multiplier(), 2);
    assert_eq!(OversampleFactor::X4.multiplier(), 4);

    let sr = 48000.0f32;
    let (mut os2, mut os4) = (Oversampler2x::new(), Oversampler4x::new());
    let (mut last2, mut last4) = ((0.0, 0.0), (0.0, 0.0));
    for _ in 0..512 {
        let [s0, s1] = os2.upsample_2x(1.0, -0.5);
        last2 = os2.downsample_2x(s0, s1);
        let s4 = os4.upsample_4x(1.0, -0.5);
        last4 = os4.downsample_4x(s4);
    }
    assert!((20.0 * last2.0.abs().log10()).abs() < 0.02 && (last2.1 + 0.5).abs() < 0.002);
    assert!((20.0 * last4.0.abs().log10()).abs() < 0.02 && (last4.1 + 0.5).abs() < 0.002);

    os2.reset();
    os4.reset();
    let (mut in_sq, mut out2_sq, mut out4_sq) = (0.0f64, 0.0f64, 0.0f64);
    for n in 0..2880 {
        let x = (2.0 * PI * 1000.0 * n as f32 / sr).sin();
        let [a, b] = os2.upsample_2x(x, x);
        let y2 = os2.downsample_2x(a, b).0;
        let q = os4.upsample_4x(x, x);
        let y4 = os4.downsample_4x(q).0;
        assert!(y2.is_finite() && y4.is_finite());
        if n >= 576 {
            in_sq += (x as f64).powi(2);
            out2_sq += (y2 as f64).powi(2);
            out4_sq += (y4 as f64).powi(2);
        }
    }
    assert!((10.0 * (out2_sq / in_sq).log10()).abs() < 0.02);
    assert!((10.0 * (out4_sq / in_sq).log10()).abs() < 0.02);

    os2.reset();
    os4.reset();
    let (mut peak2, mut peak4) = (0.0f32, 0.0f32);
    let pi64 = std::f64::consts::PI;
    for n in 0..1000 {
        let s0 = (2.0 * pi64 * 0.35 * (2 * n) as f64).sin() as f32;
        let s1 = (2.0 * pi64 * 0.35 * (2 * n + 1) as f64).sin() as f32;
        let y2 = os2.downsample_2x((s0, s0), (s1, s1)).0;
        let arr: [(f32, f32); 4] = std::array::from_fn(|k| {
            let v = (2.0 * pi64 * 0.38 * (4 * n + k) as f64).sin() as f32;
            (v, v)
        });
        let y4 = os4.downsample_4x(arr).0;
        if n > 200 {
            peak2 = peak2.max(y2.abs());
            peak4 = peak4.max(y4.abs());
        }
    }
    assert!(20.0 * peak2.max(1e-12).log10() < -85.0);
    assert!(20.0 * peak4.max(1e-12).log10() < -85.0);
}

#[test]
fn test_character_modes_passthrough_dc_and_harmonics() {
    let sr = 48000.0f32;
    let mut proc = CharacterProcessor::new();
    proc.set_sample_rate(sr);

    for &(l, r) in &[(0.0, 0.0), (0.1234567, -0.9876543), (1.5, -2.25)] {
        let (ol, or) = proc.process_stereo(l, r, CharacterMode::Clean);
        assert_eq!(ol.to_bits(), l.to_bits());
        assert_eq!(or.to_bits(), r.to_bits());
    }

    for &mode in &[CharacterMode::SubtleAnalog, CharacterMode::WarmTube] {
        proc.reset();
        let (ol, or) = proc.process_stereo(50.0, -50.0, mode);
        assert!(ol.is_finite() && or.is_finite() && ol.abs() < 10.0 && or.abs() < 10.0);
        let (nl, nr) = proc.process_stereo(f32::NAN, f32::INFINITY, mode);
        assert!(nl.is_finite() && nr.is_finite());
    }

    let measure = |mode: CharacterMode| -> (f64, f64) {
        let mut p = CharacterProcessor::new();
        p.set_sample_rate(sr);
        let (n_settle, n_meas) = (48000, 4800);
        let (mut dc, mut h1_re, mut h1_im, mut h2_re, mut h2_im) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for n in 0..(n_settle + n_meas) {
            let ph = 2.0 * std::f64::consts::PI * 1000.0 * n as f64 / sr as f64;
            let (y, _) = p.process_stereo((0.5 * ph.sin()) as f32, 0.0, mode);
            if n >= n_settle {
                let yf = y as f64;
                dc += yf;
                h1_re += yf * ph.cos();
                h1_im += yf * ph.sin();
                h2_re += yf * (2.0 * ph).cos();
                h2_im += yf * (2.0 * ph).sin();
            }
        }
        let h2_db = 20.0
            * (h2_re.hypot(h2_im) / h1_re.hypot(h1_im).max(1e-12))
                .max(1e-12)
                .log10();
        (dc.abs() / n_meas as f64, h2_db)
    };

    let (dc_clean, h2_clean) = measure(CharacterMode::Clean);
    let (dc_sub, h2_sub) = measure(CharacterMode::SubtleAnalog);
    let (dc_tube, h2_tube) = measure(CharacterMode::WarmTube);
    assert!(dc_clean < 1e-6 && h2_clean < -110.0);
    assert!(dc_sub < 5e-4 && dc_tube < 5e-4);
    assert!(
        h2_sub > -80.0 && h2_sub < -40.0,
        "SubtleAnalog H2={h2_sub:.2} dB"
    );
    assert!(
        h2_tube > h2_sub + 15.0 && h2_tube < -15.0,
        "WarmTube H2={h2_tube:.2} dB"
    );
}

use phantom_dsp::{
    display_bin_freq, AutoGainCompensator, DynamicDetector, OutputMeter, SharedMeters,
    SharedSpectrum, SpectrumProcessor, DEFAULT_TILT_DB_PER_OCT, FFT_SIZE, NUM_DISPLAY_BINS,
    SPECTRUM_FLOOR_DB,
};
use std::f32::consts::TAU;

fn sine_sample(freq_hz: f32, sr: f32, idx: usize, amp: f32) -> f32 {
    amp * (TAU * freq_hz * idx as f32 / sr).sin()
}

fn nearest_display_bin(target_hz: f32, sr: f32) -> usize {
    (0..NUM_DISPLAY_BINS)
        .min_by(|&a, &b| {
            let da = (display_bin_freq(a, sr) - target_hz).abs();
            let db = (display_bin_freq(b, sr) - target_hz).abs();
            da.partial_cmp(&db).unwrap()
        })
        .unwrap()
}

#[test]
fn test_dynamic_detector_ballistics_soft_knee_and_gr() {
    let sr = 48000.0;
    let (mut fast, mut slow) = (DynamicDetector::new(), DynamicDetector::new());
    fast.update_ballistics(sr, 2.0, 20.0);
    slow.update_ballistics(sr, 120.0, 800.0);
    fast.update_sidechain(sr, 1000.0, 1.0);
    slow.update_sidechain(sr, 1000.0, 1.0);

    // 10 ms burst at -6 dBFS, threshold -24 dBFS, range -6 dB
    let (mut fast_gr, mut slow_gr) = (0.0, 0.0);
    for i in 0..(0.010 * sr) as usize {
        let s = sine_sample(1000.0, sr, i, 0.5);
        fast_gr = fast.compute_dynamic_gain_ext(s, s, 0.0, 0.0, false, -24.0, false, -6.0);
        slow_gr = slow.compute_dynamic_gain_ext(s, s, 0.0, 0.0, false, -24.0, false, -6.0);
    }
    assert!((fast_gr - fast.last_gain_offset_db()).abs() < 1e-6);
    assert!(
        fast_gr < slow_gr - 1.0,
        "fast={fast_gr:.2}, slow={slow_gr:.2}"
    );

    // Settle on steady signal, then measure 60 ms release into silence
    for i in 0..(sr as usize) {
        let s = sine_sample(1000.0, sr, i, 0.5);
        fast.compute_dynamic_gain(s, s, -24.0, -6.0);
        slow.compute_dynamic_gain(s, s, -24.0, -6.0);
    }
    for _ in 0..(0.060 * sr) as usize {
        fast_gr = fast.compute_dynamic_gain(0.0, 0.0, -24.0, -6.0);
        slow_gr = slow.compute_dynamic_gain(0.0, 0.0, -24.0, -6.0);
    }
    assert!(
        fast_gr > slow_gr + 1.0,
        "fast={fast_gr:.2}, slow={slow_gr:.2}"
    );

    // 6 dB soft knee engages smoothly right at threshold
    let mut knee = DynamicDetector::new();
    knee.update_ballistics(sr, 5.0, 80.0);
    knee.update_sidechain(sr, 1000.0, 1.0);
    let mut at_thresh = 0.0;
    let amp = 10.0_f32.powf(-12.0 / 20.0);
    for i in 0..(0.15 * sr) as usize {
        let s = sine_sample(1000.0, sr, i, amp);
        at_thresh = knee.compute_dynamic_gain(s, s, -12.0, -6.0);
    }
    assert!(at_thresh < -0.05 && at_thresh > -1.0, "knee={at_thresh:.3}");
}

#[test]
fn test_dynamic_detector_auto_threshold_and_ext_sidechain() {
    let sr = 48000.0;
    let (mut quiet_det, mut loud_det) = (DynamicDetector::new(), DynamicDetector::new());
    for d in [&mut quiet_det, &mut loud_det] {
        d.update_ballistics(sr, 10.0, 100.0);
        d.update_sidechain(sr, 1000.0, 1.0);
    }

    let amp_quiet = 10.0_f32.powf(-32.0 / 20.0);
    let amp_loud = 10.0_f32.powf(-8.0 / 20.0);
    let (mut gr_quiet, mut gr_loud) = (0.0, 0.0);
    for i in 0..(5.0 * sr) as usize {
        let q = sine_sample(1000.0, sr, i, amp_quiet);
        let l = sine_sample(1000.0, sr, i, amp_loud);
        gr_quiet = quiet_det.compute_dynamic_gain_ext(q, q, 0.0, 0.0, false, 0.0, true, -6.0);
        gr_loud = loud_det.compute_dynamic_gain_ext(l, l, 0.0, 0.0, false, 0.0, true, -6.0);
    }
    assert!((quiet_det.auto_threshold_db() + 35.0).abs() < 2.0);
    assert!((loud_det.auto_threshold_db() + 11.0).abs() < 2.0);
    assert!(
        (gr_quiet - gr_loud).abs() < 0.6,
        "q={gr_quiet:.2}, l={gr_loud:.2}"
    );

    // +12 dB transient above settled level triggers deeper GR
    let mut burst_gr = 0.0;
    for i in 0..(0.04 * sr) as usize {
        let b = sine_sample(1000.0, sr, i, amp_quiet * 4.0);
        burst_gr = quiet_det.compute_dynamic_gain_ext(b, b, 0.0, 0.0, false, 0.0, true, -6.0);
    }
    assert!(
        burst_gr < gr_quiet - 2.0,
        "burst={burst_gr:.2}, steady={gr_quiet:.2}"
    );

    // External sidechain triggers GR even when main signal is silent
    let mut ext = DynamicDetector::new();
    ext.update_ballistics(sr, 5.0, 80.0);
    ext.update_sidechain(sr, 1000.0, 1.0);
    let (mut int_gr, mut ext_gr) = (0.0, 0.0);
    for i in 0..(0.1 * sr) as usize {
        let sc = sine_sample(1000.0, sr, i, 0.5);
        int_gr = ext.compute_dynamic_gain_ext(0.0, 0.0, sc, sc, false, -20.0, false, -6.0);
    }
    assert!(int_gr.abs() < 1e-6);
    for i in 0..(0.1 * sr) as usize {
        let sc = sine_sample(1000.0, sr, i, 0.5);
        ext_gr = ext.compute_dynamic_gain_ext(0.0, 0.0, sc, sc, true, -20.0, false, -6.0);
    }
    assert!(ext_gr < -3.0 && (ext.last_gain_offset_db() - ext_gr).abs() < 1e-6);
}

#[test]
fn test_auto_gain_compensates_boost_and_gates_silence() {
    let sr = 48000.0;
    let mut ag = AutoGainCompensator::new();
    ag.set_sample_rate(sr);

    // +6.02 dB boost -> compensated output within +-0.8 dB of input loudness
    let (total, m_start) = ((2.0 * sr) as usize, (1.8 * sr) as usize);
    let (mut in_sq, mut out_sq) = (0.0_f64, 0.0_f64);
    for i in 0..total {
        let pre = sine_sample(1000.0, sr, i, 0.15);
        let (ol, or) = ag.process_stereo(pre, pre, pre * 2.0, pre * 2.0, true);
        if i >= m_start {
            in_sq += (pre * pre) as f64;
            out_sq += 0.5 * ((ol * ol + or * or) as f64);
        }
    }
    let diff_db = 10.0 * (out_sq / in_sq).log10();
    assert!(diff_db.abs() <= 0.8, "diff={diff_db:+.2} dB");
    assert!((ag.current_makeup_db() + 6.02).abs() <= 0.8);

    // Strict +-18 dB clamp under extreme +30 dB boost
    ag.reset();
    for i in 0..(2.0 * sr) as usize {
        let pre = sine_sample(1000.0, sr, i, 0.05);
        ag.process_stereo(pre, pre, pre * 31.62, pre * 31.62, true);
    }
    assert!(ag.current_makeup_db() >= -18.01 && ag.current_makeup_db() < -17.0);

    // Silence gate (< -55 dBFS) must NOT amplify noise floor
    ag.reset();
    let noise_amp = 10.0_f32.powf(-68.0 / 20.0);
    for i in 0..(2.0 * sr) as usize {
        let pre = sine_sample(1000.0, sr, i, noise_amp);
        let (ol, or) = ag.process_stereo(pre, pre, pre * 0.25, pre * 0.25, true);
        assert!(ol.is_finite() && or.is_finite());
    }
    assert!(ag.current_makeup_db().abs() < 0.05);
}

#[test]
fn test_output_meter_bs1770_calibration_and_peaks() {
    for &sr in &[44100.0_f32, 48000.0, 96000.0] {
        let mut meter = OutputMeter::new();
        meter.set_sample_rate(sr);

        // Stereo 1 kHz sine at -14 dBFS total stereo RMS (EBU Tech 3341: -14 dBFS peak stereo)
        let amp = 10.0_f32.powf(-14.0 / 20.0);
        let n_400 = (0.4 * sr).round() as usize;
        for i in 0..n_400 {
            let s = sine_sample(1000.0, sr, i, amp);
            meter.process_sample(s, s);
        }
        let s400 = meter.snapshot();
        assert!(
            (s400.momentary_lufs + 14.0).abs() <= 0.5,
            "sr={sr}: got {:.2} LUFS",
            s400.momentary_lufs
        );

        let n_rest = (2.6 * sr).round() as usize;
        for i in n_400..(n_400 + n_rest) {
            let s = sine_sample(1000.0, sr, i, amp);
            meter.process_sample(s, s);
        }
        let shared = SharedMeters::new();
        meter.publish(&shared);
        shared.store_makeup_db(-3.5);
        let loaded = shared.load();
        assert!((loaded.short_term_lufs + 14.0).abs() <= 0.5);
        assert!((loaded.peak_l_db + 14.0).abs() <= 0.5);
        assert!((loaded.hold_r_db + 14.0).abs() <= 0.5);
        assert!((shared.load_makeup_db() + 3.5).abs() < 1e-6);
    }

    // K-weighting frequency curve: +4 dB high shelf @ 5 kHz, HPF cut @ 30 Hz
    let sr = 48000.0;
    let amp = 10.0_f32.powf(-14.0 / 20.0);
    let measure = |freq: f32| {
        let mut m = OutputMeter::new();
        m.set_sample_rate(sr);
        for i in 0..(0.6 * sr) as usize {
            let s = sine_sample(freq, sr, i, amp);
            m.process_sample(s, s);
        }
        m.snapshot().momentary_lufs
    };
    assert!(measure(5000.0) > -11.5);
    assert!(measure(30.0) < -17.0);
}

#[test]
fn test_spectrum_pre_post_tilt_and_peak_freeze() {
    let sr = 48000.0;
    let shared = SharedSpectrum::new();
    let mut proc = SpectrumProcessor::new();
    assert!((proc.tilt_db_per_oct() - DEFAULT_TILT_DB_PER_OCT).abs() < 1e-6);

    let (mut multi, mut post_4k, mut side_2k) = (
        [0.0_f32; FFT_SIZE],
        [0.0_f32; FFT_SIZE],
        [0.0_f32; FFT_SIZE],
    );
    for i in 0..FFT_SIZE {
        multi[i] = sine_sample(500.0, sr, i, 0.25)
            + sine_sample(1000.0, sr, i, 0.25)
            + sine_sample(2000.0, sr, i, 0.25);
        post_4k[i] = sine_sample(4000.0, sr, i, 0.5);
        side_2k[i] = sine_sample(2000.0, sr, i, 0.5);
    }

    shared.push_pre_samples(&multi);
    shared.push_post_samples(&post_4k);
    shared.push_side_samples(&side_2k);
    proc.update(&shared, sr);

    let b500 = nearest_display_bin(500.0, sr);
    let b1k = nearest_display_bin(1000.0, sr);
    let b2k = nearest_display_bin(2000.0, sr);
    let b4k = nearest_display_bin(4000.0, sr);

    let pre = proc.pre_bins_db();
    assert!(((pre[b2k] - pre[b1k]) - 4.5).abs() <= 0.7);
    assert!(((pre[b500] - pre[b1k]) + 4.5).abs() <= 0.7);

    let disp = shared.display_data.lock().unwrap().clone();
    assert_eq!(disp.bins_db[b4k], disp.post_bins_db[b4k]);
    assert!(disp.post_bins_db[b4k] > -10.0 && disp.side_bins_db[b2k] > -10.0);
    assert!(disp.peak_bins_db[b4k] >= disp.post_bins_db[b4k]);

    // Freeze prevents both live decay and peak changes on subsequent silence
    let (frozen_post, frozen_peak) = (disp.post_bins_db[b4k], disp.peak_bins_db[b4k]);
    proc.set_frozen(true);
    let silence = [0.0_f32; FFT_SIZE];
    for _ in 0..10 {
        shared.push_post_samples(&silence);
        proc.update(&shared, sr);
    }
    let after = shared.display_data.lock().unwrap().clone();
    assert_eq!(after.post_bins_db[b4k], frozen_post);
    assert_eq!(after.peak_bins_db[b4k], frozen_peak);

    proc.reset_peaks();
    proc.update(&shared, sr);
    assert_eq!(proc.peak_hold_bins_db()[b4k], SPECTRUM_FLOOR_DB);
}

//! Integration tests for v0.1.0 DSP parameter smoothing, 10 ms bypass crossfade,
//! exact Linear Phase PDC bypass delay alignment, and strict zero-allocation guarantees.

use phantom_dsp::{
    BandParamSmoother, BypassCrossfade, CascadedCoeffs, CascadedStereoFilter, FilterSlope,
    FilterType, LatencyBypassDelay, LinearPhaseEngine, OnePoleSmoother, LATENCY_SAMPLES,
    SMOOTH_SUBBLOCK,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::f32::consts::PI;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};

const SR: f32 = 48_000.0;

struct CountingAllocator;

static TRACKING_ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static TRACK_THREAD: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING_ACTIVE.load(Ordering::Relaxed) {
            let _ = TRACK_THREAD.try_with(|flag| {
                if flag.get() {
                    let _ = THREAD_ALLOCS.try_with(|c| c.set(c.get() + 1));
                }
            });
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn measure_thread_allocs<F: FnOnce()>(f: F) -> usize {
    TRACK_THREAD.with(|t| t.set(false));
    THREAD_ALLOCS.with(|c| c.set(0));
    TRACKING_ACTIVE.store(true, Ordering::SeqCst);
    TRACK_THREAD.with(|t| t.set(true));
    f();
    TRACK_THREAD.with(|t| t.set(false));
    THREAD_ALLOCS.with(|c| c.get())
}

fn bell(f: f32, g: f32, q: f32) -> CascadedCoeffs {
    CascadedCoeffs::calculate(FilterType::Bell, FilterSlope::Db12, SR, f, g, q, false)
}

#[test]
fn test_one_pole_and_band_param_smoother_click_free_sweep() {
    // 1. OnePoleSmoother linear + log accuracy, skip() parity, and log floor safety
    let (mut lin_step, mut lin_skip) = (OnePoleSmoother::new(0.0), OnePoleSmoother::new(0.0));
    lin_step.set_sample_rate(SR, 15.0);
    lin_skip.set_sample_rate(SR, 15.0);
    lin_step.set_target(1.0);
    lin_skip.set_target(1.0);

    let tau_n = (0.015 * SR).round() as usize;
    let mut prev = 0.0_f32;
    for _ in 0..tau_n {
        let v = lin_step.next();
        assert!(v >= prev && v <= 1.0);
        prev = v;
    }
    lin_skip.skip(tau_n);
    let exp_1tau = 1.0 - (-1.0_f32).exp();
    assert!((lin_step.current() - exp_1tau).abs() < 2e-3);
    assert!((lin_step.current() - lin_skip.current()).abs() < 1e-5);
    lin_skip.skip(tau_n * 10);
    assert!(lin_skip.is_settled(1e-4));

    let mut log_sm = OnePoleSmoother::new(100.0);
    log_sm.set_sample_rate(SR, 15.0);
    log_sm.set_target(10_000.0);
    for _ in 0..tau_n {
        let v = log_sm.next_log();
        assert!(v.is_finite() && (100.0..=10_000.0).contains(&v));
    }
    let exp_log_1tau = 100.0 * (100.0_f32).powf(exp_1tau);
    assert!((log_sm.current() - exp_log_1tau).abs() / exp_log_1tau < 0.01);

    let mut safe_log = OnePoleSmoother::new(0.0);
    safe_log.set_target(-5.0);
    let clamped = safe_log.next_log();
    assert!(clamped.is_finite() && clamped >= 1e-6);

    // 2. BandParamSmoother + CascadedStereoFilter swept transition (no click spikes)
    assert_eq!(SMOOTH_SUBBLOCK, 16);
    let (settle_n, total_n) = (1024usize, 1024 + 12_000);
    let run_sweep = |smoothed: bool| -> (f32, (f32, f32, f32), bool) {
        let mut filter = CascadedStereoFilter::new();
        let mut smoother = BandParamSmoother::new(250.0, -6.0, 1.0);
        smoother.set_sample_rate(SR, 15.0);
        filter.set_coeffs(&bell(250.0, -6.0, 1.0));

        let (mut prev_l, mut max_delta) = (0.0_f32, 0.0_f32);
        for i in 0..total_n {
            if i == settle_n {
                if smoothed {
                    smoother.set_targets(4000.0, 12.0, 3.0);
                } else {
                    filter.set_coeffs(&bell(4000.0, 12.0, 3.0));
                }
            }
            if smoothed && i >= settle_n && (i - settle_n) % SMOOTH_SUBBLOCK == 0 {
                let (f, g, q) = smoother.advance_subblock(SMOOTH_SUBBLOCK);
                filter.set_coeffs(&bell(f, g, q));
            }
            let x = 0.5 * (2.0 * PI * 440.0 * i as f32 / SR).sin();
            let (yl, yr) = filter.process_stereo(x, x);
            assert!(yl.is_finite() && yr.is_finite());
            if i >= settle_n {
                max_delta = max_delta.max((yl - prev_l).abs());
            }
            prev_l = yl;
        }
        (max_delta, smoother.current(), smoother.is_settled())
    };

    let (jump_delta, _, _) = run_sweep(false);
    let (smooth_delta, (ff, fg, fq), settled) = run_sweep(true);
    println!("Band sweep max delta: jump={jump_delta:.5}, smooth={smooth_delta:.5}");
    assert!(smooth_delta < jump_delta * 0.5 && settled);
    assert!((ff - 4000.0).abs() < 1e-2 && (fg - 12.0).abs() < 1e-3 && (fq - 3.0).abs() < 1e-3);
}

#[test]
fn test_bypass_crossfade_10ms_ramps() {
    let check_ramp = |xf: &mut BypassCrossfade, n: usize, max_step: f32, down: bool| {
        let mut prev = if down { 1.0_f32 } else { 0.0_f32 };
        for i in 0..n {
            let w = xf.next_wet();
            let step = if down { prev - w } else { w - prev };
            assert!(step >= 0.0 && step <= max_step, "bad step: {prev} -> {w}");
            if i == n / 2 - 1 {
                assert!((w - 0.5).abs() < 0.01);
            }
            prev = w;
        }
        assert_eq!(prev, if down { 0.0 } else { 1.0 });
    };

    for &sr in &[48_000.0_f32, 44_100.0_f32] {
        let (n, max_step) = ((0.010 * sr).round() as usize, 1.0 / (0.010 * sr) + 1e-5);
        let mut xf = BypassCrossfade::new(false);
        xf.set_sample_rate(sr);
        assert!(xf.is_fully_active() && !xf.is_fully_bypassed());

        xf.set_bypassed(true);
        assert!(!xf.is_fully_active() && !xf.is_fully_bypassed());
        check_ramp(&mut xf, n, max_step, true);
        assert!(xf.is_fully_bypassed());

        xf.set_bypassed(false);
        assert!(!xf.is_fully_bypassed() && !xf.is_fully_active());
        check_ramp(&mut xf, n, max_step, false);
        assert!(xf.is_fully_active());

        // Mid-ramp reversal and immediate reset
        xf.set_bypassed(true);
        let mid = (0..n / 2).fold(1.0, |_, _| xf.next_wet());
        xf.set_bypassed(false);
        let rev = xf.next_wet();
        assert!(rev > mid && (rev - mid) <= max_step);
        xf.reset(true);
        assert!(xf.is_fully_bypassed());
        xf.reset(false);
        assert!(xf.is_fully_active());
    }
}

#[test]
fn test_latency_bypass_delay_exact_alignment_with_linear_phase() {
    let total = LATENCY_SAMPLES + 1024;
    let mut delay = LatencyBypassDelay::new();
    let mut lp = LinearPhaseEngine::new();
    lp.update_from_magnitude_curve(SR, |_| 0.0);

    let (mut lp_l, mut lp_r) = (vec![0.0_f32; total], vec![0.0_f32; total]);
    (lp_l[0], lp_r[0]) = (1.0, -0.75);
    let dl_out: Vec<(f32, f32)> = (0..total)
        .map(|i| delay.process_sample(lp_l[i], lp_r[i]))
        .collect();
    for chunk in (0..total).step_by(128) {
        let end = (chunk + 128).min(total);
        lp.process_block(&mut lp_l[chunk..end], &mut lp_r[chunk..end]);
    }
    for i in 0..total {
        let exp = if i == LATENCY_SAMPLES {
            (1.0, -0.75)
        } else {
            (0.0, 0.0)
        };
        assert_eq!(dl_out[i], exp, "Delay mismatch at sample {i}");
        assert!((lp_l[i] - dl_out[i].0).abs() < 1e-3 && (lp_r[i] - dl_out[i].1).abs() < 1e-3);
    }

    // Phase-locked bypass crossfade on 1 kHz sine wave (zero comb-filtering)
    delay.reset();
    lp.reset();
    let mut xf = BypassCrossfade::new(false);
    xf.set_sample_rate(SR);
    let n = LATENCY_SAMPLES + 2048;
    let dry_in: Vec<f32> = (0..n)
        .map(|i| (2.0 * PI * 1000.0 * i as f32 / SR).sin())
        .collect();
    let (mut sig_l, mut sig_r) = (dry_in.clone(), dry_in.clone());
    lp.process_block(&mut sig_l, &mut sig_r);

    let mut max_err = 0.0_f32;
    for (i, &dry) in dry_in.iter().enumerate() {
        if i == LATENCY_SAMPLES + 256 {
            xf.set_bypassed(true);
        } else if i == LATENCY_SAMPLES + 1024 {
            xf.set_bypassed(false);
        }
        let (dl, _) = delay.process_sample(dry, dry);
        let w = xf.next_wet();
        if i >= LATENCY_SAMPLES + 128 {
            max_err = max_err.max((sig_l[i] * w + dl * (1.0 - w) - dl).abs());
        }
    }
    assert!(max_err < 1e-2, "Phase error during LP bypass: {max_err:.5}");

    delay.reset();
    let _ = delay.process_sample(f32::NAN, f32::INFINITY);
    for _ in 0..LATENCY_SAMPLES {
        assert_eq!(delay.process_sample(0.0, 0.0), (0.0, 0.0));
    }
}

#[test]
fn test_zero_allocations_during_audio_processing() {
    let sanity = measure_thread_allocs(|| {
        black_box(Vec::<u8>::with_capacity(64));
    });
    assert!(sanity >= 1, "CountingAllocator sanity check failed");

    let mut smoother = OnePoleSmoother::new(1.0);
    let mut band = BandParamSmoother::new(1000.0, 0.0, 1.0);
    let mut bypass = BypassCrossfade::new(false);
    let mut delay = LatencyBypassDelay::new();

    let allocs = measure_thread_allocs(|| {
        smoother.set_sample_rate(SR, 15.0);
        smoother.reset(0.5);
        smoother.set_target(2.0);
        for _ in 0..256 {
            black_box((smoother.next(), smoother.next_log()));
        }
        smoother.skip(64);
        smoother.skip_log(64);
        black_box((smoother.current(), smoother.is_settled(1e-4)));

        band.set_sample_rate(SR, 15.0);
        band.reset(200.0, -6.0, 0.707);
        band.set_targets(5000.0, 9.0, 2.5);
        for _ in 0..256 {
            black_box(band.advance_subblock(SMOOTH_SUBBLOCK));
        }
        black_box((band.current(), band.is_settled()));

        bypass.set_sample_rate(SR);
        for &target in &[true, false] {
            bypass.set_bypassed(target);
            for _ in 0..600 {
                black_box(bypass.next_wet());
            }
        }
        bypass.reset(false);
        black_box((bypass.is_fully_bypassed(), bypass.is_fully_active()));

        for i in 0..(LATENCY_SAMPLES + 1024) {
            let x = (i as f32) * 1e-4;
            black_box(delay.process_sample(x, -x));
        }
        delay.reset();
        for _ in 0..512 {
            black_box(delay.process_sample(0.25, -0.25));
        }
    });

    assert_eq!(allocs, 0, "Unexpected heap allocations in audio loop");
}

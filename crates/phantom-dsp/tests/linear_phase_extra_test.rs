use phantom_dsp::{LinearPhaseEngine, StereoCurveDb, LATENCY_SAMPLES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct CountingAllocator;

static TRACKING: AtomicBool = AtomicBool::new(false);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn test_linear_phase_lr_ms_crossfade_and_zero_alloc() {
    let sr = 48000.0;
    let total = LATENCY_SAMPLES + 2048;

    // 1. Verify independent Left/Right gains (+6 dB Left, -6 dB Right, zero crosstalk)
    let mut engine = LinearPhaseEngine::new();
    engine.update_curves(sr, |_| 6.0, |_| -6.0, false);

    let mut left = vec![0.0f32; total];
    let mut right = vec![0.0f32; total];
    left[0] = 1.0;
    right[0] = 1.0;
    engine.process_block(&mut left, &mut right);

    let g_pos6 = 10.0f32.powf(6.0 / 20.0);
    let g_neg6 = 10.0f32.powf(-6.0 / 20.0);
    assert!((left[LATENCY_SAMPLES] - g_pos6).abs() < 1e-3);
    assert!((right[LATENCY_SAMPLES] - g_neg6).abs() < 1e-3);

    // 2. Verify Mid/Side separation (+6 dB Mid, 0 dB Side)
    engine.reset();
    engine.update_curves(sr, |_| 6.0, |_| 0.0, true);

    // Correlated Mid impulse (L=1, R=1) -> boosted by +6 dB
    left.fill(0.0);
    right.fill(0.0);
    left[0] = 1.0;
    right[0] = 1.0;
    engine.process_block(&mut left, &mut right);
    assert!((left[LATENCY_SAMPLES] - g_pos6).abs() < 1e-3);
    assert!((right[LATENCY_SAMPLES] - g_pos6).abs() < 1e-3);

    // Anti-correlated Side impulse (L=1, R=-1) -> untouched (0 dB)
    engine.reset();
    left.fill(0.0);
    right.fill(0.0);
    left[0] = 1.0;
    right[0] = -1.0;
    engine.process_block(&mut left, &mut right);
    assert!((left[LATENCY_SAMPLES] - 1.0).abs() < 1e-3);
    assert!((right[LATENCY_SAMPLES] + 1.0).abs() < 1e-3);

    // 2b. Verify simultaneous L/R + M/S transfer matrix via update_stereo_curves
    //     L = +6 dB (g_l = 1.9953), R = 0 dB (g_r = 1.0), Mid = +6 dB (g_m = 1.9953), Side = 0 dB (g_s = 1.0)
    //     For impulse (L=1, R=0): y_l = 0.5*(g_m+g_s)*g_l, y_r = 0.5*(g_m-g_s)*g_l
    engine.reset();
    engine.update_stereo_curves(sr, |_| StereoCurveDb {
        left: 6.0,
        right: 0.0,
        mid: 6.0,
        side: 0.0,
    });
    left.fill(0.0);
    right.fill(0.0);
    left[0] = 1.0;
    engine.process_block(&mut left, &mut right);
    let expected_yl = 0.5 * (g_pos6 + 1.0) * g_pos6;
    let expected_yr = 0.5 * (g_pos6 - 1.0) * g_pos6;
    assert!((left[LATENCY_SAMPLES] - expected_yl).abs() < 2e-3);
    assert!((right[LATENCY_SAMPLES] - expected_yr).abs() < 2e-3);

    // 3. Verify click-free crossfade on live sine wave during curve & M/S transitions
    engine.reset();
    engine.update_from_magnitude_curve(sr, |_| 0.0);
    let sine_len = 8192;
    let mut s_l: Vec<f32> = (0..sine_len)
        .map(|n| 0.25 * (2.0 * PI * 440.0 * n as f32 / sr).sin())
        .collect();
    let mut s_r = s_l.clone();

    let chunk = 128;
    for blk in 0..(sine_len / chunk) {
        let start = blk * chunk;
        let end = start + chunk;
        if blk == 36 {
            engine.update_curves(sr, |_| 6.0, |_| -3.0, true);
        }
        engine.process_block(&mut s_l[start..end], &mut s_r[start..end]);
    }

    // After startup latency, verify smooth sample-to-sample derivatives (no click spikes)
    let max_expected_step = 2.0 * PI * 440.0 / sr * 0.25 * g_pos6 * 1.5;
    let mut max_step = 0.0f32;
    for i in (LATENCY_SAMPLES + 256)..(sine_len - 1) {
        max_step = max_step
            .max((s_l[i + 1] - s_l[i]).abs())
            .max((s_r[i + 1] - s_r[i]).abs());
    }
    assert!(
        max_step < max_expected_step,
        "Click detected during crossfade: max_step={:.4} > {:.4}",
        max_step,
        max_expected_step
    );

    // 4. Prove ZERO heap allocations during process_block, reset, and all update_* calls!
    let mut blk_l = [0.1f32; 333];
    let mut blk_r = [-0.1f32; 333];

    ALLOC_COUNT.store(0, Ordering::SeqCst);
    TRACKING.store(true, Ordering::SeqCst);

    engine.reset();
    engine.update_from_magnitude_curve(sr, |_| 3.0);
    engine.process_block(&mut blk_l, &mut blk_r);
    engine.update_curves(sr, |_| 4.0, |_| -2.0, false);
    engine.process_block(&mut blk_l, &mut blk_r);
    engine.update_curves(sr, |_| 2.0, |_| -4.0, true);
    engine.process_block(&mut blk_l, &mut blk_r);
    engine.update_stereo_curves(sr, |_| StereoCurveDb {
        left: 2.0,
        right: -1.0,
        mid: 1.5,
        side: -2.0,
    });
    for _ in 0..8 {
        engine.process_block(&mut blk_l, &mut blk_r);
    }

    TRACKING.store(false, Ordering::SeqCst);
    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);
    assert_eq!(allocs, 0, "Expected 0 heap allocations in audio/update path, got {}", allocs);
}

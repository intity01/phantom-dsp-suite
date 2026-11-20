//! Integration tests verifying WAV/AIFF I/O, DTW vocal alignment accuracy, confidence diagnostics,
//! ProtectedRegions & SyncAnchors, YIN pitch + harmony preservation, and 0-allocation real-time DSP.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicUsize, Ordering};

use phantom_vox_dsp::{
    analyze_alignment, analyze_pitch, compute_target_pitch_ratio, create_artificial_double,
    extract_features, generate_demo_vocal_stack, hop_samples_for_sr, read_audio_file,
    render_aligned_track, write_wav_f32, write_wav_i24, AlignConfig, AudioBuffer, ConfidenceLevel,
    DoublerParams, LiveSidechainAligner, PitchMatchMode, ProtectedRegion, SyncAnchor,
    VocalDoublerEngine, LIVE_ALIGN_LATENCY_SAMPLES,
};

struct CountingAlloc;

thread_local! {
    static TRACK_ALLOCS: Cell<bool> = const { Cell::new(false) };
}
static THREAD_ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL_ALLOC: CountingAlloc = CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCS.try_with(|c| c.get()).unwrap_or(false) {
            THREAD_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

fn max_stereo_diff(a: &AudioBuffer, b: &AudioBuffer) -> f32 {
    a.samples_l
        .iter()
        .zip(&b.samples_l)
        .chain(a.samples_r.iter().zip(&b.samples_r))
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max)
}

fn sample_tone(t: f32) -> f32 {
    (0.5 + 0.5 * (TAU * 3.0 * t).sin())
        * (0.5 * (TAU * 260.0 * t).sin() + 0.25 * (TAU * 520.0 * t).sin())
}

#[test]
fn test_1_wav_f32_and_i24_roundtrip_accuracy() {
    let (sr, n) = (48_000u32, 4096usize);
    let l = (0..n)
        .map(|i| 0.85 * (TAU * 440.0 * i as f32 / sr as f32).sin())
        .collect();
    let r = (0..n)
        .map(|i| 0.65 * (TAU * 554.37 * i as f32 / sr as f32).cos())
        .collect();
    let orig = AudioBuffer::new_stereo(sr, l, r);

    let dir = std::env::temp_dir().join("phantom_vox_dsp_test_io");
    std::fs::create_dir_all(&dir).unwrap();
    let (p_f32, p_i24) = (dir.join("rt_f32.wav"), dir.join("rt_i24.wav"));

    write_wav_f32(&p_f32, &orig).unwrap();
    let r_f32 = read_audio_file(&p_f32).unwrap();
    assert_eq!((r_f32.sample_rate, r_f32.channels, r_f32.len()), (sr, 2, n));
    assert!(max_stereo_diff(&orig, &r_f32) < 1e-6);

    write_wav_i24(&p_i24, &orig).unwrap();
    let r_i24 = read_audio_file(&p_i24).unwrap();
    assert_eq!((r_i24.sample_rate, r_i24.channels, r_i24.len()), (sr, 2, n));
    assert!(max_stereo_diff(&orig, &r_i24) < 1e-5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_2_synthetic_vocal_alignment_accuracy() {
    let sr = 44_100u32;
    let (guide, _, _, _) = generate_demo_vocal_stack(sr);
    let true_dub = |tg: f32| tg + 0.045 + 0.018 * (TAU * 0.45 * tg).sin();
    let sr_f = sr as f32;
    let mut dub_s = vec![0.0_f32; guide.len()];
    for (n, slot) in dub_s.iter_mut().enumerate() {
        let td = n as f32 / sr_f;
        let mut tg = td;
        for _ in 0..4 {
            tg = td - (0.045 + 0.018 * (TAU * 0.45 * tg).sin());
        }
        let pos = tg * sr_f;
        let idx = pos.floor() as isize;
        let frac = pos - idx as f32;
        if idx >= 0 && (idx as usize + 1) < guide.len() {
            let a = guide.samples_l[idx as usize];
            *slot = (a + frac * (guide.samples_l[idx as usize + 1] - a)) * 0.85;
        }
    }
    let dub = AudioBuffer::new_mono(sr, dub_s);
    let res = analyze_alignment(&guide, &dub, &[], &[]);
    let g_feat = extract_features(&guide.samples_l, sr);

    let mut errs_ms: Vec<f32> = g_feat
        .frames
        .iter()
        .filter(|f| f.rms_db > -30.0 && f.time_sec > 0.28 && f.time_sec < 3.55)
        .map(|f| (res.time_map.dub_sec_at_guide(f.time_sec) - true_dub(f.time_sec)).abs() * 1000.0)
        .collect();
    assert!(errs_ms.len() > 100);
    errs_ms.sort_by(|a, b| a.total_cmp(b));
    let median = errs_ms[errs_ms.len() / 2];
    let p95 = errs_ms[((errs_ms.len() as f32 * 0.95) as usize).min(errs_ms.len() - 1)];
    println!(
        "[test_2] DTW median={median:.2} ms, p95={p95:.2} ms, conf={:.2}",
        res.overall_confidence
    );
    assert!(median <= 15.0, "median {median:.2} ms > 15 ms");
    assert!(p95 <= 40.0, "p95 {p95:.2} ms > 40 ms");
    assert!(res.phrases.iter().all(|p| p.level == ConfidenceLevel::High));

    let rendered = render_aligned_track(&guide, &dub, &res, &AlignConfig::default(), &[]);
    assert!(rendered.samples_l.iter().all(|s| s.is_finite()) && rendered.peak_db() > -20.0);
}

#[test]
fn test_3_missing_phrase_low_confidence_warning() {
    let sr = 44_100u32;
    let (guide, mut dub_missing, _, _) = generate_demo_vocal_stack(sr);
    let s = (1.35 * sr as f32) as usize;
    let e = (2.60 * sr as f32).min(dub_missing.len() as f32) as usize;
    dub_missing.samples_l[s..e].fill(0.0);

    let res = analyze_alignment(&guide, &dub_missing, &[], &[]);
    println!("[test_3] phrases={:?}", res.phrases);
    let flagged = res.phrases.iter().any(|p| {
        p.end_sec > 1.50
            && p.start_sec < 2.50
            && p.level == ConfidenceLevel::Low
            && p.warning.as_deref().map_or(false, |w| !w.is_empty())
    });
    assert!(
        flagged,
        "missing phrase must report ConfidenceLevel::Low with warning"
    );
}

#[test]
fn test_4_protected_region_exact_equality_and_sync_anchor_pinning() {
    let sr = 44_100u32;
    let (guide, dub, _, _) = generate_demo_vocal_stack(sr);
    let prot = ProtectedRegion {
        start_sec: 0.40,
        end_sec: 0.90,
    };
    let anchor = SyncAnchor {
        guide_sec: 2.00,
        dub_sec: 2.12,
    };

    let res = analyze_alignment(&guide, &dub, &[anchor], &[prot]);
    let anchor_err_ms = (res.time_map.dub_sec_at_guide(2.00) - 2.12).abs() * 1000.0;
    assert!(anchor_err_ms < 10.0, "anchor err {anchor_err_ms:.2} ms");
    assert!((res.time_map.dub_sec_at_guide(0.65) - 0.65).abs() < 1e-4);

    let rendered = render_aligned_track(&guide, &dub, &res, &AlignConfig::default(), &[prot]);
    let s = (0.40 * sr as f32).round() as usize;
    let e = (0.90 * sr as f32).round() as usize;
    let max_diff = rendered.samples_l[s..e]
        .iter()
        .zip(&dub.samples_l[s..e])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    println!("[test_4] anchor_err={anchor_err_ms:.3} ms, protected max_diff={max_diff}");
    assert_eq!(max_diff, 0.0);
}

#[test]
fn test_5_yin_pitch_accuracy_and_harmony_preservation() {
    let (sr, true_hz) = (48_000u32, 220.0_f32);
    let tone: Vec<f32> = (0..sr as usize)
        .map(|i| {
            let p = TAU * true_hz * i as f32 / sr as f32;
            0.6 * p.sin() + 0.25 * (2.0 * p).sin() + 0.15 * (3.0 * p).sin()
        })
        .collect();
    let pitch = analyze_pitch(&tone, sr, hop_samples_for_sr(sr));
    let mut hz: Vec<f32> = pitch[5..pitch.len() - 5]
        .iter()
        .filter(|p| p.voiced)
        .map(|p| p.f0_hz)
        .collect();
    hz.sort_by(|a, b| a.total_cmp(b));
    let cents_err = (1200.0 * (hz[hz.len() / 2] / true_hz).log2()).abs();
    assert!(cents_err < 5.0, "YIN error {cents_err:.2} cents >= 5 cents");

    let g_hz = 261.6256_f32;
    let h_hz = g_hz * 2.0_f32.powf(4.0 / 12.0);
    assert!(
        (compute_target_pitch_ratio(g_hz, h_hz, PitchMatchMode::MatchUnisonOnly, 120.0) - 1.0)
            .abs()
            < 1e-6
    );

    let (guide, _, _, harmony) = generate_demo_vocal_stack(44_100);
    let align = analyze_alignment(&guide, &harmony, &[], &[]);
    let cfg = AlignConfig {
        pitch_mode: PitchMatchMode::MatchUnisonOnly,
        pitch_strength: 1.0,
        ..Default::default()
    };
    let rendered = render_aligned_track(&guide, &harmony, &align, &cfg, &[]);
    let hop = hop_samples_for_sr(44_100);
    let mut intervals: Vec<f32> = analyze_pitch(&guide.samples_l, 44_100, hop)
        .into_iter()
        .zip(analyze_pitch(&rendered.samples_l, 44_100, hop))
        .filter(|(g, r)| g.voiced && r.voiced && g.f0_hz > 100.0 && r.f0_hz > 100.0)
        .map(|(g, r)| 12.0 * (r.f0_hz / g.f0_hz).log2())
        .collect();
    intervals.sort_by(|a, b| a.total_cmp(b));
    let med_st = intervals[intervals.len() / 2];
    println!("[test_5] YIN err={cents_err:.3} ct, Harmony_3rd interval={med_st:+.2} st");
    assert!((med_st - 4.0).abs() < 0.45);
    assert_eq!(
        create_artificial_double(&guide, 18.0, -12.0, 1.0, 8.0).len(),
        guide.len()
    );
}

#[test]
fn test_6_realtime_zero_alloc_finite_and_mono_compatible() {
    let mut doubler = VocalDoublerEngine::new(48_000.0);
    let mut aligner = LiveSidechainAligner::new(48_000.0);
    let mut aligner_mono = LiveSidechainAligner::new(48_000.0);
    assert_eq!(aligner.latency_samples(), LIVE_ALIGN_LATENCY_SAMPLES);

    let (block, total_blocks) = (256usize, 80usize);
    let (mut in_l, mut in_r) = (vec![0.0_f32; block], vec![0.0_f32; block]);
    let (mut gl, mut gr) = (vec![0.0_f32; block], vec![0.0_f32; block]);
    let (mut out_l, mut out_r) = (vec![0.0_f32; block], vec![0.0_f32; block]);
    let mut params = DoublerParams::default();
    let mut all_finite = true;

    // Sanity-check that our counting allocator traps deliberate heap allocations.
    THREAD_ALLOC_COUNT.store(0, Ordering::SeqCst);
    TRACK_ALLOCS.with(|c| c.set(true));
    let probe = Box::new([1u8; 64]);
    let probe_n = THREAD_ALLOC_COUNT.load(Ordering::SeqCst);
    TRACK_ALLOCS.with(|c| c.set(false));
    drop(probe);
    assert!(probe_n >= 1);

    THREAD_ALLOC_COUNT.store(0, Ordering::SeqCst);
    TRACK_ALLOCS.with(|c| c.set(true));
    doubler.set_sample_rate(44_100.0);
    doubler.reset();
    aligner.set_sample_rate(44_100.0);
    aligner.reset();
    aligner_mono.set_sample_rate(44_100.0);
    aligner_mono.reset();

    for b in 0..total_blocks {
        for i in 0..block {
            let t = (b * block + i) as f32 / 44_100.0;
            let (v, vd) = (sample_tone(t), sample_tone(t - 0.015));
            (gl[i], gr[i], in_l[i], in_r[i]) = (v, v, vd, vd);
        }
        params.voices_4 = b >= total_blocks / 2;
        doubler.process_stereo(&in_l, &in_r, &mut out_l, &mut out_r, &params);
        doubler.process_mono_to_stereo(&in_l, &mut out_l, &mut out_r, &params);
        aligner.process_stereo(&in_l, &in_r, &gl, &gr, &mut out_l, &mut out_r, 0.8, 0.3);
        aligner_mono.process_mono(&in_l, &gl, &mut out_l, 0.8, 0.3);
        all_finite &= out_l.iter().chain(&out_r).all(|s| s.is_finite());
    }
    let corr_stereo = doubler.correlation();
    params.mono_check = true;
    doubler.process_stereo(&in_l, &in_r, &mut out_l, &mut out_r, &params);
    let allocs = THREAD_ALLOC_COUNT.load(Ordering::SeqCst);
    TRACK_ALLOCS.with(|c| c.set(false));

    println!(
        "[test_6] allocs={allocs}, corr={corr_stereo:.3}, off={:.2}ms, conf={:.2}",
        aligner.offset_ms(),
        aligner.confidence()
    );
    assert_eq!(allocs, 0);
    assert!(all_finite && corr_stereo > 0.0);
    assert!((aligner.offset_ms() - 15.0).abs() < 6.0 && aligner.confidence() > 0.5);
}

//! Focus band (filters on the doubled voices only), loudness-matched "Hold: Original",
//! and A/B settings slots.

use phantom_vox::ab_slots::AbSlots;
use phantom_vox::original_audition::OriginalAudition;
use phantom_vox_dsp::{DoublerParams, FocusBand, VocalDoublerEngine, FOCUS_MAX_HZ, FOCUS_MIN_HZ};
use std::f32::consts::TAU;

const SR: f32 = 48_000.0;

fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (TAU * hz * i as f32 / SR).sin())
        .collect()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

fn db(ratio: f32) -> f32 {
    20.0 * ratio.max(1e-9).log10()
}

/// RMS of a sine after the focus band, measured after the filters settle.
fn through_band(lo: f32, hi: f32, hz: f32) -> f32 {
    let mut band = FocusBand::new();
    band.sync(lo, hi, SR);
    let out: Vec<f32> = sine(hz, 0.5, 24_000)
        .into_iter()
        .map(|s| band.tick(s, s).0)
        .collect();
    rms(&out[12_000..])
}

#[test]
fn full_range_leaves_the_sound_untouched() {
    let mut band = FocusBand::new();
    band.sync(FOCUS_MIN_HZ, FOCUS_MAX_HZ, SR);
    assert!(band.is_bypassed());
    for s in sine(440.0, 0.7, 512) {
        assert_eq!(band.tick(s, -s), (s, -s));
    }
}

#[test]
fn focus_band_cuts_outside_and_keeps_inside() {
    let pass = rms(&sine(1000.0, 0.5, 12_000));
    let inside = through_band(200.0, 8000.0, 1000.0);
    assert!(db(inside / pass).abs() < 0.5, "1 kHz inside the band moved");
    let low = through_band(1000.0, FOCUS_MAX_HZ, 60.0);
    assert!(
        db(low / pass) < -30.0,
        "60 Hz under a 1 kHz high-pass: {}",
        db(low / pass)
    );
    let high = through_band(FOCUS_MIN_HZ, 1000.0, 12_000.0);
    assert!(
        db(high / pass) < -30.0,
        "12 kHz over a 1 kHz low-pass: {}",
        db(high / pass)
    );
}

/// The doubled part of the doubler's output (output minus the dry share).
fn doubled_rms(focus_low_hz: f32, input_hz: f32) -> f32 {
    let p = DoublerParams {
        mix: 1.0,
        sibilance_safe: false,
        focus_low_hz,
        ..DoublerParams::default()
    };
    let dry_gain = 1.0 - 0.55 * p.mix;
    let input = sine(input_hz, 0.4, 48_000);
    let (mut l, mut r) = (vec![0.0; input.len()], vec![0.0; input.len()]);
    let mut engine = VocalDoublerEngine::new(SR);
    engine.process_mono_to_stereo(&input, &mut l, &mut r, &p);
    let doubled: Vec<f32> = (24_000..input.len())
        .map(|i| l[i] - input[i] * dry_gain)
        .collect();
    rms(&doubled)
}

#[test]
fn focus_band_filters_only_the_doubled_voices() {
    let open = doubled_rms(FOCUS_MIN_HZ, 120.0);
    let focused = doubled_rms(1500.0, 120.0);
    assert!(open > 1e-3, "doubled voices should be audible");
    assert!(
        db(focused / open) < -24.0,
        "lows left in the doubled voices: {}",
        db(focused / open)
    );
}

#[test]
fn hold_original_matches_the_processed_loudness() {
    let mut a = OriginalAudition::new();
    a.set_sample_rate(SR);
    let original = sine(300.0, 0.1, 96_000);
    let processed = sine(300.0, 0.4, 96_000);
    a.set_active(false);
    for i in 0..48_000 {
        let out = a.tick((original[i], original[i]), (processed[i], processed[i]));
        assert_eq!(
            out.0, processed[i],
            "released: the processed sound plays unchanged"
        );
    }
    a.set_active(true);
    let out: Vec<f32> = (48_000..96_000)
        .map(|i| {
            a.tick((original[i], original[i]), (processed[i], processed[i]))
                .0
        })
        .collect();
    let diff = db(rms(&out[24_000..]) / rms(&processed[72_000..]));
    assert!(
        diff.abs() < 0.5,
        "original should play at the processed loudness, off by {diff} dB"
    );
    assert!((a.match_gain() - 4.0).abs() < 0.1);
}

#[test]
fn ab_slots_keep_each_side_and_copy() {
    let snap = |v: f32| vec![("mix_pct".to_string(), v)];
    let mut ab = AbSlots::default();
    assert_eq!(ab.switch_to(false, snap(0.1)), None, "already on A");

    let loaded = ab.switch_to(true, snap(0.1)).expect("switch to B");
    assert_eq!(loaded, snap(0.1), "an empty B starts as a copy of A");

    let back = ab.switch_to(false, snap(0.9)).expect("switch to A");
    assert_eq!(back, snap(0.1), "A kept its own settings");
    assert_eq!(ab.b, snap(0.9), "B kept the edit made while on B");

    ab.copy_to_other(snap(0.3));
    assert_eq!((ab.a.clone(), ab.b.clone()), (snap(0.3), snap(0.3)));
}

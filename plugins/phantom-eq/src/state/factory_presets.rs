//! 12 Built-In Mastering, Mixing & Creative Factory Presets for Phantom EQ

use super::snapshot::{BandSnapshotData, EqPresetSnapshot};
use crate::params::{
    ChannelPlacementParam as Ch, CharacterModeParam, FilterSlopeParam as Sl, FilterTypeParam as Ft,
    ProcessingModeParam as Pm,
};

const fn b(ft: Ft, sl: Sl, ch: Ch, freq: f32, gain: f32, q: f32) -> BandSnapshotData {
    BandSnapshotData::active(ft, sl, ch, freq, gain, q)
}

const fn bd(ft: Ft, sl: Sl, ch: Ch, freq: f32, gain: f32, q: f32, dyn_r: f32) -> BandSnapshotData {
    BandSnapshotData::active(ft, sl, ch, freq, gain, q).with_dynamics(dyn_r, -18.0)
}

const B2: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db24, Ch::Side, 30.0, 0.0, 0.71),
    b(Ft::LowShelf, Sl::Db12, Ch::Mid, 38.0, 1.2, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 260.0, -0.8, 1.8),
    b(Ft::HighShelf, Sl::Db12, Ch::Side, 12000.0, 1.8, 0.71),
];
const B3: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db18, Ch::Stereo, 18.0, 0.0, 0.71),
    b(Ft::FlatTilt, Sl::Db12, Ch::Stereo, 650.0, 1.2, 0.71),
    b(Ft::HighShelf, Sl::Db12, Ch::Stereo, 16000.0, 1.0, 0.71),
];
const B4: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db18, Ch::Stereo, 85.0, 0.0, 0.71),
    bd(Ft::Bell, Sl::Db12, Ch::Stereo, 270.0, -2.5, 1.4, -3.0),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 3200.0, 2.0, 1.2),
    b(Ft::HighShelf, Sl::Db12, Ch::Stereo, 11000.0, 2.5, 0.71),
];
const B5: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db12, Ch::Stereo, 100.0, 0.0, 0.71),
    bd(Ft::Bell, Sl::Db12, Ch::Stereo, 2800.0, 0.0, 2.5, -4.5),
    bd(Ft::Bell, Sl::Db12, Ch::Stereo, 6800.0, 0.0, 3.0, -5.0),
];
const B6: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db24, Ch::Stereo, 28.0, 0.0, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 58.0, 2.5, 1.4),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 340.0, -2.8, 1.6),
    b(Ft::HighShelf, Sl::Db12, Ch::Stereo, 7500.0, 2.2, 0.71),
];
const B7: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db36, Ch::Stereo, 26.0, 0.0, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 52.0, 3.0, 1.5),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 220.0, -4.0, 2.0),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 3500.0, 2.0, 1.4),
];
const B8: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db24, Ch::Stereo, 32.0, 0.0, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 75.0, 2.2, 1.3),
    bd(Ft::Bell, Sl::Db12, Ch::Stereo, 280.0, -2.0, 1.5, -3.0),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 1400.0, 2.8, 1.4),
];
const B9: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db18, Ch::Stereo, 75.0, 0.0, 0.71),
    bd(Ft::Bell, Sl::Db12, Ch::Stereo, 180.0, -2.0, 1.6, -3.5),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 4500.0, 1.8, 1.2),
    b(Ft::HighShelf, Sl::Db12, Ch::Stereo, 12000.0, 2.5, 0.71),
];
const B10: &[BandSnapshotData] = &[
    b(Ft::Bell, Sl::Db12, Ch::Mid, 320.0, -2.2, 1.4),
    b(Ft::LowCut, Sl::Db18, Ch::Side, 140.0, 0.0, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Side, 2400.0, 2.0, 1.1),
    b(Ft::HighShelf, Sl::Db12, Ch::Side, 9000.0, 3.0, 0.71),
];
const B11: &[BandSnapshotData] = &[b(Ft::LowCut, Sl::Brickwall, Ch::Stereo, 30.0, 0.0, 0.71)];
const B12: &[BandSnapshotData] = &[
    b(Ft::LowCut, Sl::Db24, Ch::Stereo, 380.0, 0.0, 0.71),
    b(Ft::Bell, Sl::Db12, Ch::Stereo, 1200.0, 4.0, 1.6),
    b(Ft::HighCut, Sl::Db24, Ch::Stereo, 3400.0, 0.0, 0.71),
];

/// Returns the 12 built-in Mastering, Mixing, and Creative Factory Presets.
pub fn factory_presets() -> Vec<EqPresetSnapshot> {
    use CharacterModeParam::{Clean as Cl, SubtleAnalog as Sa, WarmTube as Wt};
    use Pm::{LinearPhase as Lp, NaturalPhase as Np, ZeroLatency as Zl};
    let p = EqPresetSnapshot::with_bands;
    vec![
        p("Default (Flat Empty)", "Init", Zl, Cl, &[]),
        p("Mastering — Air & Sub Tightener", "Mastering", Lp, Sa, B2),
        p("Mastering — Warm Analog Tilt", "Mastering", Np, Wt, B3),
        p("Vocal — Lead Presence & De-Mud", "Vocals", Np, Cl, B4),
        p("Vocal — Silky De-Harsh (Dynamic)", "Vocals", Zl, Cl, B5),
        p("Drum Bus — Punch & Snap", "Drums", Zl, Sa, B6),
        p("Kick & Sub — Tight Low End", "Drums", Zl, Cl, B7),
        p("Bass Guitar — Sub Lock & Growl", "Bass", Zl, Wt, B8),
        p("Acoustic Guitar — Sparkle & Body", "Guitars", Zl, Cl, B9),
        p("Synth Pad — M/S Wide Stereo Halo", "Synths", Zl, Cl, B10),
        p("Sub-Rumble Brickwall Cut (30 Hz)", "Utility", Zl, Cl, B11),
        p("Lo-Fi Telephone Filter", "Creative", Zl, Wt, B12),
    ]
}

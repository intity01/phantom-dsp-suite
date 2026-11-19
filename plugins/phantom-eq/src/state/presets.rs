//! 12 Built-In Mastering & Mixing Factory Presets + Offline `.peq` User Preset Disk I/O
//! Pure Rust stdlib implementation (`%APPDATA%\PhantomDSP\PhantomEQ\Presets` / `~/.config/phantom-dsp/phantom-eq/presets`).

use super::snapshot::{BandSnapshotData, CharacterParam, EqPresetSnapshot, OversampleParam};
use crate::params::{
    ChannelPlacementParam as Ch, FilterSlopeParam as Sl, FilterTypeParam as Ft,
    ProcessingModeParam as Pm, NUM_BANDS,
};
use std::path::PathBuf;
use std::{fs, io};

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
    use crate::params::CharacterModeParam::{Clean as Cl, SubtleAnalog as Sa, WarmTube as Wt};
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

/// Returns the platform-appropriate offline user preset directory.
pub fn user_presets_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("PhantomDSP/PhantomEQ/Presets");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".config/phantom-dsp/phantom-eq/presets");
        }
    }
    std::env::temp_dir().join("phantom-eq-presets")
}

/// Saves a preset snapshot to disk in line-oriented `key=value` `.peq` format.
pub fn save_user_preset(snapshot: &EqPresetSnapshot) -> io::Result<PathBuf> {
    let dir = user_presets_dir();
    fs::create_dir_all(&dir)?;
    let safe: String = snapshot
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = safe.trim();
    let file = if stem.is_empty() { "preset" } else { stem };
    let path = dir.join(format!("{file}.peq"));
    fs::write(&path, serialize_peq_preset(snapshot))?;
    Ok(path)
}

/// Loads all saved `.peq` user presets from disk, sorted alphabetically by preset name.
pub fn load_user_presets() -> Vec<EqPresetSnapshot> {
    let Ok(entries) = fs::read_dir(user_presets_dir()) else {
        return Vec::new();
    };
    let mut list: Vec<EqPresetSnapshot> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("peq"))
        .filter_map(|p| fs::read_to_string(p).ok())
        .filter_map(|txt| parse_peq_preset(&txt))
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Serializes an `EqPresetSnapshot` into line-oriented `key=value` `.peq` text.
pub fn serialize_peq_preset(s: &EqPresetSnapshot) -> String {
    let mode_id = match s.mode {
        Pm::ZeroLatency => 0,
        Pm::NaturalPhase => 1,
        Pm::LinearPhase => 2,
    };
    let os_id = match s.oversampling {
        OversampleParam::Off => 0,
        OversampleParam::X2 => 1,
        OversampleParam::X4 => 2,
    };
    let mut out = format!(
        "format=PhantomEQ.v1\nname={}\nmode={}\noversampling={}\ncharacter={}\nauto_gain={}\noutput_gain_db={:.4}\nphase_invert={}\n",
        s.name.replace('\n', " "),
        mode_id,
        os_id,
        s.character.to_idx(),
        u8::from(s.auto_gain),
        s.output_gain_db,
        u8::from(s.phase_invert),
    );
    for (i, b) in s.bands.iter().enumerate().filter(|(_, b)| b.used) {
        out.push_str(&format!(
            "band.{i}={},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{},{}\n",
            u8::from(b.used),
            u8::from(b.enabled),
            b.filter_type.to_idx(),
            b.slope.to_idx(),
            b.placement.to_ui_idx(),
            b.frequency,
            b.gain,
            b.q,
            b.dynamic_range,
            b.dynamic_threshold,
            b.dynamic_attack,
            b.dynamic_release,
            u8::from(b.dynamic_auto_thresh),
            u8::from(b.dynamic_ext_sc),
            u8::from(b.spectral_enabled),
        ));
    }
    out
}

/// Parses a line-oriented `key=value` `.peq` preset file into an `EqPresetSnapshot`.
pub fn parse_peq_preset(content: &str) -> Option<EqPresetSnapshot> {
    let mut snap = EqPresetSnapshot::empty("User Preset", "User");
    let mut saw_key = false;
    for (key, val) in content.lines().filter_map(|l| l.trim().split_once('=')) {
        saw_key = true;
        let v = val.trim();
        match key.trim() {
            "name" => snap.name = v.to_string(),
            "mode" => {
                snap.mode = match v.parse::<u8>().unwrap_or(0) {
                    1 => Pm::NaturalPhase,
                    2 => Pm::LinearPhase,
                    _ => Pm::ZeroLatency,
                }
            }
            "oversampling" => {
                snap.oversampling = match v.parse::<u8>().unwrap_or(1) {
                    0 => OversampleParam::Off,
                    2 => OversampleParam::X4,
                    _ => OversampleParam::X2,
                }
            }
            "character" => snap.character = CharacterParam::from_idx(v.parse().unwrap_or(0)),
            "auto_gain" => snap.auto_gain = matches!(v, "1" | "true"),
            "output_gain_db" => snap.output_gain_db = v.parse().unwrap_or(0.0),
            "phase_invert" => snap.phase_invert = matches!(v, "1" | "true"),
            k if k.starts_with("band.") => {
                if let Ok(idx) = k[5..].parse::<usize>() {
                    if idx < NUM_BANDS {
                        if let Some(band) = parse_band_csv(v) {
                            snap.bands[idx] = band;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    saw_key.then_some(snap)
}

fn parse_band_csv(csv: &str) -> Option<BandSnapshotData> {
    let p: Vec<&str> = csv.split(',').collect();
    if p.len() < 14 {
        return None;
    }
    Some(BandSnapshotData {
        used: matches!(p[0], "1" | "true"),
        enabled: matches!(p[1], "1" | "true"),
        filter_type: Ft::from_idx(p[2].parse().ok()?),
        slope: Sl::from_idx(p[3].parse().ok()?),
        placement: Ch::from_ui_idx(p[4].parse().ok()?),
        frequency: p[5].parse().ok()?,
        gain: p[6].parse().ok()?,
        q: p[7].parse().ok()?,
        dynamic_range: p[8].parse().ok()?,
        dynamic_threshold: p[9].parse().ok()?,
        dynamic_attack: p[10].parse().ok()?,
        dynamic_release: p[11].parse().ok()?,
        dynamic_auto_thresh: matches!(p[12], "1" | "true"),
        dynamic_ext_sc: matches!(p[13], "1" | "true"),
        spectral_enabled: p.get(14).is_some_and(|&v| matches!(v, "1" | "true")),
    })
}

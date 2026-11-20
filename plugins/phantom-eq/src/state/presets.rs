//! Offline `.peq` User Preset Disk I/O, Filename Sanitization & Deletion
//! Pure Rust stdlib implementation (`%APPDATA%\PhantomDSP\PhantomEQ\Presets` / `~/.config/phantom-dsp/phantom-eq/presets`).

pub use super::factory_presets::factory_presets;
use super::snapshot::{BandSnapshotData, CharacterParam, EqPresetSnapshot, OversampleParam};
use crate::params::{
    ChannelPlacementParam as Ch, FilterSlopeParam as Sl, FilterTypeParam as Ft,
    ProcessingModeParam as Pm, NUM_BANDS,
};
use std::path::{Path, PathBuf};
use std::{fs, io};

const MAX_PRESET_STEM_LEN: usize = 80;

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

/// Sanitizes a custom user preset name into a safe cross-platform filename stem (no traversal).
pub fn sanitize_preset_stem(name: &str) -> String {
    let mut out = String::with_capacity(name.len().min(MAX_PRESET_STEM_LEN));
    let mut prev_sep = false;
    for ch in name.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ' ') {
            ch
        } else {
            '_'
        };
        if mapped == '_' && prev_sep {
            continue;
        }
        prev_sep = mapped == '_';
        if out.len() >= MAX_PRESET_STEM_LEN {
            break;
        }
        out.push(mapped);
    }
    let trimmed = out.trim_matches(|c| c == ' ' || c == '.' || c == '_');
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("con")
        || trimmed.eq_ignore_ascii_case("nul")
    {
        String::from("preset")
    } else {
        trimmed.to_string()
    }
}

/// Saves a preset snapshot to disk in line-oriented `key=value` `.peq` format.
pub fn save_user_preset(snapshot: &EqPresetSnapshot) -> io::Result<PathBuf> {
    save_user_preset_in(&user_presets_dir(), snapshot)
}

/// Saves a preset snapshot into a specific directory (helper for tests & custom paths).
pub fn save_user_preset_in(dir: &Path, snapshot: &EqPresetSnapshot) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let stem = sanitize_preset_stem(&snapshot.name);
    let path = dir.join(format!("{stem}.peq"));
    fs::write(&path, serialize_peq_preset(snapshot))?;
    Ok(path)
}

/// Deletes a saved `.peq` user preset by name from the user presets directory.
pub fn delete_user_preset(name: &str) -> io::Result<()> {
    delete_user_preset_in(&user_presets_dir(), name)
}

/// Deletes a saved `.peq` user preset by name from a specific directory.
pub fn delete_user_preset_in(dir: &Path, name: &str) -> io::Result<()> {
    let direct = dir.join(format!("{}.peq", sanitize_preset_stem(name)));
    if direct.is_file() {
        return fs::remove_file(direct);
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for path in entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("peq"))
        {
            if let Ok(txt) = fs::read_to_string(&path) {
                if parse_peq_preset(&txt).is_some_and(|p| p.name == name) {
                    return fs::remove_file(path);
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("user preset not found: {name}"),
    ))
}

/// Loads all saved `.peq` user presets from disk, sorted alphabetically by preset name.
pub fn load_user_presets() -> Vec<EqPresetSnapshot> {
    load_user_presets_in(&user_presets_dir())
}

/// Loads all `.peq` presets from a specific directory, sorted alphabetically by preset name.
pub fn load_user_presets_in(dir: &Path) -> Vec<EqPresetSnapshot> {
    let Ok(entries) = fs::read_dir(dir) else {
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
        "format=PhantomEQ.v1\nname={}\nmode={}\noversampling={}\ncharacter={}\nauto_gain={}\noutput_gain_db={:.4}\ngain_scale_pct={:.2}\nphase_invert={}\n",
        s.name.replace('\n', " "),
        mode_id,
        os_id,
        s.character.to_idx(),
        u8::from(s.auto_gain),
        s.output_gain_db,
        s.gain_scale_pct,
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
                snap.oversampling = match v.parse::<u8>().unwrap_or(0) {
                    1 => OversampleParam::X2,
                    2 => OversampleParam::X4,
                    _ => OversampleParam::Off,
                }
            }
            "character" => snap.character = CharacterParam::from_idx(v.parse().unwrap_or(0)),
            "auto_gain" => snap.auto_gain = matches!(v, "1" | "true"),
            "output_gain_db" => snap.output_gain_db = v.parse().unwrap_or(0.0),
            "gain_scale_pct" => {
                snap.gain_scale_pct = v.parse::<f32>().unwrap_or(100.0).clamp(0.0, 200.0)
            }
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

//! Plugin suite catalog, bundle layout metadata, artwork tokens, and GitHub release URL helpers.

use crate::paths::host_dist_os_dir;
use std::path::{Path, PathBuf};

pub const DEFAULT_GITHUB_REPO: &str = "intity01/phantom-dsp-suite";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStatus {
    Ready,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtPattern {
    Rings,
    Conic,
    Grid,
    Wave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseGroup {
    Equalizer,
    Vocal,
    Dynamics,
    Mastering,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub category: &'static str,
    pub kind: &'static str,
    pub browse_group: BrowseGroup,
    pub author: &'static str,
    pub license: &'static str,
    pub released: &'static str,
    pub release_notes: &'static str,
    pub description: &'static str,
    pub status: PluginStatus,
    pub vst3_bundle: &'static str,
    pub vst3_win_bin: &'static str,
    pub vst3_linux_bin: &'static str,
    pub vst3_macos_bin: &'static str,
    pub clap_file: &'static str,
    pub hue: f32,
    pub pattern: ArtPattern,
    pub formats: &'static [&'static str],
}

pub const AVAILABLE_PLUGINS: &[PluginManifest] = &[
    PluginManifest {
        id: "phantom-eq",
        name: "Phantom EQ",
        version: "0.1.0",
        category: "Equalizer",
        kind: "24-Band Dynamic EQ",
        browse_group: BrowseGroup::Equalizer,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "2026-11-19",
        release_notes: "24-band Dynamic & Spectral EQ, Natural & Linear Phase FIR, EQ Match, Floating HUD, Piano Roll & EBU R128 LUFS metering.",
        description: "24-band dynamic & spectral parametric equalizer with Linear Phase FIR, M/S routing & anti-cramping oversampling",
        status: PluginStatus::Ready,
        vst3_bundle: "Phantom EQ.vst3",
        vst3_win_bin: "Phantom EQ.vst3",
        vst3_linux_bin: "Phantom EQ.so",
        vst3_macos_bin: "Phantom EQ",
        clap_file: "Phantom EQ.clap",
        hue: 205.0,
        pattern: ArtPattern::Rings,
        formats: &["VST3", "CLAP"],
    },
    PluginManifest {
        id: "phantom-vox",
        name: "Phantom Vox",
        version: "0.1.0",
        category: "Vocal",
        kind: "Vocal Doubler & Aligner",
        browse_group: BrowseGroup::Vocal,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "2026-11-20",
        release_notes: "Up to 4-voice doubler, live sidechain alignment, Align & Double mode, 2D voice field and phase meter.",
        description: "Realtime vocal doubler with live sidechain alignment to a lead vocal",
        status: PluginStatus::Ready,
        vst3_bundle: "Phantom Vox.vst3",
        vst3_win_bin: "Phantom Vox.vst3",
        vst3_linux_bin: "Phantom Vox.so",
        vst3_macos_bin: "Phantom Vox",
        clap_file: "Phantom Vox.clap",
        hue: 280.0,
        pattern: ArtPattern::Wave,
        formats: &["VST3", "CLAP"],
    },
    PluginManifest {
        id: "phantom-comp",
        name: "Phantom Comp",
        version: "0.1.0-planned",
        category: "Dynamics",
        kind: "Mastering Compressor",
        browse_group: BrowseGroup::Dynamics,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "Planned",
        release_notes: "Visual transfer curve, lookahead detection & sidechain filter (on the roadmap).",
        description: "Mastering & bus compressor with visual transfer curve & sidechain filtering",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Comp.vst3",
        vst3_win_bin: "Phantom Comp.vst3",
        vst3_linux_bin: "Phantom Comp.so",
        vst3_macos_bin: "Phantom Comp",
        clap_file: "Phantom Comp.clap",
        hue: 155.0,
        pattern: ArtPattern::Conic,
        formats: &["VST3", "CLAP"],
    },
    PluginManifest {
        id: "phantom-limiter",
        name: "Phantom Limiter",
        version: "0.1.0-planned",
        category: "Mastering",
        kind: "True Peak Limiter",
        browse_group: BrowseGroup::Mastering,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "Planned",
        release_notes: "Brickwall ISP limiting with ITU-R BS.1770-4 LUFS metering (on the roadmap).",
        description: "True Peak mastering limiter with EBU R128 loudness metering",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Limiter.vst3",
        vst3_win_bin: "Phantom Limiter.vst3",
        vst3_linux_bin: "Phantom Limiter.so",
        vst3_macos_bin: "Phantom Limiter",
        clap_file: "Phantom Limiter.clap",
        hue: 35.0,
        pattern: ArtPattern::Wave,
        formats: &["VST3", "CLAP"],
    },
    PluginManifest {
        id: "phantom-mb",
        name: "Phantom MB",
        version: "0.1.0-planned",
        category: "Multiband",
        kind: "Multiband Dynamics",
        browse_group: BrowseGroup::Dynamics,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "Planned",
        release_notes: "Linear-phase multiband crossover with per-band solo audition (on the roadmap).",
        description: "Linear-phase multiband dynamic processor with per-band soloing",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom MB.vst3",
        vst3_win_bin: "Phantom MB.vst3",
        vst3_linux_bin: "Phantom MB.so",
        vst3_macos_bin: "Phantom MB",
        clap_file: "Phantom MB.clap",
        hue: 265.0,
        pattern: ArtPattern::Grid,
        formats: &["VST3", "CLAP"],
    },
    PluginManifest {
        id: "phantom-saturn",
        name: "Phantom Saturn",
        version: "0.1.0-planned",
        category: "Saturation",
        kind: "Harmonic Saturation",
        browse_group: BrowseGroup::Mastering,
        author: "Intity01 · Phantom DSP",
        license: "MIT / Apache-2.0",
        released: "Planned",
        release_notes: "Multiband tape, tube & transformer warmth engine (on the roadmap).",
        description: "Multiband warmth, tape & tube harmonic saturation engine",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Saturn.vst3",
        vst3_win_bin: "Phantom Saturn.vst3",
        vst3_linux_bin: "Phantom Saturn.so",
        vst3_macos_bin: "Phantom Saturn",
        clap_file: "Phantom Saturn.clap",
        hue: 10.0,
        pattern: ArtPattern::Conic,
        formats: &["VST3", "CLAP"],
    },
];

pub fn find_plugin(id: &str) -> Option<&'static PluginManifest> {
    AVAILABLE_PLUGINS
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(id) || p.name.eq_ignore_ascii_case(id))
}

/// Relative binary path inside a `.vst3` bundle for a given OS key (`windows`, `linux`, `macos`).
pub fn vst3_inner_binary_rel_for(plugin: &PluginManifest, os_key: &str) -> PathBuf {
    match os_key {
        "windows" => PathBuf::from("Contents")
            .join("x86_64-win")
            .join(plugin.vst3_win_bin),
        "macos" => PathBuf::from("Contents")
            .join("MacOS")
            .join(plugin.vst3_macos_bin),
        _ => PathBuf::from("Contents")
            .join("x86_64-linux")
            .join(plugin.vst3_linux_bin),
    }
}

/// Relative binary path inside a `.vst3` bundle for the current host OS.
pub fn vst3_inner_binary_rel(plugin: &PluginManifest) -> PathBuf {
    vst3_inner_binary_rel_for(plugin, host_dist_os_dir())
}

/// Resolves the hashable binary file inside a `.vst3` bundle (supports single-file fallback).
pub fn resolve_vst3_binary_in_bundle(
    plugin: &PluginManifest,
    bundle_path: &Path,
) -> Option<PathBuf> {
    if bundle_path.is_file() {
        return Some(bundle_path.to_path_buf());
    }
    let nested = bundle_path.join(vst3_inner_binary_rel(plugin));
    nested.is_file().then_some(nested)
}

pub fn github_releases_url(repo: &str) -> String {
    let trimmed = repo.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return format!("https://github.com/{DEFAULT_GITHUB_REPO}/releases");
    }
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        if trimmed.ends_with("/releases") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/releases")
        }
    } else {
        format!("https://github.com/{trimmed}/releases")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_ready_phantom_eq_and_planned_suite() {
        assert_eq!(AVAILABLE_PLUGINS.len(), 6);
        let eq = find_plugin("phantom-eq").expect("phantom-eq must exist");
        assert_eq!(eq.status, PluginStatus::Ready);
        let vox = find_plugin("phantom-vox").expect("phantom-vox must exist");
        assert_eq!(vox.status, PluginStatus::Ready);
        assert_eq!(vox.clap_file, "Phantom Vox.clap");
        assert_eq!(eq.vst3_bundle, "Phantom EQ.vst3");
        assert_eq!(eq.clap_file, "Phantom EQ.clap");
        assert!(find_plugin("phantom-comp").is_some());
        assert!(find_plugin("phantom-limiter").is_some());
        assert!(find_plugin("phantom-mb").is_some());
        assert!(find_plugin("phantom-saturn").is_some());
    }

    #[test]
    fn vst3_bundle_paths_across_platforms() {
        let eq = find_plugin("phantom-eq").unwrap();
        assert_eq!(
            vst3_inner_binary_rel_for(eq, "windows"),
            PathBuf::from("Contents/x86_64-win/Phantom EQ.vst3")
        );
        assert_eq!(
            vst3_inner_binary_rel_for(eq, "linux"),
            PathBuf::from("Contents/x86_64-linux/Phantom EQ.so")
        );
        assert_eq!(
            vst3_inner_binary_rel_for(eq, "macos"),
            PathBuf::from("Contents/MacOS/Phantom EQ")
        );
    }

    #[test]
    fn github_url_formatting() {
        assert_eq!(
            github_releases_url("intity01/phantom-dsp-suite"),
            "https://github.com/intity01/phantom-dsp-suite/releases"
        );
        assert_eq!(
            github_releases_url(""),
            "https://github.com/intity01/phantom-dsp-suite/releases"
        );
        assert_eq!(
            github_releases_url("https://github.com/org/repo"),
            "https://github.com/org/repo/releases"
        );
    }
}

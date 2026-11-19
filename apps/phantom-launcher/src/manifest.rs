//! Plugin suite catalog, bundle layout metadata, and GitHub release URL helpers.

use crate::paths::host_dist_os_dir;
use std::path::{Path, PathBuf};

pub const DEFAULT_GITHUB_REPO: &str = "intity01/phantom-dsp-suite";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStatus {
    Ready,
    Planned,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub status: PluginStatus,
    pub vst3_bundle: &'static str,
    pub vst3_win_bin: &'static str,
    pub vst3_linux_bin: &'static str,
    pub vst3_macos_bin: &'static str,
    pub clap_file: &'static str,
    pub accent_rgb: [u8; 3],
}

pub const AVAILABLE_PLUGINS: &[PluginManifest] = &[
    PluginManifest {
        id: "phantom-eq",
        name: "Phantom EQ",
        version: "0.1.0",
        category: "Equalizer",
        description: "8-band dynamic parametric equalizer with Linear Phase FIR, M/S routing & anti-cramping oversampling",
        status: PluginStatus::Ready,
        vst3_bundle: "Phantom EQ.vst3",
        vst3_win_bin: "Phantom EQ.vst3",
        vst3_linux_bin: "Phantom EQ.so",
        vst3_macos_bin: "Phantom EQ",
        clap_file: "Phantom EQ.clap",
        accent_rgb: [56, 189, 248],
    },
    PluginManifest {
        id: "phantom-comp",
        name: "Phantom Comp",
        version: "0.1.0-planned",
        category: "Dynamics",
        description: "Mastering & bus compressor with visual transfer curve & sidechain filtering",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Comp.vst3",
        vst3_win_bin: "Phantom Comp.vst3",
        vst3_linux_bin: "Phantom Comp.so",
        vst3_macos_bin: "Phantom Comp",
        clap_file: "Phantom Comp.clap",
        accent_rgb: [16, 185, 129],
    },
    PluginManifest {
        id: "phantom-limiter",
        name: "Phantom Limiter",
        version: "0.1.0-planned",
        category: "Mastering",
        description: "True Peak mastering limiter with EBU R128 loudness metering",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Limiter.vst3",
        vst3_win_bin: "Phantom Limiter.vst3",
        vst3_linux_bin: "Phantom Limiter.so",
        vst3_macos_bin: "Phantom Limiter",
        clap_file: "Phantom Limiter.clap",
        accent_rgb: [245, 158, 11],
    },
    PluginManifest {
        id: "phantom-mb",
        name: "Phantom MB",
        version: "0.1.0-planned",
        category: "Multiband",
        description: "Linear-phase multiband dynamic processor with per-band soloing",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom MB.vst3",
        vst3_win_bin: "Phantom MB.vst3",
        vst3_linux_bin: "Phantom MB.so",
        vst3_macos_bin: "Phantom MB",
        clap_file: "Phantom MB.clap",
        accent_rgb: [139, 92, 246],
    },
    PluginManifest {
        id: "phantom-saturn",
        name: "Phantom Saturn",
        version: "0.1.0-planned",
        category: "Saturation",
        description: "Multiband warmth, tape & tube harmonic saturation engine",
        status: PluginStatus::Planned,
        vst3_bundle: "Phantom Saturn.vst3",
        vst3_win_bin: "Phantom Saturn.vst3",
        vst3_linux_bin: "Phantom Saturn.so",
        vst3_macos_bin: "Phantom Saturn",
        clap_file: "Phantom Saturn.clap",
        accent_rgb: [239, 68, 68],
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
    if nested.is_file() {
        return Some(nested);
    }
    None
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
        assert_eq!(AVAILABLE_PLUGINS.len(), 5);
        let eq = find_plugin("phantom-eq").expect("phantom-eq must exist");
        assert_eq!(eq.status, PluginStatus::Ready);
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

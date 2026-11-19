//! Local `dist/` binary artifact discovery and on-disk installation status scanning.

use crate::integrity::{compare_files, HashComparison, VerificationStatus};
use crate::manifest::{resolve_vst3_binary_in_bundle, PluginManifest};
use crate::paths::{host_dist_os_dir, InstallPaths};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct SourceArtifacts {
    pub vst3_bundle: Option<PathBuf>,
    pub vst3_binary: Option<PathBuf>,
    pub clap_file: Option<PathBuf>,
    pub detected_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct InstallScanResult {
    pub source: SourceArtifacts,
    pub vst3: HashComparison,
    pub clap: HashComparison,
}

impl InstallScanResult {
    pub fn is_any_installed(&self) -> bool {
        self.vst3.status != VerificationStatus::NotInstalled
            || self.clap.status != VerificationStatus::NotInstalled
    }

    pub fn has_update(&self) -> bool {
        self.vst3.status == VerificationStatus::UpdateAvailable
            || self.clap.status == VerificationStatus::UpdateAvailable
    }

    pub fn is_verified_match(&self) -> bool {
        let v_ok = self.vst3.status == VerificationStatus::VerifiedMatch;
        let c_ok = self.clap.status == VerificationStatus::VerifiedMatch;
        (v_ok || c_ok) && !self.has_update()
    }

    pub fn has_any_source(&self) -> bool {
        self.source.vst3_binary.is_some() || self.source.clap_file.is_some()
    }

    pub fn primary_hash(&self) -> Option<&str> {
        self.vst3
            .display_hash()
            .or_else(|| self.clap.display_hash())
    }
}

/// Locates local `dist/` source artifacts relative to an explicit override, executable, or cwd.
pub fn locate_source_artifacts(
    plugin: &PluginManifest,
    custom_root: Option<&Path>,
) -> SourceArtifacts {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let os_dir = host_dist_os_dir();

    if let Some(root) = custom_root {
        push_os_candidates(&mut candidates, root, os_dir);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for anc in parent.ancestors().take(5) {
                push_os_candidates(&mut candidates, anc, os_dir);
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for anc in cwd.ancestors().take(4) {
            push_os_candidates(&mut candidates, anc, os_dir);
        }
    }

    let mut found = SourceArtifacts::default();
    for dir in candidates {
        if !dir.is_dir() {
            continue;
        }
        if found.vst3_bundle.is_none() {
            for bundle_candidate in [
                dir.join("vst3").join(plugin.vst3_bundle),
                dir.join(plugin.vst3_bundle),
            ] {
                if let Some(bin) = resolve_vst3_binary_in_bundle(plugin, &bundle_candidate) {
                    found.vst3_bundle = Some(bundle_candidate);
                    found.vst3_binary = Some(bin);
                    found.detected_root.get_or_insert_with(|| dir.clone());
                    break;
                }
            }
        }
        if found.clap_file.is_none() {
            for clap_candidate in [
                dir.join("clap").join(plugin.clap_file),
                dir.join(plugin.clap_file),
            ] {
                if clap_candidate.is_file() {
                    found.clap_file = Some(clap_candidate);
                    found.detected_root.get_or_insert_with(|| dir.clone());
                    break;
                }
            }
        }
        if found.vst3_binary.is_some() && found.clap_file.is_some() {
            break;
        }
    }
    found
}

fn push_os_candidates(out: &mut Vec<PathBuf>, base: &Path, os_dir: &str) {
    let dist_os = base.join("dist").join(os_dir);
    let direct_os = base.join(os_dir);
    for p in [base.to_path_buf(), dist_os, direct_os] {
        if !out.contains(&p) {
            out.push(p);
        }
    }
}

/// Scans disk for local source binaries and installed targets, computing SHA-256 checksums.
pub fn scan_plugin(
    plugin: &PluginManifest,
    paths: &InstallPaths,
    custom_root: Option<&Path>,
) -> InstallScanResult {
    let source = locate_source_artifacts(plugin, custom_root);
    let vst3_target_bundle = paths.vst3_dir.join(plugin.vst3_bundle);
    let installed_vst3_bin = resolve_vst3_binary_in_bundle(plugin, &vst3_target_bundle);
    let clap_target_file = paths.clap_dir.join(plugin.clap_file);

    let vst3 = compare_files(source.vst3_binary.as_deref(), installed_vst3_bin.as_deref());
    let clap = compare_files(
        source.clap_file.as_deref(),
        clap_target_file
            .is_file()
            .then_some(clap_target_file.as_path()),
    );

    InstallScanResult { source, vst3, clap }
}

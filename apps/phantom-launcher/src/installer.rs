//! 1-Click Install, Reinstall, Update, and Uninstall with post-copy SHA-256 verification.

use crate::discovery::locate_source_artifacts;
use crate::integrity::sha256_file;
use crate::manifest::{resolve_vst3_binary_in_bundle, PluginManifest};
use crate::paths::{host_dist_os_dir, InstallPaths};
use crate::platform::detect_running_cubase;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    pub install_vst3: bool,
    pub install_clap: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            install_vst3: true,
            install_clap: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstalledItem {
    pub format: &'static str,
    pub dest_path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct InstallReport {
    pub items: Vec<InstalledItem>,
    pub warnings: Vec<String>,
}

/// Installs/updates a plugin's VST3 bundle and/or CLAP binary, then verifies post-copy SHA-256.
pub fn install_plugin(
    plugin: &PluginManifest,
    paths: &InstallPaths,
    custom_root: Option<&Path>,
    opts: InstallOptions,
) -> Result<InstallReport, String> {
    if !opts.install_vst3 && !opts.install_clap {
        return Err("Select at least one plugin format (VST3 or CLAP) to install.".into());
    }

    let mut report = InstallReport::default();
    if let Some(daw) = detect_running_cubase() {
        report.warnings.push(format!(
            "{daw} is currently running — if overwriting fails, close Cubase and retry."
        ));
    }

    let source = locate_source_artifacts(plugin, custom_root);
    if opts.install_vst3 {
        if let (Some(src_bundle), Some(src_bin)) = (&source.vst3_bundle, &source.vst3_binary) {
            let dst_bundle = paths.vst3_dir.join(plugin.vst3_bundle);
            let item = copy_and_verify_vst3(plugin, src_bundle, src_bin, &dst_bundle)?;
            report.items.push(item);
        } else {
            report.warnings.push(format!(
                "No local VST3 source bundle found for {} in dist/{}/.",
                plugin.name,
                host_dist_os_dir()
            ));
        }
    }

    if opts.install_clap {
        if let Some(src_clap) = &source.clap_file {
            let dst_clap = paths.clap_dir.join(plugin.clap_file);
            let item = copy_and_verify_file("CLAP", src_clap, &dst_clap)?;
            report.items.push(item);
        } else {
            report.warnings.push(format!(
                "No local CLAP source binary found for {} in dist/{}/.",
                plugin.name,
                host_dist_os_dir()
            ));
        }
    }

    if report.items.is_empty() {
        return Err(format!(
            "No local {} binaries were found under dist/{}/. Build or place binaries in dist/ first.",
            plugin.name,
            host_dist_os_dir()
        ));
    }
    Ok(report)
}

fn copy_and_verify_vst3(
    plugin: &PluginManifest,
    src_bundle: &Path,
    src_bin: &Path,
    dst_bundle: &Path,
) -> Result<InstalledItem, String> {
    let expected_hash = sha256_file(src_bin)
        .map_err(|e| format!("Failed hashing source VST3 {}: {e}", src_bin.display()))?;

    if src_bundle.is_dir() {
        copy_dir_recursive(src_bundle, dst_bundle).map_err(|e| format_copy_error(dst_bundle, e))?;
    } else {
        if let Some(parent) = dst_bundle.parent() {
            fs::create_dir_all(parent).map_err(|e| format_copy_error(parent, e))?;
        }
        fs::copy(src_bundle, dst_bundle).map_err(|e| format_copy_error(dst_bundle, e))?;
    }

    let dst_bin = resolve_vst3_binary_in_bundle(plugin, dst_bundle).ok_or_else(|| {
        format!(
            "Installed VST3 binary missing inside {}",
            dst_bundle.display()
        )
    })?;
    let actual_hash = sha256_file(&dst_bin)
        .map_err(|e| format!("Failed verifying installed VST3 {}: {e}", dst_bin.display()))?;
    if actual_hash != expected_hash {
        return Err(format!(
            "SHA-256 mismatch after copying VST3! Expected {expected_hash}, got {actual_hash}"
        ));
    }
    let size_bytes = fs::metadata(&dst_bin).map(|m| m.len()).unwrap_or(0);
    Ok(InstalledItem {
        format: "VST3",
        dest_path: dst_bundle.to_path_buf(),
        sha256: actual_hash,
        size_bytes,
    })
}

fn copy_and_verify_file(
    format: &'static str,
    src: &Path,
    dst: &Path,
) -> Result<InstalledItem, String> {
    let expected_hash = sha256_file(src)
        .map_err(|e| format!("Failed hashing source {format} {}: {e}", src.display()))?;
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format_copy_error(parent, e))?;
    }
    fs::copy(src, dst).map_err(|e| format_copy_error(dst, e))?;
    let actual_hash = sha256_file(dst)
        .map_err(|e| format!("Failed verifying installed {format} {}: {e}", dst.display()))?;
    if actual_hash != expected_hash {
        return Err(format!(
            "SHA-256 mismatch after copying {format}! Expected {expected_hash}, got {actual_hash}"
        ));
    }
    let size_bytes = fs::metadata(dst).map(|m| m.len()).unwrap_or(0);
    Ok(InstalledItem {
        format,
        dest_path: dst.to_path_buf(),
        sha256: actual_hash,
        size_bytes,
    })
}

/// Cleanly removes installed VST3 bundle and CLAP file for a plugin.
pub fn uninstall_plugin(
    plugin: &PluginManifest,
    paths: &InstallPaths,
) -> Result<Vec<PathBuf>, String> {
    let mut removed = Vec::new();
    let vst3_bundle = paths.vst3_dir.join(plugin.vst3_bundle);
    if vst3_bundle.exists() {
        let res = if vst3_bundle.is_dir() {
            fs::remove_dir_all(&vst3_bundle)
        } else {
            fs::remove_file(&vst3_bundle)
        };
        res.map_err(|e| format_copy_error(&vst3_bundle, e))?;
        removed.push(vst3_bundle);
    }

    let clap_file = paths.clap_dir.join(plugin.clap_file);
    if clap_file.exists() {
        fs::remove_file(&clap_file).map_err(|e| format_copy_error(&clap_file, e))?;
        removed.push(clap_file);
    }
    Ok(removed)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn format_copy_error(dest: &Path, err: io::Error) -> String {
    match err.kind() {
        io::ErrorKind::PermissionDenied => format!(
            "Permission denied writing {} ({err}). Try User directory or close Cubase / run as Administrator.",
            dest.display()
        ),
        _ => format!("File operation failed at {}: {err}", dest.display()),
    }
}

#[cfg(test)]
#[path = "installer_tests.rs"]
mod tests;

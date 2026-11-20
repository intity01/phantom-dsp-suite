//! Pure-Rust offline `.zip` archive installer with CRC-32, SHA-256 & zip-slip protection.

use crate::installer::InstallOptions;
use crate::integrity::sha256_file;
use crate::manifest::AVAILABLE_PLUGINS;
use crate::paths::{host_dist_os_dir, InstallPaths};
use flate2::read::DeflateDecoder;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_ENTRY_BYTES: usize = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ArchiveInstallReport {
    pub archive_sha256: String,
    pub extracted_files: Vec<PathBuf>,
    pub detected_plugin_ids: Vec<&'static str>,
}

struct ZipEntryMeta {
    name: String,
    method: u16,
    crc32: u32,
    comp_size: usize,
    uncomp_size: usize,
    local_offset: usize,
}

fn u16_le(b: &[u8], off: usize) -> Result<u16, String> {
    let s = b.get(off..off + 2).ok_or("Truncated zip header")?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_le(b: &[u8], off: usize) -> Result<u32, String> {
    let s = b.get(off..off + 4).ok_or("Truncated zip header")?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn parse_central_directory(data: &[u8]) -> Result<Vec<ZipEntryMeta>, String> {
    if data.len() < 22 {
        return Err("File is too small to be a valid .zip archive".into());
    }
    let search_start = data.len().saturating_sub(65_557);
    let eocd = (search_start..=data.len() - 22)
        .rev()
        .find(|&i| data[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(|| "Invalid .zip archive (missing central directory)".to_string())?;

    let total_entries = u16_le(data, eocd + 10)? as usize;
    let cd_size = u32_le(data, eocd + 12)? as usize;
    let cd_offset = u32_le(data, eocd + 16)? as usize;
    if cd_offset.checked_add(cd_size).is_none_or(|end| end > eocd) {
        return Err("Corrupted .zip central directory bounds".into());
    }

    let mut pos = cd_offset;
    let mut out = Vec::with_capacity(total_entries);
    for _ in 0..total_entries {
        if pos + 46 > cd_offset + cd_size || data[pos..pos + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err("Corrupted central directory entry in .zip".into());
        }
        let method = u16_le(data, pos + 10)?;
        let crc32 = u32_le(data, pos + 16)?;
        let comp_size = u32_le(data, pos + 20)? as usize;
        let uncomp_size = u32_le(data, pos + 24)? as usize;
        let name_len = u16_le(data, pos + 28)? as usize;
        let extra_len = u16_le(data, pos + 30)? as usize;
        let comment_len = u16_le(data, pos + 32)? as usize;
        let local_offset = u32_le(data, pos + 42)? as usize;

        let name_end = pos + 46 + name_len;
        let name_bytes = data
            .get(pos + 46..name_end)
            .ok_or("Truncated entry filename")?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        pos = name_end + extra_len + comment_len;
        out.push(ZipEntryMeta {
            name,
            method,
            crc32,
            comp_size,
            uncomp_size,
            local_offset,
        });
    }
    Ok(out)
}

fn extract_entry_bytes(data: &[u8], meta: &ZipEntryMeta) -> Result<Vec<u8>, String> {
    if meta.uncomp_size > MAX_ENTRY_BYTES {
        return Err(format!("Archive entry {} exceeds size limit", meta.name));
    }
    let off = meta.local_offset;
    if off + 30 > data.len() || data[off..off + 4] != [0x50, 0x4b, 0x03, 0x04] {
        return Err(format!("Corrupted local header for {}", meta.name));
    }
    let name_len = u16_le(data, off + 26)? as usize;
    let extra_len = u16_le(data, off + 28)? as usize;
    let start = off + 30 + name_len + extra_len;
    let payload = data
        .get(start..start + meta.comp_size)
        .ok_or_else(|| format!("Truncated payload for {}", meta.name))?;

    let decoded = match meta.method {
        0 => payload.to_vec(),
        8 => {
            let mut decoder = DeflateDecoder::new(payload).take((MAX_ENTRY_BYTES + 1) as u64);
            let mut buf = Vec::with_capacity(meta.uncomp_size.min(1024 * 1024));
            decoder
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed inflating {}: {e}", meta.name))?;
            buf
        }
        other => {
            return Err(format!(
                "Unsupported compression method {other} for {}",
                meta.name
            ))
        }
    };
    if decoded.len() != meta.uncomp_size {
        return Err(format!("Size mismatch extracting {}", meta.name));
    }
    if crc32fast::hash(&decoded) != meta.crc32 {
        return Err(format!("CRC-32 mismatch extracting {}", meta.name));
    }
    Ok(decoded)
}

fn validate_safe_parts(raw: &str) -> Result<Vec<&str>, String> {
    if raw.contains('\0') || raw.starts_with('/') || raw.starts_with('\\') {
        return Err(format!("Unsafe path in archive: {raw}"));
    }
    let mut parts = Vec::new();
    for seg in raw.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || (seg.len() >= 2 && seg.as_bytes()[1] == b':') {
            return Err(format!("Unsafe zip-slip path in archive: {raw}"));
        }
        parts.push(seg);
    }
    Ok(parts)
}

fn matches_other_os(seg: &str, host_os: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    let other = [
        "windows",
        "linux",
        "macos",
        "windows-x64",
        "linux-x64",
        "macos-universal",
    ];
    other.iter().any(|k| lower == *k && !k.starts_with(host_os))
}

fn map_target_path(
    parts: &[&str],
    paths: &InstallPaths,
    host_os: &str,
    opts: InstallOptions,
) -> Option<(PathBuf, Option<&'static str>)> {
    if parts.iter().any(|s| *s == "__MACOSX" || *s == ".DS_Store") {
        return None;
    }
    let idx = parts.iter().position(|s| {
        let low = s.to_ascii_lowercase();
        low.ends_with(".vst3") || low.ends_with(".clap")
    })?;
    if parts[..idx]
        .iter()
        .any(|s| matches_other_os(s, host_os) || s.eq_ignore_ascii_case("au"))
    {
        return None;
    }
    let root_name = parts[idx];
    let lower = root_name.to_ascii_lowercase();
    let dest_base = if lower.ends_with(".vst3") {
        if !opts.install_vst3 {
            return None;
        }
        &paths.vst3_dir
    } else if opts.install_clap {
        &paths.clap_dir
    } else {
        return None;
    };

    let mut out = dest_base.to_path_buf();
    for seg in &parts[idx..] {
        out.push(seg);
    }
    let matched = AVAILABLE_PLUGINS.iter().find_map(|p| {
        (p.vst3_bundle.eq_ignore_ascii_case(root_name)
            || p.clap_file.eq_ignore_ascii_case(root_name))
        .then_some(p.id)
    });
    Some((out, matched))
}

/// Extracts VST3/CLAP bundles from a local `.zip` into target plugin directories offline.
pub fn install_from_zip(
    zip_path: &Path,
    paths: &InstallPaths,
    opts: InstallOptions,
) -> Result<ArchiveInstallReport, String> {
    if !opts.install_vst3 && !opts.install_clap {
        return Err("Select at least one plugin format (VST3 or CLAP) to install.".into());
    }
    let archive_sha256 =
        sha256_file(zip_path).map_err(|e| format!("Failed reading {}: {e}", zip_path.display()))?;
    let bytes =
        fs::read(zip_path).map_err(|e| format!("Failed reading {}: {e}", zip_path.display()))?;
    let entries = parse_central_directory(&bytes)?;
    let host_os = host_dist_os_dir();

    let mut total_written = 0usize;
    let mut extracted_files = Vec::new();
    let mut plugin_ids = BTreeSet::new();
    for entry in &entries {
        let parts = validate_safe_parts(&entry.name)?;
        if entry.name.ends_with('/') || entry.name.ends_with('\\') || parts.is_empty() {
            continue;
        }
        let Some((dest, matched_id)) = map_target_path(&parts, paths, host_os, opts) else {
            continue;
        };
        total_written = total_written
            .checked_add(entry.uncomp_size)
            .filter(|&t| t <= MAX_TOTAL_BYTES)
            .ok_or_else(|| "Archive total extracted size exceeds limit".to_string())?;
        let payload = extract_entry_bytes(&bytes, entry)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Write failed: {e}"))?;
        }
        fs::write(&dest, &payload)
            .map_err(|e| format!("Failed writing {}: {e}", dest.display()))?;
        #[cfg(unix)]
        if dest
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("so") || e.eq_ignore_ascii_case("clap"))
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
        }
        if let Some(id) = matched_id {
            plugin_ids.insert(id);
        }
        extracted_files.push(dest);
    }

    if extracted_files.is_empty() {
        return Err(format!(
            "No VST3/CLAP bundles for {host_os} found in {}",
            zip_path.display()
        ));
    }
    Ok(ArchiveInstallReport {
        archive_sha256,
        extracted_files,
        detected_plugin_ids: plugin_ids.into_iter().collect(),
    })
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;

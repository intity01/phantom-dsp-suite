//! Non-destructive `.pvox` JSON project serialization, source file integrity checks,
//! 32-step Undo/Redo history, Autosave, and Batch Time-Aligned WAV Export.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use phantom_vox_dsp::{AlignConfig, ProtectedRegion, SyncAnchor};
use serde::{Deserialize, Serialize};

pub use crate::export::{export_aligned_stems, ExportFormat, ExportStem};
pub use crate::sha256::sha256_hex;

pub const PVOX_SCHEMA_VERSION: u32 = 1;
pub const MAX_UNDO_STEPS: usize = 32;

/// Non-destructive source fingerprint (`size`, `mtime`, and SHA-256 hex digest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFingerprint {
    pub byte_len: u64,
    pub modified_unix_secs: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFileStatus {
    Ok,
    Virtual,
    Missing,
    Modified(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PvoxTrack {
    pub name: String,
    pub source_path: Option<PathBuf>,
    pub fingerprint: Option<SourceFingerprint>,
    pub is_guide: bool,
    pub config: AlignConfig,
    pub anchors: Vec<SyncAnchor>,
    pub protected: Vec<ProtectedRegion>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PvoxProject {
    pub schema_version: u32,
    pub sample_rate: u32,
    pub reference_index: usize,
    pub tracks: Vec<PvoxTrack>,
}

impl Default for PvoxProject {
    fn default() -> Self {
        Self {
            schema_version: PVOX_SCHEMA_VERSION,
            sample_rate: 48_000,
            reference_index: 0,
            tracks: Vec::new(),
        }
    }
}

/// Computes non-destructive `size + mtime + SHA-256` fingerprint for an external audio file.
pub fn compute_source_fingerprint(path: &Path) -> io::Result<SourceFingerprint> {
    let meta = fs::metadata(path)?;
    let bytes = fs::read(path)?;
    let modified_unix_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(SourceFingerprint {
        byte_len: meta.len(),
        modified_unix_secs,
        sha256_hex: sha256_hex(&bytes),
    })
}

/// Checks whether an external source WAV/AIFF is intact, missing, or modified since saved.
pub fn verify_source_file(
    path: Option<&Path>,
    expected: Option<&SourceFingerprint>,
) -> SourceFileStatus {
    let Some(p) = path else {
        return SourceFileStatus::Virtual;
    };
    if !p.exists() {
        return SourceFileStatus::Missing;
    }
    let Some(exp) = expected else {
        return SourceFileStatus::Ok;
    };
    match compute_source_fingerprint(p) {
        Ok(actual) => {
            if actual.sha256_hex == exp.sha256_hex && actual.byte_len == exp.byte_len {
                SourceFileStatus::Ok
            } else {
                SourceFileStatus::Modified(format!(
                    "Source modified on disk ({} -> {} bytes)",
                    exp.byte_len, actual.byte_len
                ))
            }
        }
        Err(_) => SourceFileStatus::Missing,
    }
}

pub fn save_project(path: &Path, project: &PvoxProject) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

pub fn load_project(path: &Path) -> io::Result<PvoxProject> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn autosave_path(project_path: Option<&Path>) -> PathBuf {
    if let Some(p) = project_path {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("session");
        p.with_file_name(format!(".{stem}.autosave.pvox"))
    } else {
        std::env::temp_dir().join("phantom_vox_studio.autosave.pvox")
    }
}

pub fn write_autosave(project: &PvoxProject, project_path: Option<&Path>) -> io::Result<PathBuf> {
    let dst = autosave_path(project_path);
    save_project(&dst, project)?;
    Ok(dst)
}

/// 32-step Undo/Redo history stack over `PvoxProject` snapshots.
#[derive(Debug, Default, Clone)]
pub struct HistoryStack {
    undo: Vec<PvoxProject>,
    redo: Vec<PvoxProject>,
}

impl HistoryStack {
    pub fn push(&mut self, snapshot: PvoxProject) {
        if self.undo.last() == Some(&snapshot) {
            return;
        }
        self.undo.push(snapshot);
        if self.undo.len() > MAX_UNDO_STEPS {
            let excess = self.undo.len() - MAX_UNDO_STEPS;
            self.undo.drain(0..excess);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: PvoxProject) -> Option<PvoxProject> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        if self.redo.len() > MAX_UNDO_STEPS {
            self.redo.remove(0);
        }
        Some(prev)
    }

    pub fn redo(&mut self, current: PvoxProject) -> Option<PvoxProject> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        if self.undo.len() > MAX_UNDO_STEPS {
            self.undo.remove(0);
        }
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[allow(dead_code)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

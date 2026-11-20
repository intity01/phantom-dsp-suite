use std::path::{Path, PathBuf};

use phantom_vox_dsp::{read_audio_file, AudioBuffer};

use crate::project::{
    export_aligned_stems, load_project, save_project, verify_source_file, ExportStem, PvoxProject,
    SourceFileStatus,
};
use crate::state::{StudioApp, StudioTrack};

impl StudioApp {
    pub fn save_project_to(&mut self, path: PathBuf) {
        let proj = self.to_pvox_project();
        match save_project(&path, &proj) {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.dirty_since_autosave = false;
                self.status_line = format!("Saved project to {}", path.display());
            }
            Err(e) => self.status_line = format!("Save failed: {e}"),
        }
    }

    pub fn open_project_from(&mut self, path: PathBuf) {
        match load_project(&path) {
            Ok(proj) => {
                self.push_undo();
                self.project_path = Some(path.clone());
                self.apply_project_snapshot(proj, true);
                self.status_line = format!("Opened project {}", path.display());
            }
            Err(e) => self.status_line = format!("Failed to open project: {e}"),
        }
    }

    pub(crate) fn apply_project_snapshot(&mut self, proj: PvoxProject, reload_files: bool) {
        self.warnings.clear();
        self.sample_rate = proj.sample_rate.max(8000);
        let old_tracks = std::mem::take(&mut self.tracks);

        for (i, pt) in proj.tracks.into_iter().enumerate() {
            let status = verify_source_file(pt.source_path.as_deref(), pt.fingerprint.as_ref());
            if let SourceFileStatus::Missing = &status {
                self.warnings.push(format!(
                    "Missing source file for '{}': {}",
                    pt.name,
                    pt.source_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ));
            } else if let SourceFileStatus::Modified(msg) = &status {
                self.warnings.push(format!("'{}': {msg}", pt.name));
            }

            let existing = old_tracks
                .iter()
                .find(|t| t.source_path == pt.source_path && t.name == pt.name);

            let buf = if !reload_files {
                existing
                    .map(|t| (*t.original).clone())
                    .or_else(|| {
                        pt.source_path
                            .as_ref()
                            .and_then(|p| read_audio_file(p).ok())
                    })
                    .unwrap_or_else(|| AudioBuffer::new_mono(self.sample_rate, vec![0.0; 4800]))
            } else {
                pt.source_path
                    .as_ref()
                    .and_then(|p| read_audio_file(p).ok())
                    .or_else(|| existing.map(|t| (*t.original).clone()))
                    .unwrap_or_else(|| AudioBuffer::new_mono(self.sample_rate, vec![0.0; 4800]))
            };

            let id = self.alloc_track_id();
            let mut tr = StudioTrack::new(
                id,
                pt.name,
                pt.source_path,
                pt.fingerprint,
                pt.is_guide || i == proj.reference_index,
                buf,
                pt.pan,
                i,
            );
            tr.file_status = status;
            tr.config = pt.config;
            tr.anchors = pt.anchors;
            tr.protected = pt.protected;
            tr.gain = pt.gain;
            tr.muted = pt.muted;
            tr.solo = pt.solo;
            self.tracks.push(tr);
        }
        self.selected_track_id = self.tracks.first().map(|t| t.id);
        self.sync_audio_mix();
        self.batch_process_all();
    }

    pub fn export_stems_to(&mut self, dir: &Path) {
        let total = self.total_samples();
        let stems: Vec<ExportStem<'_>> = self
            .tracks
            .iter()
            .filter(|t| !t.is_guide && !t.muted)
            .map(|t| ExportStem {
                name: &t.name,
                buffer: t.aligned.as_deref().unwrap_or(&t.original),
            })
            .collect();
        if stems.is_empty() {
            self.status_line = "No unmuted Dub stems to export.".to_owned();
            return;
        }
        match export_aligned_stems(dir, &stems, total, self.export_format) {
            Ok(files) => {
                self.status_line = format!(
                    "Exported {} DAW-synced stem(s) (0.0s aligned) to {}",
                    files.len(),
                    dir.display()
                );
            }
            Err(e) => self.status_line = format!("Export failed: {e}"),
        }
    }
}

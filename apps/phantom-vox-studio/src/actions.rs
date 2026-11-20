//! High-level studio actions: Demo Vocal Stack loader, file import, project save/load,
//! undo/redo, batch alignment, artificial L/R double creation, and DAW-synced WAV export.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phantom_vox_dsp::read_audio_file;

use crate::project::{compute_source_fingerprint, write_autosave};
use crate::state::{StudioApp, StudioTrack};
use crate::worker::WorkerJob;

impl StudioApp {
    pub fn push_undo(&mut self) {
        let snap = self.to_pvox_project();
        self.history.push(snap);
        self.dirty_since_autosave = true;
    }

    pub fn undo(&mut self) {
        let current = self.to_pvox_project();
        if let Some(prev) = self.history.undo(current) {
            self.apply_project_snapshot(prev, false);
            self.status_line = "Undo applied.".to_owned();
        }
    }

    pub fn redo(&mut self) {
        let current = self.to_pvox_project();
        if let Some(next) = self.history.redo(current) {
            self.apply_project_snapshot(next, false);
            self.status_line = "Redo applied.".to_owned();
        }
    }

    pub fn maybe_autosave(&mut self) {
        if self.dirty_since_autosave
            && !self.tracks.is_empty()
            && self.last_autosave.elapsed() >= Duration::from_secs(15)
        {
            let proj = self.to_pvox_project();
            let _ = write_autosave(&proj, self.project_path.as_deref());
            self.last_autosave = Instant::now();
            self.dirty_since_autosave = false;
        }
    }

    pub fn import_guide_file(&mut self, path: PathBuf) {
        match read_audio_file(&path) {
            Ok(buf) => {
                self.push_undo();
                self.sample_rate = buf.sample_rate.max(8000);
                let fp = compute_source_fingerprint(&path).ok();
                let name = file_stem_name(&path, "Guide");
                for t in &mut self.tracks {
                    t.is_guide = false;
                }
                let id = self.alloc_track_id();
                let tr = StudioTrack::new(id, name.clone(), Some(path), fp, true, buf, 0.0, 0);
                self.tracks.insert(0, tr);
                self.selected_track_id = Some(id);
                self.loop_end_sec = self.total_duration_sec();
                self.sync_audio_mix();
                self.batch_process_all();
                self.status_line = format!("Imported Guide '{}' ({} Hz)", name, self.sample_rate);
            }
            Err(e) => {
                self.status_line = format!("Failed to read Guide: {e}");
            }
        }
    }

    pub fn add_dub_files(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        self.push_undo();
        let mut added = 0usize;
        let mut queued = Vec::new();
        for path in paths {
            match read_audio_file(&path) {
                Ok(buf) => {
                    let is_first = self.tracks.is_empty();
                    if is_first {
                        self.sample_rate = buf.sample_rate.max(8000);
                    }
                    let fp = compute_source_fingerprint(&path).ok();
                    let name = file_stem_name(&path, "Dub");
                    let id = self.alloc_track_id();
                    let color_idx = self.tracks.len();
                    let pan = match self.tracks.len() % 3 {
                        1 => -0.55,
                        2 => 0.55,
                        _ => 0.0,
                    };
                    let tr = StudioTrack::new(
                        id,
                        name,
                        Some(path),
                        fp,
                        is_first,
                        buf,
                        if is_first { 0.0 } else { pan },
                        color_idx,
                    );
                    let idx = self.tracks.len();
                    self.tracks.push(tr);
                    self.selected_track_id = Some(id);
                    if !is_first {
                        queued.push(idx);
                    }
                    added += 1;
                }
                Err(e) => {
                    self.warnings
                        .push(format!("Could not import {}: {e}", path.display()));
                }
            }
        }
        if added > 0 {
            self.loop_end_sec = self.total_duration_sec();
            self.sync_audio_mix();
            for idx in queued {
                self.queue_track_analysis(idx);
            }
            self.status_line = format!("Imported {added} audio track(s).");
        }
    }

    pub fn set_track_as_guide(&mut self, idx: usize) {
        if idx >= self.tracks.len() || self.tracks[idx].is_guide {
            return;
        }
        self.push_undo();
        for (i, t) in self.tracks.iter_mut().enumerate() {
            t.is_guide = i == idx;
            if t.is_guide {
                t.aligned = None;
                t.aligned_peaks = None;
                t.alignment = None;
            }
        }
        let guide = self.tracks.remove(idx);
        self.tracks.insert(0, guide);
        self.sync_audio_mix();
        self.batch_process_all();
    }

    pub fn remove_track(&mut self, idx: usize) {
        if idx >= self.tracks.len() {
            return;
        }
        self.push_undo();
        let removed_id = self.tracks[idx].id;
        self.tracks.remove(idx);
        if self.selected_track_id == Some(removed_id) {
            self.selected_track_id = self.tracks.first().map(|t| t.id);
        }
        if self.guide_index().is_none() {
            if let Some(first) = self.tracks.first_mut() {
                first.is_guide = true;
            }
        }
        self.sync_audio_mix();
    }

    pub fn batch_process_all(&mut self) {
        let Some(g_idx) = self.guide_index() else {
            self.status_line = "No Guide track set — import a Guide first.".to_owned();
            return;
        };
        let mut count = 0usize;
        for i in 0..self.tracks.len() {
            if i != g_idx {
                self.queue_track_analysis(i);
                count += 1;
            }
        }
        if count > 0 {
            self.status_line = format!("Batch aligning {count} layer(s) on background worker...");
        }
    }

    pub fn create_artificial_doubles(&mut self) {
        let src_idx = self.guide_index().or_else(|| self.selected_index());
        let Some(idx) = src_idx else {
            self.status_line = "Load or import a vocal track before creating doubles.".to_owned();
            return;
        };
        self.push_undo();
        let src = Arc::clone(&self.tracks[idx].original);
        let base = self.tracks[idx].name.clone();
        let j_l = self.alloc_job_id();
        self.worker.submit(WorkerJob::CreateDouble {
            job_id: j_l,
            source: Arc::clone(&src),
            name: format!("{base} Double L"),
            timing_ms: -14.0,
            detune_cents: -9.5,
            formant_st: -0.45,
            vibrato_cents: 6.0,
            pan: -0.78,
        });
        let j_r = self.alloc_job_id();
        self.worker.submit(WorkerJob::CreateDouble {
            job_id: j_r,
            source: src,
            name: format!("{base} Double R"),
            timing_ms: 16.0,
            detune_cents: 9.5,
            formant_st: 0.45,
            vibrato_cents: 7.0,
            pan: 0.78,
        });
        self.status_line = format!("Synthesizing Artificial Double L/R from '{base}'...");
    }

    pub fn handle_dropped_paths(&mut self, paths: Vec<PathBuf>) {
        let mut dubs = Vec::new();
        for p in paths {
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "pvox" {
                self.open_project_from(p);
                return;
            } else if ext == "wav" || ext == "aif" || ext == "aiff" {
                if self.tracks.is_empty() {
                    self.import_guide_file(p);
                } else {
                    dubs.push(p);
                }
            }
        }
        if !dubs.is_empty() {
            self.add_dub_files(dubs);
        }
    }
}

fn file_stem_name(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

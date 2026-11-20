//! Core application state, runtime tracks, audio mix synchronization, and worker polling.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::audio_out::{AudioEngine, PlaybackMixData, PlaybackTrack};
use crate::file_dialog::PathModal;
use crate::project::{ExportFormat, HistoryStack, PvoxProject, PVOX_SCHEMA_VERSION};
pub use crate::track::StudioTrack;
use crate::worker::AnalysisWorker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineTool {
    Scrub,
    Anchor,
    Protect,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingAnchor {
    pub track_id: u64,
    pub guide_sec: f32,
}

pub struct StudioApp {
    pub tracks: Vec<StudioTrack>,
    pub selected_track_id: Option<u64>,
    pub sample_rate: u32,
    pub project_path: Option<PathBuf>,
    pub history: HistoryStack,
    pub audio: AudioEngine,
    pub worker: AnalysisWorker,
    pub next_track_id: u64,
    pub next_job_id: u64,
    pub zoom_px_per_sec: f32,
    pub scroll_sec: f32,
    pub loop_start_sec: f32,
    pub loop_end_sec: f32,
    pub show_segmentation: bool,
    pub show_pitch: bool,
    pub tool: TimelineTool,
    pub pending_anchor: Option<PendingAnchor>,
    pub dragging_anchor: Option<(u64, usize)>,
    pub protect_drag: Option<(u64, f32, f32)>,
    pub export_format: ExportFormat,
    pub status_line: String,
    pub warnings: Vec<String>,
    pub path_modal: Option<PathModal>,
    pub last_autosave: Instant,
    pub dirty_since_autosave: bool,
}

impl StudioApp {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            selected_track_id: None,
            sample_rate: 48_000,
            project_path: None,
            history: HistoryStack::default(),
            audio: AudioEngine::new(48_000),
            worker: AnalysisWorker::spawn(),
            next_track_id: 1,
            next_job_id: 1,
            zoom_px_per_sec: 160.0,
            scroll_sec: 0.0,
            loop_start_sec: 0.0,
            loop_end_sec: 0.0,
            show_segmentation: true,
            show_pitch: true,
            tool: TimelineTool::Scrub,
            pending_anchor: None,
            dragging_anchor: None,
            protect_drag: None,
            export_format: ExportFormat::Wav24,
            status_line: "Ready — Click 'Load Demo' or import Guide & Dub WAV/AIFF files."
                .to_owned(),
            warnings: Vec::new(),
            path_modal: None,
            last_autosave: Instant::now(),
            dirty_since_autosave: false,
        }
    }

    pub fn alloc_track_id(&mut self) -> u64 {
        let id = self.next_track_id;
        self.next_track_id += 1;
        id
    }

    pub fn alloc_job_id(&mut self) -> u64 {
        let id = self.next_job_id;
        self.next_job_id += 1;
        id
    }

    pub fn guide_index(&self) -> Option<usize> {
        self.tracks.iter().position(|t| t.is_guide)
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected_track_id?;
        self.tracks.iter().position(|t| t.id == id)
    }

    pub fn total_duration_sec(&self) -> f32 {
        let mut max_sec = 0.0_f32;
        for tr in &self.tracks {
            max_sec = max_sec.max(tr.original.duration_sec());
            if let Some(al) = &tr.aligned {
                max_sec = max_sec.max(al.duration_sec());
            }
        }
        max_sec.max(4.0)
    }

    pub fn total_samples(&self) -> usize {
        self.tracks
            .iter()
            .map(|t| {
                t.original
                    .len()
                    .max(t.aligned.as_ref().map(|b| b.len()).unwrap_or(0))
            })
            .max()
            .unwrap_or(0)
    }

    pub fn any_processing(&self) -> bool {
        self.tracks.iter().any(|t| t.processing)
    }

    pub fn to_pvox_project(&self) -> PvoxProject {
        PvoxProject {
            schema_version: PVOX_SCHEMA_VERSION,
            sample_rate: self.sample_rate,
            reference_index: self.guide_index().unwrap_or(0),
            tracks: self.tracks.iter().map(StudioTrack::to_pvox).collect(),
        }
    }

    pub fn sync_audio_mix(&mut self) {
        let total_samples = self.total_samples();
        let tracks = self
            .tracks
            .iter()
            .map(|t| PlaybackTrack {
                original: Arc::clone(&t.original),
                aligned: t.aligned.as_ref().map(Arc::clone),
                gain: t.gain,
                pan: t.pan,
                muted: t.muted,
                solo: t.solo,
                is_guide: t.is_guide,
            })
            .collect();
        self.audio.update_mix(PlaybackMixData {
            sample_rate: self.sample_rate,
            total_samples,
            tracks,
        });
        self.audio
            .set_loop_range_sec(self.loop_start_sec, self.loop_end_sec);
    }
}

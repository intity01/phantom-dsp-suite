use std::sync::Arc;

use crate::state::StudioApp;
use crate::track::StudioTrack;
use crate::worker::{WorkerEvent, WorkerJob};

impl StudioApp {
    pub fn queue_track_analysis(&mut self, track_idx: usize) {
        let Some(g_idx) = self.guide_index() else {
            return;
        };
        if track_idx == g_idx || track_idx >= self.tracks.len() {
            return;
        }
        let guide = Arc::clone(&self.tracks[g_idx].original);
        let job_id = self.alloc_job_id();
        let tr = &mut self.tracks[track_idx];
        tr.processing = true;
        tr.active_job_id = job_id;
        self.worker.submit(WorkerJob::AnalyzeAndRender {
            job_id,
            track_id: tr.id,
            guide,
            dub: Arc::clone(&tr.original),
            anchors: tr.anchors.clone(),
            protected: tr.protected.clone(),
            config: tr.config,
        });
    }

    pub fn queue_track_rerender(&mut self, track_idx: usize) {
        let Some(g_idx) = self.guide_index() else {
            return;
        };
        if track_idx == g_idx || track_idx >= self.tracks.len() {
            return;
        }
        if self.tracks[track_idx].alignment.is_none() {
            self.queue_track_analysis(track_idx);
            return;
        }
        let guide = Arc::clone(&self.tracks[g_idx].original);
        let job_id = self.alloc_job_id();
        let tr = &mut self.tracks[track_idx];
        let alignment = Arc::clone(tr.alignment.as_ref().unwrap());
        tr.processing = true;
        tr.active_job_id = job_id;
        self.worker.submit(WorkerJob::RerenderOnly {
            job_id,
            track_id: tr.id,
            guide,
            dub: Arc::clone(&tr.original),
            alignment,
            protected: tr.protected.clone(),
            config: tr.config,
        });
    }

    pub fn poll_worker(&mut self) -> bool {
        let mut changed = false;
        while let Some(evt) = self.worker.try_recv() {
            match evt {
                WorkerEvent::Aligned {
                    job_id,
                    track_id,
                    alignment,
                    aligned_buf,
                    aligned_peaks,
                    elapsed_ms,
                } => {
                    if let Some(tr) = self.tracks.iter_mut().find(|t| t.id == track_id) {
                        if tr.active_job_id == job_id {
                            let conf_pct = (alignment.overall_confidence * 100.0).round() as u32;
                            tr.alignment = Some(alignment);
                            tr.aligned = Some(aligned_buf);
                            tr.aligned_peaks = Some(aligned_peaks);
                            tr.processing = false;
                            self.status_line = format!(
                                "Aligned '{}' ({}% confidence) in {} ms",
                                tr.name, conf_pct, elapsed_ms
                            );
                            changed = true;
                        }
                    }
                }
                WorkerEvent::DoubleReady {
                    job_id: _,
                    name,
                    buffer,
                    peaks,
                    pan,
                } => {
                    let id = self.alloc_track_id();
                    let color_idx = self.tracks.len();
                    let mut tr = StudioTrack::new(
                        id,
                        name.clone(),
                        None,
                        None,
                        false,
                        (*buffer).clone(),
                        pan,
                        color_idx,
                    );
                    tr.aligned = Some(buffer);
                    tr.aligned_peaks = Some(peaks);
                    self.tracks.push(tr);
                    self.selected_track_id = Some(id);
                    self.status_line = format!("Created artificial double '{}'", name);
                    changed = true;
                }
            }
        }
        if changed {
            self.sync_audio_mix();
        }
        changed
    }
}

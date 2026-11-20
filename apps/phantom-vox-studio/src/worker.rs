//! Background worker thread (`std::thread::mpsc`) for non-blocking DTW analysis,
//! WSOLA/pitch rendering, artificial double synthesis, and waveform peak generation.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use phantom_vox_dsp::{
    analyze_alignment, create_artificial_double, render_aligned_track, AlignConfig,
    AlignmentResult, AudioBuffer, ProtectedRegion, SyncAnchor,
};

pub const PEAK_BUCKETS: usize = 1600;

/// Pre-computed min/max waveform envelope for O(width) 60 fps canvas rendering.
#[derive(Clone, Default)]
pub struct WaveformPeaks {
    pub buckets: Vec<(f32, f32)>,
    pub duration_sec: f32,
}

impl WaveformPeaks {
    pub fn from_buffer(buf: &AudioBuffer, max_buckets: usize) -> Self {
        let len = buf.len();
        let duration_sec = buf.duration_sec();
        if len == 0 || max_buckets == 0 {
            return Self {
                buckets: Vec::new(),
                duration_sec,
            };
        }
        let n = max_buckets.min(len);
        let mut buckets = Vec::with_capacity(n);
        let stereo = buf.channels >= 2 && !buf.samples_r.is_empty();
        for b in 0..n {
            let start = (b * len) / n;
            let end = (((b + 1) * len) / n).max(start + 1).min(len);
            let mut mn = 0.0_f32;
            let mut mx = 0.0_f32;
            for i in start..end {
                let s = if stereo {
                    0.5 * (buf.samples_l[i] + buf.samples_r[i])
                } else {
                    buf.samples_l[i]
                };
                if s < mn {
                    mn = s;
                }
                if s > mx {
                    mx = s;
                }
            }
            buckets.push((mn.clamp(-1.0, 1.0), mx.clamp(-1.0, 1.0)));
        }
        Self {
            buckets,
            duration_sec,
        }
    }
}

pub enum WorkerJob {
    AnalyzeAndRender {
        job_id: u64,
        track_id: u64,
        guide: Arc<AudioBuffer>,
        dub: Arc<AudioBuffer>,
        anchors: Vec<SyncAnchor>,
        protected: Vec<ProtectedRegion>,
        config: AlignConfig,
    },
    RerenderOnly {
        job_id: u64,
        track_id: u64,
        guide: Arc<AudioBuffer>,
        dub: Arc<AudioBuffer>,
        alignment: Arc<AlignmentResult>,
        protected: Vec<ProtectedRegion>,
        config: AlignConfig,
    },
    CreateDouble {
        job_id: u64,
        source: Arc<AudioBuffer>,
        name: String,
        timing_ms: f32,
        detune_cents: f32,
        formant_st: f32,
        vibrato_cents: f32,
        pan: f32,
    },
}

pub enum WorkerEvent {
    Aligned {
        job_id: u64,
        track_id: u64,
        alignment: Arc<AlignmentResult>,
        aligned_buf: Arc<AudioBuffer>,
        aligned_peaks: WaveformPeaks,
        elapsed_ms: u64,
    },
    DoubleReady {
        #[allow(dead_code)]
        job_id: u64,
        name: String,
        buffer: Arc<AudioBuffer>,
        peaks: WaveformPeaks,
        pan: f32,
    },
}

pub struct AnalysisWorker {
    tx: Sender<WorkerJob>,
    rx: Receiver<WorkerEvent>,
}

impl AnalysisWorker {
    pub fn spawn() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<WorkerJob>();
        let (evt_tx, evt_rx) = mpsc::channel::<WorkerEvent>();

        thread::spawn(move || {
            while let Ok(job) = job_rx.recv() {
                let t0 = Instant::now();
                match job {
                    WorkerJob::AnalyzeAndRender {
                        job_id,
                        track_id,
                        guide,
                        dub,
                        anchors,
                        protected,
                        config,
                    } => {
                        let alignment =
                            Arc::new(analyze_alignment(&guide, &dub, &anchors, &protected));
                        let rendered = Arc::new(render_aligned_track(
                            &guide, &dub, &alignment, &config, &protected,
                        ));
                        let peaks = WaveformPeaks::from_buffer(&rendered, PEAK_BUCKETS);
                        let _ = evt_tx.send(WorkerEvent::Aligned {
                            job_id,
                            track_id,
                            alignment,
                            aligned_buf: rendered,
                            aligned_peaks: peaks,
                            elapsed_ms: t0.elapsed().as_millis() as u64,
                        });
                    }
                    WorkerJob::RerenderOnly {
                        job_id,
                        track_id,
                        guide,
                        dub,
                        alignment,
                        protected,
                        config,
                    } => {
                        let rendered = Arc::new(render_aligned_track(
                            &guide, &dub, &alignment, &config, &protected,
                        ));
                        let peaks = WaveformPeaks::from_buffer(&rendered, PEAK_BUCKETS);
                        let _ = evt_tx.send(WorkerEvent::Aligned {
                            job_id,
                            track_id,
                            alignment,
                            aligned_buf: rendered,
                            aligned_peaks: peaks,
                            elapsed_ms: t0.elapsed().as_millis() as u64,
                        });
                    }
                    WorkerJob::CreateDouble {
                        job_id,
                        source,
                        name,
                        timing_ms,
                        detune_cents,
                        formant_st,
                        vibrato_cents,
                        pan,
                    } => {
                        let dbl = Arc::new(create_artificial_double(
                            &source,
                            timing_ms,
                            detune_cents,
                            formant_st,
                            vibrato_cents,
                        ));
                        let peaks = WaveformPeaks::from_buffer(&dbl, PEAK_BUCKETS);
                        let _ = evt_tx.send(WorkerEvent::DoubleReady {
                            job_id,
                            name,
                            buffer: dbl,
                            peaks,
                            pan,
                        });
                    }
                }
            }
        });

        Self {
            tx: job_tx,
            rx: evt_rx,
        }
    }

    pub fn submit(&self, job: WorkerJob) {
        let _ = self.tx.send(job);
    }

    pub fn try_recv(&self) -> Option<WorkerEvent> {
        self.rx.try_recv().ok()
    }
}

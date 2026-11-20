use std::path::PathBuf;
use std::sync::Arc;

use phantom_vox_dsp::{AlignConfig, AlignmentResult, AudioBuffer, ProtectedRegion, SyncAnchor};

use crate::project::{PvoxTrack, SourceFileStatus, SourceFingerprint};
use crate::worker::{WaveformPeaks, PEAK_BUCKETS};

pub struct StudioTrack {
    pub id: u64,
    pub name: String,
    pub source_path: Option<PathBuf>,
    pub fingerprint: Option<SourceFingerprint>,
    pub file_status: SourceFileStatus,
    pub is_guide: bool,
    pub original: Arc<AudioBuffer>,
    pub orig_peaks: WaveformPeaks,
    pub aligned: Option<Arc<AudioBuffer>>,
    pub aligned_peaks: Option<WaveformPeaks>,
    pub alignment: Option<Arc<AlignmentResult>>,
    pub config: AlignConfig,
    pub anchors: Vec<SyncAnchor>,
    pub protected: Vec<ProtectedRegion>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub processing: bool,
    pub active_job_id: u64,
    pub color_idx: usize,
}

impl StudioTrack {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        name: String,
        source_path: Option<PathBuf>,
        fingerprint: Option<SourceFingerprint>,
        is_guide: bool,
        buffer: AudioBuffer,
        pan: f32,
        color_idx: usize,
    ) -> Self {
        let orig_peaks = WaveformPeaks::from_buffer(&buffer, PEAK_BUCKETS);
        let file_status = if source_path.is_some() {
            SourceFileStatus::Ok
        } else {
            SourceFileStatus::Virtual
        };
        Self {
            id,
            name,
            source_path,
            fingerprint,
            file_status,
            is_guide,
            original: Arc::new(buffer),
            orig_peaks,
            aligned: None,
            aligned_peaks: None,
            alignment: None,
            config: AlignConfig::default(),
            anchors: Vec::new(),
            protected: Vec::new(),
            gain: 0.9,
            pan,
            muted: false,
            solo: false,
            processing: false,
            active_job_id: 0,
            color_idx,
        }
    }

    pub fn to_pvox(&self) -> PvoxTrack {
        PvoxTrack {
            name: self.name.clone(),
            source_path: self.source_path.clone(),
            fingerprint: self.fingerprint.clone(),
            is_guide: self.is_guide,
            config: self.config,
            anchors: self.anchors.clone(),
            protected: self.protected.clone(),
            gain: self.gain,
            pan: self.pan,
            muted: self.muted,
            solo: self.solo,
        }
    }
}

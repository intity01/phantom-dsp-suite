//! Clean-room pure-Rust vocal alignment, pitch tracking, time-stretching, and real-time vocal doubler DSP.

mod aiff_io;
mod confidence;
mod demo;
mod doubler_dsp;
mod dtw_core;
mod dtw_pins;
mod formant;
mod live_align;
mod stretch_ops;

pub mod doubler;
pub mod dtw_align;
pub mod features;
pub mod pitch;
pub mod segmentation;
pub mod time_stretch;
pub mod wav_io;

pub use doubler::{
    DoublerParams, LiveSidechainAligner, VocalDoublerEngine, LIVE_ALIGN_LATENCY_SAMPLES,
};
pub use dtw_align::{
    analyze_alignment, AlignmentResult, ConfidenceLevel, PhraseReport, ProtectedRegion, SyncAnchor,
    TimeMap,
};
pub use features::{
    extract_features, hop_samples_for_sr, FeatureFrame, FeatureTrack, FRAME_LEN, HOP_SEC,
    NUM_MEL_BANDS, NUM_MFCC,
};
pub use pitch::{
    analyze_pitch, compute_target_pitch_ratio, PitchFrame, PitchMatchMode, MAX_PITCH_HZ,
    MIN_PITCH_HZ,
};
pub use segmentation::{classify_frames, extract_phrases, FrameClass, PhraseSpan};
pub use time_stretch::{
    create_artificial_double, generate_demo_vocal_stack, render_aligned_track, AlignConfig,
};
pub use wav_io::{read_audio_file, write_wav_f32, write_wav_i24, AudioBuffer};

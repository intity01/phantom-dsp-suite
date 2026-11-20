//! Unit tests for `.pvox` project roundtrip, 32-step Undo/Redo, missing/modified source
//! file detection, DAW-synced batch WAV export, and stereo A/B + Mono Check playback mixer.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use phantom_vox_dsp::{
    read_audio_file, write_wav_f32, AlignConfig, AudioBuffer, PitchMatchMode, ProtectedRegion,
    SyncAnchor,
};

use crate::playback_mix::{render_mix_block, PlaybackMixData, PlaybackTrack, TransportShared};
use crate::project::{
    compute_source_fingerprint, export_aligned_stems, load_project, save_project,
    verify_source_file, ExportFormat, ExportStem, HistoryStack, PvoxProject, PvoxTrack,
    SourceFileStatus, MAX_UNDO_STEPS,
};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "phantom_vox_studio_{tag}_{}_{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn pvox_project_json_roundtrip_and_non_destructive_reference() {
    let dir = unique_temp_dir("roundtrip");
    let wav_path = dir.join("guide.wav");
    let orig_buf = AudioBuffer::new_mono(48_000, vec![0.1, -0.2, 0.3, -0.4, 0.25]);
    write_wav_f32(&wav_path, &orig_buf).unwrap();
    let fp = compute_source_fingerprint(&wav_path).unwrap();

    let proj = PvoxProject {
        schema_version: 1,
        sample_rate: 48_000,
        reference_index: 0,
        tracks: vec![
            PvoxTrack {
                name: "Guide Lead".into(),
                source_path: Some(wav_path.clone()),
                fingerprint: Some(fp.clone()),
                is_guide: true,
                config: AlignConfig::default(),
                anchors: vec![],
                protected: vec![],
                gain: 1.0,
                pan: 0.0,
                muted: false,
                solo: false,
            },
            PvoxTrack {
                name: "Dub L".into(),
                source_path: Some(wav_path.clone()),
                fingerprint: Some(fp.clone()),
                is_guide: false,
                config: AlignConfig {
                    timing_strength: 0.85,
                    pitch_strength: 0.60,
                    level_strength: 0.40,
                    pitch_mode: PitchMatchMode::MatchUnisonAndTuneHarmony,
                    unison_tolerance_cents: 110.0,
                    protect_sibilance: true,
                    formant_preserve: true,
                },
                anchors: vec![SyncAnchor {
                    guide_sec: 1.25,
                    dub_sec: 1.31,
                }],
                protected: vec![ProtectedRegion {
                    start_sec: 2.0,
                    end_sec: 2.5,
                }],
                gain: 0.88,
                pan: -0.65,
                muted: false,
                solo: false,
            },
        ],
    };

    let pvox_path = dir.join("session.pvox");
    save_project(&pvox_path, &proj).unwrap();

    // Ensure JSON is lightweight (does NOT embed raw WAV PCM payload) and roundtrips cleanly.
    let raw_json = fs::read_to_string(&pvox_path).unwrap();
    assert!(raw_json.len() < 4096);
    let loaded = load_project(&pvox_path).unwrap();
    assert_eq!(loaded, proj);
    assert_eq!(
        verify_source_file(Some(&wav_path), Some(&fp)),
        SourceFileStatus::Ok
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn undo_redo_history_32_step_cap_and_branch_reset() {
    let mut history = HistoryStack::default();
    for step in 0..40u32 {
        let mut p = PvoxProject::default();
        p.sample_rate = 44_100 + step;
        history.push(p);
    }
    assert_eq!(history.undo_len(), MAX_UNDO_STEPS);
    assert!(history.can_undo());
    assert!(!history.can_redo());

    let mut cur = PvoxProject::default();
    cur.sample_rate = 99_999;
    let prev = history.undo(cur.clone()).unwrap();
    assert_eq!(prev.sample_rate, 44_100 + 39);
    assert!(history.can_redo());

    let redone = history.redo(prev.clone()).unwrap();
    assert_eq!(redone.sample_rate, 99_999);

    // Pushing a new snapshot clears the redo branch.
    let back = history.undo(redone).unwrap();
    assert!(history.can_redo());
    history.push(back);
    assert!(!history.can_redo());
}

#[test]
fn missing_and_modified_source_file_warning_detection() {
    let dir = unique_temp_dir("integrity");
    let wav_path = dir.join("dub.wav");
    write_wav_f32(&wav_path, &AudioBuffer::new_mono(48_000, vec![0.2; 64])).unwrap();
    let fp = compute_source_fingerprint(&wav_path).unwrap();

    assert_eq!(
        verify_source_file(Some(&wav_path), Some(&fp)),
        SourceFileStatus::Ok
    );

    // Modify file on disk -> triggers Modified warning.
    write_wav_f32(&wav_path, &AudioBuffer::new_mono(48_000, vec![0.4; 128])).unwrap();
    assert!(matches!(
        verify_source_file(Some(&wav_path), Some(&fp)),
        SourceFileStatus::Modified(_)
    ));

    // Delete file -> triggers Missing warning.
    fs::remove_file(&wav_path).unwrap();
    assert_eq!(
        verify_source_file(Some(&wav_path), Some(&fp)),
        SourceFileStatus::Missing
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_time_aligned_wav_export_preserves_timeline_sync() {
    let dir = unique_temp_dir("export");
    let timeline_len = 4800usize; // 0.10 s at 48 kHz
    let short_dub = AudioBuffer::new_mono(48_000, vec![0.25; 2400]);
    let stereo_dub = AudioBuffer::new_stereo(48_000, vec![0.3; 3000], vec![-0.3; 3000]);

    let stems = [
        ExportStem {
            name: "Dub Left",
            buffer: &short_dub,
        },
        ExportStem {
            name: "Harmony 3rd",
            buffer: &stereo_dub,
        },
    ];

    let i24_dir = dir.join("wav24");
    let files_24 =
        export_aligned_stems(&i24_dir, &stems, timeline_len, ExportFormat::Wav24).unwrap();
    assert_eq!(files_24.len(), 2);
    for f in &files_24 {
        let loaded = read_audio_file(f).unwrap();
        assert_eq!(loaded.sample_rate, 48_000);
        assert_eq!(loaded.len(), timeline_len);
    }

    let f32_dir = dir.join("wav32f");
    let files_32 =
        export_aligned_stems(&f32_dir, &stems, timeline_len, ExportFormat::Wav32F).unwrap();
    let loaded_0 = read_audio_file(&files_32[0]).unwrap();
    assert_eq!(loaded_0.len(), timeline_len);
    assert!((loaded_0.samples_l[0] - 0.25).abs() < 1e-5);
    assert_eq!(loaded_0.samples_l[timeline_len - 1], 0.0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn playback_mixer_instant_ab_toggle_and_mono_check() {
    let orig = Arc::new(AudioBuffer::new_mono(48_000, vec![0.2; 64]));
    let aligned = Arc::new(AudioBuffer::new_mono(48_000, vec![0.6; 64]));
    let mix = PlaybackMixData {
        sample_rate: 48_000,
        total_samples: 64,
        tracks: vec![PlaybackTrack {
            original: orig,
            aligned: Some(aligned),
            gain: 1.0,
            pan: -1.0, // Hard left
            muted: false,
            solo: false,
            is_guide: false,
        }],
    };
    let ctrl = TransportShared::new(48_000);
    ctrl.playing.store(true, Ordering::Relaxed);

    // A mode (Original): hard-left pan -> L > 0, R == 0.
    ctrl.audition_b.store(false, Ordering::Relaxed);
    let mut out_a = [0i16; 16];
    render_mix_block(&mix, &ctrl, &mut out_a);
    assert!(out_a[0] > 3000 && out_a[1] == 0);

    // B mode (Aligned): higher amplitude than A.
    ctrl.playhead.store(0, Ordering::Relaxed);
    ctrl.audition_b.store(true, Ordering::Relaxed);
    let mut out_b = [0i16; 16];
    render_mix_block(&mix, &ctrl, &mut out_b);
    assert!(out_b[0] > out_a[0] * 2);

    // Mono Check folds hard-left stereo cleanly into L == R.
    ctrl.playhead.store(0, Ordering::Relaxed);
    ctrl.mono_check.store(true, Ordering::Relaxed);
    let mut out_m = [0i16; 16];
    render_mix_block(&mix, &ctrl, &mut out_m);
    assert_eq!(out_m[0], out_m[1]);
    assert!(out_m[0] > 0);
}

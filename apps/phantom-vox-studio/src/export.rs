use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use phantom_vox_dsp::{write_wav_f32, write_wav_i24, AudioBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Wav24,
    Wav32F,
}

pub struct ExportStem<'a> {
    pub name: &'a str,
    pub buffer: &'a AudioBuffer,
}

/// Exports time-aligned WAV stems starting strictly at `0.0s` padded to uniform DAW `timeline_samples`.
pub fn export_aligned_stems(
    out_dir: &Path,
    stems: &[ExportStem<'_>],
    timeline_samples: usize,
    format: ExportFormat,
) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(out_dir)?;
    let mut written = Vec::with_capacity(stems.len());
    for (idx, stem) in stems.iter().enumerate() {
        let safe = sanitize_filename(stem.name);
        let file = out_dir.join(format!("{:02}_{}_aligned.wav", idx + 1, safe));
        let synced = pad_to_timeline(stem.buffer, timeline_samples);
        match format {
            ExportFormat::Wav24 => write_wav_i24(&file, &synced)?,
            ExportFormat::Wav32F => write_wav_f32(&file, &synced)?,
        }
        written.push(file);
    }
    Ok(written)
}

fn pad_to_timeline(buf: &AudioBuffer, target_len: usize) -> AudioBuffer {
    let len = target_len.max(buf.len());
    let mut l = buf.samples_l.clone();
    l.resize(len, 0.0);
    if buf.channels >= 2 && !buf.samples_r.is_empty() {
        let mut r = buf.samples_r.clone();
        r.resize(len, 0.0);
        AudioBuffer::new_stereo(buf.sample_rate, l, r)
    } else {
        AudioBuffer::new_mono(buf.sample_rate, l)
    }
}

fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "track".to_owned()
    } else {
        s
    }
}

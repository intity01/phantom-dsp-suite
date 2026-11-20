//! Pure-Rust WAV & AIFF/AIFC audio I/O and `AudioBuffer` container.

use std::fs;
use std::io;
use std::path::Path;

use crate::aiff_io::parse_aiff;

/// Planar mono or stereo 32-bit floating-point audio buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
}

impl AudioBuffer {
    pub fn new_mono(sample_rate: u32, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            channels: 1,
            samples_l: samples,
            samples_r: Vec::new(),
        }
    }

    pub fn new_stereo(sample_rate: u32, mut samples_l: Vec<f32>, mut samples_r: Vec<f32>) -> Self {
        let len = samples_l.len().max(samples_r.len());
        samples_l.resize(len, 0.0);
        samples_r.resize(len, 0.0);
        Self {
            sample_rate,
            channels: 2,
            samples_l,
            samples_r,
        }
    }

    pub fn len(&self) -> usize {
        self.samples_l.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples_l.is_empty()
    }

    pub fn duration_sec(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples_l.len() as f32 / self.sample_rate as f32
        }
    }

    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels <= 1 || self.samples_r.is_empty() {
            return self.samples_l.clone();
        }
        self.samples_l
            .iter()
            .zip(self.samples_r.iter())
            .map(|(&l, &r)| 0.5 * (l + r))
            .collect()
    }

    pub fn peak_db(&self) -> f32 {
        let mut peak = 0.0_f32;
        for &s in &self.samples_l {
            if s.is_finite() {
                peak = peak.max(s.abs());
            }
        }
        for &s in &self.samples_r {
            if s.is_finite() {
                peak = peak.max(s.abs());
            }
        }
        if peak <= 1e-6 {
            -120.0
        } else {
            20.0 * peak.log10()
        }
    }
}

fn err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Reads a WAV (16/24/32-bit PCM or 32-bit IEEE float) or AIFF/AIFC (16/24-bit PCM) file.
pub fn read_audio_file(path: &Path) -> io::Result<AudioBuffer> {
    let data = fs::read(path)?;
    if data.len() < 12 {
        return Err(err("audio file too small"));
    }
    match &data[0..4] {
        b"RIFF" => parse_wav(&data),
        b"FORM" => parse_aiff(&data),
        _ => Err(err(
            "unrecognized audio container (expected RIFF WAV or FORM AIFF)",
        )),
    }
}

fn parse_wav(data: &[u8]) -> io::Result<AudioBuffer> {
    if &data[8..12] != b"WAVE" {
        return Err(err("RIFF file is not WAVE"));
    }
    let mut format_tag: u16 = 0;
    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut pcm_data: Option<&[u8]> = None;

    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        pos += 8;
        if pos + size > data.len() {
            break;
        }
        let body = &data[pos..pos + size];
        match id {
            b"fmt " if body.len() >= 16 => {
                format_tag = u16::from_le_bytes([body[0], body[1]]);
                channels = u16::from_le_bytes([body[2], body[3]]);
                sample_rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                bits_per_sample = u16::from_le_bytes([body[14], body[15]]);
                if format_tag == 0xFFFE && body.len() >= 40 {
                    format_tag = u16::from_le_bytes([body[24], body[25]]);
                }
            }
            b"data" => pcm_data = Some(body),
            _ => {}
        }
        pos += size + (size & 1);
    }

    if channels == 0 || sample_rate == 0 {
        return Err(err("missing WAV fmt chunk"));
    }
    let raw = pcm_data.ok_or_else(|| err("missing WAV data chunk"))?;
    let bytes_per_sample = match (format_tag, bits_per_sample) {
        (1, 16) => 2usize,
        (1, 24) => 3usize,
        (1, 32) | (3, 32) => 4usize,
        _ => return Err(err("unsupported WAV format or bit depth")),
    };

    let frame_bytes = bytes_per_sample * channels as usize;
    let num_frames = raw.len() / frame_bytes;
    let out_ch = channels.min(2);
    let mut left = Vec::with_capacity(num_frames);
    let mut right = if out_ch == 2 {
        Vec::with_capacity(num_frames)
    } else {
        Vec::new()
    };

    for f in 0..num_frames {
        let base = f * frame_bytes;
        for c in 0..out_ch as usize {
            let b = &raw[base + c * bytes_per_sample..];
            let s = decode_wav_sample(b, format_tag, bits_per_sample);
            if c == 0 {
                left.push(s);
            } else {
                right.push(s);
            }
        }
    }

    if out_ch == 2 {
        Ok(AudioBuffer::new_stereo(sample_rate, left, right))
    } else {
        Ok(AudioBuffer::new_mono(sample_rate, left))
    }
}

fn decode_wav_sample(b: &[u8], fmt: u16, bits: u16) -> f32 {
    match (fmt, bits) {
        (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
        (1, 24) => {
            let v = ((b[2] as i8 as i32) << 16) | ((b[1] as i32) << 8) | (b[0] as i32);
            v as f32 / 8_388_608.0
        }
        (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        (3, 32) => {
            let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if v.is_finite() {
                v
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

/// Writes an `AudioBuffer` as a 32-bit IEEE float WAV file.
pub fn write_wav_f32(path: &Path, buf: &AudioBuffer) -> io::Result<()> {
    let ch = if buf.channels >= 2 && !buf.samples_r.is_empty() {
        2u16
    } else {
        1u16
    };
    let n = buf.samples_l.len();
    let data_bytes = n * ch as usize * 4;
    let mut out = Vec::with_capacity(44 + data_bytes);
    write_wav_header(&mut out, 3, ch, buf.sample_rate, 32, data_bytes as u32);
    for i in 0..n {
        let l = sanitize(buf.samples_l[i]);
        out.extend_from_slice(&l.to_le_bytes());
        if ch == 2 {
            let r = sanitize(*buf.samples_r.get(i).unwrap_or(&0.0));
            out.extend_from_slice(&r.to_le_bytes());
        }
    }
    fs::write(path, out)
}

/// Writes an `AudioBuffer` as a 24-bit signed integer PCM WAV file.
pub fn write_wav_i24(path: &Path, buf: &AudioBuffer) -> io::Result<()> {
    let ch = if buf.channels >= 2 && !buf.samples_r.is_empty() {
        2u16
    } else {
        1u16
    };
    let n = buf.samples_l.len();
    let data_bytes = n * ch as usize * 3;
    let pad = data_bytes & 1;
    let mut out = Vec::with_capacity(44 + data_bytes + pad);
    write_wav_header(&mut out, 1, ch, buf.sample_rate, 24, data_bytes as u32);
    for i in 0..n {
        out.extend_from_slice(&f32_to_i24_le(buf.samples_l[i]));
        if ch == 2 {
            out.extend_from_slice(&f32_to_i24_le(*buf.samples_r.get(i).unwrap_or(&0.0)));
        }
    }
    if pad == 1 {
        out.push(0);
    }
    fs::write(path, out)
}

fn write_wav_header(out: &mut Vec<u8>, fmt_tag: u16, ch: u16, sr: u32, bits: u16, data_bytes: u32) {
    let block_align = ch * (bits / 8);
    let byte_rate = sr * block_align as u32;
    let riff_size = 36u32
        .saturating_add(data_bytes)
        .saturating_add(data_bytes & 1);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&fmt_tag.to_le_bytes());
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&sr.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
}

#[inline]
fn sanitize(s: f32) -> f32 {
    if s.is_finite() {
        s
    } else {
        0.0
    }
}

#[inline]
fn f32_to_i24_le(s: f32) -> [u8; 3] {
    let x = sanitize(s).clamp(-1.0, 1.0);
    let v = (x * 8_388_607.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
    [
        (v & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
    ]
}

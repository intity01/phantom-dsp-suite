//! Pure-Rust AIFF / AIFC big-endian audio decoder (16-bit and 24-bit PCM).

use std::io;

use crate::wav_io::AudioBuffer;

fn err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Parses an 80-bit IEEE 754 extended-precision big-endian float (AIFF sample rate).
fn parse_extended_f80(b: &[u8]) -> u32 {
    if b.len() < 10 {
        return 0;
    }
    let exp = (((b[0] & 0x7F) as i32) << 8) | (b[1] as i32);
    let mant = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    if exp == 0 && mant == 0 {
        return 0;
    }
    let val = (mant as f64) * 2.0_f64.powi(exp - 16383 - 63);
    if val.is_finite() && val > 0.0 {
        val.round() as u32
    } else {
        0
    }
}

pub(crate) fn parse_aiff(data: &[u8]) -> io::Result<AudioBuffer> {
    if data.len() < 12 || &data[0..4] != b"FORM" {
        return Err(err("invalid AIFF header"));
    }
    let form_type = &data[8..12];
    let is_aifc = form_type == b"AIFC";
    if form_type != b"AIFF" && !is_aifc {
        return Err(err("unsupported FORM container (expected AIFF or AIFC)"));
    }

    let mut channels: u16 = 0;
    let mut sample_size: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut little_endian = false;
    let mut ssnd_slice: Option<&[u8]> = None;

    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        pos += 8;
        if pos + size > data.len() {
            break;
        }
        let body = &data[pos..pos + size];
        match id {
            b"COMM" if body.len() >= 18 => {
                channels = u16::from_be_bytes([body[0], body[1]]);
                sample_size = u16::from_be_bytes([body[6], body[7]]);
                sample_rate = parse_extended_f80(&body[8..18]);
                if is_aifc && body.len() >= 22 {
                    let comp = &body[18..22];
                    if comp == b"sowt" {
                        little_endian = true;
                    } else if comp != b"NONE" {
                        return Err(err("unsupported AIFC compression codec"));
                    }
                }
            }
            b"SSND" if body.len() >= 8 => {
                let offset = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                let start = 8usize.saturating_add(offset);
                if start <= body.len() {
                    ssnd_slice = Some(&body[start..]);
                }
            }
            _ => {}
        }
        pos += size + (size & 1);
    }

    if channels == 0 || sample_rate == 0 {
        return Err(err("missing AIFF COMM chunk"));
    }
    let pcm = ssnd_slice.ok_or_else(|| err("missing AIFF SSND chunk"))?;
    let bytes_per_sample = match sample_size {
        16 => 2usize,
        24 => 3usize,
        _ => return Err(err("unsupported AIFF bit depth (expected 16 or 24)")),
    };
    let frame_bytes = bytes_per_sample * channels as usize;
    let num_frames = pcm.len() / frame_bytes;
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
            let b = &pcm[base + c * bytes_per_sample..];
            let s = decode_aiff_sample(b, sample_size, little_endian);
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

fn decode_aiff_sample(b: &[u8], bits: u16, little_endian: bool) -> f32 {
    match (bits, little_endian) {
        (16, false) => i16::from_be_bytes([b[0], b[1]]) as f32 / 32768.0,
        (16, true) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
        (24, false) => {
            let v = ((b[0] as i8 as i32) << 16) | ((b[1] as i32) << 8) | (b[2] as i32);
            v as f32 / 8_388_608.0
        }
        (24, true) => {
            let v = ((b[2] as i8 as i32) << 16) | ((b[1] as i32) << 8) | (b[0] as i32);
            v as f32 / 8_388_608.0
        }
        _ => 0.0,
    }
}

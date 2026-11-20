//! CPU rasterizers for the brand marks. The logo and wordmark are rendered at the screen's
//! exact pixel size, so they stay crisp and evenly weighted instead of being shrunk on the GPU
//! (which aliases the icon) or stitched from separate egui lines (which breaks the joins).

use eframe::egui::{Color32, ColorImage};
use std::f32::consts::PI;

/// Area-averaged resample of an unmultiplied RGBA image to `dw` x `dh` pixels.
pub fn resample_rgba(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> ColorImage {
    let (fx, fy) = (sw as f32 / dw as f32, sh as f32 / dh as f32);
    let mut pixels = Vec::with_capacity(dw * dh);
    for y in 0..dh {
        let (y0, y1) = (y as f32 * fy, (y + 1) as f32 * fy);
        for x in 0..dw {
            let (x0, x1) = (x as f32 * fx, (x + 1) as f32 * fx);
            let mut acc = [0.0_f32; 4];
            let mut wsum = 0.0_f32;
            for sy in (y0.floor() as usize)..(y1.ceil() as usize).min(sh) {
                let wy = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                for sx in (x0.floor() as usize)..(x1.ceil() as usize).min(sw) {
                    let wx = (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                    let w = wx * wy;
                    let i = (sy * sw + sx) * 4;
                    let a = src[i + 3] as f32 / 255.0;
                    for (c, slot) in acc.iter_mut().take(3).enumerate() {
                        *slot += src[i + c] as f32 * a * w;
                    }
                    acc[3] += a * w;
                    wsum += w;
                }
            }
            let inv = 1.0 / wsum.max(1e-6);
            let q = |v: f32| (v * inv).round().clamp(0.0, 255.0) as u8;
            pixels.push(Color32::from_rgba_premultiplied(
                q(acc[0]),
                q(acc[1]),
                q(acc[2]),
                q(acc[3] * 255.0),
            ));
        }
    }
    ColorImage {
        size: [dw, dh],
        pixels,
    }
}

#[derive(Clone, Copy)]
enum Mark {
    Seg(f32, f32, f32, f32),
    Circle(f32, f32, f32),
    /// Center, radius, and the start/end angles (y points down, as in SVG).
    Arc(f32, f32, f32, f32, f32),
}

/// The `phantom` wordmark from the design's SVG (`viewBox 0 0 198 56`, stroke 4.6, round caps).
const WORDMARK: &[Mark] = &[
    // p
    Mark::Seg(4.0, 21.0, 4.0, 52.0),
    Mark::Circle(14.0, 30.0, 10.0),
    // h
    Mark::Seg(32.0, 6.0, 32.0, 40.0),
    Mark::Arc(41.0, 30.0, 9.0, PI, 2.0 * PI),
    Mark::Seg(50.0, 30.0, 50.0, 40.0),
    // a
    Mark::Circle(64.0, 30.0, 10.0),
    Mark::Seg(74.0, 20.0, 74.0, 36.0),
    Mark::Arc(78.0, 36.0, 4.0, 0.5 * PI, PI),
    // n
    Mark::Seg(84.0, 40.0, 84.0, 30.0),
    Mark::Arc(93.0, 30.0, 9.0, PI, 2.0 * PI),
    Mark::Seg(102.0, 30.0, 102.0, 40.0),
    // t
    Mark::Seg(112.0, 10.0, 112.0, 34.0),
    Mark::Arc(118.0, 34.0, 6.0, 0.5 * PI, PI),
    Mark::Seg(118.0, 40.0, 120.0, 40.0),
    Mark::Seg(106.0, 20.0, 120.0, 20.0),
    // o
    Mark::Circle(134.0, 30.0, 10.0),
    // m
    Mark::Seg(152.0, 40.0, 152.0, 30.0),
    Mark::Arc(160.0, 30.0, 8.0, PI, 2.0 * PI),
    Mark::Seg(168.0, 30.0, 168.0, 40.0),
    Mark::Arc(176.0, 30.0, 8.0, PI, 2.0 * PI),
    Mark::Seg(184.0, 30.0, 184.0, 40.0),
];
const VIEW_W: f32 = 198.0;
const VIEW_H: f32 = 56.0;
const STROKE: f32 = 4.6;
const DOT: (f32, f32, f32) = (191.0, 12.0, 4.5);

/// Height in pixels of a wordmark rendered `width_px` wide.
pub fn wordmark_height_px(width_px: usize) -> usize {
    (width_px as f32 * VIEW_H / VIEW_W).ceil() as usize
}

/// Renders the wordmark with exact anti-aliasing: each pixel's coverage comes from its distance
/// to the stroke centerlines, which also gives round caps and joins for free.
pub fn rasterize_wordmark(width_px: usize, text: Color32, accent: Color32) -> ColorImage {
    let k = width_px as f32 / VIEW_W;
    let h = wordmark_height_px(width_px);
    let half = STROKE * 0.5 * k;
    let mut pixels = Vec::with_capacity(width_px * h);
    for y in 0..h {
        for x in 0..width_px {
            let (u, v) = ((x as f32 + 0.5) / k, (y as f32 + 0.5) / k);
            let d = WORDMARK
                .iter()
                .map(|&m| mark_distance(m, u, v))
                .fold(f32::MAX, f32::min);
            let text_cov = (half - d * k + 0.5).clamp(0.0, 1.0);
            let dot_d = ((u - DOT.0).hypot(v - DOT.1) - DOT.2) * k;
            let dot_cov = (0.5 - dot_d).clamp(0.0, 1.0);
            pixels.push(if dot_cov > 0.0 {
                accent.gamma_multiply(dot_cov)
            } else {
                text.gamma_multiply(text_cov)
            });
        }
    }
    ColorImage {
        size: [width_px, h],
        pixels,
    }
}

fn mark_distance(m: Mark, u: f32, v: f32) -> f32 {
    match m {
        Mark::Seg(x1, y1, x2, y2) => {
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((u - x1) * dx + (v - y1) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (u - (x1 + t * dx)).hypot(v - (y1 + t * dy))
        }
        Mark::Circle(cx, cy, r) => ((u - cx).hypot(v - cy) - r).abs(),
        Mark::Arc(cx, cy, r, a0, a1) => {
            let mut ang = (v - cy).atan2(u - cx);
            if ang < 0.0 {
                ang += 2.0 * PI;
            }
            if ang >= a0.min(a1) && ang <= a0.max(a1) {
                ((u - cx).hypot(v - cy) - r).abs()
            } else {
                let end = |a: f32| (u - (cx + r * a.cos())).hypot(v - (cy + r * a.sin()));
                end(a0).min(end(a1))
            }
        }
    }
}

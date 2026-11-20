//! Procedural GPU-cached plugin artwork textures (Rings, Conic, Grid, Wave + glowing orb).

use crate::manifest::{ArtPattern, AVAILABLE_PLUGINS};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, TextureHandle, TextureOptions, Ui, Vec2,
};
use std::collections::HashMap;
use std::f32::consts::PI;

const ART_W: usize = 240;
const ART_H: usize = 140;

pub struct ArtCache {
    textures: HashMap<&'static str, TextureHandle>,
}

impl ArtCache {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut textures = HashMap::new();
        for p in AVAILABLE_PLUGINS {
            let img = generate_artwork(p.hue, p.pattern);
            let tex = ctx.load_texture(format!("art_{}", p.id), img, TextureOptions::LINEAR);
            textures.insert(p.id, tex);
        }
        Self { textures }
    }

    pub fn draw_art(&self, ui: &mut Ui, id: &str, size: Vec2, corner_radius: u8, dimmed: bool) {
        let Some(tex) = self.textures.get(id) else {
            return;
        };
        let tint = if dimmed {
            Color32::from_rgba_unmultiplied(255, 255, 255, 130)
        } else {
            Color32::WHITE
        };
        ui.add(
            egui::Image::new((tex.id(), size))
                .fit_to_exact_size(size)
                .corner_radius(CornerRadius::same(corner_radius))
                .tint(tint),
        );
    }
}

fn hsl_to_rgb(h_deg: f32, s: f32, l: f32) -> [f32; 3] {
    let h = ((h_deg % 360.0) + 360.0) % 360.0 / 60.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c * 0.5;
    [(r1 + m) * 255.0, (g1 + m) * 255.0, (b1 + m) * 255.0]
}

fn blend(dst: &mut [f32; 3], src: [f32; 3], alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + src[i] * a;
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn generate_artwork(hue: f32, pattern: ArtPattern) -> ColorImage {
    let bg_inner = hsl_to_rgb(hue, 0.55, 0.20);
    let bg_outer = hsl_to_rgb(hue, 0.45, 0.08);
    let pat_col = match pattern {
        ArtPattern::Rings | ArtPattern::Conic => hsl_to_rgb(hue, 0.85, 0.70),
        ArtPattern::Grid => hsl_to_rgb(hue, 0.90, 0.74),
        ArtPattern::Wave => hsl_to_rgb(hue, 0.90, 0.72),
    };
    let orb_core = hsl_to_rgb(hue, 0.95, 0.80);
    let orb_glow = hsl_to_rgb(hue, 0.95, 0.62);

    let w = ART_W as f32;
    let h = ART_H as f32;
    let mut pixels = Vec::with_capacity(ART_W * ART_H * 4);

    for py in 0..ART_H {
        let y = py as f32 + 0.5;
        for px in 0..ART_W {
            let x = px as f32 + 0.5;

            // Radial background centered at (0.50, 0.62)
            let dx_bg = (x / w) - 0.50;
            let dy_bg = (y / h) - 0.62;
            let t_bg = ((dx_bg * dx_bg + dy_bg * dy_bg).sqrt() / 0.52).clamp(0.0, 1.0);
            let mut col = [
                bg_inner[0] * (1.0 - t_bg) + bg_outer[0] * t_bg,
                bg_inner[1] * (1.0 - t_bg) + bg_outer[1] * t_bg,
                bg_inner[2] * (1.0 - t_bg) + bg_outer[2] * t_bg,
            ];

            let cx = w * 0.50;
            let cy = h * if matches!(pattern, ArtPattern::Rings) {
                0.60
            } else {
                0.55
            };
            let dx = x - cx;
            let dy = y - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let pat_alpha = match pattern {
                ArtPattern::Rings => {
                    let ring = (dist % 10.5) - 9.0;
                    let on = if ring > 0.0 {
                        1.0 - (ring - 0.75).abs() / 0.75
                    } else {
                        0.0
                    };
                    let mask = 1.0 - smoothstep(h * 0.28, h * 0.72, dist);
                    on.max(0.0) * mask * 0.65
                }
                ArtPattern::Conic => {
                    let deg = ((dy.atan2(dx) * 180.0 / PI) - 10.0 + 360.0) % 18.0;
                    let on = if deg < 6.0 { 1.0 } else { 0.0 };
                    let inner = smoothstep(12.0, 16.0, dist);
                    let outer = 1.0 - smoothstep(h * 0.34, h * 0.48, dist);
                    on * inner * outer * 0.70
                }
                ArtPattern::Grid => {
                    let gx = ((x % 11.0) - 5.5).abs();
                    let gy = ((y % 11.0) - 5.5).abs();
                    let d_dot = (gx * gx + gy * gy).sqrt();
                    let dot = 1.0 - smoothstep(1.2, 2.2, d_dot);
                    let mask = 1.0 - smoothstep(h * 0.22, h * 0.50, dist);
                    dot * mask * 0.85
                }
                ArtPattern::Wave => {
                    let bar = if (x % 7.0) < 2.2 { 1.0 } else { 0.0 };
                    let ex = dx / (w * 0.42);
                    let ey = dy / (h * 0.26);
                    let mask = 1.0 - smoothstep(0.30, 0.78, (ex * ex + ey * ey).sqrt());
                    bar * mask * 0.80
                }
            };
            blend(&mut col, pat_col, pat_alpha);

            // Center glowing orb at (0.50, 0.55)
            let odx = x - w * 0.50;
            let ody = y - h * 0.55;
            let od = (odx * odx + ody * ody).sqrt();
            let bloom = (1.0 - smoothstep(2.0, 28.0, od)).powf(1.8) * 0.75;
            blend(&mut col, orb_glow, bloom);
            let core = 1.0 - smoothstep(3.8, 5.4, od);
            blend(&mut col, orb_core, core);

            pixels.push(col[0].clamp(0.0, 255.0) as u8);
            pixels.push(col[1].clamp(0.0, 255.0) as u8);
            pixels.push(col[2].clamp(0.0, 255.0) as u8);
            pixels.push(255);
        }
    }

    ColorImage::from_rgba_unmultiplied([ART_W, ART_H], &pixels)
}

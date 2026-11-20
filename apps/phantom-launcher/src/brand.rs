//! Embedded Phantom Orb icon texture, Windows viewport IconData, and connected wordmark.

use eframe::egui::{
    self, Color32, ColorImage, IconData, Painter, Pos2, Rect, Sense, Shape, Stroke, TextureHandle,
    TextureOptions, Ui, Vec2,
};
use std::f32::consts::PI;

const ICON_256_PNG: &[u8] = include_bytes!("../assets/icons/icon-256.png");

fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let buf_size = reader.output_buffer_size()?;
    let mut raw = vec![0u8; buf_size];
    let info = reader.next_frame(&mut raw).ok()?;
    let data = &raw[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((rgba, info.width, info.height))
}

/// Decodes the embedded 256x256 PNG icon into `egui::IconData` for the native OS window.
pub fn load_window_icon() -> Option<IconData> {
    let (rgba, width, height) = decode_png_rgba(ICON_256_PNG)?;
    Some(IconData {
        rgba,
        width,
        height,
    })
}

pub struct BrandAssets {
    pub logo_tex: TextureHandle,
}

impl BrandAssets {
    pub fn load(ctx: &egui::Context) -> Self {
        let (rgba, w, h) =
            decode_png_rgba(ICON_256_PNG).unwrap_or_else(|| (vec![10, 132, 255, 255], 1, 1));
        let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let logo_tex = ctx.load_texture("phantom_launcher_logo", image, TextureOptions::LINEAR);
        Self { logo_tex }
    }

    pub fn draw_logo_icon(&self, ui: &mut Ui, size: f32) {
        ui.add(
            egui::Image::new((self.logo_tex.id(), Vec2::splat(size)))
                .fit_to_exact_size(Vec2::splat(size)),
        );
    }
}

/// Draws the connected monoline `phantom` wordmark (`198:56` viewBox) with round terminals.
pub fn draw_wordmark(ui: &mut Ui, width: f32, text_color: Color32, accent_color: Color32) {
    let height = width * (56.0 / 198.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_wordmark(ui.painter(), rect, text_color, accent_color);
}

fn paint_wordmark(painter: &Painter, rect: Rect, text_color: Color32, accent_color: Color32) {
    let sx = rect.width() / 198.0;
    let sy = rect.height() / 56.0;
    let s = sx.min(sy);
    let sw = (4.6 * s).max(1.2);
    let stroke = Stroke::new(sw, text_color);
    let pt = |x: f32, y: f32| Pos2::new(rect.left() + x * sx, rect.top() + y * sy);
    let cap = |p: Pos2| painter.circle_filled(p, sw * 0.5, text_color);
    let seg = |x1: f32, y1: f32, x2: f32, y2: f32| {
        let a = pt(x1, y1);
        let b = pt(x2, y2);
        painter.line_segment([a, b], stroke);
        cap(a);
        cap(b);
    };
    let draw_arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let steps = 14;
        let pts: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
            painter.add(Shape::line(pts, stroke));
            cap(first);
            cap(last);
        }
    };

    // p
    seg(4.0, 21.0, 4.0, 52.0);
    painter.circle_stroke(pt(14.0, 30.0), 10.0 * s, stroke);
    // h
    seg(32.0, 6.0, 32.0, 40.0);
    draw_arc(41.0, 30.0, 9.0, PI, 2.0 * PI);
    seg(50.0, 30.0, 50.0, 40.0);
    // a
    painter.circle_stroke(pt(64.0, 30.0), 10.0 * s, stroke);
    seg(74.0, 20.0, 74.0, 36.0);
    draw_arc(78.0, 36.0, 4.0, PI, PI * 0.5);
    // n
    seg(84.0, 40.0, 84.0, 30.0);
    draw_arc(93.0, 30.0, 9.0, PI, 2.0 * PI);
    seg(102.0, 30.0, 102.0, 40.0);
    // t
    seg(112.0, 10.0, 112.0, 34.0);
    draw_arc(118.0, 34.0, 6.0, PI, PI * 0.5);
    seg(118.0, 40.0, 120.0, 40.0);
    seg(106.0, 20.0, 120.0, 20.0);
    // o
    painter.circle_stroke(pt(134.0, 30.0), 10.0 * s, stroke);
    // m
    seg(152.0, 40.0, 152.0, 30.0);
    draw_arc(160.0, 30.0, 8.0, PI, 2.0 * PI);
    seg(168.0, 30.0, 168.0, 40.0);
    draw_arc(176.0, 30.0, 8.0, PI, 2.0 * PI);
    seg(184.0, 30.0, 184.0, 40.0);
    // Accent dot
    painter.circle_filled(pt(191.0, 12.0), 4.5 * s, accent_color);
}

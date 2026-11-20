//! Shared Phantom Orb Icon & Connected Monoline Wordmark for DSP Plugins and Launcher.

use nih_plug_egui::egui::{
    self, Color32, ColorImage, Id, Painter, Pos2, Sense, Shape, Stroke, TextureHandle,
    TextureOptions, Ui, Vec2,
};
use std::f32::consts::PI;

const ORB_ICON_64: &[u8] = include_bytes!("../assets/phantom-icon-64.png");

/// Decodes a PNG byte slice into tightly packed RGBA8 pixels `(rgba, width, height)`.
pub fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let buf_size = reader.output_buffer_size()?;
    let mut raw = vec![0u8; buf_size];
    let info = reader.next_frame(&mut raw).ok()?;
    let bytes = &raw[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((rgba, info.width, info.height))
}

/// Renders the cached 64x64 glass orb icon texture at `size x size` points.
pub fn draw_phantom_orb_icon(ui: &mut Ui, size: f32) {
    let key = Id::new("phantom_ui_orb_tex_64");
    // Never call back into the context (e.g. `load_texture`) inside `data_mut`: the context
    // lock is not re-entrant, so that deadlocks the first frame and leaves a blank window.
    let cached = ui.ctx().data(|d| d.get_temp::<TextureHandle>(key));
    let tex = cached.unwrap_or_else(|| {
        let (rgba, w, h) = decode_png_rgba(ORB_ICON_64).unwrap_or_else(|| (vec![0, 0, 0, 0], 1, 1));
        let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let handle = ui
            .ctx()
            .load_texture("phantom_ui_orb_64", image, TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(key, handle.clone()));
        handle
    });
    ui.add(egui::Image::new((tex.id(), Vec2::splat(size))).fit_to_exact_size(Vec2::splat(size)));
}

/// Renders the monoline connected `phantom` wordmark (`198:56` viewBox) with round terminals.
pub fn draw_phantom_wordmark(ui: &mut Ui, height: f32, text_color: Color32, accent_color: Color32) {
    let width = height * (198.0 / 56.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_wordmark(ui.painter(), rect, text_color, accent_color);
}

fn paint_wordmark(painter: &Painter, rect: egui::Rect, text_color: Color32, accent_color: Color32) {
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
    let arc_pts = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32, steps: usize| -> Vec<Pos2> {
        (0..=steps)
            .map(|i| {
                let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };
    let draw_arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let pts = arc_pts(cx, cy, r, a0, a1, 14);
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

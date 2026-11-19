//! Real-Time FFT Audio Spectrum Analyzer View
//! Slice-based Pre-EQ, Post-EQ, and Peak-Hold traces using premultiplied-safe translucency.

use crate::views::curve_plot::EqCanvasMapping;
use nih_plug_egui::egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke};

const SPECTRUM_MIN_DB: f32 = -90.0;
const SPECTRUM_RANGE_DB: f32 = 100.0;

fn bins_to_points(map: &EqCanvasMapping, bins_db: &[f32]) -> Vec<Pos2> {
    let n = bins_db.len();
    let mut points = Vec::with_capacity(n);
    if n < 2 {
        return points;
    }
    let denom = (n - 1) as f32;
    for (i, &db) in bins_db.iter().enumerate() {
        let norm = i as f32 / denom;
        let x = map.rect.min.x + norm * map.rect.width();
        let db_norm = ((db - SPECTRUM_MIN_DB) / SPECTRUM_RANGE_DB).clamp(0.0, 1.0);
        let y = map.rect.max.y - db_norm * map.rect.height();
        points.push(Pos2::new(x, y));
    }
    points
}

/// Renders a single filled spectrum trace from `bins_db` with vertical fade to 0 alpha at bottom.
/// Pass colors created via `Color32::from_rgba_unmultiplied` for clean OpenGL translucency.
pub fn draw_spectrum_trace(
    painter: &Painter,
    map: &EqCanvasMapping,
    bins_db: &[f32],
    fill_color: Color32,
    line_color: Color32,
) {
    let points = bins_to_points(map, bins_db);
    if points.len() < 2 {
        return;
    }

    let bottom_y = map.rect.max.y;
    let bot_col = Color32::TRANSPARENT;
    let mut mesh = Mesh::default();
    for w in points.windows(2) {
        if (bottom_y - w[0].y) < 0.5 && (bottom_y - w[1].y) < 0.5 {
            continue;
        }
        let idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(w[0], fill_color);
        mesh.colored_vertex(Pos2::new(w[0].x, bottom_y), bot_col);
        mesh.colored_vertex(w[1], fill_color);
        mesh.colored_vertex(Pos2::new(w[1].x, bottom_y), bot_col);
        mesh.add_triangle(idx, idx + 1, idx + 2);
        mesh.add_triangle(idx + 1, idx + 3, idx + 2);
    }
    if !mesh.indices.is_empty() {
        painter.add(Shape::mesh(mesh));
    }

    painter.add(Shape::line(points, Stroke::new(1.0_f32, line_color)));
}

/// Renders a hairline spectrum contour (ideal for Peak Hold or Side-channel overlay).
pub fn draw_spectrum_line(
    painter: &Painter,
    map: &EqCanvasMapping,
    bins_db: &[f32],
    line_color: Color32,
    stroke_width: f32,
) {
    let points = bins_to_points(map, bins_db);
    if points.len() < 2 {
        return;
    }
    painter.add(Shape::line(points, Stroke::new(stroke_width, line_color)));
}

/// Renders dual Pre-EQ + Post-EQ spectrum traces with optional Peak-Hold hairline.
/// - Pre-EQ: soft warm gray-blue (alpha 16 fill, alpha 74 contour)
/// - Post-EQ: tasteful translucent cyan (alpha 24 fill, alpha 108 contour)
/// - Peak Hold: delicate sky-silver hairline (alpha 118)
#[allow(clippy::too_many_arguments)]
pub fn draw_dual_spectrum(
    painter: &Painter,
    map: &EqCanvasMapping,
    pre_bins: &[f32],
    post_bins: &[f32],
    peak_bins: Option<&[f32]>,
    show_pre: bool,
    show_post: bool,
    show_peaks: bool,
) {
    if show_pre && pre_bins.len() > 1 {
        draw_spectrum_trace(
            painter,
            map,
            pre_bins,
            Color32::from_rgba_unmultiplied(156, 168, 188, 16),
            Color32::from_rgba_unmultiplied(168, 180, 200, 74),
        );
    }

    if show_post && post_bins.len() > 1 {
        draw_spectrum_trace(
            painter,
            map,
            post_bins,
            Color32::from_rgba_unmultiplied(14, 165, 233, 24),
            Color32::from_rgba_unmultiplied(56, 189, 248, 108),
        );
    }

    if show_peaks {
        if let Some(peaks) = peak_bins {
            if peaks.len() > 1 {
                draw_spectrum_line(
                    painter,
                    map,
                    peaks,
                    Color32::from_rgba_unmultiplied(186, 230, 253, 118),
                    1.0_f32,
                );
            }
        }
    }
}

/// Convenience single-trace Post-EQ spectrum renderer taking a `&[f32]` dB slice directly.
pub fn draw_spectrum(painter: &Painter, map: &EqCanvasMapping, bins_db: &[f32]) {
    draw_spectrum_trace(
        painter,
        map,
        bins_db,
        Color32::from_rgba_unmultiplied(14, 165, 233, 24),
        Color32::from_rgba_unmultiplied(56, 189, 248, 108),
    );
}

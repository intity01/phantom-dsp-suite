//! EQ Frequency Response Curve Renderer
//! Non-self-intersecting premultiplied-safe translucent gradient curve meshes & strokes.

use crate::theme::with_alpha;
use nih_plug_egui::egui::{Color32, CornerRadius, Mesh, Painter, Pos2, Shape, Stroke, StrokeKind};
use phantom_dsp::BiquadCoeffs;

pub use crate::views::canvas_grid::{
    draw_canvas_grid, draw_empty_canvas_hint, EqCanvasMapping, DB_SCALE_OPTIONS,
};

/// Builds a non-self-intersecting vertical gradient triangle strip between `points` and `base_y`.
/// Fades cleanly to 0 alpha (`Color32::TRANSPARENT`) at `base_y` and splits baseline crossings.
fn add_baseline_fill_mesh(
    painter: &Painter,
    points: &[Pos2],
    base_y: f32,
    rgb: [u8; 3],
    max_alpha: u8,
) {
    if points.len() < 2 || max_alpha == 0 {
        return;
    }

    let color_for_dist = |dist: f32| -> Color32 {
        let k = (dist.abs() / 4.0).clamp(0.0, 1.0);
        let a = (max_alpha as f32 * k).round() as u8;
        if a == 0 {
            Color32::TRANSPARENT
        } else {
            Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], a)
        }
    };

    let base_col = Color32::TRANSPARENT;
    let mut mesh = Mesh::default();

    for w in points.windows(2) {
        let p0 = w[0];
        let p1 = w[1];
        let d0 = p0.y - base_y;
        let d1 = p1.y - base_y;
        if d0.abs() < 0.25 && d1.abs() < 0.25 {
            continue;
        }
        let b0 = Pos2::new(p0.x, base_y);
        let b1 = Pos2::new(p1.x, base_y);
        let c0 = color_for_dist(d0);
        let c1 = color_for_dist(d1);

        if (d0 > 0.0 && d1 < 0.0) || (d0 < 0.0 && d1 > 0.0) {
            let t = (-d0) / (d1 - d0);
            let cross = Pos2::new(p0.x + (p1.x - p0.x) * t, base_y);
            let idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p0, c0);
            mesh.colored_vertex(b0, base_col);
            mesh.colored_vertex(cross, base_col);
            mesh.colored_vertex(p1, c1);
            mesh.colored_vertex(b1, base_col);
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx + 2, idx + 3, idx + 4);
        } else {
            let idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p0, c0);
            mesh.colored_vertex(b0, base_col);
            mesh.colored_vertex(p1, c1);
            mesh.colored_vertex(b1, base_col);
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx + 1, idx + 3, idx + 2);
        }
    }

    if !mesh.indices.is_empty() {
        painter.add(Shape::mesh(mesh));
    }
}

fn sample_curve_points<F: FnMut(f32) -> f32>(
    map: &EqCanvasMapping,
    num_points: usize,
    mut eval_db: F,
) -> Vec<Pos2> {
    let mut points = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let norm = i as f32 / (num_points - 1) as f32;
        let freq = map.min_freq * (map.max_freq / map.min_freq).powf(norm);
        let x = map.rect.min.x + norm * map.rect.width();
        let db = eval_db(freq);
        points.push(Pos2::new(x, map.db_to_y(db)));
    }
    points
}

/// Draws a single band's response curve using any magnitude evaluator `eval_db(freq_hz) -> db`.
/// Supports cascaded variable-slope filters cleanly without coupling to DSP struct layouts.
pub fn draw_band_curve_fn<F: FnMut(f32) -> f32>(
    painter: &Painter,
    map: &EqCanvasMapping,
    eval_db: F,
    band_color: Color32,
    enabled: bool,
    is_soloed: bool,
) {
    let points = sample_curve_points(map, 200, eval_db);
    let [r, g, b, _] = band_color.to_srgba_unmultiplied();
    let zero_y = map.db_to_y(0.0);

    if enabled || is_soloed {
        let fill_alpha = if is_soloed { 44 } else { 28 };
        let stroke_alpha = if is_soloed { 225 } else { 170 };
        let stroke_w = if is_soloed { 2.0_f32 } else { 1.5_f32 };
        add_baseline_fill_mesh(painter, &points, zero_y, [r, g, b], fill_alpha);
        painter.add(Shape::line(
            points,
            Stroke::new(stroke_w, with_alpha(band_color, stroke_alpha)),
        ));
    } else {
        painter.add(Shape::line(
            points,
            Stroke::new(1.0_f32, with_alpha(band_color, 65)),
        ));
    }
}

/// Draws a single biquad band curve (convenience wrapper over `draw_band_curve_fn`).
pub fn draw_band_curve(
    painter: &Painter,
    map: &EqCanvasMapping,
    coeffs: &BiquadCoeffs,
    sample_rate: f32,
    band_color: Color32,
    enabled: bool,
    is_soloed: bool,
) {
    draw_band_curve_fn(
        painter,
        map,
        |freq| coeffs.magnitude_db_at(freq, sample_rate),
        band_color,
        enabled,
        is_soloed,
    );
}

/// Draws the combined master EQ curve using any magnitude evaluator `eval_db(freq_hz) -> db`.
pub fn draw_combined_curve_fn<F: FnMut(f32) -> f32>(
    painter: &Painter,
    map: &EqCanvasMapping,
    eval_db: F,
) {
    let points = sample_curve_points(map, 240, eval_db);
    let zero_y = map.db_to_y(0.0);
    if points.len() > 1 {
        add_baseline_fill_mesh(painter, &points, zero_y, [56, 189, 248], 26);
        painter.add(Shape::line(
            points.clone(),
            Stroke::new(4.0_f32, Color32::from_rgba_unmultiplied(56, 189, 248, 30)),
        ));
        painter.add(Shape::line(
            points,
            Stroke::new(2.0_f32, Color32::from_rgb(241, 245, 249)),
        ));
    }

    painter.rect_stroke(
        map.rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
        StrokeKind::Inside,
    );
}

/// Draws the combined master EQ curve from a slice of single-stage `BiquadCoeffs`.
pub fn draw_combined_curve(
    painter: &Painter,
    map: &EqCanvasMapping,
    active_coeffs: &[BiquadCoeffs],
    sample_rate: f32,
) {
    draw_combined_curve_fn(painter, map, |freq| {
        active_coeffs
            .iter()
            .map(|c| c.magnitude_db_at(freq, sample_rate))
            .sum()
    });
}

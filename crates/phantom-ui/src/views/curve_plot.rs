//! EQ Frequency Response Canvas & Grid Renderer
//! High-performance logarithmic frequency axis (20 Hz - 20,000 Hz) and dB gain axis.

use crate::theme::{BG_DISPLAY, TEXT_MUTED};
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
};
use phantom_dsp::BiquadCoeffs;

pub struct EqCanvasMapping {
    pub rect: Rect,
    pub min_freq: f32,
    pub max_freq: f32,
    pub min_db: f32,
    pub max_db: f32,
}

impl EqCanvasMapping {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -24.0,
            max_db: 24.0,
        }
    }

    #[inline]
    pub fn freq_to_x(&self, f: f32) -> f32 {
        let t = ((f / self.min_freq).ln() / (self.max_freq / self.min_freq).ln()).clamp(0.0, 1.0);
        self.rect.min.x + t * self.rect.width()
    }

    #[inline]
    pub fn x_to_freq(&self, x: f32) -> f32 {
        let t = ((x - self.rect.min.x) / self.rect.width()).clamp(0.0, 1.0);
        self.min_freq * (self.max_freq / self.min_freq).powf(t)
    }

    #[inline]
    pub fn db_to_y(&self, db: f32) -> f32 {
        let t = ((db - self.min_db) / (self.max_db - self.min_db)).clamp(0.0, 1.0);
        self.rect.max.y - t * self.rect.height()
    }

    #[inline]
    pub fn y_to_db(&self, y: f32) -> f32 {
        let t = ((self.rect.max.y - y) / self.rect.height()).clamp(0.0, 1.0);
        self.min_db + t * (self.max_db - self.min_db)
    }
}

pub fn draw_canvas_grid(painter: &Painter, map: &EqCanvasMapping) {
    painter.rect_filled(map.rect, CornerRadius::same(6), BG_DISPLAY);

    let freqs = [
        (20.0, "20"),
        (50.0, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1000.0, "1k"),
        (2000.0, "2k"),
        (5000.0, "5k"),
        (10000.0, "10k"),
        (20000.0, "20k"),
    ];

    for &(f, label) in &freqs {
        let x = map.freq_to_x(f);
        painter.line_segment(
            [Pos2::new(x, map.rect.min.y), Pos2::new(x, map.rect.max.y)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 14)),
        );
        painter.text(
            Pos2::new(x, map.rect.max.y - 12.0),
            Align2::CENTER_BOTTOM,
            label,
            FontId::proportional(9.0),
            Color32::from_rgba_unmultiplied(148, 163, 184, 150),
        );
    }

    let dbs = [-18.0_f32, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0];
    for &db in &dbs {
        let y = map.db_to_y(db);
        let is_zero = db.abs() < 0.1;
        let stroke_color = if is_zero {
            Color32::from_rgba_unmultiplied(255, 255, 255, 55)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 16)
        };
        let stroke_width = if is_zero { 1.2_f32 } else { 1.0_f32 };
        painter.line_segment(
            [Pos2::new(map.rect.min.x, y), Pos2::new(map.rect.max.x, y)],
            Stroke::new(stroke_width, stroke_color),
        );
        let text = format!("{:+2.0} dB", db);
        painter.text(
            Pos2::new(map.rect.min.x + 8.0, y - 2.0),
            Align2::LEFT_BOTTOM,
            text,
            FontId::proportional(9.0),
            TEXT_MUTED,
        );
    }
}

/// Builds a non-self-intersecting gradient triangle strip between a curve and a horizontal baseline.
/// Properly splits segments that cross `base_y` so both boosts and cuts render cleanly.
fn add_baseline_fill_mesh(
    painter: &Painter,
    points: &[Pos2],
    base_y: f32,
    curve_color: Color32,
    base_color: Color32,
) {
    if points.len() < 2 {
        return;
    }

    let mut mesh = Mesh::default();
    for w in points.windows(2) {
        let p0 = w[0];
        let p1 = w[1];
        let d0 = p0.y - base_y;
        let d1 = p1.y - base_y;
        let b0 = Pos2::new(p0.x, base_y);
        let b1 = Pos2::new(p1.x, base_y);

        if (d0 > 0.0 && d1 < 0.0) || (d0 < 0.0 && d1 > 0.0) {
            let t = (-d0) / (d1 - d0);
            let cross = Pos2::new(p0.x + (p1.x - p0.x) * t, base_y);
            let idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p0, curve_color);
            mesh.colored_vertex(b0, base_color);
            mesh.colored_vertex(cross, base_color);
            mesh.colored_vertex(p1, curve_color);
            mesh.colored_vertex(b1, base_color);
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx + 2, idx + 3, idx + 4);
        } else {
            let idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(p0, curve_color);
            mesh.colored_vertex(b0, base_color);
            mesh.colored_vertex(p1, curve_color);
            mesh.colored_vertex(b1, base_color);
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx + 1, idx + 3, idx + 2);
        }
    }

    painter.add(Shape::mesh(mesh));
}

/// Draws the currently selected band's individual frequency response curve with a subtle tinted fill
/// in that band's color (FabFilter Pro-Q style), intended to be layered beneath the master curve.
pub fn draw_band_curve(
    painter: &Painter,
    map: &EqCanvasMapping,
    coeffs: &BiquadCoeffs,
    sample_rate: f32,
    band_color: Color32,
    enabled: bool,
    is_soloed: bool,
) {
    let num_points = 200;
    let mut points = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let norm = i as f32 / (num_points - 1) as f32;
        let freq = map.min_freq * (map.max_freq / map.min_freq).powf(norm);
        let x = map.rect.min.x + norm * map.rect.width();
        let db = coeffs.magnitude_db_at(freq, sample_rate);
        points.push(Pos2::new(x, map.db_to_y(db)));
    }

    let [r, g, b, _] = band_color.to_srgba_unmultiplied();
    let zero_y = map.db_to_y(0.0);

    if enabled || is_soloed {
        let fill_alpha = if is_soloed { 72 } else { 44 };
        let base_alpha = if is_soloed { 18 } else { 8 };
        let stroke_alpha = if is_soloed { 230 } else { 175 };
        let stroke_w = if is_soloed { 2.0_f32 } else { 1.5_f32 };

        add_baseline_fill_mesh(
            painter,
            &points,
            zero_y,
            Color32::from_rgba_unmultiplied(r, g, b, fill_alpha),
            Color32::from_rgba_unmultiplied(r, g, b, base_alpha),
        );
        painter.add(Shape::line(
            points,
            Stroke::new(stroke_w, Color32::from_rgba_unmultiplied(r, g, b, stroke_alpha)),
        ));
    } else {
        // Subtle ghost preview when the selected band is disabled
        painter.add(Shape::line(
            points,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(r, g, b, 65)),
        ));
    }
}

pub fn draw_combined_curve(
    painter: &Painter,
    map: &EqCanvasMapping,
    active_coeffs: &[BiquadCoeffs],
    sample_rate: f32,
) {
    let num_points = 240;
    let mut points = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let norm = i as f32 / (num_points - 1) as f32;
        let freq = map.min_freq * (map.max_freq / map.min_freq).powf(norm);
        let x = map.rect.min.x + norm * map.rect.width();

        let mut total_db = 0.0_f32;
        for c in active_coeffs {
            total_db += c.magnitude_db_at(freq, sample_rate);
        }

        let y = map.db_to_y(total_db);
        points.push(Pos2::new(x, y));
    }

    let zero_y = map.db_to_y(0.0);
    if points.len() > 1 {
        add_baseline_fill_mesh(
            painter,
            &points,
            zero_y,
            Color32::from_rgba_unmultiplied(56, 189, 248, 34),
            Color32::from_rgba_unmultiplied(56, 189, 248, 6),
        );

        // Smooth master EQ curve stroke
        painter.add(Shape::line(
            points,
            Stroke::new(2.2_f32, Color32::from_rgb(241, 245, 249)),
        ));
    }

    painter.rect_stroke(
        map.rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
        StrokeKind::Inside,
    );
}

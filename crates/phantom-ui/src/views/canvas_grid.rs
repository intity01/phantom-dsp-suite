//! EQ Canvas Coordinate Mapping, Adaptive Grid Renderer & Empty-Canvas Guidance
//! Logarithmic frequency axis (20 Hz - 20 kHz) and variable ±dB scale (±3..±30 dB).

use crate::theme::BG_DISPLAY;
use nih_plug_egui::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke};

/// Standard Pro-Q 4 vertical dB scale options (±dB).
pub const DB_SCALE_OPTIONS: [f32; 5] = [3.0, 6.0, 12.0, 24.0, 30.0];

pub struct EqCanvasMapping {
    pub rect: Rect,
    pub min_freq: f32,
    pub max_freq: f32,
    pub min_db: f32,
    pub max_db: f32,
}

impl EqCanvasMapping {
    /// Creates a canvas mapping with Pro-Q 4's default ±12 dB vertical range.
    pub fn new(rect: Rect) -> Self {
        Self::with_db_range(rect, 12.0)
    }

    /// Creates a canvas mapping with a symmetric `±max_db` range (e.g. 3, 6, 12, 24, 30 dB).
    pub fn with_db_range(rect: Rect, max_db: f32) -> Self {
        let range = max_db.abs().clamp(1.0, 48.0);
        Self {
            rect,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -range,
            max_db: range,
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
        (20.0_f32, "20"),
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

    let vline_col = Color32::from_rgba_unmultiplied(255, 255, 255, 18);
    let label_col = Color32::from_rgba_unmultiplied(148, 163, 184, 130);
    let label_y = map.rect.max.y - 4.0;

    for (idx, &(f, label)) in freqs.iter().enumerate() {
        let x = map.freq_to_x(f);
        let is_left = idx == 0;
        let is_right = idx + 1 == freqs.len();

        if !is_left && !is_right {
            painter.line_segment(
                [Pos2::new(x, map.rect.min.y), Pos2::new(x, map.rect.max.y)],
                Stroke::new(1.0_f32, vline_col),
            );
        }

        let (pos, align) = if is_left {
            (
                Pos2::new(map.rect.min.x + 4.0, label_y),
                Align2::LEFT_BOTTOM,
            )
        } else if is_right {
            (
                Pos2::new(map.rect.max.x - 4.0, label_y),
                Align2::RIGHT_BOTTOM,
            )
        } else {
            (Pos2::new(x, label_y), Align2::CENTER_BOTTOM)
        };
        painter.text(pos, align, label, FontId::proportional(9.0), label_col);
    }

    // Adaptive horizontal dB grid lines matching Pro-Q 4 scales
    let max_db = map.max_db.abs();
    let step: f32 = if max_db <= 3.5 {
        1.0
    } else if max_db <= 6.5 {
        2.0
    } else if max_db <= 12.5 {
        3.0
    } else {
        6.0
    };

    let limit = max_db - step * 0.45;
    let max_steps = (limit / step).floor() as i32;
    let hline_col = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
    let zero_col = Color32::from_rgba_unmultiplied(255, 255, 255, 48);
    let zero_label_col = Color32::from_rgba_unmultiplied(186, 200, 218, 140);

    for k in -max_steps..=max_steps {
        let db = k as f32 * step;
        let y = map.db_to_y(db);
        let is_zero = k == 0;
        painter.line_segment(
            [Pos2::new(map.rect.min.x, y), Pos2::new(map.rect.max.x, y)],
            Stroke::new(
                if is_zero { 1.2_f32 } else { 1.0_f32 },
                if is_zero { zero_col } else { hline_col },
            ),
        );

        let text = if is_zero {
            "0 dB".to_string()
        } else {
            format!("{:+.0} dB", db)
        };
        painter.text(
            Pos2::new(map.rect.min.x + 6.0, y - 2.0),
            Align2::LEFT_BOTTOM,
            text,
            FontId::proportional(9.0),
            if is_zero { zero_label_col } else { label_col },
        );
    }
}

/// Renders a subtle muted instruction at the bottom-center of an empty flat EQ canvas.
pub fn draw_empty_canvas_hint(painter: &Painter, map: &EqCanvasMapping) {
    painter.text(
        Pos2::new(map.rect.center().x, map.rect.max.y - 24.0),
        Align2::CENTER_BOTTOM,
        "Double-click or drag on the curve to add an EQ band",
        FontId::proportional(11.5),
        Color32::from_rgba_unmultiplied(148, 163, 184, 118),
    );
}

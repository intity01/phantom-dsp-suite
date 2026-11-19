//! Lucide-style Mini Vector Filter Shape Icons
//! Renders crisp schematic frequency-response glyphs for all 10 Pro-Q 4 filter shapes:
//! 0: Bell, 1: LowCut, 2: HighCut, 3: LowShelf, 4: HighShelf,
//! 5: Notch, 6: BandPass, 7: TiltShelf, 8: FlatTilt, 9: AllPass.

use crate::theme::with_alpha;
use nih_plug_egui::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};
use std::f32::consts::PI;

/// Draws a crisp mini filter frequency-response glyph inside `rect`.
///
/// Shape indices match canonical Pro-Q 4 order:
/// - `0`: Bell
/// - `1`: LowCut (High-Pass)
/// - `2`: HighCut (Low-Pass)
/// - `3`: LowShelf
/// - `4`: HighShelf
/// - `5`: Notch
/// - `6`: BandPass
/// - `7`: TiltShelf
/// - `8`: FlatTilt
/// - `9`: AllPass
pub fn draw_filter_shape_icon(
    painter: &Painter,
    rect: Rect,
    shape_idx: usize,
    color: Color32,
    stroke_width: f32,
) {
    let inner = rect.shrink2(nih_plug_egui::egui::vec2(
        (rect.width() * 0.10).max(1.0),
        (rect.height() * 0.16).max(1.0),
    ));
    let cy = inner.center().y;
    let half_h = inner.height() * 0.5;

    // Subtle 0 dB center guide line
    painter.line_segment(
        [Pos2::new(inner.min.x, cy), Pos2::new(inner.max.x, cy)],
        Stroke::new(0.75_f32, with_alpha(color, 42)),
    );

    let steps: usize = 16;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = inner.min.x + t * inner.width();
        let norm_y = eval_shape_y(shape_idx, t);
        let y = (cy - norm_y * half_h).clamp(inner.min.y, inner.max.y);
        pts.push(Pos2::new(x, y));
    }

    painter.add(Shape::line(pts, Stroke::new(stroke_width, color)));

    // Distinctive phase-crossing dot for AllPass so it reads clearly at tiny sizes
    if shape_idx == 9 {
        painter.circle_filled(inner.center(), (stroke_width * 0.95).max(1.3), color);
    }
}

/// Evaluates normalized vertical displacement in `[-1.0, +1.0]` at horizontal `t in [0.0, 1.0]`.
#[inline]
fn eval_shape_y(shape_idx: usize, t: f32) -> f32 {
    let u = t - 0.5;
    match shape_idx {
        // 0: Bell (smooth symmetric Gaussian peak)
        0 => 0.95 * (-(u * 5.2).powi(2)).exp() - 0.12,
        // 1: LowCut / High-Pass (steep rolloff on left -> flat top right)
        1 => {
            if t < 0.36 {
                let d = (0.36 - t) / 0.36;
                0.35 - 1.35 * d * d
            } else {
                0.35
            }
        }
        // 2: HighCut / Low-Pass (flat top left -> steep rolloff on right)
        2 => {
            if t > 0.64 {
                let d = (t - 0.64) / 0.36;
                0.35 - 1.35 * d * d
            } else {
                0.35
            }
        }
        // 3: LowShelf (elevated left plateau -> smooth S-transition to 0)
        3 => {
            let s = 1.0 / (1.0 + ((t - 0.45) * 10.0).exp());
            -0.25 + 1.05 * s
        }
        // 4: HighShelf (0 on left -> smooth S-transition to elevated right plateau)
        4 => {
            let s = 1.0 / (1.0 + (-(t - 0.55) * 10.0).exp());
            -0.25 + 1.05 * s
        }
        // 5: Notch (flat top -> sharp deep dip at center)
        5 => 0.30 - 1.25 * (-(u * 8.5).powi(2)).exp(),
        // 6: BandPass (steep skirts on both sides -> rounded central passband)
        6 => -0.85 + 1.75 * (-(u * 4.0).powi(2)).exp(),
        // 7: TiltShelf (symmetric high left shelf -> smooth S -> low right shelf)
        7 => 0.78 * (-u * 6.5).tanh(),
        // 8: FlatTilt (linear diagonal tilt across spectrum)
        8 => -1.50 * u,
        // 9: AllPass (flat outer magnitude with crisp central phase-shift S-wave)
        9 => {
            let env = (-(u * 4.5).powi(2)).exp();
            0.95 * env * (u * 2.0 * PI * 1.45).sin()
        }
        _ => 0.95 * (-(u * 5.2).powi(2)).exp() - 0.12,
    }
}

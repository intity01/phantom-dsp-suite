//! Lucide-style Pixel-Perfect Vector Icons for Audio Controls
//! Crisp, anti-aliased vector paths rendered directly via egui Painter (zero emoji dependencies).

use crate::theme::with_alpha;
use nih_plug_egui::egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};
use std::f32::consts::PI;

pub use crate::filter_icons::draw_filter_shape_icon;

/// Draws a Lucide-style Power / Bypass Icon.
pub fn draw_power_icon(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    let num_segments = 16;
    let start_angle = 45.0 * PI / 180.0;
    let end_angle = 315.0 * PI / 180.0;
    let mut arc_pts = Vec::with_capacity(num_segments + 1);
    for i in 0..=num_segments {
        let a = start_angle + (end_angle - start_angle) * (i as f32 / num_segments as f32);
        arc_pts.push(Pos2::new(
            center.x + radius * a.sin(),
            center.y - radius * a.cos(),
        ));
    }
    painter.add(Shape::line(arc_pts, stroke));

    let top_y = center.y - radius * 1.15;
    let mid_y = center.y - radius * 0.15;
    painter.line_segment(
        [Pos2::new(center.x, top_y), Pos2::new(center.x, mid_y)],
        stroke,
    );
}

/// Draws a Phase Invert Icon (Ø).
pub fn draw_phase_icon(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    painter.circle_stroke(center, radius, stroke);
    let slash_offset = radius * 1.25;
    painter.line_segment(
        [
            Pos2::new(center.x - slash_offset, center.y + slash_offset),
            Pos2::new(center.x + slash_offset, center.y - slash_offset),
        ],
        stroke,
    );
}

/// Draws an Oversampling HQ Chip Icon.
pub fn draw_chip_icon(
    painter: &Painter,
    center: Pos2,
    size: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    let half = size * 0.5;
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    painter.rect_stroke(rect, CornerRadius::same(2), stroke, StrokeKind::Inside);

    let pin_len = size * 0.25;
    for &y_off in &[-half * 0.5, half * 0.5] {
        painter.line_segment(
            [
                Pos2::new(center.x - half - pin_len, center.y + y_off),
                Pos2::new(center.x - half, center.y + y_off),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(center.x + half, center.y + y_off),
                Pos2::new(center.x + half + pin_len, center.y + y_off),
            ],
            stroke,
        );
    }
}

/// Draws a Lucide-style Padlock Icon (replaces missing-glyph padlock emoji).
pub fn draw_lock_icon(
    painter: &Painter,
    center: Pos2,
    size: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    let body_w = size * 0.74;
    let body_h = size * 0.50;
    let body_top = center.y - size * 0.04;
    let body_rect = Rect::from_min_size(
        Pos2::new(center.x - body_w * 0.5, body_top),
        Vec2::new(body_w, body_h),
    );
    painter.rect_filled(body_rect, CornerRadius::same(2), with_alpha(color, 34));
    painter.rect_stroke(body_rect, CornerRadius::same(2), stroke, StrokeKind::Middle);

    // U-shaped shackle arch sitting neatly on the top edge of the lock body
    let shackle_r = size * 0.21;
    let arch_cy = body_top - size * 0.16;
    let segments = 12;
    let mut shackle = Vec::with_capacity(segments + 3);
    shackle.push(Pos2::new(center.x - shackle_r, body_top));
    for i in 0..=segments {
        let a = PI + PI * (i as f32 / segments as f32);
        shackle.push(Pos2::new(
            center.x + shackle_r * a.cos(),
            arch_cy + shackle_r * a.sin(),
        ));
    }
    shackle.push(Pos2::new(center.x + shackle_r, body_top));
    painter.add(Shape::line(shackle, stroke));

    // Tiny centered keyhole dot
    painter.circle_filled(body_rect.center(), (stroke_width * 0.75).max(1.0), color);
}

/// Draws a Lucide-style Headphones Icon (for Per-Band Solo Audition).
pub fn draw_headphones_icon(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    let r = radius * 0.90;
    let arch_cy = center.y + radius * 0.08;

    // Top headband arch (left side -> over top -> right side)
    let segments = 14;
    let mut band_pts = Vec::with_capacity(segments + 3);
    band_pts.push(Pos2::new(center.x - r, arch_cy + radius * 0.30));
    for i in 0..=segments {
        let a = PI + PI * (i as f32 / segments as f32);
        band_pts.push(Pos2::new(center.x + r * a.cos(), arch_cy + r * a.sin()));
    }
    band_pts.push(Pos2::new(center.x + r, arch_cy + radius * 0.30));
    painter.add(Shape::line(band_pts, stroke));

    // Left and right rounded earcup capsules
    let cup_w = (radius * 0.44).max(2.2);
    let cup_h = (radius * 0.78).max(3.6);
    let cup_top = arch_cy + radius * 0.04;
    let left_cup = Rect::from_min_size(Pos2::new(center.x - r, cup_top), Vec2::new(cup_w, cup_h));
    let right_cup = Rect::from_min_size(
        Pos2::new(center.x + r - cup_w, cup_top),
        Vec2::new(cup_w, cup_h),
    );
    let cup_fill = with_alpha(color, 52);
    for cup in [left_cup, right_cup] {
        painter.rect_filled(cup, CornerRadius::same(2), cup_fill);
        painter.rect_stroke(cup, CornerRadius::same(2), stroke, StrokeKind::Middle);
    }
}

/// Draws a crisp Lucide-style Close / Delete (`×`) Icon.
pub fn draw_close_icon(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    stroke_width: f32,
) {
    let stroke = Stroke::new(stroke_width, color);
    let d = radius * 0.72;
    painter.line_segment(
        [
            Pos2::new(center.x - d, center.y - d),
            Pos2::new(center.x + d, center.y + d),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(center.x - d, center.y + d),
            Pos2::new(center.x + d, center.y - d),
        ],
        stroke,
    );
}

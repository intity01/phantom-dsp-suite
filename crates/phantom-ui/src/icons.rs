//! Lucide-style Pixel-Perfect Vector Icons for Audio Controls
//! Crisp, anti-aliased vector paths rendered directly via egui Painter.

use nih_plug_egui::egui::{Color32, Painter, Pos2, Stroke};
use std::f32::consts::PI;

/// Draws a Lucide-style Power / Bypass Icon
pub fn draw_power_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32, stroke_width: f32) {
    let stroke = Stroke::new(stroke_width, color);
    // Draw open circular arc (from 45 deg to 315 deg)
    let num_segments = 16;
    let start_angle = 45.0 * PI / 180.0;
    let end_angle = 315.0 * PI / 180.0;
    for i in 0..num_segments {
        let a1 = start_angle + (end_angle - start_angle) * (i as f32 / num_segments as f32);
        let a2 = start_angle + (end_angle - start_angle) * ((i + 1) as f32 / num_segments as f32);
        let p1 = Pos2::new(center.x + radius * a1.sin(), center.y - radius * a1.cos());
        let p2 = Pos2::new(center.x + radius * a2.sin(), center.y - radius * a2.cos());
        painter.line_segment([p1, p2], stroke);
    }
    // Vertical line in top gap
    let top_y = center.y - radius * 1.15;
    let mid_y = center.y - radius * 0.15;
    painter.line_segment([Pos2::new(center.x, top_y), Pos2::new(center.x, mid_y)], stroke);
}

/// Draws a Phase Invert Icon (Ø)
pub fn draw_phase_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32, stroke_width: f32) {
    let stroke = Stroke::new(stroke_width, color);
    painter.circle_stroke(center, radius, stroke);
    let slash_offset = radius * 1.3;
    painter.line_segment(
        [
            Pos2::new(center.x - slash_offset, center.y + slash_offset),
            Pos2::new(center.x + slash_offset, center.y - slash_offset),
        ],
        stroke,
    );
}

/// Draws an Oversampling HQ Chip Icon
pub fn draw_chip_icon(painter: &Painter, center: Pos2, size: f32, color: Color32, stroke_width: f32) {
    let stroke = Stroke::new(stroke_width, color);
    let half = size * 0.5;
    // Main rectangle
    let rect = nih_plug_egui::egui::Rect::from_center_size(center, nih_plug_egui::egui::Vec2::splat(size));
    painter.rect_stroke(rect, nih_plug_egui::egui::CornerRadius::same(2), stroke, nih_plug_egui::egui::StrokeKind::Inside);

    // Pins on left & right
    let pin_len = size * 0.25;
    for &y_off in &[-half * 0.5, half * 0.5] {
        painter.line_segment(
            [Pos2::new(center.x - half - pin_len, center.y + y_off), Pos2::new(center.x - half, center.y + y_off)],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(center.x + half, center.y + y_off), Pos2::new(center.x + half + pin_len, center.y + y_off)],
            stroke,
        );
    }
}

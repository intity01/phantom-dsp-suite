//! Precision Pro-Audio Rotary Knob Widget
//! Dark obsidian dial with 270-degree metallic track arc (135°..405°), bipolar/unipolar
//! glowing value arc, fine Shift-drag precision, mouse wheel step, and double-click reset.

use crate::theme::{with_alpha, TEXT_BRIGHT, TEXT_MUTED};
use nih_plug_egui::egui::{Align2, Color32, FontId, Painter, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use std::f32::consts::PI;
use std::hash::Hash;

const DEG_135: f32 = 0.75 * PI;
const DEG_270: f32 = 1.50 * PI;
const SWEEP_RAD: f32 = 1.50 * PI;

const MIN_FREQ_HZ: f32 = 10.0;
const MAX_FREQ_HZ: f32 = 30000.0;
const MIN_Q: f32 = 0.1;
const MAX_Q: f32 = 30.0;

pub struct RotaryKnobConfig<'a> {
    pub label: &'a str,
    pub value_text: &'a str,
    pub normalized: f32,
    pub default_normalized: f32,
    pub accent_color: Color32,
    pub diameter: f32,
    pub bipolar: bool,
}

#[inline]
pub fn freq_to_norm(freq_hz: f32) -> f32 {
    let f = freq_hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ);
    ((f / MIN_FREQ_HZ).ln() / (MAX_FREQ_HZ / MIN_FREQ_HZ).ln()).clamp(0.0, 1.0)
}

#[inline]
pub fn norm_to_freq(norm: f32) -> f32 {
    let t = norm.clamp(0.0, 1.0);
    MIN_FREQ_HZ * (MAX_FREQ_HZ / MIN_FREQ_HZ).powf(t)
}

#[inline]
pub fn q_to_norm(q: f32) -> f32 {
    let clamped = q.clamp(MIN_Q, MAX_Q);
    ((clamped / MIN_Q).ln() / (MAX_Q / MIN_Q).ln()).clamp(0.0, 1.0)
}

#[inline]
pub fn norm_to_q(norm: f32) -> f32 {
    let t = norm.clamp(0.0, 1.0);
    MIN_Q * (MAX_Q / MIN_Q).powf(t)
}

#[inline]
pub fn linear_to_norm(value: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < 1e-6 {
        return 0.5;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

#[inline]
pub fn norm_to_linear(norm: f32, min: f32, max: f32) -> f32 {
    min + norm.clamp(0.0, 1.0) * (max - min)
}

fn draw_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    stroke: Stroke,
) {
    let diff = end_angle - start_angle;
    if diff.abs() < 0.01 {
        return;
    }
    let segments = ((diff.abs() / PI) * 20.0).ceil().clamp(4.0, 32.0) as usize;
    let mut pts = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let a = start_angle + diff * t;
        pts.push(Pos2::new(
            center.x + radius * a.cos(),
            center.y + radius * a.sin(),
        ));
    }
    painter.add(Shape::line(pts, stroke));
}

/// Draws a precision pro-audio rotary knob and handles vertical drag, Shift fine-adjust,
/// mouse-wheel stepping, and double-click reset to default.
/// Returns `Some(new_normalized)` in `0.0..=1.0` whenever the user adjusts the knob.
pub fn draw_rotary_knob(
    ui: &mut Ui,
    id_salt: impl Hash,
    cfg: &RotaryKnobConfig<'_>,
) -> Option<f32> {
    ui.push_id(id_salt, |ui| {
        let width = (cfg.diameter + 26.0).max(58.0);
        let height = cfg.diameter + 22.0;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

        let cur = cfg.normalized.clamp(0.0, 1.0);
        let shift = ui.input(|i| i.modifiers.shift);
        let mut updated: Option<f32> = None;

        if response.double_clicked() {
            let rst = cfg.default_normalized.clamp(0.0, 1.0);
            if (rst - cur).abs() > 1e-5 {
                updated = Some(rst);
            }
        } else if response.dragged() {
            let dy = response.drag_delta().y;
            if dy.abs() > 0.0 {
                let speed = if shift { 0.001 } else { 0.005 };
                let next = (cur - dy * speed).clamp(0.0, 1.0);
                if (next - cur).abs() > 1e-5 {
                    updated = Some(next);
                }
            }
        } else if response.hovered() {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.01 {
                let step = if shift { 0.004 } else { 0.02 };
                let dir = if scroll_y > 0.0 { 1.0 } else { -1.0 };
                let next = (cur + dir * step).clamp(0.0, 1.0);
                if (next - cur).abs() > 1e-5 {
                    updated = Some(next);
                }
            }
        }

        let display_norm = updated.unwrap_or(cur);
        let is_active = response.dragged() || response.hovered();
        let painter = ui.painter();

        // Top label
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 4.5),
            Align2::CENTER_CENTER,
            cfg.label,
            FontId::proportional(8.8),
            if is_active { TEXT_BRIGHT } else { TEXT_MUTED },
        );

        let center = Pos2::new(rect.center().x, rect.min.y + 10.5 + cfg.diameter * 0.5);
        let outer_r = cfg.diameter * 0.5;
        let body_r = (outer_r - 2.6).max(6.0);
        let arc_r = outer_r - 0.6;

        // Soft ambient shadow & obsidian dial body
        painter.circle_filled(
            Pos2::new(center.x, center.y + 1.2),
            body_r + 1.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 110),
        );
        let body_fill = if response.dragged() {
            Color32::from_rgb(28, 32, 44)
        } else if response.hovered() {
            Color32::from_rgb(23, 26, 36)
        } else {
            Color32::from_rgb(16, 18, 26)
        };
        painter.circle_filled(center, body_r, body_fill);
        painter.circle_stroke(
            center,
            body_r,
            Stroke::new(
                1.0_f32,
                if is_active {
                    with_alpha(cfg.accent_color, 115)
                } else {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 28)
                },
            ),
        );

        // Background 270° track arc (135°..405°)
        draw_arc(
            painter,
            center,
            arc_r,
            DEG_135,
            DEG_135 + SWEEP_RAD,
            Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
        );

        // Active glowing value arc
        let val_angle = DEG_135 + display_norm * SWEEP_RAD;
        let start_angle = if cfg.bipolar { DEG_270 } else { DEG_135 };
        draw_arc(
            painter,
            center,
            arc_r,
            start_angle,
            val_angle,
            Stroke::new(3.8_f32, with_alpha(cfg.accent_color, 64)),
        );
        draw_arc(
            painter,
            center,
            arc_r,
            start_angle,
            val_angle,
            Stroke::new(2.2_f32, cfg.accent_color),
        );

        // Pointer indicator line & glowing cap dot
        let inner_pt = Pos2::new(
            center.x + (body_r * 0.24) * val_angle.cos(),
            center.y + (body_r * 0.24) * val_angle.sin(),
        );
        let outer_pt = Pos2::new(
            center.x + (body_r - 2.2) * val_angle.cos(),
            center.y + (body_r - 2.2) * val_angle.sin(),
        );
        painter.line_segment([inner_pt, outer_pt], Stroke::new(1.7_f32, TEXT_BRIGHT));
        painter.circle_filled(outer_pt, 1.5_f32, cfg.accent_color);

        // Crisp value readout below dial
        painter.text(
            Pos2::new(rect.center().x, rect.max.y - 4.5),
            Align2::CENTER_CENTER,
            cfg.value_text,
            FontId::proportional(9.2),
            if response.dragged() {
                cfg.accent_color
            } else {
                TEXT_BRIGHT
            },
        );

        updated
    })
    .inner
}

/// Convenience shorthand for drawing a compact rotary knob in the Floating HUD.
#[allow(clippy::too_many_arguments)]
pub fn draw_compact_knob(
    ui: &mut Ui,
    id_salt: impl Hash,
    label: &str,
    value_text: &str,
    normalized: f32,
    default_normalized: f32,
    accent_color: Color32,
    diameter: f32,
    bipolar: bool,
) -> Option<f32> {
    draw_rotary_knob(
        ui,
        id_salt,
        &RotaryKnobConfig {
            label,
            value_text,
            normalized,
            default_normalized,
            accent_color,
            diameter,
            bipolar,
        },
    )
}

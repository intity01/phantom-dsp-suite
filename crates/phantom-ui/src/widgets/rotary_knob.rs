//! Precision Pro-Audio Rotary Knob Widget
//! Dark obsidian dial with 270-degree metallic track arc (135°..405°), bipolar/unipolar
//! glowing value arc, fine Shift-drag precision, mouse wheel step, double-click reset,
//! and optional inline keyboard value/note input.

use crate::theme::{with_alpha, TEXT_BRIGHT, TEXT_MUTED};
use crate::widgets::knob_edit::{
    cancel_knob_edit, draw_knob_value_readout, KnobEditKind, KNOB_HELP,
};
use nih_plug_egui::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2,
};
use std::f32::consts::PI;
use std::hash::Hash;

const DEG_135: f32 = 0.75 * PI;
const DEG_270: f32 = 1.50 * PI;
const SWEEP_RAD: f32 = 1.50 * PI;
const VALUE_STRIP_H: f32 = 13.5;

pub(crate) const MIN_FREQ_HZ: f32 = 10.0;
pub(crate) const MAX_FREQ_HZ: f32 = 30000.0;
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
    MIN_FREQ_HZ * (MAX_FREQ_HZ / MIN_FREQ_HZ).powf(norm.clamp(0.0, 1.0))
}

#[inline]
pub fn q_to_norm(q: f32) -> f32 {
    let clamped = q.clamp(MIN_Q, MAX_Q);
    ((clamped / MIN_Q).ln() / (MAX_Q / MIN_Q).ln()).clamp(0.0, 1.0)
}

#[inline]
pub fn norm_to_q(norm: f32) -> f32 {
    MIN_Q * (MAX_Q / MIN_Q).powf(norm.clamp(0.0, 1.0))
}

#[inline]
pub fn linear_to_norm(value: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < 1e-6 {
        0.5
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    }
}

#[inline]
pub fn norm_to_linear(norm: f32, min: f32, max: f32) -> f32 {
    min + norm.clamp(0.0, 1.0) * (max - min)
}

fn draw_arc(painter: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let diff = a1 - a0;
    if diff.abs() < 0.01 {
        return;
    }
    let segs = ((diff.abs() / PI) * 20.0).ceil().clamp(4.0, 32.0) as usize;
    let pts = (0..=segs)
        .map(|i| {
            let a = a0 + diff * (i as f32 / segs as f32);
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts, stroke));
}

fn paint_dial(
    painter: &Painter,
    rect: Rect,
    cfg: &RotaryKnobConfig<'_>,
    display_norm: f32,
    hovered: bool,
    dragged: bool,
) {
    let is_active = dragged || hovered;
    let ac = cfg.accent_color;
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
    let body_fill = if dragged {
        Color32::from_rgb(28, 32, 44)
    } else if hovered {
        Color32::from_rgb(23, 26, 36)
    } else {
        Color32::from_rgb(16, 18, 26)
    };
    let ring_col = if is_active {
        with_alpha(ac, 115)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 28)
    };
    painter.circle_filled(center, body_r, body_fill);
    painter.circle_stroke(center, body_r, Stroke::new(1.0_f32, ring_col));

    // Background 270° track arc (135°..405°) & active glowing value arc
    let track = Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34));
    draw_arc(painter, center, arc_r, DEG_135, DEG_135 + SWEEP_RAD, track);
    let val_a = DEG_135 + display_norm * SWEEP_RAD;
    let start_a = if cfg.bipolar { DEG_270 } else { DEG_135 };
    draw_arc(
        painter,
        center,
        arc_r,
        start_a,
        val_a,
        Stroke::new(3.8_f32, with_alpha(ac, 64)),
    );
    draw_arc(
        painter,
        center,
        arc_r,
        start_a,
        val_a,
        Stroke::new(2.2_f32, ac),
    );

    // Pointer indicator line & glowing cap dot
    let (ca, sa) = (val_a.cos(), val_a.sin());
    let inner_pt = Pos2::new(center.x + body_r * 0.24 * ca, center.y + body_r * 0.24 * sa);
    let outer_pt = Pos2::new(
        center.x + (body_r - 2.2) * ca,
        center.y + (body_r - 2.2) * sa,
    );
    painter.line_segment([inner_pt, outer_pt], Stroke::new(1.7_f32, TEXT_BRIGHT));
    painter.circle_filled(outer_pt, 1.5_f32, ac);
}

/// Draws a precision pro-audio rotary knob and handles vertical drag, Shift fine-adjust,
/// mouse-wheel stepping, and double-click reset to default.
/// Returns `Some(new_normalized)` in `0.0..=1.0` whenever the user adjusts the knob.
pub fn draw_rotary_knob(
    ui: &mut Ui,
    id_salt: impl Hash,
    cfg: &RotaryKnobConfig<'_>,
) -> Option<f32> {
    draw_editable_rotary_knob(ui, id_salt, cfg, KnobEditKind::None)
}

/// Draws a rotary knob with optional inline keyboard editing on the bottom value readout strip.
/// Double-clicking the dial circle resets to `cfg.default_normalized`; clicking/double-clicking
/// the bottom value strip opens an inline `TextEdit` when `edit_kind != KnobEditKind::None`.
pub fn draw_editable_rotary_knob(
    ui: &mut Ui,
    id_salt: impl Hash,
    cfg: &RotaryKnobConfig<'_>,
    edit_kind: KnobEditKind,
) -> Option<f32> {
    ui.push_id(id_salt, |ui| {
        let width = (cfg.diameter + 26.0).max(58.0);
        let height = cfg.diameter + 22.0;
        let (_, rect) = ui.allocate_space(Vec2::new(width, height));
        let value_rect =
            Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - VALUE_STRIP_H), rect.max);
        let dial_rect = if edit_kind == KnobEditKind::None {
            rect
        } else {
            Rect::from_min_max(rect.min, Pos2::new(rect.max.x, value_rect.min.y))
        };
        let resp = ui.interact(dial_rect, ui.id().with("dial"), Sense::click_and_drag());
        let resp = resp.on_hover_text(KNOB_HELP);
        let edit_id = ui.id().with("val_edit");

        let cur = cfg.normalized.clamp(0.0, 1.0);
        let shift = ui.input(|i| i.modifiers.shift);
        let mut candidate: Option<f32> = None;

        if resp.double_clicked() {
            cancel_knob_edit(ui, edit_id);
            candidate = Some(cfg.default_normalized.clamp(0.0, 1.0));
        } else if resp.dragged() {
            cancel_knob_edit(ui, edit_id);
            let dy = resp.drag_delta().y;
            if dy.abs() > 0.0 {
                let speed = if shift { 0.001 } else { 0.005 };
                candidate = Some((cur - dy * speed).clamp(0.0, 1.0));
            }
        } else if resp.hovered() {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.01 {
                cancel_knob_edit(ui, edit_id);
                let step = if shift { 0.004 } else { 0.02 };
                candidate = Some((cur + scroll_y.signum() * step).clamp(0.0, 1.0));
            }
        }

        let mut updated = candidate.filter(|&n| (n - cur).abs() > 1e-5);
        paint_dial(
            ui.painter(),
            rect,
            cfg,
            updated.unwrap_or(cur),
            resp.hovered(),
            resp.dragged(),
        );

        let edited = draw_knob_value_readout(
            ui,
            edit_id,
            value_rect,
            cfg.value_text,
            cfg.accent_color,
            resp.dragged(),
            edit_kind,
        );
        if updated.is_none() {
            updated = edited;
        }
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
    draw_editable_compact_knob(
        ui,
        id_salt,
        label,
        value_text,
        normalized,
        default_normalized,
        accent_color,
        diameter,
        bipolar,
        KnobEditKind::None,
    )
}

/// Draws a compact rotary knob with inline keyboard value editing on its bottom readout strip.
#[allow(clippy::too_many_arguments)]
pub fn draw_editable_compact_knob(
    ui: &mut Ui,
    id_salt: impl Hash,
    label: &str,
    value_text: &str,
    normalized: f32,
    default_normalized: f32,
    accent_color: Color32,
    diameter: f32,
    bipolar: bool,
    edit_kind: KnobEditKind,
) -> Option<f32> {
    let cfg = RotaryKnobConfig {
        label,
        value_text,
        normalized,
        default_normalized,
        accent_color,
        diameter,
        bipolar,
    };
    draw_editable_rotary_knob(ui, id_salt, &cfg, edit_kind)
}

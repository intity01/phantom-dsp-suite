use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke};
use phantom_vox_dsp::{FrameClass, PitchFrame, MAX_PITCH_HZ, MIN_PITCH_HZ};

use crate::theme::{frame_class_color, with_alpha, ACCENT_PURPLE, TEXT_DIMMED};
use crate::timeline_paint::LaneView;

pub fn paint_time_grid(p: &Painter, rect: Rect, view: &LaneView) {
    let step = if view.px_per_sec >= 240.0 {
        0.25_f32
    } else if view.px_per_sec >= 100.0 {
        0.5_f32
    } else {
        1.0_f32
    };
    let start_t = (view.scroll_sec / step).floor() * step;
    let end_t = view.x_to_sec(rect, rect.right()) + step;
    let mut t = start_t.max(0.0);
    while t <= end_t {
        let x = view.sec_to_x(rect, t);
        if x >= rect.left() && x <= rect.right() {
            p.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(0.6_f32, with_alpha(TEXT_DIMMED, 38)),
            );
        }
        t += step;
    }
}

pub fn paint_protected_span(
    p: &Painter,
    rect: Rect,
    view: &LaneView,
    start_sec: f32,
    end_sec: f32,
    preview: bool,
) {
    let x0 = view
        .sec_to_x(rect, start_sec)
        .clamp(rect.left(), rect.right());
    let x1 = view
        .sec_to_x(rect, end_sec)
        .clamp(rect.left(), rect.right());
    if x1 <= x0 + 1.0 {
        return;
    }
    let r = Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x1, rect.bottom()));
    p.rect_filled(
        r,
        CornerRadius::same(3),
        with_alpha(ACCENT_PURPLE, if preview { 50 } else { 42 }),
    );
    p.rect_stroke(
        r,
        CornerRadius::same(3),
        Stroke::new(1.2_f32, with_alpha(ACCENT_PURPLE, 200)),
        egui::StrokeKind::Inside,
    );
    if r.width() > 36.0 {
        p.text(
            Pos2::new(r.left() + 5.0, r.top() + 9.0),
            Align2::LEFT_CENTER,
            "LOCKED",
            FontId::proportional(9.0),
            ACCENT_PURPLE,
        );
    }
}

pub fn paint_segmentation_strip(
    p: &Painter,
    rect: Rect,
    view: &LaneView,
    hop_sec: f32,
    classes: &[FrameClass],
) {
    for (i, &cls) in classes.iter().enumerate() {
        if cls == FrameClass::Silence {
            continue;
        }
        let x0 = view.sec_to_x(rect, i as f32 * hop_sec);
        let x1 = view.sec_to_x(rect, (i + 1) as f32 * hop_sec);
        if x1 < rect.left() || x0 > rect.right() {
            continue;
        }
        let c = frame_class_color(cls);
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(x0.max(rect.left()), rect.top() + 1.0),
                Pos2::new(x1.min(rect.right()), rect.bottom() - 1.0),
            ),
            CornerRadius::ZERO,
            with_alpha(c, 195),
        );
    }
}

pub fn paint_pitch_contour(
    p: &Painter,
    rect: Rect,
    view: &LaneView,
    hop_sec: f32,
    pitch: &[PitchFrame],
    color: Color32,
) {
    let min_l = MIN_PITCH_HZ.ln();
    let max_l = MAX_PITCH_HZ.ln();
    let mut prev: Option<Pos2> = None;
    for (i, pf) in pitch.iter().enumerate() {
        if !pf.voiced || pf.f0_hz < MIN_PITCH_HZ {
            prev = None;
            continue;
        }
        let x = view.sec_to_x(rect, i as f32 * hop_sec);
        if x < rect.left() - 8.0 || x > rect.right() + 8.0 {
            prev = None;
            continue;
        }
        let norm = ((pf.f0_hz.clamp(MIN_PITCH_HZ, MAX_PITCH_HZ).ln() - min_l) / (max_l - min_l))
            .clamp(0.0, 1.0);
        let y = rect.bottom() - 4.0 - norm * (rect.height() - 8.0);
        let pt = Pos2::new(x, y);
        if let Some(a) = prev {
            if (pt.x - a.x).abs() < 24.0 && (pt.y - a.y).abs() < 40.0 {
                p.line_segment([a, pt], Stroke::new(1.5_f32, color));
            }
        }
        prev = Some(pt);
    }
}

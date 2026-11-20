//! Timeline canvas painting: layered waveforms (Original vs Aligned), Confidence Bars,
//! Sync Anchor pins, Protected Region spans, Segmentation badges, and YIN Pitch contours.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke};
use phantom_vox_dsp::AlignmentResult;

use crate::state::StudioTrack;
use crate::theme::{
    confidence_color, layer_color, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_PINK, BG_DISPLAY,
    BORDER_SUBTLE, TEXT_BRIGHT, WAVE_ORIG,
};
use crate::timeline_overlays::{
    paint_pitch_contour, paint_protected_span, paint_segmentation_strip, paint_time_grid,
};
use crate::worker::WaveformPeaks;

pub struct LaneView {
    pub scroll_sec: f32,
    pub px_per_sec: f32,
    pub audition_b: bool,
    pub show_segmentation: bool,
    pub show_pitch: bool,
    pub playhead_sec: f32,
    pub loop_enabled: bool,
    pub loop_start_sec: f32,
    pub loop_end_sec: f32,
}

impl LaneView {
    #[inline]
    pub fn sec_to_x(&self, rect: Rect, sec: f32) -> f32 {
        rect.left() + (sec - self.scroll_sec) * self.px_per_sec
    }

    #[inline]
    pub fn x_to_sec(&self, rect: Rect, x: f32) -> f32 {
        (self.scroll_sec + (x - rect.left()) / self.px_per_sec).max(0.0)
    }
}

pub fn paint_track_lane(
    painter: &Painter,
    rect: Rect,
    track: &StudioTrack,
    guide_alignment: Option<&AlignmentResult>,
    view: &LaneView,
    pending_guide_sec: Option<f32>,
    protect_preview: Option<(f32, f32)>,
) {
    let p = painter.with_clip_rect(rect);
    p.rect_filled(rect, CornerRadius::same(4), BG_DISPLAY);

    paint_time_grid(&p, rect, view);

    // Loop shaded zone.
    if view.loop_enabled && view.loop_end_sec > view.loop_start_sec + 0.05 {
        let lx0 = view
            .sec_to_x(rect, view.loop_start_sec)
            .clamp(rect.left(), rect.right());
        let lx1 = view
            .sec_to_x(rect, view.loop_end_sec)
            .clamp(rect.left(), rect.right());
        if lx1 > lx0 {
            p.rect_filled(
                Rect::from_min_max(Pos2::new(lx0, rect.top()), Pos2::new(lx1, rect.bottom())),
                CornerRadius::ZERO,
                with_alpha(ACCENT_CYAN, 10),
            );
        }
    }

    let conf_h = if track.is_guide { 0.0 } else { 14.0 };
    let seg_h = if view.show_segmentation { 8.0 } else { 0.0 };
    let wave_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + seg_h + 2.0),
        Pos2::new(rect.right(), rect.bottom() - conf_h - 2.0),
    );

    // Center zero line.
    p.line_segment(
        [
            Pos2::new(rect.left(), wave_rect.center().y),
            Pos2::new(rect.right(), wave_rect.center().y),
        ],
        Stroke::new(0.8_f32, with_alpha(BORDER_SUBTLE, 180)),
    );

    // Protected Regions (purple locked spans).
    for reg in &track.protected {
        paint_protected_span(&p, wave_rect, view, reg.start_sec, reg.end_sec, false);
    }
    if let Some((a, b)) = protect_preview {
        paint_protected_span(&p, wave_rect, view, a.min(b), a.max(b), true);
    }

    // Layered waveforms: Original (dimmed slate) vs Aligned (bright accent).
    let accent = if track.is_guide {
        ACCENT_CYAN
    } else {
        layer_color(track.color_idx)
    };
    if track.is_guide {
        paint_waveform(&p, wave_rect, &track.orig_peaks, view, accent, 175, 1.0);
    } else {
        let orig_alpha = if view.audition_b { 75 } else { 200 };
        paint_waveform(
            &p,
            wave_rect,
            &track.orig_peaks,
            view,
            WAVE_ORIG,
            orig_alpha,
            0.92,
        );
        if let Some(al_peaks) = &track.aligned_peaks {
            let al_alpha = if view.audition_b { 215 } else { 70 };
            paint_waveform(&p, wave_rect, al_peaks, view, accent, al_alpha, 1.0);
        }
    }

    // Segmentation overlay bar along top of lane.
    if view.show_segmentation {
        let seg_rect = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + seg_h));
        let classes = if track.is_guide {
            guide_alignment.map(|a| (a.time_map.hop_sec, a.guide_classes.as_slice()))
        } else {
            track
                .alignment
                .as_ref()
                .map(|a| (a.time_map.hop_sec, a.dub_classes.as_slice()))
        };
        if let Some((hop_sec, cls)) = classes {
            paint_segmentation_strip(&p, seg_rect, view, hop_sec, cls);
        }
    }

    // YIN Pitch contour overlay.
    if view.show_pitch {
        if track.is_guide {
            if let Some(al) = guide_alignment {
                paint_pitch_contour(
                    &p,
                    wave_rect,
                    view,
                    al.time_map.hop_sec,
                    &al.guide_pitch,
                    ACCENT_CYAN,
                );
            }
        } else if let Some(al) = &track.alignment {
            paint_pitch_contour(
                &p,
                wave_rect,
                view,
                al.time_map.hop_sec,
                &al.guide_pitch,
                with_alpha(ACCENT_CYAN, 110),
            );
            paint_pitch_contour(
                &p,
                wave_rect,
                view,
                al.time_map.hop_sec,
                &al.dub_pitch,
                ACCENT_PINK,
            );
        }
    }

    // Manual Sync Anchors (Guide -> Dub pin markers & connecting warp vector).
    for (idx, anc) in track.anchors.iter().enumerate() {
        let gx = view.sec_to_x(rect, anc.guide_sec);
        let dx = view.sec_to_x(rect, anc.dub_sec);
        let top_p = Pos2::new(gx, wave_rect.top() + 4.0);
        let bot_p = Pos2::new(dx, wave_rect.bottom() - 4.0);
        p.line_segment([top_p, bot_p], Stroke::new(1.8_f32, ACCENT_AMBER));
        p.circle_filled(top_p, 4.5, ACCENT_CYAN);
        p.circle_filled(bot_p, 5.0, ACCENT_AMBER);
        p.text(
            Pos2::new(dx + 7.0, bot_p.y - 4.0),
            Align2::LEFT_CENTER,
            format!("A{}", idx + 1),
            FontId::proportional(9.5),
            ACCENT_AMBER,
        );
    }

    if let Some(g_sec) = pending_guide_sec {
        let gx = view.sec_to_x(rect, g_sec);
        p.line_segment(
            [Pos2::new(gx, rect.top()), Pos2::new(gx, rect.bottom())],
            Stroke::new(1.5_f32, ACCENT_AMBER),
        );
        p.text(
            Pos2::new(gx + 5.0, rect.top() + 14.0),
            Align2::LEFT_CENTER,
            "Click Dub pos to pin",
            FontId::proportional(10.0),
            ACCENT_AMBER,
        );
    }

    // Phrase Confidence Bar along bottom of Dub lane.
    if !track.is_guide {
        let bar_rect = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - conf_h), rect.max);
        p.rect_filled(bar_rect, CornerRadius::ZERO, Color32::from_rgb(16, 18, 25));
        if let Some(al) = &track.alignment {
            for phr in &al.phrases {
                let x0 = view.sec_to_x(bar_rect, phr.start_sec).max(bar_rect.left());
                let x1 = view.sec_to_x(bar_rect, phr.end_sec).min(bar_rect.right());
                if x1 > x0 + 1.0 {
                    let c = confidence_color(phr.level);
                    let pr = Rect::from_min_max(
                        Pos2::new(x0, bar_rect.top() + 2.0),
                        Pos2::new(x1, bar_rect.bottom() - 2.0),
                    );
                    p.rect_filled(pr, CornerRadius::same(3), with_alpha(c, 210));
                }
            }
        }
    }

    // Playhead vertical line.
    let ph_x = view.sec_to_x(rect, view.playhead_sec);
    if ph_x >= rect.left() && ph_x <= rect.right() {
        p.line_segment(
            [Pos2::new(ph_x, rect.top()), Pos2::new(ph_x, rect.bottom())],
            Stroke::new(1.6_f32, TEXT_BRIGHT),
        );
    }

    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, BORDER_SUBTLE),
        egui::StrokeKind::Inside,
    );
}

fn paint_waveform(
    p: &Painter,
    rect: Rect,
    peaks: &WaveformPeaks,
    view: &LaneView,
    color: Color32,
    alpha: u8,
    scale: f32,
) {
    if peaks.buckets.is_empty() || peaks.duration_sec <= 0.0 {
        return;
    }
    let n = peaks.buckets.len();
    let mid_y = rect.center().y;
    let half_h = rect.height() * 0.45 * scale;
    let stroke = Stroke::new(1.2_f32, with_alpha(color, alpha));
    let px_count = (rect.width().ceil() as usize).max(1);
    for px in 0..px_count {
        let x = rect.left() + px as f32;
        let t0 = view.x_to_sec(rect, x);
        let t1 = view.x_to_sec(rect, x + 1.0);
        if t0 >= peaks.duration_sec || t1 <= 0.0 {
            continue;
        }
        let b0 = ((t0 / peaks.duration_sec) * n as f32).floor().max(0.0) as usize;
        let b1 = (((t1 / peaks.duration_sec) * n as f32).ceil() as usize)
            .max(b0 + 1)
            .min(n);
        let mut mn = 0.0_f32;
        let mut mx = 0.0_f32;
        for &(a, b) in &peaks.buckets[b0.min(n - 1)..b1] {
            mn = mn.min(a);
            mx = mx.max(b);
        }
        let y0 = mid_y - mx * half_h;
        let y1 = mid_y - mn * half_h;
        p.line_segment([Pos2::new(x, y0), Pos2::new(x, y1.max(y0 + 1.0))], stroke);
    }
}

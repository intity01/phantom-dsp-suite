//! Right Visualizer Card: Live Guide vs Dub Alignment Monitor
//!
//! Displays scrolling Guide Sidechain (Cyan) vs Dub Input (Purple) waveforms, live `±ms`
//! timing offset readout + bipolar deflection bar, color-coded Confidence badge
//! (`HIGH` green / `MED` amber / `LOW` red), PDC readout, and L/R input/output peak bars.

use super::align_view_meters::{draw_offset_strip, draw_peak_row};
use crate::params::VoxModeParam;
use crate::telemetry::{VoxTelemetrySnapshot, WAVE_BINS};
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Margin, Painter, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, BG_CARD,
    BG_DISPLAY, TEXT_BRIGHT, TEXT_DIMMED,
};

fn confidence_badge(snap: &VoxTelemetrySnapshot) -> (&'static str, Color32) {
    if !snap.sidechain_active {
        ("LOW · NO SC", ACCENT_RED)
    } else if snap.confidence >= 0.70 {
        ("HIGH", ACCENT_GREEN)
    } else if snap.confidence >= 0.35 {
        ("MED", ACCENT_AMBER)
    } else {
        ("LOW", ACCENT_RED)
    }
}

fn draw_wave_lane(
    painter: &Painter,
    rect: Rect,
    bins: &[f32; WAVE_BINS],
    color: Color32,
    label: &str,
) {
    let mid_y = rect.center().y;
    let half_h = (rect.height() * 0.43).max(6.0);
    painter.line_segment(
        [Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 16)),
    );

    let step = rect.width() / (WAVE_BINS.saturating_sub(1).max(1) as f32);
    let mut top_pts = Vec::with_capacity(WAVE_BINS);
    let mut bot_pts = Vec::with_capacity(WAVE_BINS);
    for (i, &amp) in bins.iter().enumerate() {
        let x = rect.min.x + i as f32 * step;
        let h = amp.clamp(0.0, 1.0).powf(0.65) * half_h;
        top_pts.push(Pos2::new(x, mid_y - h));
        bot_pts.push(Pos2::new(x, mid_y + h));
        if h > 1.0 {
            painter.line_segment(
                [Pos2::new(x, mid_y - h), Pos2::new(x, mid_y + h)],
                Stroke::new((step * 0.72).max(1.2_f32), with_alpha(color, 64)),
            );
        }
    }
    painter.add(Shape::line(
        top_pts,
        Stroke::new(1.3_f32, with_alpha(color, 225)),
    ));
    painter.add(Shape::line(
        bot_pts,
        Stroke::new(1.1_f32, with_alpha(color, 150)),
    ));
    painter.text(
        Pos2::new(rect.min.x + 7.0, rect.min.y + 9.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(8.5),
        color,
    );
}

pub fn draw_align_card(
    ui: &mut Ui,
    size: Vec2,
    mode: VoxModeParam,
    snap: &VoxTelemetrySnapshot,
    guide_wave: &[f32; WAVE_BINS],
    dub_wave: &[f32; WAVE_BINS],
) {
    let (badge_txt, badge_col) = confidence_badge(snap);
    let badge_label = if snap.sidechain_active {
        format!("{} {:.0}%", badge_txt, snap.confidence * 100.0)
    } else {
        badge_txt.to_string()
    };

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            // Pin the width: the right-to-left badge row below would otherwise grow the card
            // to the whole window and push the other card out of view.
            ui.set_width((size.x - 20.0).max(120.0));
            ui.set_min_height((size.y - 20.0).max(120.0));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("GUIDE VS DUB ALIGNMENT")
                        .font(FontId::proportional(10.0))
                        .strong()
                        .color(TEXT_BRIGHT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::NONE
                        .fill(with_alpha(badge_col, 38))
                        .stroke(Stroke::new(1.0_f32, with_alpha(badge_col, 150)))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(6, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(badge_label)
                                    .font(FontId::proportional(8.8))
                                    .strong()
                                    .color(badge_col),
                            );
                        });
                });
            });

            ui.add_space(4.0);
            draw_offset_strip(ui, snap, mode);
            ui.add_space(6.0);

            let wave_h = (ui.available_height() - 34.0).max(96.0);
            let (wave_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), wave_h), Sense::hover());
            let p = ui.painter();
            p.rect_filled(wave_rect, CornerRadius::same(6), BG_DISPLAY);
            p.rect_stroke(
                wave_rect,
                CornerRadius::same(6),
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
                StrokeKind::Inside,
            );

            let split_y = wave_rect.center().y;
            let top_lane = Rect::from_min_max(wave_rect.min, Pos2::new(wave_rect.max.x, split_y));
            let bot_lane = Rect::from_min_max(Pos2::new(wave_rect.min.x, split_y), wave_rect.max);
            p.line_segment(
                [
                    Pos2::new(wave_rect.min.x, split_y),
                    Pos2::new(wave_rect.max.x, split_y),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
            );
            draw_wave_lane(p, top_lane, guide_wave, ACCENT_CYAN, "GUIDE SIDECHAIN");
            draw_wave_lane(p, bot_lane, dub_wave, ACCENT_PURPLE, "DUB INPUT");

            if !snap.sidechain_active {
                p.text(
                    top_lane.center(),
                    Align2::CENTER_CENTER,
                    "Route Lead Vocal to 'Guide' Sidechain",
                    FontId::proportional(9.2),
                    TEXT_DIMMED,
                );
            }

            ui.add_space(5.0);
            draw_peak_row(ui, snap);
        });
}

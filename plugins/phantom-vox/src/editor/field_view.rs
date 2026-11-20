//! Left Visualizer Card: Stereo Vocal Field Polar / Arc Display + Phase Correlation Meter
//!
//! Renders a 180-degree vocal soundstage with Center Lead + animated L/R (and 2L/2R) Doubler
//! voices driven by `timing_ms`, `detune_cents`, `formant_st`, `width_pct`, and `mono_check`.

use super::phase_meter::draw_phase_correlation_bar;
use crate::params::{VoicesParam, VoxModeParam};
use crate::telemetry::VoxTelemetrySnapshot;
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Margin, Painter, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, BG_CARD, BG_DISPLAY,
    TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};
use std::f32::consts::PI;

pub struct FieldViewParams {
    pub mode: VoxModeParam,
    pub voices: VoicesParam,
    pub timing_ms: f32,
    pub detune_cents: f32,
    pub formant_st: f32,
    pub width_pct: f32,
    pub mono_check: bool,
    pub mix_pct: f32,
    pub bypassed: bool,
}

fn draw_semicircle(painter: &Painter, origin: Pos2, radius: f32, stroke: Stroke) {
    let segs = 36usize;
    let pts: Vec<Pos2> = (0..=segs)
        .map(|i| {
            let a = PI + PI * (i as f32 / segs as f32);
            Pos2::new(origin.x + radius * a.cos(), origin.y + radius * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts, stroke));
}

fn formant_tint(formant_st: f32, base: Color32) -> Color32 {
    if formant_st > 0.35 {
        ACCENT_PURPLE
    } else if formant_st < -0.35 {
        ACCENT_AMBER
    } else {
        base
    }
}

fn draw_polar_stage(
    painter: &Painter,
    rect: Rect,
    p: &FieldViewParams,
    snap: &VoxTelemetrySnapshot,
    time_s: f64,
) {
    painter.rect_filled(rect, CornerRadius::same(6), BG_DISPLAY);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
        StrokeKind::Inside,
    );

    let origin = Pos2::new(rect.center().x, rect.max.y - 24.0);
    let max_r = ((rect.width() * 0.44).min(rect.height() - 40.0)).max(54.0);
    let grid = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
    for frac in [0.33_f32, 0.66, 1.0] {
        draw_semicircle(painter, origin, max_r * frac, grid);
    }
    for deg in [180.0_f32, 135.0, 90.0, 45.0, 0.0] {
        let a = PI + deg.to_radians();
        let end = Pos2::new(origin.x + max_r * a.cos(), origin.y + max_r * a.sin());
        painter.line_segment([origin, end], grid);
    }

    let lbl_font = FontId::proportional(9.0);
    painter.text(
        Pos2::new(origin.x - max_r - 9.0, origin.y - 4.0),
        Align2::RIGHT_CENTER,
        "L",
        lbl_font.clone(),
        TEXT_DIMMED,
    );
    painter.text(
        Pos2::new(origin.x + max_r + 9.0, origin.y - 4.0),
        Align2::LEFT_CENTER,
        "R",
        lbl_font.clone(),
        TEXT_DIMMED,
    );
    painter.text(
        Pos2::new(origin.x, origin.y - max_r - 9.0),
        Align2::CENTER_CENTER,
        "C",
        lbl_font,
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(origin.x + max_r * 0.68, origin.y - max_r * 0.68),
        Align2::LEFT_CENTER,
        "35ms",
        FontId::proportional(8.0),
        TEXT_DIMMED,
    );

    let doubler_active = p.mode != VoxModeParam::LiveAlign && !p.bypassed;
    let level_norm = ((snap.out_peak_l_db.max(snap.out_peak_r_db) + 54.0) / 54.0).clamp(0.0, 1.0);
    let mix_alpha = ((0.25 + 0.75 * (p.mix_pct * 0.01)) * 255.0).clamp(40.0, 255.0) as u8;

    if doubler_active {
        let voice_specs: [(&str, f32, f32, f32, f32); 4] = [
            ("L1", -0.68, 0.85, 0.43, 0.0),
            ("R1", 0.68, 1.05, 0.59, 1.57),
            ("L2", -0.96, 1.38, 0.73, 3.14),
            ("R2", 0.96, 1.55, 0.89, 4.71),
        ];
        let count = if p.voices == VoicesParam::FourVoices {
            4
        } else {
            2
        };
        let width_scale = if p.mono_check {
            0.0
        } else {
            (p.width_pct / 150.0).clamp(0.0, 1.0)
        };
        let timing_norm = (p.timing_ms / 35.0).clamp(0.0, 1.0);
        let detune_wobble = (p.detune_cents / 35.0).clamp(0.0, 1.0) * 0.07;
        let formant_shift_y = -p.formant_st * 2.4;

        for &(tag, pan, depth_scale, hz, phase) in voice_specs.iter().take(count) {
            let lfo = ((time_s as f32) * hz * 2.0 * PI + phase).sin();
            let angle = -0.5 * PI + pan * width_scale * (0.44 * PI) + lfo * detune_wobble;
            let radius = (max_r
                * (0.24 + 0.56 * timing_norm * depth_scale * 0.72 + lfo * detune_wobble))
                .clamp(18.0, max_r * 0.96);
            let pos = Pos2::new(
                origin.x + radius * angle.cos(),
                (origin.y + radius * angle.sin() + formant_shift_y)
                    .clamp(rect.min.y + 16.0, origin.y - 6.0),
            );
            let col = formant_tint(
                p.formant_st,
                if pan < 0.0 { ACCENT_CYAN } else { ACCENT_GREEN },
            );
            painter.line_segment(
                [origin, pos],
                Stroke::new(1.1_f32, with_alpha(col, (mix_alpha as f32 * 0.38) as u8)),
            );
            let pr = 7.5 + level_norm * 4.0;
            painter.circle_filled(pos, pr + 3.5, with_alpha(col, 34));
            painter.circle_filled(pos, pr, with_alpha(col, mix_alpha));
            painter.circle_stroke(pos, pr, Stroke::new(1.1_f32, TEXT_BRIGHT));
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                tag,
                FontId::proportional(7.5),
                BG_DISPLAY,
            );
        }
    } else {
        painter.text(
            Pos2::new(origin.x, rect.center().y - 10.0),
            Align2::CENTER_CENTER,
            if p.bypassed {
                "BYPASSED"
            } else {
                "DOUBLER STANDBY (LIVE ALIGN)"
            },
            FontId::proportional(9.5),
            TEXT_DIMMED,
        );
    }

    let lead_col = if p.bypassed { TEXT_DIMMED } else { ACCENT_CYAN };
    let lead_r = 10.0 + level_norm * 5.0;
    painter.circle_filled(origin, lead_r + 6.0, with_alpha(lead_col, 32));
    painter.circle_filled(origin, lead_r, with_alpha(lead_col, 220));
    painter.circle_stroke(origin, lead_r, Stroke::new(1.4_f32, TEXT_BRIGHT));
    painter.text(
        origin,
        Align2::CENTER_CENTER,
        "LEAD",
        FontId::proportional(7.5),
        BG_DISPLAY,
    );

    let caption = format!(
        "{:.1}ms · {:.0}ct · {:+.1}st · {:.0}%W",
        p.timing_ms, p.detune_cents, p.formant_st, p.width_pct
    );
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 10.0),
        Align2::LEFT_CENTER,
        caption,
        FontId::proportional(8.8),
        TEXT_MUTED,
    );
}

pub fn draw_field_card(ui: &mut Ui, size: Vec2, p: &FieldViewParams, snap: &VoxTelemetrySnapshot) {
    let time_s = ui.input(|i| i.time);
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
                    RichText::new("STEREO VOCAL FIELD")
                        .font(FontId::proportional(10.0))
                        .strong()
                        .color(TEXT_BRIGHT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let badge = if p.mono_check {
                        ("MONO CHECK", ACCENT_AMBER)
                    } else if p.mode == VoxModeParam::LiveAlign {
                        ("ALIGN PASS", ACCENT_GREEN)
                    } else if p.voices == VoicesParam::FourVoices {
                        ("4 VOICES", ACCENT_PURPLE)
                    } else {
                        ("2 VOICES", ACCENT_CYAN)
                    };
                    ui.label(
                        RichText::new(badge.0)
                            .font(FontId::proportional(9.0))
                            .strong()
                            .color(badge.1),
                    );
                });
            });

            ui.add_space(4.0);
            let polar_h = (ui.available_height() - 40.0).max(120.0);
            let (polar_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), polar_h), Sense::hover());
            draw_polar_stage(ui.painter(), polar_rect, p, snap, time_s);

            ui.add_space(6.0);
            draw_phase_correlation_bar(ui, snap.phase_corr);
        });
}

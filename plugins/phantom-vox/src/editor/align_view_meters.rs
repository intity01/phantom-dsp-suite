//! Sub-components for align visualizer: offset bipolar strip and peak level meters.

use crate::params::VoxModeParam;
use crate::telemetry::VoxTelemetrySnapshot;
use nih_plug_egui::egui::{
    self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_RED, BG_DISPLAY, TEXT_BRIGHT,
    TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_offset_strip(ui: &mut Ui, snap: &VoxTelemetrySnapshot, mode: VoxModeParam) {
    let off = snap.offset_ms.clamp(-45.0, 45.0);
    let status = if !snap.sidechain_active {
        ("STANDBY", TEXT_DIMMED)
    } else if off.abs() < 1.5 {
        ("LOCKED", ACCENT_GREEN)
    } else if off > 0.0 {
        ("DUB LATE", ACCENT_AMBER)
    } else {
        ("DUB EARLY", ACCENT_CYAN)
    };

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{:+5.1} ms", off))
                .font(FontId::monospace(15.0))
                .strong()
                .color(if snap.sidechain_active {
                    TEXT_BRIGHT
                } else {
                    TEXT_DIMMED
                }),
        );
        ui.label(
            RichText::new(status.0)
                .font(FontId::proportional(9.0))
                .strong()
                .color(status.1),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pdc_txt = if mode == VoxModeParam::Doubler || snap.latency_samples == 0 {
                "PDC 0 ms".to_string()
            } else {
                format!(
                    "PDC {:.1} ms",
                    snap.latency_samples as f32 * 1000.0 / snap.sample_rate.max(8000.0)
                )
            };
            ui.label(
                RichText::new(pdc_txt)
                    .font(FontId::monospace(9.0))
                    .color(TEXT_MUTED),
            );
        });
    });

    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(bar, CornerRadius::same(3), BG_DISPLAY);
    let mid_x = bar.center().x;
    let cur_x = mid_x + (off / 40.0).clamp(-1.0, 1.0) * (bar.width() * 0.48);
    let bar_col = if !snap.sidechain_active {
        TEXT_DIMMED
    } else if off.abs() < 4.0 {
        ACCENT_GREEN
    } else if off.abs() < 16.0 {
        ACCENT_AMBER
    } else {
        ACCENT_RED
    };
    let fill = Rect::from_min_max(
        Pos2::new(mid_x.min(cur_x), bar.min.y + 2.0),
        Pos2::new(mid_x.max(cur_x), bar.max.y - 2.0),
    );
    p.rect_filled(fill, CornerRadius::same(2), with_alpha(bar_col, 195));
    p.line_segment(
        [Pos2::new(mid_x, bar.min.y), Pos2::new(mid_x, bar.max.y)],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
    );
    p.rect_stroke(
        bar,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
        StrokeKind::Inside,
    );
}

pub fn draw_peak_row(ui: &mut Ui, snap: &VoxTelemetrySnapshot) {
    let norm = |db: f32| ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let draw_pair = |ui: &mut Ui, tag: &str, l_db: f32, r_db: f32, col: Color32, w: f32| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tag)
                        .font(FontId::proportional(8.2))
                        .strong()
                        .color(TEXT_MUTED),
                );
                ui.label(
                    RichText::new(format!("{:.0}/{:.0} dB", l_db.max(-60.0), r_db.max(-60.0)))
                        .font(FontId::monospace(8.2))
                        .color(TEXT_DIMMED),
                );
            });
            for db in [l_db, r_db] {
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 4.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(r, CornerRadius::same(2), BG_DISPLAY);
                let fill_w = r.width() * norm(db);
                if fill_w > 0.5 {
                    let c = if db > -1.0 { ACCENT_RED } else { col };
                    p.rect_filled(
                        Rect::from_min_size(r.min, Vec2::new(fill_w, r.height())),
                        CornerRadius::same(2),
                        c,
                    );
                }
            }
        });
    };
    let half_w = ((ui.available_width() - 12.0) * 0.5).max(80.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        draw_pair(
            ui,
            "IN",
            snap.in_peak_l_db,
            snap.in_peak_r_db,
            ACCENT_CYAN,
            half_w,
        );
        draw_pair(
            ui,
            "OUT",
            snap.out_peak_l_db,
            snap.out_peak_r_db,
            ACCENT_GREEN,
            half_w,
        );
    });
}

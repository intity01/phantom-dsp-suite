//! Live Phase Correlation Meter (`-1.0 .. +1.0`) with Red/Amber/Green Safety Zones

use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};
use phantom_ui::{
    with_alpha, ACCENT_AMBER, ACCENT_GREEN, ACCENT_RED, BG_DISPLAY, TEXT_BRIGHT, TEXT_DIMMED,
    TEXT_MUTED,
};

pub fn draw_phase_correlation_bar(ui: &mut Ui, corr: f32) {
    let corr = corr.clamp(-1.0, 1.0);
    let corr_col = if corr < 0.0 {
        ACCENT_RED
    } else if corr < 0.45 {
        ACCENT_AMBER
    } else {
        ACCENT_GREEN
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("PHASE CORRELATION")
                .font(FontId::proportional(8.8))
                .strong()
                .color(TEXT_MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:+.2}", corr))
                    .font(FontId::monospace(10.0))
                    .strong()
                    .color(corr_col),
            );
        });
    });

    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(bar, CornerRadius::same(3), BG_DISPLAY);

    let x_at = |v: f32| bar.min.x + ((v + 1.0) * 0.5).clamp(0.0, 1.0) * bar.width();
    for (a, b, col) in [
        (-1.0_f32, 0.0_f32, ACCENT_RED),
        (0.0, 0.45, ACCENT_AMBER),
        (0.45, 1.0, ACCENT_GREEN),
    ] {
        let zr = Rect::from_min_max(
            Pos2::new(x_at(a), bar.min.y + 2.0),
            Pos2::new(x_at(b), bar.max.y - 2.0),
        );
        p.rect_filled(zr, CornerRadius::same(2), with_alpha(col, 48));
    }

    let (x0, xv) = (x_at(0.0), x_at(corr));
    let fill_r = Rect::from_min_max(
        Pos2::new(x0.min(xv), bar.min.y + 3.0),
        Pos2::new(x0.max(xv), bar.max.y - 3.0),
    );
    p.rect_filled(fill_r, CornerRadius::same(2), with_alpha(corr_col, 175));
    p.line_segment(
        [Pos2::new(x0, bar.min.y), Pos2::new(x0, bar.max.y)],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
    );
    p.line_segment(
        [
            Pos2::new(xv, bar.min.y + 1.0),
            Pos2::new(xv, bar.max.y - 1.0),
        ],
        Stroke::new(2.2_f32, TEXT_BRIGHT),
    );
    p.rect_stroke(
        bar,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 26)),
        StrokeKind::Inside,
    );

    let tick_font = FontId::proportional(7.8);
    p.text(
        Pos2::new(bar.min.x + 4.0, bar.center().y),
        Align2::LEFT_CENTER,
        "-1",
        tick_font.clone(),
        TEXT_DIMMED,
    );
    p.text(
        Pos2::new(x0, bar.center().y),
        Align2::CENTER_CENTER,
        "0",
        tick_font.clone(),
        TEXT_MUTED,
    );
    p.text(
        Pos2::new(bar.max.x - 4.0, bar.center().y),
        Align2::RIGHT_CENTER,
        "+1",
        tick_font,
        TEXT_DIMMED,
    );
}

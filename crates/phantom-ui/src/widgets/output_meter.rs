//! Compact Vertical Stereo Output Peak & EBU R128 LUFS Meter Widget
//! Designed to sit cleanly along the right edge of the main EQ frequency canvas.

use crate::theme::{
    with_alpha, ACCENT_AMBER, ACCENT_GREEN, ACCENT_RED, BG_DISPLAY, TEXT_BRIGHT, TEXT_DIMMED,
    TEXT_MUTED,
};
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

/// Recommended fixed width (in logical points) for the vertical output meter strip.
pub const OUTPUT_METER_WIDTH: f32 = 54.0;

#[derive(Debug, Clone, Copy)]
pub struct OutputMeterData {
    pub peak_l_db: f32,
    pub peak_r_db: f32,
    pub hold_l_db: f32,
    pub hold_r_db: f32,
    pub momentary_lufs: f32,
    pub short_term_lufs: f32,
}

impl Default for OutputMeterData {
    fn default() -> Self {
        Self {
            peak_l_db: -96.0,
            peak_r_db: -96.0,
            hold_l_db: -96.0,
            hold_r_db: -96.0,
            momentary_lufs: -70.0,
            short_term_lufs: -70.0,
        }
    }
}

/// Piecewise audio meter mapping (`-60..=+3 dBFS` -> `0.0..=1.0`) giving high resolution near 0 dB.
#[inline]
fn db_to_meter_norm(db: f32) -> f32 {
    if !db.is_finite() || db <= -60.0 {
        return 0.0;
    }
    if db >= 3.0 {
        return 1.0;
    }
    if db >= 0.0 {
        0.94 + (db / 3.0) * 0.06
    } else if db >= -12.0 {
        0.62 + ((db + 12.0) / 12.0) * 0.32
    } else if db >= -24.0 {
        0.36 + ((db + 24.0) / 12.0) * 0.26
    } else if db >= -48.0 {
        0.10 + ((db + 48.0) / 24.0) * 0.26
    } else {
        ((db + 60.0) / 12.0) * 0.10
    }
}

fn draw_channel_bar(
    painter: &nih_plug_egui::egui::Painter,
    bar_rect: Rect,
    peak_db: f32,
    hold_db: f32,
) {
    painter.rect_filled(
        bar_rect,
        CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(255, 255, 255, 10),
    );

    let level_norm = db_to_meter_norm(peak_db);
    let y_for = |norm: f32| bar_rect.max.y - norm * bar_rect.height();

    // Segmented green (-60..-12), amber (-12..-3), red (-3..+3)
    let split_amber = db_to_meter_norm(-12.0);
    let split_red = db_to_meter_norm(-3.0);

    let segments = [
        (0.0, split_amber, ACCENT_GREEN),
        (split_amber, split_red, ACCENT_AMBER),
        (split_red, 1.0, ACCENT_RED),
    ];

    for (lo, hi, color) in segments {
        if level_norm <= lo {
            break;
        }
        let seg_top = level_norm.min(hi);
        let seg_rect = Rect::from_min_max(
            Pos2::new(bar_rect.min.x, y_for(seg_top)),
            Pos2::new(bar_rect.max.x, y_for(lo)),
        );
        if seg_rect.height() > 0.3 {
            painter.rect_filled(seg_rect, CornerRadius::ZERO, color);
        }
    }

    // Crisp 2px peak-hold cap
    let hold_norm = db_to_meter_norm(hold_db);
    if hold_norm > 0.01 {
        let hy = y_for(hold_norm);
        let cap_col = if hold_db >= -0.2 {
            ACCENT_RED
        } else if hold_db >= -6.0 {
            ACCENT_AMBER
        } else {
            Color32::from_rgba_unmultiplied(226, 232, 240, 200)
        };
        painter.line_segment(
            [Pos2::new(bar_rect.min.x, hy), Pos2::new(bar_rect.max.x, hy)],
            Stroke::new(2.0_f32, cap_col),
        );
    }

    // Top clip indicator cap when peak >= 0 dBFS
    if hold_db >= 0.0 || peak_db >= 0.0 {
        let clip_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width(), 3.0));
        painter.rect_filled(clip_rect, CornerRadius::same(1), ACCENT_RED);
    }
}

/// Draws a sleek vertical stereo peak meter with dB scale ticks and live LUFS numeric readout.
pub fn draw_output_meter(ui: &mut Ui, data: &OutputMeterData, height: f32) {
    let total_size = Vec2::new(OUTPUT_METER_WIDTH, height.max(120.0));
    let (response, painter) = ui.allocate_painter(total_size, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, CornerRadius::same(6), BG_DISPLAY);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
        StrokeKind::Inside,
    );

    let readout_h = 34.0;
    let meter_top = rect.min.y + 8.0;
    let meter_bot = (rect.max.y - readout_h - 6.0).max(meter_top + 40.0);
    let meter_h = meter_bot - meter_top;

    let bar_w = 8.0;
    let bar_gap = 3.0;
    let left_x = rect.min.x + 8.0;
    let right_x = left_x + bar_w + bar_gap;

    let bar_l = Rect::from_min_size(Pos2::new(left_x, meter_top), Vec2::new(bar_w, meter_h));
    let bar_r = Rect::from_min_size(Pos2::new(right_x, meter_top), Vec2::new(bar_w, meter_h));

    draw_channel_bar(&painter, bar_l, data.peak_l_db, data.hold_l_db);
    draw_channel_bar(&painter, bar_r, data.peak_r_db, data.hold_r_db);

    // Subtle dB scale marks along right side of meter bars
    let tick_col = Color32::from_rgba_unmultiplied(148, 163, 184, 125);
    let tick_x = right_x + bar_w + 3.0;
    for &(db, label) in &[
        (0.0_f32, "0"),
        (-6.0, "-6"),
        (-12.0, "-12"),
        (-24.0, "-24"),
        (-48.0, "-48"),
    ] {
        let y = meter_bot - db_to_meter_norm(db) * meter_h;
        painter.line_segment(
            [Pos2::new(right_x + bar_w + 1.0, y), Pos2::new(tick_x, y)],
            Stroke::new(1.0_f32, with_alpha(TEXT_MUTED, 70)),
        );
        painter.text(
            Pos2::new(rect.max.x - 4.0, y),
            Align2::RIGHT_CENTER,
            label,
            FontId::proportional(8.0),
            tick_col,
        );
    }

    // Divider line above numeric LUFS readout
    let sep_y = rect.max.y - readout_h;
    painter.line_segment(
        [
            Pos2::new(rect.min.x + 5.0, sep_y),
            Pos2::new(rect.max.x - 5.0, sep_y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
    );

    // Numeric LUFS readout (momentary loudness)
    let lufs = data.momentary_lufs;
    let (lufs_str, lufs_col) = if lufs.is_finite() && lufs > -69.5 {
        let col = if lufs > -6.0 {
            ACCENT_RED
        } else if lufs > -10.0 {
            ACCENT_AMBER
        } else {
            TEXT_BRIGHT
        };
        (format!("{:.1}", lufs), col)
    } else {
        ("---".to_string(), TEXT_DIMMED)
    };

    painter.text(
        Pos2::new(rect.center().x, sep_y + 13.0),
        Align2::CENTER_CENTER,
        lufs_str,
        FontId::proportional(10.5),
        lufs_col,
    );
    painter.text(
        Pos2::new(rect.center().x, sep_y + 25.0),
        Align2::CENTER_CENTER,
        "LUFS",
        FontId::proportional(8.0),
        TEXT_MUTED,
    );

    response.on_hover_ui(|ui| {
        let fmt_db = |v: f32| {
            if v.is_finite() && v > -90.0 {
                format!("{:+.1} dBFS", v)
            } else {
                "-inf dBFS".to_string()
            }
        };
        let fmt_lu = |v: f32| {
            if v.is_finite() && v > -69.5 {
                format!("{:.1} LUFS", v)
            } else {
                "--- LUFS".to_string()
            }
        };
        ui.label(format!("Peak L: {}", fmt_db(data.peak_l_db)));
        ui.label(format!("Peak R: {}", fmt_db(data.peak_r_db)));
        ui.label(format!("Momentary: {}", fmt_lu(data.momentary_lufs)));
        ui.label(format!("Short-Term: {}", fmt_lu(data.short_term_lufs)));
    });
}

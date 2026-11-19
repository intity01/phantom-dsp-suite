//! Compact Icon Buttons & Unit-Clean Sliders for Band Inspector
//! Keeps `band_inspector.rs` clean, modular, and strictly under 300 lines.

use crate::icons::{
    draw_close_icon, draw_filter_shape_icon, draw_headphones_icon, draw_power_icon,
};
use crate::theme::{with_alpha, ACCENT_AMBER, ACCENT_RED, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED};
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Slider, Stroke,
    StrokeKind, Ui, Vec2,
};

/// Draws a sleek square vector icon button with active tint and hover feedback.
pub fn draw_icon_button(
    ui: &mut Ui,
    size: Vec2,
    active: bool,
    active_color: Color32,
    hover_color: Color32,
    sense: Sense,
    draw_icon: impl FnOnce(&egui::Painter, egui::Pos2, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter();

    let bg = if active {
        with_alpha(active_color, 44)
    } else if response.hovered() {
        with_alpha(hover_color, 30)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 10)
    };
    let border = if active {
        with_alpha(active_color, 175)
    } else if response.hovered() {
        with_alpha(hover_color, 110)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    };
    let icon_col = if active {
        active_color
    } else if response.hovered() {
        hover_color
    } else {
        TEXT_MUTED
    };

    painter.rect_filled(rect, CornerRadius::same(4), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    draw_icon(painter, rect.center(), icon_col);
    response
}

/// Draws the Band Power (On/Off) Lucide vector button.
pub fn power_button(ui: &mut Ui, enabled: bool, band_color: Color32) -> Response {
    draw_icon_button(
        ui,
        Vec2::new(24.0, 22.0),
        enabled,
        band_color,
        band_color,
        Sense::click(),
        |painter, center, color| draw_power_icon(painter, center, 5.5, color, 1.5),
    )
    .on_hover_text(if enabled {
        "Bypass band (right-click node)"
    } else {
        "Enable band (right-click node)"
    })
}

/// Draws the prominent Headphone Solo Audition button supporting BOTH click-toggle & press-and-hold.
pub fn solo_headphones_button(ui: &mut Ui, is_soloed: bool) -> Response {
    draw_icon_button(
        ui,
        Vec2::new(28.0, 22.0),
        is_soloed,
        ACCENT_AMBER,
        ACCENT_AMBER,
        Sense::click_and_drag(),
        |painter, center, color| draw_headphones_icon(painter, center, 6.2, color, 1.5),
    )
    .on_hover_text("Band Solo Audition — click to toggle, or press & hold to audition")
}

/// Draws the Lucide Close (`×`) button to delete a band.
pub fn delete_band_button(ui: &mut Ui) -> Response {
    draw_icon_button(
        ui,
        Vec2::new(22.0, 22.0),
        false,
        ACCENT_RED,
        ACCENT_RED,
        Sense::click(),
        |painter, center, color| draw_close_icon(painter, center, 5.0, color, 1.5),
    )
    .on_hover_text("Delete band (Del / Backspace)")
}

/// Draws a compact Filter Shape ComboBox with Lucide mini curve icons for all 10 shapes.
pub fn shape_combo(
    ui: &mut Ui,
    current_idx: usize,
    filter_names: &[&'static str],
    accent: Color32,
) -> Option<usize> {
    let mut picked = None;
    let current_name = filter_names.get(current_idx).copied().unwrap_or("Bell");

    // Inline preview icon directly beside the shape dropdown
    let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 14.0), Sense::hover());
    draw_filter_shape_icon(ui.painter(), icon_rect, current_idx, accent, 1.5);

    egui::ComboBox::from_id_salt("phantom_band_shape")
        .width(86.0)
        .selected_text(RichText::new(current_name).font(FontId::proportional(10.5)))
        .show_ui(ui, |ui| {
            for (i, &name) in filter_names.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(18.0, 12.0), Sense::hover());
                    let c = if i == current_idx { accent } else { TEXT_MUTED };
                    draw_filter_shape_icon(ui.painter(), r, i, c, 1.4);
                    if ui
                        .selectable_label(
                            i == current_idx,
                            RichText::new(name).font(FontId::proportional(10.5)),
                        )
                        .clicked()
                    {
                        picked = Some(i);
                    }
                });
            }
        });
    picked
}

/// Draws a compact Filter Slope ComboBox (`6 dB/oct` .. `Brickwall`).
pub fn slope_combo(ui: &mut Ui, current_idx: usize, slope_names: &[&'static str]) -> Option<usize> {
    let mut picked = None;
    let current_name = slope_names.get(current_idx).copied().unwrap_or("12 dB");
    egui::ComboBox::from_id_salt("phantom_band_slope")
        .width(66.0)
        .selected_text(RichText::new(current_name).font(FontId::proportional(10.5)))
        .show_ui(ui, |ui| {
            for (i, &name) in slope_names.iter().enumerate() {
                if ui
                    .selectable_label(
                        i == current_idx,
                        RichText::new(name).font(FontId::proportional(10.5)),
                    )
                    .clicked()
                {
                    picked = Some(i);
                }
            }
        });
    picked
}

/// Parses typed numeric values with optional `Hz`, `kHz`, `dB`, or `ms` unit suffixes.
pub fn parse_audio_number(s: &str, is_freq: bool) -> Option<f64> {
    let trimmed = s.trim();
    let lower = trimmed.to_ascii_lowercase();
    if is_freq && lower.ends_with("khz") {
        return lower
            .trim_end_matches("khz")
            .trim()
            .parse::<f64>()
            .ok()
            .map(|v| v * 1000.0);
    }
    if is_freq && lower.ends_with('k') {
        return lower
            .trim_end_matches('k')
            .trim()
            .parse::<f64>()
            .ok()
            .map(|v| v * 1000.0);
    }
    let cleaned = lower
        .trim_end_matches("hz")
        .trim_end_matches("db")
        .trim_end_matches("ms")
        .trim();
    cleaned.parse::<f64>().ok()
}

/// Adds a compact slider with custom unit formatting (no duplicate suffix).
pub fn compact_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    logarithmic: bool,
    is_freq: bool,
    formatter: impl Fn(f64, std::ops::RangeInclusive<usize>) -> String + 'static,
) -> Response {
    ui.label(
        RichText::new(label)
            .font(FontId::proportional(10.5))
            .color(TEXT_MUTED),
    );
    let mut slider = Slider::new(value, range)
        .custom_formatter(formatter)
        .custom_parser(move |s| parse_audio_number(s, is_freq));
    if logarithmic {
        slider = slider.logarithmic(true);
    }
    ui.add(slider)
}

/// Renders a subtle muted live Gain Reduction numeric badge.
pub fn draw_live_gr_pill(ui: &mut Ui, live_gr_db: f32) {
    let (txt, col) = if live_gr_db.abs() > 0.05 {
        let c = if live_gr_db > 0.0 {
            Color32::from_rgb(52, 211, 153)
        } else {
            Color32::from_rgb(251, 113, 133)
        };
        (format!("GR {:+.1} dB", live_gr_db), c)
    } else {
        ("GR 0.0 dB".to_string(), TEXT_DIMMED)
    };
    ui.label(
        RichText::new(txt)
            .font(FontId::proportional(10.0))
            .color(col),
    );
}

/// Draws a compact HUD pill button with active tint and hover highlight.
pub fn draw_hud_pill(
    ui: &mut Ui,
    w: f32,
    h: f32,
    text: &str,
    active: bool,
    accent: Color32,
) -> Response {
    let (rect, res) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hov = res.hovered();
    let bg = if active {
        with_alpha(accent, 48)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, if hov { 24 } else { 11 })
    };
    let border = if active || hov {
        with_alpha(accent, if active { 180 } else { 110 })
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 26)
    };
    let fg = if active {
        accent
    } else if hov {
        TEXT_BRIGHT
    } else {
        TEXT_MUTED
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), bg);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(9.0),
        fg,
    );
    res
}

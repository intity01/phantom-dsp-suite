//! Pro-Q 4 Spectrum Grab Peak Detector & Interactive Resonance Overlay
//! Detects prominent resonant peaks (> 4.5 dB prominence above neighboring trough, > -65 dB),
//! estimates Q from peak bandwidth, and renders glowing grab handles with musical note readouts.

use crate::theme::{with_alpha, ACCENT_AMBER, ACCENT_CYAN, TEXT_BRIGHT};
use crate::views::curve_plot::EqCanvasMapping;
use crate::views::piano_roll::freq_to_midi_note;
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};

const SPECTRUM_MIN_DB: f32 = -90.0;
const SPECTRUM_RANGE_DB: f32 = 100.0;
const MIN_PEAK_DB: f32 = -65.0;
const MIN_PROMINENCE_DB: f32 = 4.5;
const HOVER_HIT_RADIUS: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumPeak {
    pub bin_idx: usize,
    pub freq_hz: f32,
    pub db: f32,
    pub screen_pos: Pos2,
    pub estimated_q: f32,
}

#[inline]
fn bin_to_freq_and_pos(
    map: &EqCanvasMapping,
    bin_idx: usize,
    num_bins: usize,
    db: f32,
) -> (f32, Pos2) {
    let denom = (num_bins.saturating_sub(1)).max(1) as f32;
    let norm = (bin_idx as f32 / denom).clamp(0.0, 1.0);
    let x = map.rect.min.x + norm * map.rect.width();
    let freq_hz = map.x_to_freq(x);
    let db_norm = ((db - SPECTRUM_MIN_DB) / SPECTRUM_RANGE_DB).clamp(0.0, 1.0);
    let y = map.rect.max.y - db_norm * map.rect.height();
    (freq_hz, Pos2::new(x, y))
}

fn estimate_peak_q(
    map: &EqCanvasMapping,
    bins_db: &[f32],
    center_idx: usize,
    prominence: f32,
) -> f32 {
    let n = bins_db.len();
    let peak_db = bins_db[center_idx];
    let drop_db = (prominence * 0.5).clamp(2.2, 6.0);
    let target_db = peak_db - drop_db;

    let mut left_idx = center_idx as f32;
    for i in (1..=center_idx).rev() {
        if bins_db[i - 1] <= target_db {
            let span = (bins_db[i] - bins_db[i - 1]).max(0.1);
            let frac = ((bins_db[i] - target_db) / span).clamp(0.0, 1.0);
            left_idx = i as f32 - frac;
            break;
        }
        left_idx = (i - 1) as f32;
    }

    let mut right_idx = center_idx as f32;
    for i in center_idx..(n - 1) {
        if bins_db[i + 1] <= target_db {
            let span = (bins_db[i] - bins_db[i + 1]).max(0.1);
            let frac = ((bins_db[i] - target_db) / span).clamp(0.0, 1.0);
            right_idx = i as f32 + frac;
            break;
        }
        right_idx = (i + 1) as f32;
    }

    let denom = (n - 1).max(1) as f32;
    let x_lo = map.rect.min.x + (left_idx / denom).clamp(0.0, 1.0) * map.rect.width();
    let x_hi = map.rect.min.x + (right_idx / denom).clamp(0.0, 1.0) * map.rect.width();
    let f_lo = map.x_to_freq(x_lo).max(10.0);
    let f_hi = map.x_to_freq(x_hi).max(f_lo + 1.0);
    let bw_oct = (f_hi / f_lo).log2().max(0.07);
    (1.414 / bw_oct).clamp(1.2, 18.0)
}

/// Detects up to `max_peaks` prominent local maxima (`> 4.5 dB` prominence, `> -65 dB`)
/// across `bins_db` and estimates resonant Q from local peak bandwidth.
pub fn detect_spectrum_peaks(
    map: &EqCanvasMapping,
    bins_db: &[f32],
    max_peaks: usize,
) -> Vec<SpectrumPeak> {
    let n = bins_db.len();
    if n < 7 || max_peaks == 0 {
        return Vec::new();
    }

    let window = (n / 14).clamp(6, 24);
    let min_sep = (n / 28).clamp(4, 12);
    let mut candidates: Vec<(f32, SpectrumPeak)> = Vec::new();

    for i in 2..(n - 2) {
        let db = bins_db[i];
        if db <= MIN_PEAK_DB || db < bins_db[i - 1] || db < bins_db[i + 1] {
            continue;
        }
        if db == bins_db[i - 1] && db == bins_db[i + 1] {
            continue;
        }

        let l_start = i.saturating_sub(window);
        let r_end = (i + window).min(n - 1);
        let left_trough = bins_db[l_start..i].iter().copied().fold(db, f32::min);
        let right_trough = bins_db[(i + 1)..=r_end].iter().copied().fold(db, f32::min);
        let ref_trough = left_trough.max(right_trough);
        let prominence = db - ref_trough;

        if prominence >= MIN_PROMINENCE_DB {
            let (freq_hz, screen_pos) = bin_to_freq_and_pos(map, i, n, db);
            let estimated_q = estimate_peak_q(map, bins_db, i, prominence);
            let score = prominence + (db - MIN_PEAK_DB) * 0.25;
            candidates.push((
                score,
                SpectrumPeak {
                    bin_idx: i,
                    freq_hz,
                    db,
                    screen_pos,
                    estimated_q,
                },
            ));
        }
    }

    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut chosen: Vec<SpectrumPeak> = Vec::with_capacity(max_peaks);
    for (_, peak) in candidates {
        if chosen.len() >= max_peaks {
            break;
        }
        if chosen
            .iter()
            .all(|p| p.bin_idx.abs_diff(peak.bin_idx) >= min_sep)
        {
            chosen.push(peak);
        }
    }
    chosen
}

/// Renders glowing cyan Spectrum Grab rings and frequency/note callout tags over detected peaks.
/// Returns `Some(hovered_peak)` if `pointer_pos` is within `16px` of a detected peak.
pub fn draw_spectrum_grab_overlay(
    painter: &Painter,
    map: &EqCanvasMapping,
    peaks: &[SpectrumPeak],
    pointer_pos: Option<Pos2>,
) -> Option<SpectrumPeak> {
    if peaks.is_empty() {
        return None;
    }

    let mut hovered_idx: Option<usize> = None;
    let mut best_dist = HOVER_HIT_RADIUS;
    if let Some(ptr) = pointer_pos {
        for (idx, p) in peaks.iter().enumerate() {
            let d = ptr.distance(p.screen_pos);
            if d <= best_dist {
                best_dist = d;
                hovered_idx = Some(idx);
            }
        }
    }

    for (idx, peak) in peaks.iter().enumerate() {
        let is_hov = hovered_idx == Some(idx);
        let col = if is_hov { ACCENT_AMBER } else { ACCENT_CYAN };
        let r = if is_hov { 8.0_f32 } else { 5.5_f32 };

        painter.line_segment(
            [
                peak.screen_pos,
                Pos2::new(
                    peak.screen_pos.x,
                    (peak.screen_pos.y + 22.0).min(map.rect.max.y),
                ),
            ],
            Stroke::new(1.0_f32, with_alpha(col, if is_hov { 140 } else { 70 })),
        );
        painter.circle_filled(
            peak.screen_pos,
            r + 3.0,
            with_alpha(col, if is_hov { 55 } else { 28 }),
        );
        painter.circle_stroke(
            peak.screen_pos,
            r,
            Stroke::new(
                if is_hov { 2.0_f32 } else { 1.4_f32 },
                with_alpha(col, if is_hov { 240 } else { 175 }),
            ),
        );
        painter.circle_filled(peak.screen_pos, 2.2, col);
    }

    // Draw frequency/note callout tag for hovered peak (or top prominent peak)
    let callout_idx = hovered_idx.unwrap_or(0);
    if let Some(peak) = peaks.get(callout_idx) {
        let is_hov = hovered_idx.is_some();
        let (_, note, oct, _) = freq_to_midi_note(peak.freq_hz);
        let f_str = if peak.freq_hz >= 1000.0 {
            format!("{:.2} kHz", peak.freq_hz / 1000.0)
        } else {
            format!("{:.0} Hz", peak.freq_hz)
        };
        let tag = if is_hov {
            format!("{} ({}{}) • Drag to tame", f_str, note, oct)
        } else {
            format!("{} ({}{})", f_str, note, oct)
        };
        let tag_w = if is_hov { 176.0 } else { 96.0 };
        let tag_h = 18.0;
        let tx = (peak.screen_pos.x - tag_w * 0.5).clamp(
            map.rect.min.x + 6.0,
            (map.rect.max.x - tag_w - 6.0).max(map.rect.min.x + 6.0),
        );
        let ty = (peak.screen_pos.y - tag_h - 10.0).max(map.rect.min.y + 6.0);
        let tag_rect = Rect::from_min_size(Pos2::new(tx, ty), Vec2::new(tag_w, tag_h));
        let col = if is_hov { ACCENT_AMBER } else { ACCENT_CYAN };

        painter.rect_filled(
            tag_rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(14, 16, 24, 232),
        );
        painter.rect_stroke(
            tag_rect,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, with_alpha(col, if is_hov { 195 } else { 120 })),
            StrokeKind::Inside,
        );
        painter.text(
            tag_rect.center(),
            Align2::CENTER_CENTER,
            tag,
            FontId::proportional(9.0),
            if is_hov { TEXT_BRIGHT } else { col },
        );
    }

    hovered_idx.map(|i| peaks[i])
}

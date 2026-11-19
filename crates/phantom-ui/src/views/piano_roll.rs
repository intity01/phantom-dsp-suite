//! Logarithmic Piano Roll Frequency Bar & Musical Note Snapping
//! Maps MIDI 24 (`C1 @ 32.7 Hz`) through MIDI 115 (`G8 @ 6272 Hz`) onto the EQ frequency axis
//! with natural/accidental key rendering, active band note markers, and cents readout badge.

use crate::theme::{band_color, with_alpha, ACCENT_CYAN, BG_DISPLAY, TEXT_BRIGHT, TEXT_MUTED};
use crate::views::curve_plot::EqCanvasMapping;
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

const FIRST_MIDI: u8 = 24; // C1 (~32.70 Hz)
const LAST_MIDI: u8 = 115; // G8 (~6271.93 Hz)

#[inline]
fn is_accidental(midi_note: u8) -> bool {
    matches!(midi_note % 12, 1 | 3 | 6 | 8 | 10)
}

/// Converts a frequency in Hz to `(midi_note, note_name, octave, cents_offset)` using `A4 = 440.0 Hz` (MIDI 69).
pub fn freq_to_midi_note(freq_hz: f32) -> (u8, &'static str, i8, i32) {
    let f = freq_hz.clamp(8.175_799, 25000.0);
    let exact_midi = 69.0 + 12.0 * (f / 440.0).log2();
    let nearest = exact_midi.round().clamp(0.0, 127.0) as i32;
    let cents = ((exact_midi - nearest as f32) * 100.0).round() as i32;
    let note_name = NOTE_NAMES[(nearest.rem_euclid(12)) as usize];
    let octave = (nearest / 12 - 1) as i8;
    (nearest as u8, note_name, octave, cents)
}

/// Converts a MIDI note number (`0..=127`, where `69 = A4 = 440.0 Hz`) to frequency in Hz.
#[inline]
pub fn midi_note_to_freq(midi_note: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0)
}

/// Snaps an arbitrary frequency in Hz to the exact center frequency of the nearest musical semitone.
#[inline]
pub fn snap_freq_to_nearest_semitone(freq_hz: f32) -> f32 {
    let (midi, _, _, _) = freq_to_midi_note(freq_hz);
    midi_note_to_freq(midi)
}

/// Draws a sleek 20px logarithmic piano roll keyboard strip along `piano_rect`.
/// Returns `Some(snapped_freq_hz)` when `hovered_freq` is active so clicking snaps to that key.
pub fn draw_piano_roll(
    painter: &Painter,
    map: &EqCanvasMapping,
    piano_rect: Rect,
    active_band_freqs: &[(usize, f32)],
    selected_band: Option<usize>,
    hovered_freq: Option<f32>,
) -> Option<f32> {
    painter.rect_filled(piano_rect, CornerRadius::same(4), BG_DISPLAY);

    let hovered_midi = hovered_freq.map(|f| freq_to_midi_note(f).0);
    let selected_freq = selected_band.and_then(|sel| {
        active_band_freqs
            .iter()
            .find(|&&(idx, _)| idx == sel)
            .map(|&(_, f)| f)
    });
    let selected_midi = selected_freq.map(|f| freq_to_midi_note(f).0);

    // Pass 1: Draw natural keys & semitone slots across C1..G8
    for midi in FIRST_MIDI..=LAST_MIDI {
        let f_lo = 440.0 * 2.0_f32.powf((midi as f32 - 0.5 - 69.0) / 12.0);
        let f_hi = 440.0 * 2.0_f32.powf((midi as f32 + 0.5 - 69.0) / 12.0);
        let x0 = map.freq_to_x(f_lo).max(piano_rect.min.x);
        let x1 = map.freq_to_x(f_hi).min(piano_rect.max.x);
        if x1 <= x0 + 0.5 {
            continue;
        }
        let key_rect = Rect::from_min_max(
            Pos2::new(x0, piano_rect.min.y),
            Pos2::new(x1, piano_rect.max.y),
        );
        let is_hov = hovered_midi == Some(midi);
        let is_sel = selected_midi == Some(midi);

        if !is_accidental(midi) {
            let fill = if is_hov {
                with_alpha(ACCENT_CYAN, 76)
            } else if is_sel {
                Color32::from_rgb(48, 56, 74)
            } else if midi % 12 == 0 {
                Color32::from_rgb(35, 39, 52)
            } else {
                Color32::from_rgb(28, 31, 42)
            };
            painter.rect_filled(key_rect, CornerRadius::ZERO, fill);
            painter.line_segment(
                [
                    Pos2::new(x0, piano_rect.min.y),
                    Pos2::new(x0, piano_rect.max.y),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
            );

            // Octave C1..C8 marker
            if midi % 12 == 0 {
                let oct = (midi as i32 / 12) - 1;
                painter.line_segment(
                    [
                        Pos2::new(x0, piano_rect.min.y),
                        Pos2::new(x0, piano_rect.max.y),
                    ],
                    Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 55)),
                );
                painter.text(
                    Pos2::new(x0 + 2.5, piano_rect.max.y - 1.5),
                    Align2::LEFT_BOTTOM,
                    format!("C{}", oct),
                    FontId::proportional(8.5),
                    if is_hov || is_sel {
                        TEXT_BRIGHT
                    } else {
                        TEXT_MUTED
                    },
                );
            }
        }
    }

    // Pass 2: Draw accidental (sharp/black) keys in upper 60% of strip
    let black_h = piano_rect.height() * 0.58;
    for midi in FIRST_MIDI..=LAST_MIDI {
        if !is_accidental(midi) {
            continue;
        }
        let f_lo = 440.0 * 2.0_f32.powf((midi as f32 - 0.45 - 69.0) / 12.0);
        let f_hi = 440.0 * 2.0_f32.powf((midi as f32 + 0.45 - 69.0) / 12.0);
        let x0 = map.freq_to_x(f_lo).max(piano_rect.min.x);
        let x1 = map.freq_to_x(f_hi).min(piano_rect.max.x);
        if x1 <= x0 + 0.5 {
            continue;
        }
        let k_rect = Rect::from_min_max(
            Pos2::new(x0, piano_rect.min.y),
            Pos2::new(x1, piano_rect.min.y + black_h),
        );
        let fill = if hovered_midi == Some(midi) {
            with_alpha(ACCENT_CYAN, 130)
        } else if selected_midi == Some(midi) {
            Color32::from_rgb(38, 46, 64)
        } else {
            Color32::from_rgb(14, 16, 23)
        };
        painter.rect_filled(k_rect, CornerRadius::same(1), fill);
        painter.rect_stroke(
            k_rect,
            CornerRadius::same(1),
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 140)),
            StrokeKind::Inside,
        );
    }

    // Pass 3: Colored dot markers for active EQ bands
    for &(band_idx, freq) in active_band_freqs {
        let x = map.freq_to_x(freq);
        if x < piano_rect.min.x || x > piano_rect.max.x {
            continue;
        }
        let col = band_color(band_idx);
        let pos = Pos2::new(x, piano_rect.max.y - 4.5);
        if selected_band == Some(band_idx) {
            painter.circle_filled(pos, 4.6, with_alpha(col, 95));
            painter.circle_stroke(pos, 4.2, Stroke::new(1.2_f32, Color32::WHITE));
        }
        painter.circle_filled(pos, 2.8, col);
    }

    // Pass 4: Live musical note readout pill (e.g. "A4 • 440.0 Hz (-2 ct)")
    if let Some(disp_freq) = hovered_freq.or(selected_freq) {
        let (_, note, oct, cents) = freq_to_midi_note(disp_freq);
        let f_str = if disp_freq >= 1000.0 {
            format!("{:.2} kHz", disp_freq / 1000.0)
        } else {
            format!("{:.1} Hz", disp_freq)
        };
        let label = format!("{}{} • {} ({:+} ct)", note, oct, f_str, cents);
        let pill_w = 138.0;
        let pill_rect = Rect::from_min_size(
            Pos2::new(piano_rect.max.x - pill_w - 4.0, piano_rect.min.y + 2.0),
            Vec2::new(pill_w, (piano_rect.height() - 4.0).max(14.0)),
        );
        painter.rect_filled(
            pill_rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(15, 17, 25, 232),
        );
        painter.rect_stroke(
            pill_rect,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, with_alpha(ACCENT_CYAN, 130)),
            StrokeKind::Inside,
        );
        painter.text(
            pill_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.0),
            TEXT_BRIGHT,
        );
    }

    painter.rect_stroke(
        piano_rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 28)),
        StrokeKind::Inside,
    );

    hovered_freq.map(snap_freq_to_nearest_semitone)
}

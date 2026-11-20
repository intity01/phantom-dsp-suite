//! Inline Keyboard Numeric & Musical-Note Input for Rotary Knobs
//! Parses numeric Hz/kHz/dB/Q values and 12-TET musical notes (`A4 = 440 Hz`, `C#3`, `Db4`, etc.),
//! and renders a compact dark inline `TextEdit` directly inside a knob's bottom value readout rect.

use crate::theme::{with_alpha, BG_DISPLAY, TEXT_BRIGHT};
use crate::widgets::inspector_controls::parse_audio_number;
use crate::widgets::rotary_knob::{
    freq_to_norm, linear_to_norm, q_to_norm, MAX_FREQ_HZ, MIN_FREQ_HZ,
};
use nih_plug_egui::egui::{
    text::{CCursor, CCursorRange},
    Align, Align2, Color32, Context, CornerRadius, CursorIcon, Direction, FontId, Id, Key, Layout,
    Margin, Painter, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnobEditKind {
    None,
    FreqHz,
    GainDb { min: f32, max: f32 },
    Q,
}

fn parse_numeric_freq(trimmed: &str) -> Option<f32> {
    let lower = trimmed.to_ascii_lowercase();
    let (num_str, mult) = if let Some(s) = lower.strip_suffix("khz") {
        (s.trim(), 1000.0_f64)
    } else if let Some(s) = lower.strip_suffix('k') {
        (s.trim(), 1000.0_f64)
    } else if let Some(s) = lower.strip_suffix("hz") {
        (s.trim(), 1.0_f64)
    } else {
        (lower.as_str(), 1.0_f64)
    };
    let val = num_str.parse::<f64>().ok()?;
    let hz = (val.is_finite() && val > 0.0).then_some((val * mult) as f32)?;
    hz.is_finite().then(|| hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ))
}

fn parse_musical_note(trimmed: &str) -> Option<f32> {
    const SEMI_A_TO_G: [i32; 7] = [9, 11, 0, 2, 4, 5, 7];
    let mut chars = trimmed.chars();
    let ch = chars.next()?.to_ascii_uppercase();
    let base_semi = matches!(ch, 'A'..='G').then(|| SEMI_A_TO_G[(ch as u8 - b'A') as usize])?;
    let rest = chars.as_str();
    let (acc, oct_str) = if let Some(r) = rest.strip_prefix(['#', '♯']) {
        (1_i32, r)
    } else if let Some(r) = rest.strip_prefix(['b', 'B', '♭']) {
        (-1_i32, r)
    } else {
        (0_i32, rest)
    };
    if oct_str.is_empty() || oct_str.starts_with('+') {
        return None;
    }
    let octave: i32 = oct_str.parse().ok()?;
    let midi = (-1..=10)
        .contains(&octave)
        .then_some((octave + 1) * 12 + base_semi + acc)?;
    let freq =
        (midi >= 0).then(|| (440.0_f64 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0)) as f32)?;
    (MIN_FREQ_HZ..=MAX_FREQ_HZ).contains(&freq).then_some(freq)
}

/// Parses a frequency in Hz (supporting `Hz`, `k`, `kHz` suffixes, clamped to `10.0..=30000.0`)
/// or a case-insensitive 12-TET musical note (`A4 = 440.0 Hz`, `C#3`, `Db4`, `F#2`, `Bb5`, `B-1`).
/// Hover help shown on every editable knob.
pub const KNOB_HELP: &str = "Drag up/down to adjust · Shift for fine · Scroll to nudge
\n                         Double-click to reset · Click the value to type it";

pub fn parse_freq_or_note(text: &str) -> Option<f32> {
    let trimmed = text.trim();
    (!trimmed.is_empty())
        .then(|| parse_numeric_freq(trimmed).or_else(|| parse_musical_note(trimmed)))
        .flatten()
}

/// Parses an inline-edited text value according to `kind` and converts it to `0.0..=1.0`.
pub fn parse_knob_edit_to_norm(kind: KnobEditKind, text: &str) -> Option<f32> {
    match kind {
        KnobEditKind::None => None,
        KnobEditKind::FreqHz => parse_freq_or_note(text).map(freq_to_norm),
        KnobEditKind::GainDb { min, max } => {
            let t = text.trim();
            if t.eq_ignore_ascii_case("off") {
                return Some(linear_to_norm(0.0, min, max));
            }
            let v = parse_audio_number(t, false)?;
            v.is_finite()
                .then(|| linear_to_norm((v as f32).clamp(min, max), min, max))
        }
        KnobEditKind::Q => {
            let v = parse_audio_number(text, false)?;
            (v.is_finite() && v > 0.0).then(|| q_to_norm(v as f32))
        }
    }
}

/// Cancels any active inline edit for `edit_id` without mutating the parameter.
pub fn cancel_knob_edit(ui: &mut Ui, edit_id: Id) {
    ui.data_mut(|d| d.remove_temp::<String>(edit_id));
    ui.memory_mut(|m| m.surrender_focus(edit_id));
}

fn select_all_text(ctx: &Context, edit_id: Id, text: &str) {
    let mut state = TextEdit::load_state(ctx, edit_id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::default(),
        CCursor::new(text.chars().count()),
    )));
    state.store(ctx, edit_id);
}

fn initial_edit_text(kind: KnobEditKind, value_text: &str) -> String {
    let t = value_text.trim();
    if matches!(kind, KnobEditKind::GainDb { .. }) && t.eq_ignore_ascii_case("off") {
        "0.0".into()
    } else {
        t.into()
    }
}

fn paint_box(p: &Painter, r: Rect, fill: Color32, border: Color32) {
    let rad = CornerRadius::same(3);
    p.rect_filled(r, rad, fill);
    p.rect_stroke(r, rad, Stroke::new(1.0_f32, border), StrokeKind::Inside);
}

fn paint_label(p: &Painter, r: Rect, txt: &str, col: Color32) {
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        txt,
        FontId::proportional(9.2),
        col,
    );
}

/// Renders the bottom value readout strip of a rotary knob. When `edit_kind != KnobEditKind::None`,
/// clicking/double-clicking the strip activates an inline dark `TextEdit::singleline` (auto-focused
/// with all text selected), commits on `Enter` or focus loss, and cancels cleanly on `Escape`.
pub fn draw_knob_value_readout(
    ui: &mut Ui,
    edit_id: Id,
    rect: Rect,
    value_text: &str,
    accent: Color32,
    is_dragging: bool,
    edit_kind: KnobEditKind,
) -> Option<f32> {
    let fg = if is_dragging { accent } else { TEXT_BRIGHT };
    if edit_kind == KnobEditKind::None {
        paint_label(ui.painter(), rect, value_text, fg);
        return None;
    }

    let (escape, enter) = ui.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
    let mut committed: Option<f32> = None;
    let is_editing = ui.memory_mut(|m| {
        m.interested_in_focus(edit_id, ui.layer_id());
        m.has_focus(edit_id)
    });

    if !is_editing {
        if let Some(buf) = ui.data_mut(|d| d.remove_temp::<String>(edit_id)) {
            if !escape {
                committed = parse_knob_edit_to_norm(edit_kind, &buf);
            }
        }
    }

    if is_editing {
        let mut fresh = false;
        let mut buf = ui
            .data_mut(|d| d.get_temp::<String>(edit_id))
            .unwrap_or_else(|| {
                fresh = true;
                initial_edit_text(edit_kind, value_text)
            });
        if fresh {
            select_all_text(ui.ctx(), edit_id, &buf);
        }

        paint_box(ui.painter(), rect, BG_DISPLAY, with_alpha(accent, 210));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::centered_and_justified(Direction::TopDown)),
        );
        child.set_clip_rect(rect);
        child.visuals_mut().selection.bg_fill = with_alpha(accent, 85);
        child.visuals_mut().text_cursor.stroke = Stroke::new(1.2_f32, TEXT_BRIGHT);

        let te_resp = child.add(
            TextEdit::singleline(&mut buf)
                .id(edit_id)
                .frame(false)
                .text_color(TEXT_BRIGHT)
                .font(FontId::proportional(9.0))
                .margin(Margin::symmetric(3, 1))
                .horizontal_align(Align::Center)
                .vertical_align(Align::Center)
                .desired_width((rect.width() - 6.0).max(24.0))
                .min_size(rect.size())
                .clip_text(true),
        );

        if escape {
            cancel_knob_edit(ui, edit_id);
        } else if enter || te_resp.lost_focus() {
            cancel_knob_edit(ui, edit_id);
            committed = parse_knob_edit_to_norm(edit_kind, &buf);
        } else {
            ui.data_mut(|d| d.insert_temp(edit_id, buf));
        }
    } else {
        let val_resp = ui
            .interact(rect, edit_id, Sense::click())
            .on_hover_cursor(CursorIcon::Text);
        if val_resp.hovered() && !is_dragging {
            let bg = Color32::from_rgba_unmultiplied(255, 255, 255, 16);
            paint_box(ui.painter(), rect, bg, with_alpha(accent, 110));
        }
        paint_label(ui.painter(), rect, value_text, fg);
        if val_resp.clicked() || val_resp.double_clicked() {
            let initial = initial_edit_text(edit_kind, value_text);
            select_all_text(ui.ctx(), edit_id, &initial);
            ui.data_mut(|d| d.insert_temp(edit_id, initial));
            ui.memory_mut(|m| m.request_focus(edit_id));
            ui.ctx().request_repaint();
        }
    }

    committed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::rotary_knob::{norm_to_freq, norm_to_linear, norm_to_q};

    #[test]
    fn test_parse_freq_numeric_and_notes() {
        let cases = [
            ("440", 440.0),
            ("440 Hz", 440.0),
            ("120hz", 120.0),
            ("2.5k", 2500.0),
            ("2.5 kHz", 2500.0),
            ("5", 10.0),
            ("50000", 30000.0),
            ("A4", 440.0),
            ("a4", 440.0),
            ("C#4", 277.1826),
            ("Db4", 277.1826),
            ("C#3", 138.5913),
            ("F#2", 92.4986),
            ("Bb5", 932.3275),
            ("B-1", 15.4339),
        ];
        for (inp, exp) in cases {
            let got = parse_freq_or_note(inp).unwrap_or_else(|| panic!("failed on {inp:?}"));
            assert!((got - exp).abs() < 1e-2, "{inp:?}: got {got}, exp {exp}");
        }
        let (cs4, db4) = (parse_freq_or_note("C#4"), parse_freq_or_note("Db4"));
        assert!((cs4.unwrap() - db4.unwrap()).abs() < 1e-4);
        for bad in [
            "", "  ", "invalid", "H4", "C-1", "C-2", "B10", "A99", "0", "-440", "NaN", "inf",
            "C##4",
        ] {
            assert_eq!(parse_freq_or_note(bad), None, "expected None for {bad:?}");
        }
    }

    #[test]
    fn test_normalized_value_roundtrips() {
        let p = parse_knob_edit_to_norm;
        let gain = |min, max| KnobEditKind::GainDb { min, max };
        assert!((norm_to_freq(p(KnobEditKind::FreqHz, "A4").unwrap()) - 440.0).abs() < 1e-2);
        assert!((norm_to_freq(p(KnobEditKind::FreqHz, "2.5k").unwrap()) - 2500.0).abs() < 1e-1);
        let g = p(gain(-30.0, 30.0), "+6.5 dB").unwrap();
        assert!((norm_to_linear(g, -30.0, 30.0) - 6.5).abs() < 1e-3);
        let d = p(gain(-24.0, 24.0), "OFF").unwrap();
        assert!(norm_to_linear(d, -24.0, 24.0).abs() < 1e-4);
        assert!((norm_to_q(p(KnobEditKind::Q, "4.25").unwrap()) - 4.25).abs() < 1e-3);
        assert_eq!(p(KnobEditKind::Q, "0"), None);
        assert_eq!(p(KnobEditKind::None, "440"), None);
    }
}

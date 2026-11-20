//! Focus band strip under the vocal stage. Drag its left and right edges to choose which
//! frequencies get doubled and widened; the doubled voices are filtered out elsewhere.
//! Both values can also be typed by clicking them.

use crate::live_mirror::LiveSetter;
use crate::params::PhantomVoxParams;
use nih_plug::prelude::FloatParam;
use nih_plug_egui::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Key, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, TextEdit, Ui, Vec2,
};
use phantom_ui::{
    parse_freq_or_note, with_alpha, ACCENT_CYAN, BG_DISPLAY, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};
use phantom_vox_dsp::{FOCUS_MAX_HZ, FOCUS_MIN_HZ};

/// Height of the whole strip (title row + graph).
pub const FOCUS_STRIP_H: f32 = 42.0;
const GRAPH_H: f32 = 22.0;
/// The two edges never get closer than this ratio (about a third of an octave).
const MIN_RATIO: f32 = 1.25;

const HELP: &str = "Focus band: only these frequencies are doubled and widened\n\
     Drag an edge to move it · Double-click an edge to reset it · Click a number to type it";

fn f_to_t(hz: f32) -> f32 {
    ((hz / FOCUS_MIN_HZ).ln() / (FOCUS_MAX_HZ / FOCUS_MIN_HZ).ln()).clamp(0.0, 1.0)
}

fn t_to_f(t: f32) -> f32 {
    FOCUS_MIN_HZ * (FOCUS_MAX_HZ / FOCUS_MIN_HZ).powf(t.clamp(0.0, 1.0))
}

pub fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// Clickable value that turns into a text box; returns a typed frequency on Enter.
fn typed_value(ui: &mut Ui, id: Id, hz: f32) -> Option<f32> {
    let editing: Option<String> = ui.data(|d| d.get_temp(id));
    let Some(mut text) = editing else {
        let label = RichText::new(format_hz(hz))
            .font(FontId::proportional(9.5))
            .color(TEXT_BRIGHT);
        let res = ui
            .add(egui::Label::new(label).sense(Sense::click()))
            .on_hover_text("Click to type a frequency (e.g. 150, 2.5k, 12k)");
        if res.clicked() {
            ui.data_mut(|d| d.insert_temp(id, format!("{hz:.0}")));
        }
        return None;
    };
    let res = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(52.0)
            .font(FontId::proportional(9.5)),
    );
    if !res.has_focus() && !res.lost_focus() {
        res.request_focus();
    }
    let (enter, escape) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if res.lost_focus() || enter || escape {
        ui.data_mut(|d| d.remove::<String>(id));
        return (!escape).then(|| parse_freq_or_note(&text)).flatten();
    }
    ui.data_mut(|d| d.insert_temp(id, text));
    None
}

fn set_edge(setter: &LiveSetter, param: &FloatParam, hz: f32) {
    setter.set_once(param, hz.clamp(FOCUS_MIN_HZ, FOCUS_MAX_HZ));
}

fn paint_graph(ui: &Ui, rect: Rect, lo_x: f32, hi_x: f32, hovered: Option<usize>) {
    let p = ui.painter();
    let r4 = CornerRadius::same(4);
    p.rect_filled(rect, r4, BG_DISPLAY);
    let band = Rect::from_x_y_ranges(lo_x..=hi_x, rect.y_range());
    p.rect_filled(band, CornerRadius::ZERO, with_alpha(ACCENT_CYAN, 46));
    for (hz, txt) in [(100.0, "100"), (1000.0, "1k"), (10_000.0, "10k")] {
        let x = rect.left() + f_to_t(hz) * rect.width();
        let grid = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 26));
        p.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            grid,
        );
        p.text(
            Pos2::new(x + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            txt,
            FontId::proportional(7.5),
            TEXT_DIMMED,
        );
    }
    for (idx, x) in [lo_x, hi_x].into_iter().enumerate() {
        let w = if hovered == Some(idx) { 2.5 } else { 1.6 };
        let a = Pos2::new(x, rect.top() + 1.0);
        let b = Pos2::new(x, rect.bottom() - 1.0);
        p.line_segment([a, b], Stroke::new(w, ACCENT_CYAN));
        let grip = Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(6.0, 10.0));
        p.rect_filled(grip, CornerRadius::same(2), ACCENT_CYAN);
    }
    let border = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 24));
    p.rect_stroke(rect, r4, border, StrokeKind::Inside);
}

pub fn draw_focus_strip(ui: &mut Ui, setter: &LiveSetter, params: &PhantomVoxParams, on: bool) {
    let lo = setter.read(&params.focus_low_hz);
    let hi = setter.read(&params.focus_high_hz);
    let res = ui.add_enabled_ui(on, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                RichText::new("FOCUS BAND")
                    .font(FontId::proportional(9.0))
                    .strong()
                    .color(TEXT_MUTED),
            )
            .on_hover_text(HELP);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(hz) = typed_value(ui, ui.id().with("focus_hi_txt"), hi) {
                    set_edge(setter, &params.focus_high_hz, hz.max(lo * MIN_RATIO));
                }
                ui.label(
                    RichText::new("to")
                        .font(FontId::proportional(9.0))
                        .color(TEXT_DIMMED),
                );
                if let Some(hz) = typed_value(ui, ui.id().with("focus_lo_txt"), lo) {
                    set_edge(setter, &params.focus_low_hz, hz.min(hi / MIN_RATIO));
                }
            });
        });

        let size = Vec2::new(ui.available_width(), GRAPH_H);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let x_of = |hz: f32| rect.left() + f_to_t(hz) * rect.width();
        let (lo_x, hi_x) = (x_of(lo), x_of(hi));
        let nearest = |x: f32| usize::from((x - lo_x).abs() > (x - hi_x).abs());
        let drag_id = resp.id.with("edge");
        let pointer_x = resp
            .interact_pointer_pos()
            .or(resp.hover_pos())
            .map(|p| p.x);

        if resp.drag_started() {
            if let Some(x) = pointer_x {
                let edge = nearest(x);
                ui.data_mut(|d| d.insert_temp(drag_id, edge));
                let param = [&params.focus_low_hz, &params.focus_high_hz][edge];
                setter.begin_set_parameter(param);
            }
        }
        let active: Option<usize> = ui.data(|d| d.get_temp(drag_id));
        if let (Some(edge), Some(x), true) = (active, pointer_x, resp.dragged()) {
            let hz = t_to_f((x - rect.left()) / rect.width());
            if edge == 0 {
                setter.set_parameter(&params.focus_low_hz, hz.min(hi / MIN_RATIO));
            } else {
                setter.set_parameter(&params.focus_high_hz, hz.max(lo * MIN_RATIO));
            }
        }
        if resp.drag_stopped() {
            if let Some(edge) = active {
                setter.end_set_parameter([&params.focus_low_hz, &params.focus_high_hz][edge]);
            }
            ui.data_mut(|d| d.remove::<usize>(drag_id));
        }
        if resp.double_clicked() {
            match pointer_x.map(nearest) {
                Some(0) => set_edge(setter, &params.focus_low_hz, FOCUS_MIN_HZ),
                Some(_) => set_edge(setter, &params.focus_high_hz, FOCUS_MAX_HZ),
                None => {}
            }
        }
        let hovered = active.or_else(|| resp.hover_pos().map(|p| nearest(p.x)));
        paint_graph(ui, rect, lo_x, hi_x, hovered);
        resp.on_hover_text(HELP);
    });
    res.response
        .on_disabled_hover_text("Live Align has no doubled voices to focus");
}

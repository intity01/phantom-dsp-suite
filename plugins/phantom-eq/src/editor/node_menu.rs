//! Pro-Q 4 Right-Click Band Node Context Menu
//! Dark studio-grade context menu anchored at click point (clamped inside `map.rect`) offering
//! Bypass, Solo, all 10 Filter Shapes, all 8 Slopes, all 5 Placements, Invert Gain, Dynamics,
//! Spectral Dynamics, and Delete Band. Blocks background canvas clicks while open.

use super::display_helpers::delete_band;
use super::window_size::local_pointer;
use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::{
    ChannelPlacementParam as Ch, FilterSlopeParam, FilterTypeParam, PhantomEqParams, FILTER_NAMES,
    NUM_BANDS, SLOPE_NAMES,
};
use nih_plug_egui::egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, PointerButton, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};
use phantom_ui::widgets::inspector_controls::draw_hud_pill;
use phantom_ui::{
    band_color, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED,
    TEXT_BRIGHT, TEXT_DIMMED,
};

const MENU_W: f32 = 212.0;
const MENU_H: f32 = 382.0;
const PLACE_ORDER: [Ch; 5] = [Ch::Stereo, Ch::Left, Ch::Right, Ch::Mid, Ch::Side];
const PLACE_NAMES: [&str; 5] = ["Stereo", "Left", "Right", "Mid", "Side"];

#[inline]
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Draws the right-click context menu when open. Returns `true` if open on entry.
pub fn draw_node_context_menu(
    ui: &mut Ui,
    map_rect: Rect,
    setter: &LiveSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
) -> bool {
    let Some((idx, anchor)) = state.node_menu else {
        return false;
    };
    let live = shared.live.as_ref();
    if idx >= NUM_BANDS || !live.read(&params.bands[idx].used) {
        state.node_menu = None;
        return false;
    }

    let menu_rect = Rect::from_min_size(
        Pos2::new(
            anchor.x.clamp(
                map_rect.min.x + 6.0,
                (map_rect.max.x - MENU_W - 6.0).max(map_rect.min.x + 6.0),
            ),
            anchor.y.clamp(
                map_rect.min.y + 6.0,
                (map_rect.max.y - MENU_H - 6.0).max(map_rect.min.y + 6.0),
            ),
        ),
        Vec2::new(MENU_W, MENU_H),
    );

    let band = &params.bands[idx];
    let accent = band_color(idx);
    let enabled = live.read(&band.enabled);
    let is_solo = state.effective_solo(params, live) == Some(idx);
    let cur_shape = live.read(&band.filter_type).to_idx();
    let cur_slope = live.read(&band.slope).to_idx();
    let cur_place = PLACE_ORDER
        .iter()
        .position(|&p| p == live.read(&band.placement))
        .unwrap_or(0);
    let cur_gain = live.read(&band.gain);
    let cur_spec = live.read(&band.spectral_dynamics);
    let has_dyn = live.read(&band.dynamic_range).abs() > 0.05;

    let p = ui.painter();
    let r = CornerRadius::same(7);
    p.rect_filled(
        menu_rect.translate(Vec2::new(0.0, 4.0)),
        r,
        rgba(0, 0, 0, 155),
    );
    p.rect_filled(menu_rect, r, rgba(16, 18, 26, 248));
    p.rect_stroke(
        menu_rect,
        r,
        Stroke::new(1.1_f32, with_alpha(accent, 175)),
        StrokeKind::Inside,
    );

    let inner = menu_rect.shrink2(Vec2::new(7.0, 6.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::top_down(Align::LEFT)),
    );
    child.set_clip_rect(menu_rect);
    child.spacing_mut().item_spacing = Vec2::new(3.0, 2.5);

    let mut close = false;
    let freq = live.read(&band.frequency);
    let freq_str = if freq >= 1000.0 {
        format!("{:.2} kHz", freq / 1000.0)
    } else {
        format!("{freq:.0} Hz")
    };
    let hdr = format!("BAND B{}  •  {freq_str}", idx + 1);
    child.label(
        RichText::new(hdr)
            .font(FontId::proportional(9.5))
            .strong()
            .color(accent),
    );

    if menu_row(
        &mut child,
        if enabled {
            "Bypass Band"
        } else {
            "Enable Band"
        },
        if enabled { TEXT_BRIGHT } else { ACCENT_GREEN },
    ) {
        setter.set_once(&band.enabled, !enabled);
        close = true;
    }
    if menu_row(
        &mut child,
        if is_solo { "Unsolo Band" } else { "Solo Band" },
        if is_solo { ACCENT_AMBER } else { TEXT_BRIGHT },
    ) {
        if is_solo {
            state.latched_solo = None;
            state.solo_hold_band = None;
            state.alt_solo_band = None;
            shared.clear_solo();
        } else {
            state.latched_solo = Some(idx);
        }
        close = true;
    }

    divider(&mut child);
    if let Some(s) = pill_grid(
        &mut child,
        "SHAPE",
        &FILTER_NAMES,
        2,
        97.0,
        cur_shape,
        accent,
    ) {
        setter.set_once(&band.filter_type, FilterTypeParam::from_idx(s));
        close = true;
    }
    if let Some(sl) = pill_grid(
        &mut child,
        "SLOPE",
        &SLOPE_NAMES,
        4,
        47.0,
        cur_slope,
        accent,
    ) {
        setter.set_once(&band.slope, FilterSlopeParam::from_idx(sl));
        close = true;
    }
    if let Some(pl) = pill_grid(
        &mut child,
        "PLACEMENT",
        &PLACE_NAMES,
        3,
        64.0,
        cur_place,
        ACCENT_CYAN,
    ) {
        setter.set_once(&band.placement, PLACE_ORDER[pl]);
        close = true;
    }

    divider(&mut child);
    if menu_row(&mut child, "Invert Gain", TEXT_BRIGHT) {
        setter.set_once(&band.gain, (-cur_gain).clamp(-30.0, 30.0));
        close = true;
    }
    if menu_row(
        &mut child,
        if has_dyn {
            "Clear Dynamics"
        } else {
            "Dynamic (-4 dB)"
        },
        if has_dyn { ACCENT_AMBER } else { ACCENT_CYAN },
    ) {
        setter.set_once(&band.dynamic_range, if has_dyn { 0.0 } else { -4.0 });
        if has_dyn {
            setter.set_once(&band.spectral_dynamics, false);
        }
        close = true;
    }
    if menu_row(
        &mut child,
        if cur_spec {
            "Spectral Dynamics  [ON]"
        } else {
            "Spectral Dynamics"
        },
        if cur_spec { ACCENT_PURPLE } else { TEXT_BRIGHT },
    ) {
        let next = !cur_spec;
        setter.set_once(&band.spectral_dynamics, next);
        if next && !has_dyn {
            setter.set_once(&band.dynamic_range, -4.0);
        }
        close = true;
    }

    divider(&mut child);
    if menu_row(&mut child, "Delete Band", ACCENT_RED) {
        delete_band(setter, params, state, shared, idx);
        close = true;
    }

    let (raw_ptr, pressed) = ui.input(|i| {
        let p = i.pointer.interact_pos().or(i.pointer.hover_pos());
        let any = i.pointer.primary_pressed()
            || i.pointer.secondary_pressed()
            || i.pointer.button_pressed(PointerButton::Middle);
        (p, any)
    });
    if close || (pressed && local_pointer(ui, raw_ptr).is_some_and(|pt| !menu_rect.contains(pt))) {
        state.node_menu = None;
    }
    true
}

fn pill_grid(
    ui: &mut Ui,
    title: &str,
    names: &[&str],
    cols: usize,
    pill_w: f32,
    active_idx: usize,
    accent: Color32,
) -> Option<usize> {
    ui.label(
        RichText::new(title)
            .font(FontId::proportional(8.2))
            .strong()
            .color(TEXT_DIMMED),
    );
    let mut picked = None;
    for (r_idx, chunk) in names.chunks(cols).enumerate() {
        let w = if chunk.len() < cols { 97.0 } else { pill_w };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (c_idx, &name) in chunk.iter().enumerate() {
                let idx = r_idx * cols + c_idx;
                if draw_hud_pill(ui, w, 17.0, name, idx == active_idx, accent).clicked() {
                    picked = Some(idx);
                }
            }
        });
    }
    picked
}

fn divider(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(MENU_W - 14.0, 4.0), Sense::hover());
    let y = r.center().y;
    ui.painter().line_segment(
        [Pos2::new(r.min.x, y), Pos2::new(r.max.x, y)],
        Stroke::new(1.0_f32, rgba(255, 255, 255, 18)),
    );
}

fn menu_row(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let (rect, res) = ui.allocate_exact_size(Vec2::new(MENU_W - 14.0, 18.0), Sense::click());
    if res.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), rgba(255, 255, 255, 20));
    }
    ui.painter().text(
        Pos2::new(rect.min.x + 5.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(9.8),
        color,
    );
    res.clicked()
}

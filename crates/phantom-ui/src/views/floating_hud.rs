//! Pro-Q 4 Floating Band Popover HUD
//! Compact dark-glass floating card (`296 x 116 px`) positioned automatically near the selected
//! EQ node with 4 precision Rotary Knobs, Headphone Solo Audition, Shape/Slope/Routing pills,
//! Spectral Dynamics (`SPEC`) toggle, and Dynamic EQ ballistics micro-strip.

use crate::icons::draw_filter_shape_icon;
use crate::theme::{
    band_color, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_PURPLE, TEXT_BRIGHT, TEXT_DIMMED,
};
use crate::views::curve_plot::EqCanvasMapping;
use crate::widgets::band_inspector::{draw_hud_knobs_row, BandInspectorData, BandInspectorEvents};
use crate::widgets::inspector_controls::{
    delete_band_button, draw_hud_pill, power_button, solo_headphones_button,
};
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2,
};

pub const FLOATING_HUD_WIDTH: f32 = 296.0;
pub const FLOATING_HUD_HEIGHT: f32 = 116.0;

fn cycle_pill(ui: &mut Ui, w: f32, txt: &str, col: Color32, idx: (usize, usize)) -> Option<usize> {
    let n = idx.1.max(1);
    let res = draw_hud_pill(ui, w, 22.0, txt, idx.0 != 0 && col == ACCENT_CYAN, col);
    if res.clicked() {
        Some((idx.0 + 1) % n)
    } else if res.secondary_clicked() {
        Some((idx.0 + n - 1) % n)
    } else {
        None
    }
}

fn draw_top_strip(
    ui: &mut Ui,
    data: &BandInspectorData,
    accent: Color32,
    filter_names: &[&'static str],
    slope_names: &[&'static str],
    routing_names: &[&'static str],
    events: &mut BandInspectorEvents,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        if power_button(ui, data.is_enabled, accent).clicked() {
            events.toggle_enabled = true;
        }

        let badge = format!("B{}", data.selected_idx + 1);
        if draw_hud_pill(ui, 24.0, 22.0, &badge, true, accent).clicked()
            && data.used_bands_mask != 0
        {
            events.select_band = (1..=data.num_bands)
                .map(|off| (data.selected_idx + off) % data.num_bands)
                .find(|&c| (data.used_bands_mask & (1u32 << c)) != 0);
        }

        let solo_res = solo_headphones_button(ui, data.is_soloed);
        events.solo_held = solo_res.is_pointer_button_down_on();
        if solo_res.clicked() {
            events.toggle_solo = true;
        }

        let n_sh = filter_names.len().max(1);
        let sh_name = filter_names
            .get(data.filter_shape_idx)
            .copied()
            .unwrap_or("Bell");
        let (sh_rect, sh_res) = ui.allocate_exact_size(Vec2::new(62.0, 22.0), Sense::click());
        let p = ui.painter();
        let sh_a = if sh_res.hovered() { 24 } else { 12 };
        p.rect_filled(
            sh_rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(255, 255, 255, sh_a),
        );
        p.rect_stroke(
            sh_rect,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, with_alpha(accent, 110)),
            StrokeKind::Inside,
        );
        let ic_r = Rect::from_min_size(sh_rect.min + Vec2::new(3.0, 5.0), Vec2::new(14.0, 12.0));
        draw_filter_shape_icon(p, ic_r, data.filter_shape_idx, accent, 1.3);
        p.text(
            Pos2::new(sh_rect.min.x + 19.0, sh_rect.center().y),
            Align2::LEFT_CENTER,
            sh_name,
            FontId::proportional(8.8),
            TEXT_BRIGHT,
        );
        if sh_res.clicked() {
            events.set_filter_shape = Some((data.filter_shape_idx + 1) % n_sh);
        } else if sh_res.secondary_clicked() {
            events.set_filter_shape = Some((data.filter_shape_idx + n_sh - 1) % n_sh);
        }

        let sl_txt = slope_names
            .get(data.slope_idx)
            .copied()
            .unwrap_or("12 dB")
            .trim_end_matches("/oct");
        events.set_slope = cycle_pill(
            ui,
            38.0,
            sl_txt,
            accent,
            (data.slope_idx, slope_names.len()),
        );

        let rt_short = match routing_names
            .get(data.routing_idx)
            .copied()
            .unwrap_or("Stereo")
        {
            "Stereo" => "ST",
            "Mid" => "M",
            "Side" => "S",
            "Left" => "L",
            "Right" => "R",
            other => other,
        };
        events.set_routing = cycle_pill(
            ui,
            23.0,
            rt_short,
            ACCENT_CYAN,
            (data.routing_idx, routing_names.len()),
        );

        if draw_hud_pill(ui, 33.0, 22.0, "SPEC", data.spectral_enabled, ACCENT_PURPLE).clicked() {
            events.toggle_spectral = true;
        }

        if delete_band_button(ui).clicked() {
            events.delete_band = true;
        }
    });
}

/// Renders the Pro-Q 4 Floating Band HUD Popover near `node_pos`, clamped inside `map.rect`.
/// Returns `(BandInspectorEvents, hud_rect)` so canvas interaction ignores clicks inside the HUD.
pub fn draw_floating_band_hud(
    ui: &mut Ui,
    map: &EqCanvasMapping,
    node_pos: Pos2,
    data: &BandInspectorData,
    filter_names: &[&'static str],
    slope_names: &[&'static str],
    routing_names: &[&'static str],
) -> (BandInspectorEvents, Rect) {
    let mut events = BandInspectorEvents::default();
    let accent = band_color(data.selected_idx);

    let min_x = map.rect.min.x + 8.0;
    let hud_x =
        (node_pos.x - FLOATING_HUD_WIDTH * 0.5).clamp(min_x, (map.rect.max.x - 304.0).max(min_x));
    let below = (node_pos.y - 135.0) < map.rect.min.y;
    let raw_y = if below {
        node_pos.y + 18.0
    } else {
        node_pos.y - FLOATING_HUD_HEIGHT - 16.0
    };
    let min_y = map.rect.min.y + 6.0;
    let max_y = (map.rect.max.y - FLOATING_HUD_HEIGHT - 6.0).max(min_y);
    let hud_rect = Rect::from_min_size(
        Pos2::new(hud_x, raw_y.clamp(min_y, max_y)),
        Vec2::new(FLOATING_HUD_WIDTH, FLOATING_HUD_HEIGHT),
    );

    let p = ui.painter();
    let ay = if below {
        hud_rect.min.y
    } else {
        hud_rect.max.y
    };
    let ax = node_pos
        .x
        .clamp(hud_rect.min.x + 16.0, hud_rect.max.x - 16.0);
    p.line_segment(
        [node_pos, Pos2::new(ax, ay)],
        Stroke::new(1.2_f32, with_alpha(accent, 95)),
    );
    p.rect_filled(
        hud_rect.translate(Vec2::new(0.0, 3.0)),
        CornerRadius::same(8),
        Color32::from_rgba_unmultiplied(0, 0, 0, 135),
    );
    p.rect_filled(
        hud_rect,
        CornerRadius::same(8),
        Color32::from_rgba_unmultiplied(18, 20, 29, 244),
    );
    p.rect_stroke(
        hud_rect,
        CornerRadius::same(8),
        Stroke::new(1.2_f32, with_alpha(accent, 185)),
        StrokeKind::Inside,
    );

    let inner = hud_rect.shrink2(Vec2::new(6.0, 4.0));
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        draw_top_strip(
            ui,
            data,
            accent,
            filter_names,
            slope_names,
            routing_names,
            &mut events,
        );
        ui.add_space(1.5);
        draw_hud_knobs_row(ui, data, accent, &mut events);
        if data.dyn_range.abs() > 0.05 {
            ui.add_space(1.0);
            draw_dyn_micro_strip(ui, data, &mut events);
        }
    });

    (events, hud_rect)
}

fn mini_drag(ui: &mut Ui, w: f32, txt: &str, v: f32, b: (f32, f32, f32), en: bool) -> Option<f32> {
    let sense = if en {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, res) = ui.allocate_exact_size(Vec2::new(w, 16.0), sense);
    let act = en && (res.dragged() || res.hovered());
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::same(3),
        Color32::from_rgba_unmultiplied(255, 255, 255, if act { 20 } else { 9 }),
    );
    let brd = Color32::from_rgba_unmultiplied(255, 255, 255, if act { 55 } else { 20 });
    p.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, brd),
        StrokeKind::Inside,
    );
    let fg = if !en {
        TEXT_DIMMED
    } else if res.dragged() {
        ACCENT_CYAN
    } else {
        TEXT_BRIGHT
    };
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        txt,
        FontId::proportional(8.5),
        fg,
    );
    let d = (-res.drag_delta().y + res.drag_delta().x * 0.5) * b.2;
    (en && res.dragged() && d.abs() > 1e-5).then(|| (v + d).clamp(b.0, b.1))
}

fn draw_dyn_micro_strip(ui: &mut Ui, d: &BandInspectorData, ev: &mut BandInspectorEvents) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.5;
        if draw_hud_pill(ui, 36.0, 16.0, "AUTO", d.dyn_auto_thresh, ACCENT_CYAN).clicked() {
            ev.toggle_dyn_auto_thresh = true;
        }
        let (t_s, a_s, r_s) = (
            format!("T:{:.0}dB", d.dyn_threshold),
            format!("A:{:.0}ms", d.dyn_attack_ms),
            format!("R:{:.0}ms", d.dyn_release_ms),
        );
        let t_en = !d.dyn_auto_thresh;
        ev.set_dyn_threshold = mini_drag(ui, 56.0, &t_s, d.dyn_threshold, (-60.0, 0.0, 0.25), t_en);
        ev.set_dyn_attack_ms = mini_drag(ui, 54.0, &a_s, d.dyn_attack_ms, (0.5, 250.0, 0.8), true);
        ev.set_dyn_release_ms =
            mini_drag(ui, 58.0, &r_s, d.dyn_release_ms, (10.0, 2000.0, 5.0), true);
        if draw_hud_pill(ui, 26.0, 16.0, "SC", d.dyn_ext_sc, ACCENT_AMBER).clicked() {
            ev.toggle_dyn_ext_sc = true;
        }
    });
}

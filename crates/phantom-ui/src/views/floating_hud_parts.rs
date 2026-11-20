//! Floating Band HUD rows: the top control strip (band, solo, shape, slope, channel, spectral)
//! and the dynamic-EQ micro strip. Every control spells out what it is and explains itself on
//! hover, and controls that have no effect for the current filter shape are dimmed.

use crate::icons::draw_filter_shape_icon;
use crate::theme::{
    with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_PURPLE, TEXT_BRIGHT, TEXT_DIMMED,
};
use crate::widgets::band_inspector::{BandInspectorData, BandInspectorEvents};
use crate::widgets::inspector_controls::{
    delete_band_button, draw_hud_pill, power_button, solo_headphones_button,
};
use nih_plug_egui::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

/// Width the top strip's controls need, used to centre them in the HUD.
pub const TOP_STRIP_W: f32 = 355.0;

#[inline]
pub fn add_center_pad(ui: &mut Ui, content_w: f32) {
    let pad = ((ui.available_width() - content_w) * 0.5).max(0.0);
    if pad > 0.0 {
        ui.add_space(pad);
    }
}

/// Click steps forward, right-click steps back through `idx.1` options.
fn cycle(res: &Response, idx: (usize, usize)) -> Option<usize> {
    let n = idx.1.max(1);
    if res.clicked() {
        Some((idx.0 + 1) % n)
    } else if res.secondary_clicked() {
        Some((idx.0 + n - 1) % n)
    } else {
        None
    }
}

/// True when the slope setting changes the sound for this shape (only Flat Tilt ignores it).
pub fn shape_uses_slope(shape: &str) -> bool {
    shape != "Flat Tilt"
}

/// True when the gain setting changes the sound for this shape (cuts and all-pass ignore it).
pub fn shape_uses_gain(shape: &str) -> bool {
    !matches!(shape, "Low Cut" | "High Cut" | "All Pass")
}

fn shape_button(ui: &mut Ui, data: &BandInspectorData, name: &str, accent: Color32) -> Response {
    let (rect, res) = ui.allocate_exact_size(Vec2::new(70.0, 22.0), Sense::click());
    let p = ui.painter();
    let r4 = CornerRadius::same(4);
    let fill_a = if res.hovered() { 24 } else { 12 };
    p.rect_filled(
        rect,
        r4,
        Color32::from_rgba_unmultiplied(255, 255, 255, fill_a),
    );
    let stroke = Stroke::new(1.0_f32, with_alpha(accent, 110));
    p.rect_stroke(rect, r4, stroke, StrokeKind::Inside);
    let ic_r = Rect::from_min_size(rect.min + Vec2::new(3.0, 5.0), Vec2::new(14.0, 12.0));
    draw_filter_shape_icon(p, ic_r, data.filter_shape_idx, accent, 1.3);
    let text_pos = Pos2::new(rect.min.x + 19.0, rect.center().y);
    p.text(
        text_pos,
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(8.8),
        TEXT_BRIGHT,
    );
    res.on_hover_text(format!(
        "Filter shape: {name}\nClick for the next shape, right-click for the previous one"
    ))
}

pub fn draw_top_strip(
    ui: &mut Ui,
    data: &BandInspectorData,
    accent: Color32,
    names: (&[&'static str], &[&'static str], &[&'static str]),
    events: &mut BandInspectorEvents,
) {
    let (filter_names, slope_names, routing_names) = names;
    let shape = filter_names
        .get(data.filter_shape_idx)
        .copied()
        .unwrap_or("Bell");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        add_center_pad(ui, TOP_STRIP_W);

        if power_button(ui, data.is_enabled, accent).clicked() {
            events.toggle_enabled = true;
        }
        let band = format!("Band {}", data.selected_idx + 1);
        let band_res = draw_hud_pill(ui, 44.0, 22.0, &band, true, accent)
            .on_hover_text("Selected band — click to jump to the next band in use");
        if band_res.clicked() && data.used_bands_mask != 0 {
            events.select_band = (1..=data.num_bands)
                .map(|off| (data.selected_idx + off) % data.num_bands)
                .find(|&c| (data.used_bands_mask & (1u32 << c)) != 0);
        }
        let solo_res = solo_headphones_button(ui, data.is_soloed);
        events.solo_held = solo_res.is_pointer_button_down_on();
        if solo_res.clicked() {
            events.toggle_solo = true;
        }

        let n_sh = filter_names.len();
        events.set_filter_shape = cycle(
            &shape_button(ui, data, shape, accent),
            (data.filter_shape_idx, n_sh),
        );

        let slope = slope_names.get(data.slope_idx).copied().unwrap_or("12 dB");
        let slope_txt = format!("{}/oct", slope.trim_end_matches("/oct").trim());
        let uses_slope = shape_uses_slope(shape);
        let slope_res = ui
            .add_enabled_ui(uses_slope, |ui| {
                draw_hud_pill(ui, 56.0, 22.0, &slope_txt, false, accent).on_hover_text(
                    "Slope: how steeply the filter rises or falls\n\
                     Click for steeper, right-click for gentler",
                )
            })
            .inner
            .on_disabled_hover_text(format!("{shape} has no slope setting"));
        if uses_slope {
            events.set_slope = cycle(&slope_res, (data.slope_idx, slope_names.len()));
        }

        let routing = routing_names
            .get(data.routing_idx)
            .copied()
            .unwrap_or("Stereo");
        let routing_res =
            draw_hud_pill(ui, 44.0, 22.0, routing, data.routing_idx != 0, ACCENT_CYAN)
                .on_hover_text(
                    "Channels this band processes: Stereo, Mid, Side, Left or Right\n\
                 Click for the next option, right-click for the previous one",
                );
        events.set_routing = cycle(&routing_res, (data.routing_idx, routing_names.len()));

        let spec_res = draw_hud_pill(
            ui,
            48.0,
            22.0,
            "Spectral",
            data.spectral_enabled,
            ACCENT_PURPLE,
        )
        .on_hover_text(
            "Spectral dynamics: tame only the frequencies that poke out, not the whole band",
        );
        if spec_res.clicked() {
            events.toggle_spectral = true;
        }
        if delete_band_button(ui).clicked() {
            events.delete_band = true;
        }
    });
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
    let r3 = CornerRadius::same(3);
    let fill = Color32::from_rgba_unmultiplied(255, 255, 255, if act { 20 } else { 9 });
    p.rect_filled(rect, r3, fill);
    let brd = Color32::from_rgba_unmultiplied(255, 255, 255, if act { 55 } else { 20 });
    p.rect_stroke(rect, r3, Stroke::new(1.0_f32, brd), StrokeKind::Inside);
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
    let res = if en {
        res.on_hover_text("Drag up/down (or left/right) to adjust")
    } else {
        res.on_hover_text("Set automatically while Auto is on")
    };
    let d = (-res.drag_delta().y + res.drag_delta().x * 0.5) * b.2;
    (en && res.dragged() && d.abs() > 1e-5).then(|| (v + d).clamp(b.0, b.1))
}

pub fn draw_dyn_micro_strip(ui: &mut Ui, d: &BandInspectorData, ev: &mut BandInspectorEvents) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.5;
        add_center_pad(ui, 314.0);
        let auto = draw_hud_pill(ui, 32.0, 16.0, "Auto", d.dyn_auto_thresh, ACCENT_CYAN)
            .on_hover_text("Automatic threshold that follows the level of the material");
        if auto.clicked() {
            ev.toggle_dyn_auto_thresh = true;
        }
        let (t_s, a_s, r_s) = (
            format!("Threshold {:.0} dB", d.dyn_threshold),
            format!("Attack {:.0} ms", d.dyn_attack_ms),
            format!("Release {:.0} ms", d.dyn_release_ms),
        );
        let t_en = !d.dyn_auto_thresh;
        ev.set_dyn_threshold = mini_drag(ui, 80.0, &t_s, d.dyn_threshold, (-60.0, 0.0, 0.25), t_en);
        ev.set_dyn_attack_ms = mini_drag(ui, 62.0, &a_s, d.dyn_attack_ms, (0.5, 250.0, 0.8), true);
        ev.set_dyn_release_ms =
            mini_drag(ui, 74.0, &r_s, d.dyn_release_ms, (10.0, 2000.0, 5.0), true);
        let sc = draw_hud_pill(ui, 52.0, 16.0, "Sidechain", d.dyn_ext_sc, ACCENT_AMBER)
            .on_hover_text("Trigger this band from the plugin's external sidechain input");
        if sc.clicked() {
            ev.toggle_dyn_ext_sc = true;
        }
    });
}

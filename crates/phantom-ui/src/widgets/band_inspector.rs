//! Selected Band Parameter Inspector HUD
//! Compact multi-row control strip for up to 24 Pro-Q 4 bands with Lucide vector icons,
//! Headphone Solo Audition (click-toggle & press-and-hold), variable slopes, and Dynamics.

use crate::theme::{
    band_color, ACCENT_CYAN, ACCENT_GREEN, ACCENT_RED, BG_CARD, TEXT_DIMMED, TEXT_MUTED,
};
use crate::widgets::inspector_controls::{
    delete_band_button, power_button, shape_combo, slope_combo, solo_headphones_button,
};
use crate::widgets::inspector_rows::{
    draw_band_selector, draw_core_sliders_row, draw_dynamics_row,
};
use crate::widgets::rotary_knob::{
    draw_compact_knob, freq_to_norm, linear_to_norm, norm_to_freq, norm_to_linear, norm_to_q,
    q_to_norm,
};
use nih_plug_egui::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Ui};

pub struct BandInspectorData {
    pub selected_idx: usize,
    pub num_bands: usize,
    /// Bitmask of created (`used`) bands (bit `i` = band `i` exists). Pass `0` if all shown.
    pub used_bands_mask: u32,
    pub is_enabled: bool,
    pub is_soloed: bool,
    pub soloed_band: Option<usize>,
    pub filter_shape_idx: usize,
    pub slope_idx: usize,
    pub routing_idx: usize,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub dyn_range: f32,
    pub dyn_threshold: f32,
    pub dyn_attack_ms: f32,
    pub dyn_release_ms: f32,
    pub dyn_auto_thresh: bool,
    pub dyn_ext_sc: bool,
    pub spectral_enabled: bool,
    pub live_gr_db: f32,
}

#[derive(Debug, Default)]
pub struct BandInspectorEvents {
    pub select_band: Option<usize>,
    pub toggle_enabled: bool,
    /// Fires on quick click of the Headphone Solo button (click-to-toggle).
    pub toggle_solo: bool,
    /// True continuously while the pointer button is held down on the Headphone Solo button.
    pub solo_held: bool,
    pub delete_band: bool,
    pub set_filter_shape: Option<usize>,
    pub set_slope: Option<usize>,
    pub set_routing: Option<usize>,
    pub set_freq: Option<f32>,
    pub set_gain: Option<f32>,
    pub set_q: Option<f32>,
    pub set_dyn_range: Option<f32>,
    pub set_dyn_threshold: Option<f32>,
    pub set_dyn_attack_ms: Option<f32>,
    pub set_dyn_release_ms: Option<f32>,
    pub toggle_dyn_auto_thresh: bool,
    pub toggle_dyn_ext_sc: bool,
    pub toggle_spectral: bool,
}

/// Draws a sleek single-line inspector placeholder when 0 EQ bands have been created yet.
pub fn draw_empty_band_inspector(ui: &mut Ui) {
    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(12, 8))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("NO ACTIVE BANDS")
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(TEXT_MUTED),
                );
                ui.separator();
                ui.label(
                    RichText::new(
                        "Double-click or drag on the frequency canvas to create your first EQ band",
                    )
                    .font(FontId::proportional(10.5))
                    .color(TEXT_DIMMED),
                );
            });
        });
}

pub fn draw_band_inspector(
    ui: &mut Ui,
    data: &BandInspectorData,
    filter_names: &[&'static str],
    slope_names: &[&'static str],
    routing_names: &[&'static str],
) -> BandInspectorEvents {
    let mut events = BandInspectorEvents::default();
    let accent = band_color(data.selected_idx);

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(12, 8))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            // Compact slider & spacing geometry fits comfortably inside 1020px with zero clipping
            ui.spacing_mut().slider_width = 66.0;
            ui.spacing_mut().item_spacing.x = 6.0;

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("BAND")
                        .font(FontId::proportional(10.0))
                        .strong()
                        .color(TEXT_MUTED),
                );
                draw_band_selector(ui, data, &mut events);
                ui.separator();

                if power_button(ui, data.is_enabled, accent).clicked() {
                    events.toggle_enabled = true;
                }
                let solo_res = solo_headphones_button(ui, data.is_soloed);
                events.solo_held = solo_res.is_pointer_button_down_on();
                if solo_res.clicked() {
                    events.toggle_solo = true;
                }
                if delete_band_button(ui).clicked() {
                    events.delete_band = true;
                }

                ui.separator();
                if let Some(s) = shape_combo(ui, data.filter_shape_idx, filter_names, accent) {
                    events.set_filter_shape = Some(s);
                }
                if let Some(sl) = slope_combo(ui, data.slope_idx, slope_names) {
                    events.set_slope = Some(sl);
                }

                ui.separator();
                for (i, &name) in routing_names.iter().enumerate() {
                    if ui
                        .selectable_label(
                            data.routing_idx == i,
                            RichText::new(name).font(FontId::proportional(10.0)),
                        )
                        .clicked()
                    {
                        events.set_routing = Some(i);
                    }
                }
            });

            ui.add_space(4.0);
            draw_core_sliders_row(ui, data, &mut events);
            ui.add_space(3.0);
            draw_dynamics_row(ui, data, &mut events);
        });

    events
}

/// Renders the 4 Rotary Knobs (`FREQ`, `GAIN`, `Q`, `DYN`) inside the Floating Band HUD Popover.
pub fn draw_hud_knobs_row(
    ui: &mut Ui,
    data: &BandInspectorData,
    accent: Color32,
    events: &mut BandInspectorEvents,
) {
    let has_dyn = data.dyn_range.abs() > 0.05;
    let diam: f32 = if has_dyn { 30.0 } else { 36.0 };
    let idx = data.selected_idx;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let knob_w = (diam + 26.0).max(58.0);
        let row_w = 4.0 * knob_w + 3.0 * 6.0;
        let pad = ((ui.available_width() - row_w) * 0.5).max(0.0);
        if pad > 0.0 {
            ui.add_space(pad);
        }
        let f_txt = if data.freq >= 1000.0 {
            format!("{:.2} kHz", data.freq / 1000.0)
        } else {
            format!("{:.1} Hz", data.freq)
        };
        if let Some(n) = draw_compact_knob(
            ui,
            ("hf", idx),
            "FREQ",
            &f_txt,
            freq_to_norm(data.freq),
            freq_to_norm(1000.0),
            accent,
            diam,
            false,
        ) {
            events.set_freq = Some(norm_to_freq(n));
        }
        let g_txt = format!("{:+.1} dB", data.gain);
        if let Some(n) = draw_compact_knob(
            ui,
            ("hg", idx),
            "GAIN",
            &g_txt,
            linear_to_norm(data.gain, -30.0, 30.0),
            0.5,
            accent,
            diam,
            true,
        ) {
            events.set_gain = Some(norm_to_linear(n, -30.0, 30.0));
        }
        let q_txt = format!("{:.2}", data.q);
        if let Some(n) = draw_compact_knob(
            ui,
            ("hq", idx),
            "Q",
            &q_txt,
            q_to_norm(data.q),
            q_to_norm(1.0),
            accent,
            diam,
            false,
        ) {
            events.set_q = Some(norm_to_q(n));
        }
        let d_txt = if has_dyn {
            format!("{:+.1} dB", data.dyn_range)
        } else {
            "OFF".into()
        };
        let d_col = if data.dyn_range > 0.05 {
            ACCENT_GREEN
        } else if data.dyn_range < -0.05 {
            ACCENT_RED
        } else {
            ACCENT_CYAN
        };
        if let Some(n) = draw_compact_knob(
            ui,
            ("hd", idx),
            "DYN",
            &d_txt,
            linear_to_norm(data.dyn_range, -24.0, 24.0),
            0.5,
            d_col,
            diam,
            true,
        ) {
            events.set_dyn_range = Some(norm_to_linear(n, -24.0, 24.0));
        }
    });
}

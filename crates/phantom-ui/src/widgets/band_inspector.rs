//! Selected Band Parameter Inspector HUD
//! Controls for Frequency, Gain, Q, Dynamic EQ, Filter Shape, Channel Routing, and Solo Audition.

use crate::theme::{ACCENT_AMBER, BAND_COLORS, BG_CARD, TEXT_MUTED};
use nih_plug_egui::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Slider, Ui};

pub struct BandInspectorData {
    pub selected_idx: usize,
    pub num_bands: usize,
    pub is_enabled: bool,
    pub is_soloed: bool,
    pub soloed_band: Option<usize>,
    pub filter_shape_idx: usize,
    pub routing_idx: usize,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub dyn_range: f32,
    pub dyn_threshold: f32,
}

pub struct BandInspectorEvents {
    pub select_band: Option<usize>,
    pub toggle_enabled: bool,
    pub toggle_solo: bool,
    pub set_filter_shape: Option<usize>,
    pub set_routing: Option<usize>,
    pub set_freq: Option<f32>,
    pub set_gain: Option<f32>,
    pub set_q: Option<f32>,
    pub set_dyn_range: Option<f32>,
    pub set_dyn_threshold: Option<f32>,
}

pub fn draw_band_inspector(
    ui: &mut Ui,
    data: &BandInspectorData,
    filter_names: &[&'static str],
    routing_names: &[&'static str],
) -> BandInspectorEvents {
    let mut events = BandInspectorEvents {
        select_band: None,
        toggle_enabled: false,
        toggle_solo: false,
        set_filter_shape: None,
        set_routing: None,
        set_freq: None,
        set_gain: None,
        set_q: None,
        set_dyn_range: None,
        set_dyn_threshold: None,
    };

    let band_color = BAND_COLORS[data.selected_idx % 8];

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(14, 10))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            // Top Row: Band tabs, Enable switch, Solo audition, Filter Shape, Routing
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("BAND:")
                        .font(FontId::proportional(11.0))
                        .strong()
                        .color(TEXT_MUTED),
                );
                for i in 0..data.num_bands {
                    let is_active = data.selected_idx == i;
                    let is_tab_solo = data.soloed_band == Some(i);
                    let label = if is_tab_solo {
                        format!("{}•", i + 1)
                    } else {
                        format!("{}", i + 1)
                    };
                    let color = if is_tab_solo {
                        ACCENT_AMBER
                    } else if is_active {
                        Color32::WHITE
                    } else {
                        BAND_COLORS[i % 8]
                    };
                    let btn = ui.selectable_label(
                        is_active,
                        RichText::new(label)
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(color),
                    );
                    if btn.clicked() {
                        events.select_band = Some(i);
                    }
                }

                ui.separator();

                // Band Enable/Bypass toggle
                let toggle_btn = ui.selectable_label(
                    data.is_enabled,
                    RichText::new(if data.is_enabled { "ON" } else { "OFF" })
                        .font(FontId::proportional(11.0))
                        .strong()
                        .color(if data.is_enabled {
                            band_color
                        } else {
                            TEXT_MUTED
                        }),
                );
                if toggle_btn.clicked() {
                    events.toggle_enabled = true;
                }

                // Per-Band Solo (Audition) toggle
                let solo_btn = ui
                    .selectable_label(
                        data.is_soloed,
                        RichText::new("SOLO")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(if data.is_soloed {
                                ACCENT_AMBER
                            } else {
                                TEXT_MUTED
                            }),
                    )
                    .on_hover_text(
                        "Audition this band's frequency region (Bandpass / Cut monitor)",
                    );
                if solo_btn.clicked() {
                    events.toggle_solo = true;
                }

                ui.separator();

                // Filter Shape
                ui.label(
                    RichText::new("SHAPE:")
                        .font(FontId::proportional(10.5))
                        .color(TEXT_MUTED),
                );
                for (i, &name) in filter_names.iter().enumerate() {
                    let btn = ui.selectable_label(
                        data.filter_shape_idx == i,
                        RichText::new(name).font(FontId::proportional(10.0)),
                    );
                    if btn.clicked() {
                        events.set_filter_shape = Some(i);
                    }
                }

                ui.separator();

                // Routing
                ui.label(
                    RichText::new("CH:")
                        .font(FontId::proportional(10.5))
                        .color(TEXT_MUTED),
                );
                for (i, &name) in routing_names.iter().enumerate() {
                    let btn = ui.selectable_label(
                        data.routing_idx == i,
                        RichText::new(name).font(FontId::proportional(10.0)),
                    );
                    if btn.clicked() {
                        events.set_routing = Some(i);
                    }
                }
            });

            ui.add_space(6.0);

            // Controls row: Freq, Gain, Q, Dynamic Range, Dynamic Threshold
            ui.horizontal(|ui| {
                let mut freq_val = data.freq;
                ui.label(RichText::new("Freq:").font(FontId::proportional(11.0)));
                let freq_res = ui.add(
                    Slider::new(&mut freq_val, 10.0..=30000.0)
                        .logarithmic(true)
                        .suffix(" Hz")
                        .custom_formatter(|v, _| {
                            if v >= 1000.0 {
                                format!("{:.2} kHz", v / 1000.0)
                            } else {
                                format!("{:.1} Hz", v)
                            }
                        }),
                );
                if freq_res.changed() {
                    events.set_freq = Some(freq_val);
                }

                ui.add_space(8.0);

                let mut gain_val = data.gain;
                ui.label(RichText::new("Gain:").font(FontId::proportional(11.0)));
                let gain_res = ui.add(
                    Slider::new(&mut gain_val, -24.0..=24.0)
                        .suffix(" dB")
                        .custom_formatter(|v, _| format!("{:+.1} dB", v)),
                );
                if gain_res.changed() {
                    events.set_gain = Some(gain_val);
                }

                ui.add_space(8.0);

                let mut q_val = data.q;
                ui.label(RichText::new("Q:").font(FontId::proportional(11.0)));
                let q_res = ui.add(
                    Slider::new(&mut q_val, 0.1..=30.0)
                        .logarithmic(true)
                        .custom_formatter(|v, _| format!("{:.2}", v)),
                );
                if q_res.changed() {
                    events.set_q = Some(q_val);
                }

                ui.separator();

                let mut dyn_range_val = data.dyn_range;
                ui.label(RichText::new("Dyn Range:").font(FontId::proportional(11.0)));
                let dyn_res = ui.add(
                    Slider::new(&mut dyn_range_val, -24.0..=24.0)
                        .suffix(" dB")
                        .custom_formatter(|v, _| {
                            if v.abs() < 0.1 {
                                "OFF".to_string()
                            } else {
                                format!("{:+.1} dB", v)
                            }
                        }),
                );
                if dyn_res.changed() {
                    events.set_dyn_range = Some(dyn_range_val);
                }

                ui.add_space(8.0);

                let mut thresh_val = data.dyn_threshold;
                ui.label(RichText::new("Threshold:").font(FontId::proportional(11.0)));
                let thresh_res = ui.add(
                    Slider::new(&mut thresh_val, -60.0..=0.0)
                        .suffix(" dB")
                        .custom_formatter(|v, _| format!("{:.1} dB", v)),
                );
                if thresh_res.changed() {
                    events.set_dyn_threshold = Some(thresh_val);
                }
            });
        });

    events
}

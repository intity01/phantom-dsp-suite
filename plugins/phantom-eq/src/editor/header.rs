//! Top Header Bar
//! Displays branding, Processing Mode (Zero Latency vs Linear Phase + latency readout),
//! active solo indicator, analyzer toggle, oversampling toggle, and global bypass.

use super::{EditorShared, EditorState};
use crate::params::{PhantomEqParams, ProcessingModeParam};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Align, Color32, FontId, Layout, Margin, RichText, Ui};
use phantom_dsp::LATENCY_SAMPLES;
use phantom_ui::{
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, BG_CARD, TEXT_BRIGHT,
    TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_header(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
) {
    let mode = params.mode.value();
    let is_linear = mode == ProcessingModeParam::LinearPhase;

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Brand title
                ui.label(
                    RichText::new("PHANTOM")
                        .font(FontId::proportional(16.0))
                        .strong()
                        .color(TEXT_BRIGHT),
                );
                ui.label(
                    RichText::new("EQ")
                        .font(FontId::proportional(16.0))
                        .strong()
                        .color(ACCENT_CYAN),
                );

                ui.separator();

                // Processing Mode Segmented Toggle: ZERO LATENCY vs LINEAR PHASE
                let zl_btn = ui
                    .selectable_label(
                        !is_linear,
                        RichText::new("ZERO LATENCY")
                            .font(FontId::proportional(10.5))
                            .strong()
                            .color(if !is_linear { ACCENT_CYAN } else { TEXT_MUTED }),
                    )
                    .on_hover_text("Minimum-phase analog-style IIR filtering (0 samples latency)");
                if zl_btn.clicked() && is_linear {
                    setter.begin_set_parameter(&params.mode);
                    setter.set_parameter(&params.mode, ProcessingModeParam::ZeroLatency);
                    setter.end_set_parameter(&params.mode);
                }

                let lp_btn = ui
                    .selectable_label(
                        is_linear,
                        RichText::new("LINEAR PHASE")
                            .font(FontId::proportional(10.5))
                            .strong()
                            .color(if is_linear { ACCENT_PURPLE } else { TEXT_MUTED }),
                    )
                    .on_hover_text("Symmetric FIR mastering filter with zero phase distortion");
                if lp_btn.clicked() && !is_linear {
                    setter.begin_set_parameter(&params.mode);
                    setter.set_parameter(&params.mode, ProcessingModeParam::LinearPhase);
                    setter.end_set_parameter(&params.mode);
                }

                // Exact latency badge when Linear Phase is active
                if is_linear {
                    let reported = shared.latency_samples();
                    let samples = if reported > 0 {
                        reported
                    } else {
                        LATENCY_SAMPLES as u32
                    };
                    let ms = (samples as f32 / state.sample_rate.max(1.0)) * 1000.0;
                    ui.label(
                        RichText::new(format!("({samples} smp / {ms:.1} ms)"))
                            .font(FontId::proportional(10.0))
                            .color(ACCENT_PURPLE),
                    );
                }

                // Active solo warning pill (click to clear)
                if let Some(solo_idx) = shared.solo_band() {
                    ui.separator();
                    let solo_pill = ui
                        .selectable_label(
                            true,
                            RichText::new(format!("SOLO B{}", solo_idx + 1))
                                .font(FontId::proportional(10.5))
                                .strong()
                                .color(ACCENT_AMBER),
                        )
                        .on_hover_text("Click to disable Band Solo audition");
                    if solo_pill.clicked() {
                        shared.clear_solo();
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Bypass
                    let bypass_val = params.bypass.value();
                    let btn = ui.selectable_label(
                        bypass_val,
                        RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
                            .font(FontId::proportional(11.0))
                            .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED }),
                    );
                    if btn.clicked() {
                        setter.begin_set_parameter(&params.bypass);
                        setter.set_parameter(&params.bypass, !bypass_val);
                        setter.end_set_parameter(&params.bypass);
                    }

                    // 2x HQ Oversampling (applies to Zero Latency IIR mode)
                    let ac_val = params.anti_cramping.value();
                    let ac_color = if is_linear {
                        TEXT_DIMMED
                    } else if ac_val {
                        ACCENT_GREEN
                    } else {
                        TEXT_MUTED
                    };
                    let ac_btn = ui
                        .selectable_label(
                            ac_val && !is_linear,
                            RichText::new("2x HQ OS")
                                .font(FontId::proportional(11.0))
                                .color(ac_color),
                        )
                        .on_hover_text(if is_linear {
                            "2x Anti-Cramping Oversampling applies in Zero Latency mode"
                        } else {
                            "2x Oversampling to eliminate high-frequency IIR cramping"
                        });
                    if ac_btn.clicked() {
                        setter.begin_set_parameter(&params.anti_cramping);
                        setter.set_parameter(&params.anti_cramping, !ac_val);
                        setter.end_set_parameter(&params.anti_cramping);
                    }

                    // Spectrum Analyzer toggle
                    let spec_btn = ui.selectable_label(
                        state.show_spectrum,
                        RichText::new("ANALYZER")
                            .font(FontId::proportional(11.0))
                            .color(if state.show_spectrum {
                                Color32::from_rgb(6, 182, 212)
                            } else {
                                TEXT_MUTED
                            }),
                    );
                    if spec_btn.clicked() {
                        state.show_spectrum = !state.show_spectrum;
                    }

                    ui.label(
                        RichText::new("OFFLINE • ZERO TELEMETRY")
                            .font(FontId::proportional(9.5))
                            .color(TEXT_DIMMED),
                    );
                });
            });
        });
}

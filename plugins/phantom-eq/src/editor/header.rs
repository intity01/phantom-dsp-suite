//! Top Header Bar
//! Displays branding, Processing Mode (Zero Latency / Natural Phase / Linear Phase + latency),
//! Oversampling (Off / 2x / 4x), active Headphone Solo pill, Pre/Post/Freeze/Tilt Analyzer
//! controls, Bypass, and crisp Lucide Lock icon (`draw_lock_icon`) offline badge.

use super::{EditorShared, EditorState};
use crate::params::{OversamplingParam, PhantomEqParams, ProcessingModeParam};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{
    self, Align, Color32, FontId, Layout, Margin, RichText, Sense, Ui, Vec2,
};
use phantom_dsp::LATENCY_SAMPLES;
use phantom_ui::{
    draw_headphones_icon, draw_lock_icon, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE,
    ACCENT_RED, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
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
        .inner_margin(Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                // Brand title
                ui.label(
                    RichText::new("PHANTOM")
                        .font(FontId::proportional(15.0))
                        .strong()
                        .color(TEXT_BRIGHT),
                );
                ui.label(
                    RichText::new("EQ")
                        .font(FontId::proportional(15.0))
                        .strong()
                        .color(ACCENT_CYAN),
                );

                ui.separator();

                // Processing Mode: ZERO LATENCY | NATURAL PHASE | LINEAR PHASE
                draw_mode_button(
                    ui,
                    setter,
                    params,
                    mode,
                    ProcessingModeParam::ZeroLatency,
                    "ZERO",
                    ACCENT_CYAN,
                    "Zero Latency IIR filtering (0 samples latency)",
                );
                draw_mode_button(
                    ui,
                    setter,
                    params,
                    mode,
                    ProcessingModeParam::NaturalPhase,
                    "NATURAL",
                    ACCENT_GREEN,
                    "Natural Phase analog-matched cramp-free IIR (0 samples latency, no pre-ringing)",
                );
                draw_mode_button(
                    ui,
                    setter,
                    params,
                    mode,
                    ProcessingModeParam::LinearPhase,
                    "LINEAR",
                    ACCENT_PURPLE,
                    "Linear Phase symmetric FIR mastering mode (zero phase distortion)",
                );

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
                            .font(FontId::proportional(9.5))
                            .color(ACCENT_PURPLE),
                    );
                }

                ui.separator();

                // Oversampling Selector: OFF / 2x / 4x (applies in Zero & Natural Phase)
                let os = params.oversampling.value();
                for &(variant, lbl) in &[
                    (OversamplingParam::Off, "1x"),
                    (OversamplingParam::X2, "2x"),
                    (OversamplingParam::X4, "4x"),
                ] {
                    let active = os == variant && !is_linear;
                    let col = if is_linear {
                        TEXT_DIMMED
                    } else if active {
                        ACCENT_GREEN
                    } else {
                        TEXT_MUTED
                    };
                    if ui
                        .selectable_label(
                            active,
                            RichText::new(lbl)
                                .font(FontId::proportional(10.0))
                                .strong()
                                .color(col),
                        )
                        .on_hover_text(if is_linear {
                            "Polyphase oversampling applies in Zero Latency & Natural Phase modes"
                        } else {
                            "Polyphase half-band oversampling (1x Off / 2x HQ / 4x HQ)"
                        })
                        .clicked()
                    {
                        setter.begin_set_parameter(&params.oversampling);
                        setter.set_parameter(&params.oversampling, variant);
                        setter.end_set_parameter(&params.oversampling);
                    }
                }

                // Active Solo Audition pill with Lucide Headphones icon (click to clear)
                if let Some(solo_idx) = state.effective_solo(params) {
                    ui.separator();
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                    draw_headphones_icon(ui.painter(), icon_rect.center(), 5.5, ACCENT_AMBER, 1.4);
                    let pill = ui
                        .selectable_label(
                            true,
                            RichText::new(format!("SOLO B{}", solo_idx + 1))
                                .font(FontId::proportional(10.0))
                                .strong()
                                .color(ACCENT_AMBER),
                        )
                        .on_hover_text("Click to disable Band Solo audition");
                    if pill.clicked() {
                        state.latched_solo = None;
                        state.solo_hold_band = None;
                        state.alt_solo_band = None;
                        shared.clear_solo();
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Bypass toggle
                    let bypass_val = params.bypass.value();
                    if ui
                        .selectable_label(
                            bypass_val,
                            RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
                                .font(FontId::proportional(10.5))
                                .strong()
                                .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED }),
                        )
                        .clicked()
                    {
                        setter.begin_set_parameter(&params.bypass);
                        setter.set_parameter(&params.bypass, !bypass_val);
                        setter.end_set_parameter(&params.bypass);
                    }

                    ui.separator();

                    // Lucide Lock vector icon + OFFLINE badge (zero emoji glyph issues)
                    ui.label(
                        RichText::new("OFFLINE")
                            .font(FontId::proportional(9.5))
                            .color(TEXT_DIMMED),
                    );
                    let (lock_rect, _) =
                        ui.allocate_exact_size(Vec2::new(12.0, 14.0), Sense::hover());
                    draw_lock_icon(ui.painter(), lock_rect.center(), 10.0, TEXT_DIMMED, 1.3);

                    ui.separator();

                    // Spectrum Tilt cycle (4.5 -> 3.0 -> 0.0 dB/oct)
                    let tilt = state.spectrum_processor.tilt_db_per_oct();
                    if ui
                        .selectable_label(
                            tilt > 0.1,
                            RichText::new(format!("{tilt:.1}dB/o"))
                                .font(FontId::proportional(9.5))
                                .color(if tilt > 0.1 { ACCENT_CYAN } else { TEXT_MUTED }),
                        )
                        .on_hover_text("Spectrum tilt slope (click to cycle 4.5 / 3.0 / 0.0 dB/oct)")
                        .clicked()
                    {
                        let next = if tilt > 4.0 {
                            3.0
                        } else if tilt > 1.0 {
                            0.0
                        } else {
                            4.5
                        };
                        state.spectrum_processor.set_tilt_db_per_oct(next);
                    }

                    // Spectrum Freeze / Peak Hold toggle (right-click resets peaks)
                    let frozen = state.spectrum_processor.is_frozen();
                    let freeze_res = ui
                        .selectable_label(
                            frozen,
                            RichText::new("FREEZE")
                                .font(FontId::proportional(9.5))
                                .color(if frozen { ACCENT_AMBER } else { TEXT_MUTED }),
                        )
                        .on_hover_text("Freeze spectrum & peak hold (right-click to reset peaks)");
                    if freeze_res.clicked() {
                        state.spectrum_processor.set_frozen(!frozen);
                    }
                    if freeze_res.secondary_clicked() {
                        state.spectrum_processor.reset_peaks();
                    }

                    // Post-EQ & Pre-EQ spectrum toggles
                    if ui
                        .selectable_label(
                            state.show_post_spectrum,
                            RichText::new("POST")
                                .font(FontId::proportional(9.5))
                                .color(if state.show_post_spectrum {
                                    Color32::from_rgb(56, 189, 248)
                                } else {
                                    TEXT_MUTED
                                }),
                        )
                        .on_hover_text("Toggle Post-EQ spectrum trace")
                        .clicked()
                    {
                        state.show_post_spectrum = !state.show_post_spectrum;
                    }
                    if ui
                        .selectable_label(
                            state.show_pre_spectrum,
                            RichText::new("PRE")
                                .font(FontId::proportional(9.5))
                                .color(if state.show_pre_spectrum {
                                    Color32::from_rgb(168, 180, 200)
                                } else {
                                    TEXT_MUTED
                                }),
                        )
                        .on_hover_text("Toggle Pre-EQ spectrum trace")
                        .clicked()
                    {
                        state.show_pre_spectrum = !state.show_pre_spectrum;
                    }
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn draw_mode_button(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &PhantomEqParams,
    current: ProcessingModeParam,
    target: ProcessingModeParam,
    label: &str,
    active_color: Color32,
    tooltip: &str,
) {
    let active = current == target;
    let res = ui
        .selectable_label(
            active,
            RichText::new(label)
                .font(FontId::proportional(10.0))
                .strong()
                .color(if active { active_color } else { TEXT_MUTED }),
        )
        .on_hover_text(tooltip);
    if res.clicked() && !active {
        setter.begin_set_parameter(&params.mode);
        setter.set_parameter(&params.mode, target);
        setter.end_set_parameter(&params.mode);
    }
}

//! Top Header Bar & Pro-Q 4 Intelligent Tool Toggles
//! Displays branding, Processing Mode, Oversampling, Preset/Undo/Redo/A-B bar, active Solo pill,
//! `[HUD / DOCK]`, `[PIANO]`, `[SNAP]`, `[GRAB]`, `[COLLISION]`, `[EQ MATCH]`, Analyzer controls,
//! Bypass, and Lucide Lock offline badge.

use super::{match_panel, preset_bar, EditorShared, EditorState};
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
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("PHANTOM")
                        .font(FontId::proportional(14.5))
                        .strong()
                        .color(TEXT_BRIGHT),
                );
                ui.label(
                    RichText::new("EQ")
                        .font(FontId::proportional(14.5))
                        .strong()
                        .color(ACCENT_CYAN),
                );
                ui.separator();

                let modes = [
                    (ProcessingModeParam::ZeroLatency, "ZERO", ACCENT_CYAN),
                    (ProcessingModeParam::NaturalPhase, "NATURAL", ACCENT_GREEN),
                    (ProcessingModeParam::LinearPhase, "LINEAR", ACCENT_PURPLE),
                ];
                for (target, lbl, col) in modes {
                    let active = mode == target;
                    let txt = RichText::new(lbl)
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(if active { col } else { TEXT_MUTED });
                    if ui.selectable_label(active, txt).clicked() && !active {
                        setter.begin_set_parameter(&params.mode);
                        setter.set_parameter(&params.mode, target);
                        setter.end_set_parameter(&params.mode);
                    }
                }

                if is_linear {
                    let reported = shared.latency_samples();
                    let samples = if reported > 0 {
                        reported
                    } else {
                        LATENCY_SAMPLES as u32
                    };
                    let ms = (samples as f32 / state.sample_rate.max(1.0)) * 1000.0;
                    ui.label(
                        RichText::new(format!("({ms:.0}ms)"))
                            .font(FontId::proportional(9.0))
                            .color(ACCENT_PURPLE),
                    );
                }
                ui.separator();

                let os = params.oversampling.value();
                for (variant, lbl) in [
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
                    let txt = RichText::new(lbl)
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(col);
                    if ui.selectable_label(active, txt).clicked() {
                        setter.begin_set_parameter(&params.oversampling);
                        setter.set_parameter(&params.oversampling, variant);
                        setter.end_set_parameter(&params.oversampling);
                    }
                }
                ui.separator();

                preset_bar::draw_preset_bar(ui, setter, params, state);

                if let Some(solo_idx) = state.effective_solo(params) {
                    ui.separator();
                    let (r, _) = ui.allocate_exact_size(Vec2::new(13.0, 13.0), Sense::hover());
                    draw_headphones_icon(ui.painter(), r.center(), 5.2, ACCENT_AMBER, 1.4);
                    let txt = RichText::new(format!("SOLO B{}", solo_idx + 1))
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(ACCENT_AMBER);
                    if ui.selectable_label(true, txt).clicked() {
                        state.latched_solo = None;
                        state.solo_hold_band = None;
                        state.alt_solo_band = None;
                        shared.clear_solo();
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    draw_right_tools(ui, setter, params, state);
                });
            });
        });

    if state.show_eq_match {
        ui.add_space(2.0);
        match_panel::draw_eq_match_bar(ui, setter, params, state);
    }
}

fn draw_right_tools(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    let bypass_val = params.bypass.value();
    let byp_txt = RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
        .font(FontId::proportional(10.0))
        .strong()
        .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED });
    if ui.selectable_label(bypass_val, byp_txt).clicked() {
        setter.begin_set_parameter(&params.bypass);
        setter.set_parameter(&params.bypass, !bypass_val);
        setter.end_set_parameter(&params.bypass);
    }

    let (lock_rect, _) = ui.allocate_exact_size(Vec2::new(11.0, 13.0), Sense::hover());
    draw_lock_icon(ui.painter(), lock_rect.center(), 9.5, TEXT_DIMMED, 1.3);
    ui.separator();

    let hud_lbl = match (state.show_floating_hud, state.show_docked_inspector) {
        (true, false) => "HUD",
        (false, true) => "DOCK",
        _ => "HUD+DOCK",
    };
    let hud_txt = RichText::new(hud_lbl)
        .font(FontId::proportional(9.5))
        .strong()
        .color(ACCENT_CYAN);
    if ui
        .selectable_label(state.show_floating_hud, hud_txt)
        .on_hover_text("Toggle Band Inspector layout: Floating HUD (Pro-Q 4) / Docked / Both")
        .clicked()
    {
        (state.show_floating_hud, state.show_docked_inspector) =
            match (state.show_floating_hud, state.show_docked_inspector) {
                (true, false) => (false, true),
                (false, true) => (true, true),
                _ => (true, false),
            };
    }

    tool_toggle(
        ui,
        &mut state.show_piano_roll,
        "PIANO",
        ACCENT_CYAN,
        "Show 20px C1..C8 Piano Roll bar",
    );
    tool_toggle(
        ui,
        &mut state.snap_to_notes,
        "SNAP",
        ACCENT_GREEN,
        "Snap band frequency to nearest musical semitone",
    );
    tool_toggle(
        ui,
        &mut state.spectrum_grab_enabled,
        "GRAB",
        ACCENT_CYAN,
        "Spectrum Grab: click/drag resonant peaks to tame",
    );
    tool_toggle(
        ui,
        &mut state.show_collisions,
        "COLLISION",
        ACCENT_RED,
        "Multi-Instance Collision Detection overlay",
    );
    tool_toggle(
        ui,
        &mut state.show_eq_match,
        "EQ MATCH",
        ACCENT_AMBER,
        "Open Reference Spectrum EQ Match panel",
    );
    ui.separator();

    let tilt = state.spectrum_processor.tilt_db_per_oct();
    let tilt_txt = RichText::new(format!("{tilt:.1}dB"))
        .font(FontId::proportional(9.5))
        .color(if tilt > 0.1 { ACCENT_CYAN } else { TEXT_MUTED });
    if ui.selectable_label(tilt > 0.1, tilt_txt).clicked() {
        let next = if tilt > 4.0 {
            3.0
        } else if tilt > 1.0 {
            0.0
        } else {
            4.5
        };
        state.spectrum_processor.set_tilt_db_per_oct(next);
    }

    let frozen = state.spectrum_processor.is_frozen();
    let frz_txt = RichText::new("FREEZE")
        .font(FontId::proportional(9.5))
        .color(if frozen { ACCENT_AMBER } else { TEXT_MUTED });
    let freeze_res = ui.selectable_label(frozen, frz_txt);
    if freeze_res.clicked() {
        state.spectrum_processor.set_frozen(!frozen);
    }
    if freeze_res.secondary_clicked() {
        state.spectrum_processor.reset_peaks();
    }

    tool_toggle(
        ui,
        &mut state.show_post_spectrum,
        "POST",
        Color32::from_rgb(56, 189, 248),
        "Toggle Post-EQ spectrum trace",
    );
    tool_toggle(
        ui,
        &mut state.show_pre_spectrum,
        "PRE",
        Color32::from_rgb(168, 180, 200),
        "Toggle Pre-EQ spectrum trace",
    );
}

fn tool_toggle(ui: &mut Ui, flag: &mut bool, label: &str, active_col: Color32, tip: &str) {
    let txt = RichText::new(label)
        .font(FontId::proportional(9.5))
        .strong()
        .color(if *flag { active_col } else { TEXT_MUTED });
    if ui.selectable_label(*flag, txt).on_hover_text(tip).clicked() {
        *flag = !*flag;
    }
}

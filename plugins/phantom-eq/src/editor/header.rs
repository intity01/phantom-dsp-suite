//! Two-Row Non-Overlapping Top Header Bar
//! Row 1: Branding, Processing Mode, Oversampling, Preset/Undo/Redo/A-B bar, Lock badge & Bypass.
//! Row 2 (`header_row2`): `[+ BAND]`, active band pills, Solo, Analyzer, Tools & Window Sizes.

use super::{header_row2, match_panel, preset_bar, EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::{OversamplingParam, PhantomEqParams, ProcessingModeParam};
use nih_plug_egui::egui::{self, Align, FontId, Layout, Margin, RichText, Sense, Ui, Vec2};
use phantom_dsp::LATENCY_SAMPLES;
use phantom_ui::{
    draw_lock_icon, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, BG_CARD, TEXT_BRIGHT,
    TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_header(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
) {
    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            draw_row_one(ui, setter, params, state, shared);
            ui.add_space(3.0);
            header_row2::draw_row_two(ui, setter, params, state, shared);
        });

    if state.show_eq_match {
        ui.add_space(2.0);
        match_panel::draw_eq_match_bar(ui, setter, params, state);
    }
}

fn draw_row_one(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
) {
    let mode = setter.read(&params.mode);
    let is_linear = mode == ProcessingModeParam::LinearPhase;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("PHANTOM")
                .font(FontId::proportional(14.0))
                .strong()
                .color(TEXT_BRIGHT),
        );
        ui.label(
            RichText::new("EQ")
                .font(FontId::proportional(14.0))
                .strong()
                .color(ACCENT_CYAN),
        );
        ui.separator();

        for (target, lbl, col) in [
            (ProcessingModeParam::ZeroLatency, "ZERO", ACCENT_CYAN),
            (ProcessingModeParam::NaturalPhase, "NATURAL", ACCENT_GREEN),
            (ProcessingModeParam::LinearPhase, "LINEAR", ACCENT_PURPLE),
        ] {
            let active = mode == target;
            let txt = RichText::new(lbl)
                .font(FontId::proportional(9.5))
                .strong()
                .color(if active { col } else { TEXT_MUTED });
            if ui.selectable_label(active, txt).clicked() && !active {
                setter.set_once(&params.mode, target);
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

        let os = setter.read(&params.oversampling);
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
                setter.set_once(&params.oversampling, variant);
            }
        }
        ui.separator();

        preset_bar::draw_preset_bar(ui, setter, params, state);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let bypass_val = setter.read(&params.bypass);
            let byp_txt = RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
                .font(FontId::proportional(10.0))
                .strong()
                .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED });
            if ui
                .selectable_label(bypass_val, byp_txt)
                .on_hover_text("Toggle VST3 Bypass (synced with host bypass button)")
                .clicked()
            {
                setter.set_once(&params.bypass, !bypass_val);
            }
            let (lock_rect, _) = ui.allocate_exact_size(Vec2::new(11.0, 13.0), Sense::hover());
            draw_lock_icon(ui.painter(), lock_rect.center(), 9.5, TEXT_DIMMED, 1.3);
        });
    });
}

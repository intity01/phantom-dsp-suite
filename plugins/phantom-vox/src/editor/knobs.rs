//! Interactive Rotary Knobs Row for Phantom Vox
//!
//! Renders all 8 controls (`TIMING`, `DETUNE`, `FORMANT`, `WIDTH`, `TIGHTNESS`, `LEVEL`,
//! `MIX`, `OUT`) using `phantom_ui::draw_editable_rotary_knob`, supporting drag, Shift-fine,
//! mouse wheel, double-click reset, and inline keyboard numeric entry.

use crate::live_mirror::LiveSetter;
use crate::params::{PhantomVoxParams, VoxModeParam};
use nih_plug::prelude::{FloatParam, Param};
use nih_plug_egui::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Ui};
use phantom_ui::{
    draw_editable_rotary_knob, linear_to_norm, norm_to_linear, KnobEditKind, RotaryKnobConfig,
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED,
};

#[allow(clippy::too_many_arguments)]
fn vox_knob(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    param: &FloatParam,
    setter: &LiveSetter,
    min: f32,
    max: f32,
    default_val: f32,
    value_text: &str,
    accent: Color32,
    bipolar: bool,
) {
    let val = setter.read(param);
    let cfg = RotaryKnobConfig {
        label,
        value_text,
        normalized: linear_to_norm(val, min, max),
        default_normalized: linear_to_norm(default_val, min, max),
        accent_color: accent,
        diameter: 42.0,
        bipolar,
    };
    if let Some(new_norm) =
        draw_editable_rotary_knob(ui, id_salt, &cfg, KnobEditKind::GainDb { min, max })
    {
        let plain = norm_to_linear(new_norm, min, max).clamp(min, max);
        setter.set_once(param, plain);
    }
}

fn section_header(ui: &mut Ui, title: &str, col: Color32) {
    ui.label(
        RichText::new(title)
            .font(FontId::proportional(8.2))
            .strong()
            .color(col),
    );
}

pub fn draw_knobs_row(ui: &mut Ui, setter: &LiveSetter, params: &PhantomVoxParams) {
    let mode = setter.read(&params.mode);
    let doubler_col = if mode == VoxModeParam::LiveAlign {
        TEXT_DIMMED
    } else {
        ACCENT_CYAN
    };
    let formant_col = if mode == VoxModeParam::LiveAlign {
        TEXT_DIMMED
    } else {
        ACCENT_PURPLE
    };
    let align_col = if mode == VoxModeParam::Doubler {
        TEXT_DIMMED
    } else {
        ACCENT_GREEN
    };
    let level_col = if mode == VoxModeParam::Doubler {
        TEXT_DIMMED
    } else {
        ACCENT_AMBER
    };

    let timing_txt = format!("{:.1} ms", setter.read(&params.timing_ms));
    let detune_txt = format!("{:.1} ct", setter.read(&params.detune_cents));
    let formant_txt = format!("{:+.1} st", setter.read(&params.formant_st));
    let width_txt = format!("{:.0}%", setter.read(&params.width_pct));
    let tight_txt = format!("{:.0}%", setter.read(&params.align_tightness));
    let level_txt = format!("{:.0}%", setter.read(&params.level_match));
    let mix_txt = format!("{:.0}%", setter.read(&params.mix_pct));
    let out_txt = format!("{:+.1} dB", setter.read(&params.output_gain));

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 24.0).max(600.0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                // Doubler group (4 knobs)
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    section_header(ui, "DOUBLER ENGINE", doubler_col);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        vox_knob(
                            ui,
                            "vox_timing",
                            "TIMING",
                            &params.timing_ms,
                            setter,
                            0.0,
                            35.0,
                            params.timing_ms.default_plain_value(),
                            &timing_txt,
                            doubler_col,
                            false,
                        );
                        vox_knob(
                            ui,
                            "vox_detune",
                            "DETUNE",
                            &params.detune_cents,
                            setter,
                            0.0,
                            35.0,
                            params.detune_cents.default_plain_value(),
                            &detune_txt,
                            doubler_col,
                            false,
                        );
                        vox_knob(
                            ui,
                            "vox_formant",
                            "FORMANT",
                            &params.formant_st,
                            setter,
                            -4.0,
                            4.0,
                            params.formant_st.default_plain_value(),
                            &formant_txt,
                            formant_col,
                            true,
                        );
                        vox_knob(
                            ui,
                            "vox_width",
                            "WIDTH",
                            &params.width_pct,
                            setter,
                            0.0,
                            150.0,
                            params.width_pct.default_plain_value(),
                            &width_txt,
                            doubler_col,
                            false,
                        );
                    });
                });

                ui.separator();

                // Sidechain Align group (2 knobs)
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    section_header(ui, "LIVE ALIGN", align_col);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        vox_knob(
                            ui,
                            "vox_tightness",
                            "TIGHTNESS",
                            &params.align_tightness,
                            setter,
                            0.0,
                            100.0,
                            params.align_tightness.default_plain_value(),
                            &tight_txt,
                            align_col,
                            false,
                        );
                        vox_knob(
                            ui,
                            "vox_level",
                            "LEVEL",
                            &params.level_match,
                            setter,
                            0.0,
                            100.0,
                            params.level_match.default_plain_value(),
                            &level_txt,
                            level_col,
                            false,
                        );
                    });
                });

                ui.separator();

                // Output group (2 knobs)
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    section_header(ui, "OUTPUT", TEXT_BRIGHT);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        vox_knob(
                            ui,
                            "vox_mix",
                            "MIX",
                            &params.mix_pct,
                            setter,
                            0.0,
                            100.0,
                            params.mix_pct.default_plain_value(),
                            &mix_txt,
                            ACCENT_CYAN,
                            false,
                        );
                        vox_knob(
                            ui,
                            "vox_out",
                            "OUT",
                            &params.output_gain,
                            setter,
                            -24.0,
                            24.0,
                            params.output_gain.default_plain_value(),
                            &out_txt,
                            ACCENT_GREEN,
                            true,
                        );
                    });
                });
            });
        });
}

//! Pro-Q 4 Bottom Control Strip Popovers (Mode/Engine & Analyzer/Tools)
//! Dark, studio-grade popup menus keeping the main canvas calm and clutter-free.

use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::{CharacterModeParam, OversamplingParam, PhantomEqParams, ProcessingModeParam};
use nih_plug_egui::egui::{Color32, FontId, RichText, Ui};
use phantom_dsp::LATENCY_SAMPLES;
use phantom_ui::{
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, TEXT_BRIGHT, TEXT_DIMMED,
    TEXT_MUTED,
};

pub fn draw_engine_popover(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &EditorState,
    shared: &EditorShared,
) {
    let mode = setter.read(&params.mode);
    let os = setter.read(&params.oversampling);
    let ch = setter.read(&params.character);
    let is_linear = mode == ProcessingModeParam::LinearPhase;

    let mut lbl = match mode {
        ProcessingModeParam::ZeroLatency => "Zero Latency".to_string(),
        ProcessingModeParam::NaturalPhase => "Natural Phase".to_string(),
        ProcessingModeParam::LinearPhase => {
            let reported = shared.latency_samples();
            let samples = if reported > 0 {
                reported
            } else {
                LATENCY_SAMPLES as u32
            };
            let ms = (samples as f32 / state.sample_rate.max(1.0)) * 1000.0;
            format!("Linear ({ms:.0}ms)")
        }
    };
    if !is_linear {
        match os {
            OversamplingParam::X2 => lbl.push_str(" • 2x"),
            OversamplingParam::X4 => lbl.push_str(" • 4x"),
            OversamplingParam::Off => {}
        }
    }
    match ch {
        CharacterModeParam::SubtleAnalog => lbl.push_str(" • Analog"),
        CharacterModeParam::WarmTube => lbl.push_str(" • Tube"),
        CharacterModeParam::Clean => {}
    }
    lbl.push_str(" ▾");

    let lbl_col = match mode {
        ProcessingModeParam::ZeroLatency => TEXT_BRIGHT,
        ProcessingModeParam::NaturalPhase => ACCENT_GREEN,
        ProcessingModeParam::LinearPhase => ACCENT_PURPLE,
    };

    ui.menu_button(
        RichText::new(lbl)
            .font(FontId::proportional(9.8))
            .color(lbl_col),
        |ui| {
            ui.set_min_width(172.0);
            section_heading(ui, "PROCESSING MODE");
            let modes = [
                (
                    ProcessingModeParam::ZeroLatency,
                    "Zero Latency",
                    ACCENT_CYAN,
                ),
                (
                    ProcessingModeParam::NaturalPhase,
                    "Natural Phase",
                    ACCENT_GREEN,
                ),
                (
                    ProcessingModeParam::LinearPhase,
                    "Linear Phase",
                    ACCENT_PURPLE,
                ),
            ];
            for (variant, name, col) in modes {
                if choice_row(ui, mode == variant, name, col) && mode != variant {
                    setter.set_once(&params.mode, variant);
                }
            }

            ui.separator();
            section_heading(
                ui,
                if is_linear {
                    "OVERSAMPLING (IIR MODES)"
                } else {
                    "OVERSAMPLING"
                },
            );
            let os_col = if is_linear { TEXT_DIMMED } else { ACCENT_GREEN };
            for (variant, name) in [
                (OversamplingParam::Off, "Off (1x)"),
                (OversamplingParam::X2, "2x HQ"),
                (OversamplingParam::X4, "4x Mastering"),
            ] {
                if choice_row(ui, os == variant && !is_linear, name, os_col) {
                    setter.set_once(&params.oversampling, variant);
                }
            }

            ui.separator();
            section_heading(ui, "CHARACTER");
            let chars = [
                (CharacterModeParam::Clean, "Clean", ACCENT_CYAN),
                (
                    CharacterModeParam::SubtleAnalog,
                    "Subtle Analog",
                    ACCENT_AMBER,
                ),
                (CharacterModeParam::WarmTube, "Warm Tube", ACCENT_PURPLE),
            ];
            for (variant, name, col) in chars {
                if choice_row(ui, ch == variant, name, col) {
                    setter.set_once(&params.character, variant);
                }
            }
        },
    );
}

pub fn draw_analyzer_tools_popover(ui: &mut Ui, state: &mut EditorState) {
    let base = match (state.show_pre_spectrum, state.show_post_spectrum) {
        (true, true) => "Analyzer: Pre+Post",
        (true, false) => "Analyzer: Pre",
        (false, true) => "Analyzer: Post",
        (false, false) => "Analyzer: Off",
    };
    let frozen = state.spectrum_processor.is_frozen();
    let title = if frozen {
        format!("{base} • Frozen ▾")
    } else {
        format!("{base} ▾")
    };
    let col = if frozen {
        ACCENT_AMBER
    } else if state.show_pre_spectrum || state.show_post_spectrum {
        TEXT_BRIGHT
    } else {
        TEXT_MUTED
    };

    ui.menu_button(
        RichText::new(title)
            .font(FontId::proportional(9.8))
            .color(col),
        |ui| {
            ui.set_min_width(196.0);
            section_heading(ui, "SPECTRUM ANALYZER");
            toggle_row(
                ui,
                &mut state.show_pre_spectrum,
                "Pre-EQ Spectrum",
                ACCENT_CYAN,
            );
            toggle_row(
                ui,
                &mut state.show_post_spectrum,
                "Post-EQ Spectrum",
                ACCENT_CYAN,
            );
            toggle_row(ui, &mut state.show_peak_hold, "Peak Hold", ACCENT_CYAN);

            ui.horizontal(|ui| {
                let frz_col = if frozen { ACCENT_AMBER } else { TEXT_MUTED };
                if choice_row(ui, frozen, "Freeze Spectrum", frz_col) {
                    state.spectrum_processor.set_frozen(!frozen);
                }
                if choice_row(ui, false, "Reset Peaks", TEXT_MUTED) {
                    state.spectrum_processor.reset_peaks();
                }
            });

            let tilt = state.spectrum_processor.tilt_db_per_oct();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Tilt:")
                        .font(FontId::proportional(9.8))
                        .color(TEXT_MUTED),
                );
                for (val, lbl) in [(0.0_f32, "0.0"), (3.0, "3.0"), (4.5, "4.5 dB/oct")] {
                    if choice_row(ui, (tilt - val).abs() < 0.25, lbl, ACCENT_CYAN) {
                        state.spectrum_processor.set_tilt_db_per_oct(val);
                    }
                }
            });

            ui.separator();
            section_heading(ui, "PRO-Q TOOLS");
            toggle_row(
                ui,
                &mut state.spectrum_grab_enabled,
                "Spectrum Grab",
                ACCENT_CYAN,
            );
            toggle_row(
                ui,
                &mut state.show_collisions,
                "Multi-Instance Collision",
                ACCENT_RED,
            );
            toggle_row(ui, &mut state.show_eq_match, "EQ Match Panel", ACCENT_AMBER);

            ui.separator();
            section_heading(ui, "BAND EDITOR");
            toggle_row(
                ui,
                &mut state.show_floating_hud,
                "Floating HUD",
                ACCENT_CYAN,
            );
            toggle_row(
                ui,
                &mut state.show_docked_inspector,
                "Docked Panel",
                ACCENT_CYAN,
            );
            toggle_row(
                ui,
                &mut state.snap_to_notes,
                "Note Snap (Ctrl)",
                ACCENT_GREEN,
            );
        },
    );
}

fn section_heading(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .font(FontId::proportional(8.8))
            .strong()
            .color(TEXT_DIMMED),
    );
}

fn choice_row(ui: &mut Ui, active: bool, label: &str, active_col: Color32) -> bool {
    ui.selectable_label(
        active,
        RichText::new(label)
            .font(FontId::proportional(10.0))
            .color(if active { active_col } else { TEXT_MUTED }),
    )
    .clicked()
}

fn toggle_row(ui: &mut Ui, flag: &mut bool, label: &str, active_col: Color32) {
    let prefix = if *flag { "●  " } else { "○  " };
    if ui
        .selectable_label(
            *flag,
            RichText::new(format!("{prefix}{label}"))
                .font(FontId::proportional(10.0))
                .color(if *flag { active_col } else { TEXT_MUTED }),
        )
        .clicked()
    {
        *flag = !*flag;
    }
}

//! Right Inspector panel: Alignment Strength, Pitch Match Mode, Pitch/Level/Sibilance/Formant
//! controls, Artificial Double L/R generator, and clickable Phrase Confidence Report list.

use eframe::egui::{self, CornerRadius, Margin, RichText, Stroke, Ui, Vec2};
use phantom_vox_dsp::PitchMatchMode;

use crate::phrase_report::{draw_phrase_report, pitch_mode_label};
use crate::project::SourceFileStatus;
use crate::state::StudioApp;
use crate::theme::{
    pill_badge, toggle_chip, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE,
    ACCENT_RED, BG_CARD, BORDER_SUBTLE, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_inspector(ui: &mut Ui, app: &mut StudioApp) {
    ui.label(
        RichText::new("INSPECTOR")
            .size(11.5)
            .strong()
            .color(TEXT_MUTED),
    );
    ui.add_space(4.0);

    let Some(idx) = app.selected_index() else {
        ui.label(
            RichText::new("Select a vocal track to inspect alignment settings & confidence.")
                .color(TEXT_DIMMED),
        );
        return;
    };

    let is_guide = app.tracks[idx].is_guide;
    let track_name = app.tracks[idx].name.clone();
    let duration = app.tracks[idx].original.duration_sec();
    let channels = app.tracks[idx].original.channels;
    let file_status = app.tracks[idx].file_status.clone();
    let num_anchors = app.tracks[idx].anchors.len();
    let num_protected = app.tracks[idx].protected.len();

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&track_name)
                        .strong()
                        .size(13.5)
                        .color(TEXT_BRIGHT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let role = if is_guide { "GUIDE REF" } else { "DUB LAYER" };
                    let col = if is_guide { ACCENT_CYAN } else { ACCENT_GREEN };
                    pill_badge(ui, role, col, with_alpha(col, 34));
                });
            });
            ui.label(
                RichText::new(format!(
                    "{duration:.2}s  •  {} ch  •  {} Hz",
                    channels, app.tracks[idx].original.sample_rate
                ))
                .size(11.0)
                .color(TEXT_MUTED),
            );
            match &file_status {
                SourceFileStatus::Missing => {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("WARNING: External source WAV missing on disk!")
                            .size(11.0)
                            .strong()
                            .color(ACCENT_RED),
                    );
                }
                SourceFileStatus::Modified(msg) => {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("WARNING: {msg}"))
                            .size(11.0)
                            .color(ACCENT_AMBER),
                    );
                }
                _ => {}
            }
        });

    ui.add_space(8.0);

    // Artificial Double L/R button works on Guide or selected Dub.
    if ui
        .add(
            egui::Button::new(
                RichText::new("✦ Create Artificial Double L/R")
                    .strong()
                    .color(ACCENT_CYAN),
            )
            .fill(with_alpha(ACCENT_CYAN, 28))
            .stroke(Stroke::new(1.0_f32, with_alpha(ACCENT_CYAN, 160)))
            .corner_radius(CornerRadius::same(7))
            .min_size(Vec2::new(ui.available_width(), 30.0)),
        )
        .clicked()
    {
        app.create_artificial_doubles();
    }

    if is_guide {
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Guide tracks act as the timing & pitch reference.\nSelect any Dub track below to edit alignment strength, harmony-safe pitch mode, and phrase confidence.",
            )
            .size(11.5)
            .color(TEXT_MUTED),
        );
        return;
    }

    ui.add_space(8.0);
    let mut cfg = app.tracks[idx].config;
    let mut cfg_changed = false;

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new("ALIGNMENT & PITCH")
                    .size(10.5)
                    .strong()
                    .color(TEXT_MUTED),
            );
            ui.add_space(4.0);

            let mut align_pct = cfg.timing_strength * 100.0;
            ui.label(RichText::new(format!("Alignment Strength: {:.0}%", align_pct)).size(11.5));
            if ui
                .add(egui::Slider::new(&mut align_pct, 0.0..=100.0).show_value(false))
                .changed()
            {
                cfg.timing_strength = align_pct / 100.0;
                cfg_changed = true;
            }

            ui.add_space(4.0);
            ui.label(RichText::new("Pitch Match Mode").size(11.5));
            let modes = [
                (PitchMatchMode::Off, "Off"),
                (PitchMatchMode::MatchAllToGuide, "Match All"),
                (PitchMatchMode::MatchUnisonOnly, "Unison Only"),
                (
                    PitchMatchMode::MatchUnisonAndTuneHarmony,
                    "Unison + Tune Harmony",
                ),
            ];
            egui::ComboBox::from_id_salt("pitch_match_mode_box")
                .width(ui.available_width() - 4.0)
                .selected_text(pitch_mode_label(cfg.pitch_mode))
                .show_ui(ui, |ui| {
                    for (mode, lbl) in modes {
                        if ui
                            .selectable_value(&mut cfg.pitch_mode, mode, lbl)
                            .changed()
                        {
                            cfg_changed = true;
                        }
                    }
                });

            let mut pitch_pct = cfg.pitch_strength * 100.0;
            ui.label(RichText::new(format!("Pitch Strength: {:.0}%", pitch_pct)).size(11.5));
            if ui
                .add(egui::Slider::new(&mut pitch_pct, 0.0..=100.0).show_value(false))
                .changed()
            {
                cfg.pitch_strength = pitch_pct / 100.0;
                cfg_changed = true;
            }

            let mut level_pct = cfg.level_strength * 100.0;
            ui.label(RichText::new(format!("Level Match: {:.0}%", level_pct)).size(11.5));
            if ui
                .add(egui::Slider::new(&mut level_pct, 0.0..=100.0).show_value(false))
                .changed()
            {
                cfg.level_strength = level_pct / 100.0;
                cfg_changed = true;
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if toggle_chip(ui, "Sibilance Protect", cfg.protect_sibilance, ACCENT_AMBER)
                    .clicked()
                {
                    cfg.protect_sibilance = !cfg.protect_sibilance;
                    cfg_changed = true;
                }
                if toggle_chip(ui, "Formant Preserve", cfg.formant_preserve, ACCENT_CYAN).clicked()
                {
                    cfg.formant_preserve = !cfg.formant_preserve;
                    cfg_changed = true;
                }
            });
        });

    if cfg_changed {
        app.push_undo();
        app.tracks[idx].config = cfg;
        app.queue_track_rerender(idx);
    }

    ui.add_space(6.0);
    // Anchors & Protected Regions summary controls.
    ui.horizontal(|ui| {
        pill_badge(
            ui,
            &format!("Anchors: {num_anchors}"),
            ACCENT_AMBER,
            with_alpha(ACCENT_AMBER, 30),
        );
        if num_anchors > 0 && ui.small_button("Clear").clicked() {
            app.push_undo();
            app.tracks[idx].anchors.clear();
            app.queue_track_analysis(idx);
        }
        pill_badge(
            ui,
            &format!("Locked: {num_protected}"),
            ACCENT_PURPLE,
            with_alpha(ACCENT_PURPLE, 30),
        );
        if num_protected > 0 && ui.small_button("Clear").clicked() {
            app.push_undo();
            app.tracks[idx].protected.clear();
            app.queue_track_analysis(idx);
        }
    });

    ui.add_space(8.0);
    draw_phrase_report(ui, app, idx);
}

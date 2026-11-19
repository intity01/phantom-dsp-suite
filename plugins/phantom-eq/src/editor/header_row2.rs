//! Header Row 2 — Band Quick-Add/Pills, Solo Badge, Analyzer, Pro-Q Tools & Fixed Window Sizes
//! Keeps Row 2 strictly <= 820 px so it never overlaps at any window preset (`S / M / L / XL`).

use super::display_helpers::create_smart_default_band;
use super::window_size::{apply_size_preset, preset_index_for_size, SIZE_PRESETS};
use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::PhantomEqParams;
use nih_plug_egui::egui::{Align, Color32, FontId, Layout, RichText, Sense, Ui, Vec2};
use phantom_ui::{
    band_color, draw_headphones_icon, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_RED,
    TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

const MAX_INLINE_BAND_PILLS: usize = 8;

pub fn draw_row_two(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
) {
    let live = shared.live.as_ref();
    let now = ui.input(|i| i.time);

    ui.horizontal(|ui| {
        let add_txt = RichText::new("+ BAND")
            .font(FontId::proportional(9.5))
            .strong()
            .color(ACCENT_CYAN);
        if ui
            .selectable_label(true, add_txt)
            .on_hover_text("Create a new EQ band at the next uncrowded frequency")
            .clicked()
        {
            let _ = create_smart_default_band(setter, params, state, now);
        }

        let mut shown = 0usize;
        let mut hidden = 0usize;
        for (i, band) in params.bands.iter().enumerate() {
            if !live.read(&band.used) {
                continue;
            }
            let is_sel = state.selected_band == Some(i);
            if shown < MAX_INLINE_BAND_PILLS || is_sel {
                shown += 1;
                let col = if live.read(&band.enabled) {
                    band_color(i)
                } else {
                    TEXT_DIMMED
                };
                let pill_txt = RichText::new(format!("B{}", i + 1))
                    .font(FontId::proportional(9.0))
                    .strong()
                    .color(if is_sel { TEXT_BRIGHT } else { col });
                if ui
                    .selectable_label(is_sel, pill_txt)
                    .on_hover_text(format!("Select Band {}", i + 1))
                    .clicked()
                {
                    state.selected_band = Some(i);
                }
            } else {
                hidden += 1;
            }
        }
        if hidden > 0 {
            ui.label(
                RichText::new(format!("+{hidden}"))
                    .font(FontId::proportional(9.0))
                    .color(TEXT_DIMMED),
            );
        }

        if let Some(solo_idx) = state.effective_solo(params, live) {
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

        ui.separator();
        draw_analyzer_and_tools(ui, state);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let cur_size_idx = preset_index_for_size(params.editor_state.size());
            for idx in (0..SIZE_PRESETS.len()).rev() {
                let p = SIZE_PRESETS[idx];
                let active = idx == cur_size_idx;
                let txt = RichText::new(p.label)
                    .font(FontId::proportional(9.5))
                    .strong()
                    .color(if active { ACCENT_CYAN } else { TEXT_MUTED });
                if ui
                    .selectable_label(active, txt)
                    .on_hover_text(format!(
                        "Fixed window size {} ({}x{}, {:.0}% scale)",
                        p.label,
                        p.width,
                        p.height,
                        p.zoom * 100.0
                    ))
                    .clicked()
                {
                    apply_size_preset(ui.ctx(), setter.host, &params.editor_state, idx);
                }
            }
            ui.label(
                RichText::new("SIZE:")
                    .font(FontId::proportional(9.0))
                    .color(TEXT_DIMMED),
            );
        });
    });
}

fn draw_analyzer_and_tools(ui: &mut Ui, state: &mut EditorState) {
    tool_toggle(
        ui,
        &mut state.show_pre_spectrum,
        "PRE",
        Color32::from_rgb(168, 180, 200),
        "Toggle Pre-EQ spectrum trace",
    );
    tool_toggle(
        ui,
        &mut state.show_post_spectrum,
        "POST",
        Color32::from_rgb(56, 189, 248),
        "Toggle Post-EQ spectrum trace",
    );
    let frozen = state.spectrum_processor.is_frozen();
    let frz_res = ui.selectable_label(
        frozen,
        RichText::new("FREEZE")
            .font(FontId::proportional(9.5))
            .color(if frozen { ACCENT_AMBER } else { TEXT_MUTED }),
    );
    if frz_res.clicked() {
        state.spectrum_processor.set_frozen(!frozen);
    }
    if frz_res.secondary_clicked() {
        state.spectrum_processor.reset_peaks();
    }
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

    ui.separator();
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
        .on_hover_text("Toggle Band Inspector layout: Floating HUD / Docked / Both")
        .clicked()
    {
        (state.show_floating_hud, state.show_docked_inspector) =
            match (state.show_floating_hud, state.show_docked_inspector) {
                (true, false) => (false, true),
                (false, true) => (true, true),
                _ => (true, false),
            };
    }
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

//! Preset Browser, A/B State Bank & 64-Step Undo/Redo Header Controls
//! Integrates 12 Factory Presets + Offline `.peq` User Presets, `<` / `>` quick navigation,
//! `[Save]`, `[Undo]` / `[Redo]`, `[A / B]` slot toggle, and `[Copy]` into the top header bar.

use super::EditorState;
use crate::params::{PhantomEqParams, NUM_BANDS};
use crate::state::{load_user_presets, save_user_preset, EqPresetSnapshot};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, FontId, RichText, Ui};
use phantom_ui::{ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED};

pub fn draw_preset_bar(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    let factory_len = state.factory_presets.len();
    let total_presets = (factory_len + state.user_presets.len()).max(1);
    let cur_idx = state.current_preset_idx.min(total_presets - 1);

    let active_preset = if cur_idx < factory_len {
        state.factory_presets.get(cur_idx)
    } else {
        state.user_presets.get(cur_idx - factory_len)
    };
    let is_modified = active_preset
        .map(|p| !p.approx_eq(&state.history.active_snapshot))
        .unwrap_or(false);
    let base_name = active_preset
        .map(|p| p.name.as_str())
        .unwrap_or("Default (Flat Empty)");
    let display_title = if is_modified {
        format!("{base_name} *")
    } else {
        base_name.to_string()
    };

    // Prev `<` preset button
    if ui
        .selectable_label(
            false,
            RichText::new("<")
                .font(FontId::proportional(10.0))
                .strong()
                .color(TEXT_BRIGHT),
        )
        .on_hover_text("Previous preset")
        .clicked()
    {
        let prev_idx = (cur_idx + total_presets - 1) % total_presets;
        apply_preset_by_index(setter, params, state, prev_idx);
    }

    // Preset ComboBox selector (12 Factory Presets + User Presets)
    let mut chosen_idx: Option<usize> = None;
    egui::ComboBox::from_id_salt("phantom_eq_preset_selector")
        .width(172.0)
        .selected_text(
            RichText::new(display_title)
                .font(FontId::proportional(10.0))
                .color(TEXT_BRIGHT),
        )
        .show_ui(ui, |ui| {
            for (idx, preset) in state.factory_presets.iter().enumerate() {
                let label = format!("[{}] {}", preset.category, preset.name);
                if ui.selectable_label(cur_idx == idx, label).clicked() {
                    chosen_idx = Some(idx);
                }
            }
            if !state.user_presets.is_empty() {
                ui.separator();
                for (u_off, preset) in state.user_presets.iter().enumerate() {
                    let idx = factory_len + u_off;
                    let label = format!("[User] {}", preset.name);
                    if ui.selectable_label(cur_idx == idx, label).clicked() {
                        chosen_idx = Some(idx);
                    }
                }
            }
        });
    if let Some(idx) = chosen_idx {
        apply_preset_by_index(setter, params, state, idx);
    }

    // Next `>` preset button
    if ui
        .selectable_label(
            false,
            RichText::new(">")
                .font(FontId::proportional(10.0))
                .strong()
                .color(TEXT_BRIGHT),
        )
        .on_hover_text("Next preset")
        .clicked()
    {
        let next_idx = (cur_idx + 1) % total_presets;
        apply_preset_by_index(setter, params, state, next_idx);
    }

    // `[Save]` offline `.peq` User Preset button
    if ui
        .selectable_label(
            false,
            RichText::new("Save")
                .font(FontId::proportional(9.5))
                .strong()
                .color(ACCENT_GREEN),
        )
        .on_hover_text("Save current EQ state as an offline .peq user preset")
        .clicked()
    {
        let mut snap = EqPresetSnapshot::capture_from(params);
        let next_num = state.user_presets.len() + 1;
        snap.name = format!("User Preset {next_num:02}");
        if save_user_preset(&snap).is_ok() {
            state.user_presets = load_user_presets();
            if let Some(pos) = state.user_presets.iter().position(|p| p.name == snap.name) {
                state.current_preset_idx = factory_len + pos;
            }
        }
    }

    ui.separator();

    // `[Undo]` & `[Redo]` buttons
    let can_undo = state.history.can_undo();
    if ui
        .selectable_label(
            false,
            RichText::new("Undo")
                .font(FontId::proportional(9.5))
                .color(if can_undo { TEXT_BRIGHT } else { TEXT_DIMMED }),
        )
        .on_hover_text(format!(
            "Undo last edit (Ctrl+Z, {} steps)",
            state.history.undo_count()
        ))
        .clicked()
        && can_undo
    {
        state.history.undo(params, setter);
        sync_selected_band_after_load(params, state);
    }

    let can_redo = state.history.can_redo();
    if ui
        .selectable_label(
            false,
            RichText::new("Redo")
                .font(FontId::proportional(9.5))
                .color(if can_redo { TEXT_BRIGHT } else { TEXT_DIMMED }),
        )
        .on_hover_text(format!(
            "Redo edit (Ctrl+Y, {} steps)",
            state.history.redo_count()
        ))
        .clicked()
        && can_redo
    {
        state.history.redo(params, setter);
        sync_selected_band_after_load(params, state);
    }

    // `[A / B]` Compare toggle button + `[Copy]` button
    let is_b = state.history.is_slot_b();
    let ab_label = if is_b { "A / [B]" } else { "[A] / B" };
    let ab_col = if is_b { ACCENT_AMBER } else { ACCENT_CYAN };
    if ui
        .selectable_label(
            true,
            RichText::new(ab_label)
                .font(FontId::proportional(9.5))
                .strong()
                .color(ab_col),
        )
        .on_hover_text("Toggle between A and B comparison slots")
        .clicked()
    {
        state.history.toggle_ab(params, setter);
        sync_selected_band_after_load(params, state);
    }

    let copy_txt = if is_b { "Copy B>A" } else { "Copy A>B" };
    if ui
        .selectable_label(
            false,
            RichText::new(copy_txt)
                .font(FontId::proportional(9.5))
                .color(TEXT_MUTED),
        )
        .on_hover_text("Copy active slot state into the opposite A/B comparison slot")
        .clicked()
    {
        state.history.copy_active_to_other(params);
    }
}

pub fn apply_preset_by_index(
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    idx: usize,
) {
    let factory_len = state.factory_presets.len();
    let maybe_preset = if idx < factory_len {
        state.factory_presets.get(idx).cloned()
    } else {
        state.user_presets.get(idx - factory_len).cloned()
    };
    if let Some(preset) = maybe_preset {
        state.current_preset_idx = idx;
        state.history.apply_preset(&preset, params, setter);
        sync_selected_band_after_load(params, state);
    }
}

pub fn sync_selected_band_after_load(params: &PhantomEqParams, state: &mut EditorState) {
    state.selected_band = (0..NUM_BANDS).find(|&i| params.bands[i].used.value());
}

//! Preset Browser, Custom User Preset Naming/Deletion, A/B State Bank & 64-Step Undo/Redo
//! Integrates 12 Factory Presets + Offline `.peq` User Presets, `<` / `>` quick navigation,
//! inline custom-named `[Save]`, per-preset `×` delete, `[Undo]`/`[Redo]`, `[A / B]`, and `[Copy]`.

use super::EditorState;
use crate::live_mirror::{LiveParamMirror, LiveSetter};
use crate::params::{PhantomEqParams, NUM_BANDS};
use crate::state::{delete_user_preset, load_user_presets, save_user_preset, EqPresetSnapshot};
use nih_plug_egui::egui::{self, Align, Color32, FontId, Key, Layout, RichText, TextEdit, Ui};
use phantom_ui::{
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_RED, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_preset_bar(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    let factory_len = state.factory_presets.len();
    let total_presets = (factory_len + state.user_presets.len()).max(1);
    let cur_idx = state.current_preset_idx.min(total_presets - 1);

    let (is_modified, base_name) = {
        let active_preset = if cur_idx < factory_len {
            state.factory_presets.get(cur_idx)
        } else {
            state.user_presets.get(cur_idx - factory_len)
        };
        let modified = active_preset
            .map(|p| !p.approx_eq(&state.history.active_snapshot))
            .unwrap_or(false);
        let name = active_preset
            .map(|p| p.name.clone())
            .unwrap_or_else(|| String::from("Default (Flat Empty)"));
        (modified, name)
    };
    let display_title = if is_modified {
        format!("{base_name} *")
    } else {
        base_name.clone()
    };

    if bar_btn(ui, false, "<", 10.0, true, TEXT_BRIGHT, "Previous preset") {
        apply_preset_by_index(
            setter,
            params,
            state,
            (cur_idx + total_presets - 1) % total_presets,
        );
    }

    let mut chosen_idx: Option<usize> = None;
    let mut delete_name: Option<String> = None;
    egui::ComboBox::from_id_salt("phantom_eq_preset_selector")
        .width(165.0)
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
                    ui.horizontal(|ui| {
                        let label = format!("[User] {}", preset.name);
                        if ui.selectable_label(cur_idx == idx, label).clicked() {
                            chosen_idx = Some(idx);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let x = RichText::new("×")
                                .font(FontId::proportional(10.0))
                                .strong()
                                .color(ACCENT_RED);
                            if ui
                                .small_button(x)
                                .on_hover_text(format!("Delete user preset \"{}\"", preset.name))
                                .clicked()
                            {
                                delete_name = Some(preset.name.clone());
                            }
                        });
                    });
                }
            }
        });
    if let Some(name) = delete_name {
        if delete_user_preset(&name).is_ok() {
            state.user_presets = load_user_presets();
            let new_total = (factory_len + state.user_presets.len()).max(1);
            if state.current_preset_idx >= new_total || base_name == name {
                state.current_preset_idx = 0;
            }
        }
    } else if let Some(idx) = chosen_idx {
        apply_preset_by_index(setter, params, state, idx);
    }

    if bar_btn(ui, false, ">", 10.0, true, TEXT_BRIGHT, "Next preset") {
        apply_preset_by_index(setter, params, state, (cur_idx + 1) % total_presets);
    }

    draw_save_preset_control(ui, setter, params, state, factory_len, cur_idx);
    ui.separator();
    draw_history_ab_controls(ui, setter, params, state);
}

fn draw_save_preset_control(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    factory_len: usize,
    cur_idx: usize,
) {
    if let Some(mut draft) = state.saving_preset_name.take() {
        let edit_res = ui.add(
            TextEdit::singleline(&mut draft)
                .desired_width(110.0)
                .font(FontId::proportional(9.8))
                .hint_text("Preset name..."),
        );
        if state.focus_preset_name {
            edit_res.request_focus();
            state.focus_preset_name = false;
        }
        let enter_commit = (edit_res.lost_focus() || edit_res.has_focus())
            && ui.input(|i| i.key_pressed(Key::Enter));
        let esc_cancel = ui.input(|i| i.key_pressed(Key::Escape));
        let save_clicked = bar_btn(
            ui,
            true,
            "Save",
            9.5,
            true,
            ACCENT_GREEN,
            "Save .peq (Enter)",
        );
        let cancel_clicked = bar_btn(ui, false, "×", 10.0, false, TEXT_MUTED, "Cancel (Esc)");

        if esc_cancel || cancel_clicked {
            return;
        }
        if enter_commit || save_clicked {
            let trimmed = draft.trim();
            if !trimmed.is_empty() {
                let mut snap = EqPresetSnapshot::capture_with(params, setter.live);
                snap.name = trimmed.to_string();
                if save_user_preset(&snap).is_ok() {
                    state.user_presets = load_user_presets();
                    if let Some(pos) = state.user_presets.iter().position(|p| p.name == snap.name) {
                        state.current_preset_idx = factory_len + pos;
                    }
                }
            }
            return;
        }
        state.saving_preset_name = Some(draft);
    } else if bar_btn(
        ui,
        false,
        "Save",
        9.5,
        true,
        ACCENT_GREEN,
        "Save current EQ state as a named offline .peq user preset",
    ) {
        let suggested = if cur_idx >= factory_len {
            state
                .user_presets
                .get(cur_idx - factory_len)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("My Preset {:02}", state.user_presets.len() + 1))
        } else {
            format!("My Preset {:02}", state.user_presets.len() + 1)
        };
        state.saving_preset_name = Some(suggested);
        state.focus_preset_name = true;
    }
}

fn draw_history_ab_controls(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    let can_undo = state.history.can_undo();
    let undo_tip = format!(
        "Undo last edit (Ctrl+Z, {} steps)",
        state.history.undo_count()
    );
    let undo_col = if can_undo { TEXT_BRIGHT } else { TEXT_DIMMED };
    if bar_btn(ui, false, "Undo", 9.5, false, undo_col, &undo_tip) && can_undo {
        state.history.undo(params, setter);
        sync_selected_band_after_load(params, setter.live, state);
    }

    let can_redo = state.history.can_redo();
    let redo_tip = format!("Redo edit (Ctrl+Y, {} steps)", state.history.redo_count());
    let redo_col = if can_redo { TEXT_BRIGHT } else { TEXT_DIMMED };
    if bar_btn(ui, false, "Redo", 9.5, false, redo_col, &redo_tip) && can_redo {
        state.history.redo(params, setter);
        sync_selected_band_after_load(params, setter.live, state);
    }

    let is_b = state.history.is_slot_b();
    let ab_label = if is_b { "A/[B]" } else { "[A]/B" };
    let ab_col = if is_b { ACCENT_AMBER } else { ACCENT_CYAN };
    if bar_btn(
        ui,
        true,
        ab_label,
        9.5,
        true,
        ab_col,
        "Toggle between A and B comparison slots",
    ) {
        state.history.toggle_ab(params, setter);
        sync_selected_band_after_load(params, setter.live, state);
    }

    let copy_tip = if is_b {
        "Copy active slot B into slot A"
    } else {
        "Copy active slot A into slot B"
    };
    if bar_btn(ui, false, "Copy", 9.5, false, TEXT_MUTED, copy_tip) {
        state.history.copy_active_to_other(params, setter.live);
    }
}

fn bar_btn(
    ui: &mut Ui,
    selected: bool,
    label: &str,
    size: f32,
    strong: bool,
    color: Color32,
    tip: &str,
) -> bool {
    let mut rt = RichText::new(label)
        .font(FontId::proportional(size))
        .color(color);
    if strong {
        rt = rt.strong();
    }
    ui.selectable_label(selected, rt)
        .on_hover_text(tip)
        .clicked()
}

pub fn apply_preset_by_index(
    setter: &LiveSetter,
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
        sync_selected_band_after_load(params, setter.live, state);
    }
}

pub fn sync_selected_band_after_load(
    params: &PhantomEqParams,
    live: &LiveParamMirror,
    state: &mut EditorState,
) {
    if state
        .selected_band
        .is_some_and(|s| s >= NUM_BANDS || !live.read(&params.bands[s].used))
    {
        state.selected_band = None;
    }
    state.node_menu = None;
}

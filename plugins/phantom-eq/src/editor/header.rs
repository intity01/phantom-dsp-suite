//! Ultra-Clean Pro-Q 4 Single-Row Top Header Bar
//! Left: Branding, Preset Browser (`< [ComboBox 165px] > Save`), Undo/Redo, A/B & Copy.
//! Right: Active Solo pill (when soloed), Fixed Window Size presets (`S M L XL`), Lock & Bypass.

use super::window_size::{apply_size_preset, preset_index_for_size, SIZE_PRESETS};
use super::{match_panel, preset_bar, EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::PhantomEqParams;
use nih_plug_egui::egui::{
    self, Align, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2,
};
use phantom_ui::{
    draw_headphones_icon, draw_lock_icon, draw_phantom_orb_icon, draw_phantom_wordmark, with_alpha,
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_RED, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_header(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
) {
    let live = shared.live.as_ref();

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.horizontal(|ui| {
                draw_phantom_orb_icon(ui, 20.0);
                draw_phantom_wordmark(ui, 16.0, TEXT_BRIGHT, ACCENT_CYAN);
                egui::Frame::NONE
                    .fill(with_alpha(ACCENT_CYAN, 34))
                    .stroke(Stroke::new(1.0_f32, with_alpha(ACCENT_CYAN, 95)))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(5, 1))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("EQ")
                                .font(FontId::proportional(10.5))
                                .strong()
                                .color(ACCENT_CYAN),
                        );
                    });
                ui.separator();

                preset_bar::draw_preset_bar(ui, setter, params, state);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let bypass_val = setter.read(&params.bypass);
                    let byp_txt = RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED });
                    if ui
                        .selectable_label(bypass_val, byp_txt)
                        .on_hover_text("Toggle VST3 Bypass (synced with host bypass button)")
                        .clicked()
                    {
                        setter.set_once(&params.bypass, !bypass_val);
                    }
                    let (lock_rect, _) =
                        ui.allocate_exact_size(Vec2::new(11.0, 13.0), Sense::hover());
                    draw_lock_icon(ui.painter(), lock_rect.center(), 9.5, TEXT_DIMMED, 1.3);

                    ui.separator();

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
                                "Window size {} ({}x{}, {:.0}% scale)",
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

                    if let Some(solo_idx) = state.effective_solo(params, live) {
                        ui.separator();
                        let txt = RichText::new(format!("SOLO B{}", solo_idx + 1))
                            .font(FontId::proportional(9.5))
                            .strong()
                            .color(ACCENT_AMBER);
                        let pill_clicked = ui
                            .selectable_label(true, txt)
                            .on_hover_text("Click to clear active band solo")
                            .clicked();
                        let (r, icon_res) =
                            ui.allocate_exact_size(Vec2::new(13.0, 13.0), Sense::click());
                        draw_headphones_icon(ui.painter(), r.center(), 5.2, ACCENT_AMBER, 1.4);
                        if pill_clicked || icon_res.clicked() {
                            state.latched_solo = None;
                            state.solo_hold_band = None;
                            state.alt_solo_band = None;
                            shared.clear_solo();
                        }
                    }
                });
            });
        });

    if state.show_eq_match {
        ui.add_space(2.0);
        match_panel::draw_eq_match_bar(ui, setter, params, state);
    }
}

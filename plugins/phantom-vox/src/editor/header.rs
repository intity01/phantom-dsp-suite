//! Slim Top Header Bar for Phantom Vox
//!
//! Layout: `PHANTOM VOX` | Mode tabs `DOUBLER` `LIVE ALIGN` `COMBO` | `2V / 4V` |
//! `A` `B` `Copy` `Hold: Original` | `S M L` window size presets | Lock icon + `BYPASS`.

use super::compare::draw_compare_controls;
use super::window_size::{apply_size_preset, preset_index_for_size, SIZE_PRESETS};
use crate::live_mirror::LiveSetter;
use crate::params::{PhantomVoxParams, VoicesParam, VoxModeParam};
use nih_plug_egui::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    draw_lock_icon, draw_phantom_orb_icon, draw_phantom_wordmark, with_alpha, ACCENT_CYAN,
    ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};
use std::sync::atomic::AtomicBool;

fn mode_tab(ui: &mut Ui, label: &str, active: bool, accent: Color32, w: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
    let hov = resp.hovered();
    let fill = if active {
        with_alpha(accent, 44)
    } else if hov {
        Color32::from_rgba_unmultiplied(255, 255, 255, 20)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 9)
    };
    let border = if active {
        with_alpha(accent, 185)
    } else if hov {
        with_alpha(accent, 95)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    };
    let fg = if active {
        accent
    } else if hov {
        TEXT_BRIGHT
    } else {
        TEXT_MUTED
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.5),
        fg,
    );
    resp.clicked()
}

pub fn draw_header(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomVoxParams,
    audition: &AtomicBool,
) {
    let mode = setter.read(&params.mode);
    let voices = setter.read(&params.voices);
    let bypass_val = setter.read(&params.bypass);

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        ))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                draw_phantom_orb_icon(ui, 19.0);
                draw_phantom_wordmark(ui, 15.0, TEXT_BRIGHT, ACCENT_CYAN);
                egui::Frame::NONE
                    .fill(with_alpha(ACCENT_PURPLE, 38))
                    .stroke(Stroke::new(1.0_f32, with_alpha(ACCENT_PURPLE, 110)))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(5, 1))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("VOX")
                                .font(FontId::proportional(10.0))
                                .strong()
                                .color(ACCENT_PURPLE),
                        );
                    });

                ui.separator();

                if mode_tab(
                    ui,
                    "DOUBLER",
                    mode == VoxModeParam::Doubler,
                    ACCENT_CYAN,
                    66.0,
                ) {
                    setter.set_once(&params.mode, VoxModeParam::Doubler);
                }
                if mode_tab(
                    ui,
                    "LIVE ALIGN",
                    mode == VoxModeParam::LiveAlign,
                    ACCENT_GREEN,
                    76.0,
                ) {
                    setter.set_once(&params.mode, VoxModeParam::LiveAlign);
                }
                if mode_tab(
                    ui,
                    "COMBO",
                    mode == VoxModeParam::AlignAndDouble,
                    ACCENT_PURPLE,
                    58.0,
                ) {
                    setter.set_once(&params.mode, VoxModeParam::AlignAndDouble);
                }

                ui.separator();

                let voice_accent = if mode == VoxModeParam::LiveAlign {
                    TEXT_DIMMED
                } else {
                    ACCENT_CYAN
                };
                if mode_tab(
                    ui,
                    "2V",
                    voices == VoicesParam::TwoVoices,
                    voice_accent,
                    30.0,
                ) {
                    setter.set_once(&params.voices, VoicesParam::TwoVoices);
                }
                if mode_tab(
                    ui,
                    "4V",
                    voices == VoicesParam::FourVoices,
                    voice_accent,
                    30.0,
                ) {
                    setter.set_once(&params.voices, VoicesParam::FourVoices);
                }

                ui.separator();
                draw_compare_controls(ui, setter, params, audition);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let byp_txt = RichText::new(if bypass_val { "BYPASS ON" } else { "BYPASS" })
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(if bypass_val { ACCENT_RED } else { TEXT_MUTED });
                    if ui
                        .selectable_label(bypass_val, byp_txt)
                        .on_hover_text("Toggle VST3 Bypass (synced with Cubase insert bypass)")
                        .clicked()
                    {
                        setter.set_once(&params.bypass, !bypass_val);
                    }

                    let (lock_rect, _) =
                        ui.allocate_exact_size(Vec2::new(11.0, 14.0), Sense::hover());
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
                });
            });
        });
}

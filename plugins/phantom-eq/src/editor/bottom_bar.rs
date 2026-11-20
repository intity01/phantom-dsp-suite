//! Pro-Q 4 Slim Bottom Control Strip (~28 px)
//! Left: Mode/Engine popover, Analyzer & Tools popover, Piano/Snap toggle, Display Scale dropdown.
//! Right: Phase Invert (Ø), Auto-Gain, Global Gain Scale (0%-200%), Output Gain, and PHANTOM DSP badge.

use super::bottom_popovers::{draw_analyzer_tools_popover, draw_engine_popover};
use super::{EditorShared, EditorState};
use crate::live_mirror::LiveSetter;
use crate::params::PhantomEqParams;
use nih_plug_egui::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Slider,
    Stroke, StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    draw_icon_button, draw_phase_icon, parse_audio_number, with_alpha, ACCENT_AMBER, ACCENT_CYAN,
    ACCENT_GREEN, BG_CARD, DB_SCALE_OPTIONS, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_bottom_bar(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    shared: &EditorShared,
    makeup_db: f32,
) {
    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.spacing_mut().slider_width = 72.0;

            ui.horizontal(|ui| {
                draw_engine_popover(ui, setter, params, state, shared);
                draw_analyzer_tools_popover(ui, state);

                ui.separator();

                let piano_txt = RichText::new("Piano")
                    .font(FontId::proportional(9.8))
                    .strong()
                    .color(if state.show_piano_roll {
                        ACCENT_CYAN
                    } else {
                        TEXT_MUTED
                    });
                if ui
                    .selectable_label(state.show_piano_roll, piano_txt)
                    .on_hover_text("Toggle 20px C1..C8 Piano Roll frequency strip")
                    .clicked()
                {
                    state.show_piano_roll = !state.show_piano_roll;
                }
                if state.show_piano_roll {
                    let snap_txt = RichText::new("Snap")
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(if state.snap_to_notes {
                            ACCENT_GREEN
                        } else {
                            TEXT_MUTED
                        });
                    if ui
                        .selectable_label(state.snap_to_notes, snap_txt)
                        .on_hover_text("Snap band frequencies to nearest musical semitone (Ctrl)")
                        .clicked()
                    {
                        state.snap_to_notes = !state.snap_to_notes;
                    }
                }

                ui.separator();

                egui::ComboBox::from_id_salt("phantom_display_scale")
                    .width(62.0)
                    .selected_text(
                        RichText::new(format!("±{:.0} dB", state.scale_db))
                            .font(FontId::proportional(9.8))
                            .color(ACCENT_CYAN),
                    )
                    .show_ui(ui, |ui| {
                        for &opt in &DB_SCALE_OPTIONS {
                            if ui
                                .selectable_label(
                                    (state.scale_db - opt).abs() < 0.5,
                                    RichText::new(format!("±{:.0} dB", opt))
                                        .font(FontId::proportional(10.0)),
                                )
                                .clicked()
                            {
                                state.scale_db = opt;
                            }
                        }
                    });

                draw_right_master_controls(ui, setter, params, makeup_db);
            });
        });
}

fn draw_right_master_controls(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomEqParams,
    makeup_db: f32,
) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(
            RichText::new("PHANTOM DSP")
                .font(FontId::proportional(9.0))
                .strong()
                .color(TEXT_DIMMED),
        );
        ui.separator();

        let mut gain_val = setter.read(&params.output_gain);
        let slider_res = ui.add(
            Slider::new(&mut gain_val, -24.0..=24.0)
                .custom_formatter(|v, _| format!("{:+.1} dB", v))
                .custom_parser(|s| parse_audio_number(s, false)),
        );
        if slider_res.changed() {
            setter.set_once(&params.output_gain, gain_val);
        }
        ui.label(
            RichText::new("OUT")
                .font(FontId::proportional(9.5))
                .strong()
                .color(TEXT_MUTED),
        );

        draw_gain_scale_pill(ui, setter, params);

        let auto_gain = setter.read(&params.auto_gain);
        let ag_text = if auto_gain {
            format!("AUTO {:+.1} dB", makeup_db)
        } else {
            "AUTO".to_string()
        };
        if ui
            .selectable_label(
                auto_gain,
                RichText::new(ag_text)
                    .font(FontId::proportional(9.5))
                    .strong()
                    .color(if auto_gain { ACCENT_GREEN } else { TEXT_MUTED }),
            )
            .on_hover_text("Automatic loudness-matched makeup gain")
            .clicked()
        {
            setter.set_once(&params.auto_gain, !auto_gain);
        }

        let phase_inv = setter.read(&params.phase_invert);
        if draw_icon_button(
            ui,
            Vec2::new(22.0, 19.0),
            phase_inv,
            ACCENT_AMBER,
            ACCENT_AMBER,
            Sense::click(),
            |painter, center, color| draw_phase_icon(painter, center, 4.8, color, 1.45),
        )
        .on_hover_text("Invert output polarity (180° phase flip)")
        .clicked()
        {
            setter.set_once(&params.phase_invert, !phase_inv);
        }
    });
}

fn draw_gain_scale_pill(ui: &mut Ui, setter: &LiveSetter, params: &PhantomEqParams) {
    let cur = setter.read(&params.gain_scale);
    let is_scaled = (cur - 100.0).abs() > 0.5;
    let (rect, res) = ui.allocate_exact_size(Vec2::new(44.0, 19.0), Sense::click_and_drag());
    let p = ui.painter();
    let r = CornerRadius::same(4);
    let active_vis = res.hovered() || res.dragged();
    let fill = if is_scaled {
        with_alpha(ACCENT_CYAN, if active_vis { 42 } else { 26 })
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, if active_vis { 20 } else { 10 })
    };
    let border = if is_scaled || res.dragged() {
        with_alpha(ACCENT_CYAN, 145)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 28)
    };
    p.rect_filled(rect, r, fill);
    p.rect_stroke(rect, r, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{cur:.0}%"),
        FontId::proportional(9.5),
        if is_scaled || res.dragged() {
            ACCENT_CYAN
        } else {
            TEXT_BRIGHT
        },
    );

    if res.double_clicked() {
        setter.set_once(&params.gain_scale, 100.0);
    } else if res.drag_started() {
        setter.begin_set_parameter(&params.gain_scale);
    } else if res.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.25
        } else {
            1.0
        };
        let delta = (-res.drag_delta().y + res.drag_delta().x * 0.5) * 0.6 * fine;
        if delta.abs() > 1e-4 {
            setter.set_parameter(&params.gain_scale, (cur + delta).clamp(0.0, 200.0));
        }
    }
    if res.drag_stopped() {
        setter.end_set_parameter(&params.gain_scale);
    }
    res.on_hover_text(
        "Global Gain Scale (0%-200%): drag to scale band gains & dynamics, double-click resets 100%",
    );
}

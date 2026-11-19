//! Master Output Bottom Bar
//! Controls for Master Gain, Phase Invert, and Anti-Cramping status.

use crate::theme::{BG_CARD, TEXT_DIMMED, TEXT_MUTED};
use nih_plug_egui::egui::{self, Align, Color32, FontId, Layout, Margin, RichText, Slider, Ui};

pub struct MasterBarEvents {
    pub toggle_phase_invert: bool,
    pub set_output_gain: Option<f32>,
}

pub fn draw_master_bar(ui: &mut Ui, phase_inverted: bool, output_gain: f32) -> MasterBarEvents {
    let mut events = MasterBarEvents {
        toggle_phase_invert: false,
        set_output_gain: None,
    };

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Phase Invert (Ø)
                let phase_btn = ui.selectable_label(
                    phase_inverted,
                    RichText::new("Ø PHASE INVERT")
                        .font(FontId::proportional(11.0))
                        .color(if phase_inverted {
                            Color32::from_rgb(245, 158, 11)
                        } else {
                            TEXT_MUTED
                        }),
                );
                if phase_btn.clicked() {
                    events.toggle_phase_invert = true;
                }

                ui.add_space(16.0);

                // Master Output Gain
                let mut gain_val = output_gain;
                ui.label(
                    RichText::new("OUTPUT GAIN:")
                        .font(FontId::proportional(11.0))
                        .strong()
                        .color(TEXT_MUTED),
                );
                let slider_res = ui.add(
                    Slider::new(&mut gain_val, -24.0..=24.0)
                        .suffix(" dB")
                        .custom_formatter(|v, _| format!("{:+.1} dB", v)),
                );
                if slider_res.changed() {
                    events.set_output_gain = Some(gain_val);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("PHANTOM DSP SUITE • OPEN-SOURCE")
                            .font(FontId::proportional(10.0))
                            .color(TEXT_DIMMED),
                    );
                });
            });
        });

    events
}

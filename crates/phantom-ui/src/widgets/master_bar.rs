//! Master Output Bottom Bar
//! Compact, clip-free controls for Phase Invert, Auto-Gain, Output Gain, Character,
//! Display dB Scale, live Peak/LUFS mini-readout, and right-aligned PHANTOM DSP badge.

use crate::icons::draw_phase_icon;
use crate::theme::{
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PURPLE, ACCENT_RED, BG_CARD, TEXT_DIMMED,
    TEXT_MUTED,
};
use crate::views::curve_plot::DB_SCALE_OPTIONS;
use crate::widgets::inspector_controls::{draw_icon_button, parse_audio_number};
use nih_plug_egui::egui::{self, Align, FontId, Layout, Margin, RichText, Sense, Slider, Ui, Vec2};

const CHARACTER_LABELS: [&str; 3] = ["CLEAN", "ANALOG", "TUBE"];

#[derive(Debug, Clone, Copy)]
pub struct MasterBarData {
    pub phase_inverted: bool,
    pub output_gain_db: f32,
    pub auto_gain: bool,
    pub auto_gain_makeup_db: f32,
    /// `0` = Clean, `1` = Subtle Analog, `2` = Warm Tube
    pub character_idx: usize,
    /// Active vertical canvas range in ±dB (`3`, `6`, `12`, `24`, `30`).
    pub scale_db: f32,
    /// Combined stereo peak in dBFS for compact mini-readout.
    pub peak_db: f32,
    /// Momentary loudness in LUFS for compact mini-readout.
    pub momentary_lufs: f32,
}

impl Default for MasterBarData {
    fn default() -> Self {
        Self {
            phase_inverted: false,
            output_gain_db: 0.0,
            auto_gain: false,
            auto_gain_makeup_db: 0.0,
            character_idx: 0,
            scale_db: 12.0,
            peak_db: -96.0,
            momentary_lufs: -70.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MasterBarEvents {
    pub toggle_phase_invert: bool,
    pub set_output_gain: Option<f32>,
    pub toggle_auto_gain: bool,
    pub set_character: Option<usize>,
    pub set_scale_db: Option<f32>,
}

pub fn draw_master_bar(ui: &mut Ui, data: &MasterBarData) -> MasterBarEvents {
    let mut events = MasterBarEvents::default();

    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.spacing_mut().slider_width = 74.0;
            ui.spacing_mut().item_spacing.x = 6.0;

            ui.horizontal(|ui| {
                // Lucide Phase Invert (Ø) icon toggle
                if draw_icon_button(
                    ui,
                    Vec2::new(24.0, 20.0),
                    data.phase_inverted,
                    ACCENT_AMBER,
                    ACCENT_AMBER,
                    Sense::click(),
                    |painter, center, color| draw_phase_icon(painter, center, 5.0, color, 1.5),
                )
                .on_hover_text("Invert output polarity (180° phase flip)")
                .clicked()
                {
                    events.toggle_phase_invert = true;
                }

                // Auto-Gain toggle + live makeup readout
                let ag_text = if data.auto_gain {
                    format!("AUTO {:+.1} dB", data.auto_gain_makeup_db)
                } else {
                    "AUTO GAIN".to_string()
                };
                if ui
                    .selectable_label(
                        data.auto_gain,
                        RichText::new(ag_text)
                            .font(FontId::proportional(10.0))
                            .strong()
                            .color(if data.auto_gain {
                                ACCENT_GREEN
                            } else {
                                TEXT_MUTED
                            }),
                    )
                    .on_hover_text("Automatic loudness-matched makeup gain")
                    .clicked()
                {
                    events.toggle_auto_gain = true;
                }

                ui.separator();

                // Master Output Gain (unit formatted cleanly via custom_formatter)
                let mut gain_val = data.output_gain_db;
                ui.label(
                    RichText::new("OUT:")
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(TEXT_MUTED),
                );
                let slider_res = ui.add(
                    Slider::new(&mut gain_val, -24.0..=24.0)
                        .custom_formatter(|v, _| format!("{:+.1} dB", v))
                        .custom_parser(|s| parse_audio_number(s, false)),
                );
                if slider_res.changed() {
                    events.set_output_gain = Some(gain_val);
                }

                ui.separator();

                // Character Mode selector (Clean / Analog / Tube)
                for (i, &label) in CHARACTER_LABELS.iter().enumerate() {
                    let active = data.character_idx == i;
                    let col = if active {
                        match i {
                            0 => ACCENT_CYAN,
                            1 => ACCENT_AMBER,
                            _ => ACCENT_PURPLE,
                        }
                    } else {
                        TEXT_MUTED
                    };
                    if ui
                        .selectable_label(
                            active,
                            RichText::new(label)
                                .font(FontId::proportional(9.5))
                                .strong()
                                .color(col),
                        )
                        .clicked()
                    {
                        events.set_character = Some(i);
                    }
                }

                ui.separator();

                // Display dB Scale Selector (±3 / ±6 / ±12 / ±24 / ±30 dB)
                egui::ComboBox::from_id_salt("phantom_display_scale")
                    .width(62.0)
                    .selected_text(
                        RichText::new(format!("±{:.0} dB", data.scale_db))
                            .font(FontId::proportional(10.0))
                            .color(ACCENT_CYAN),
                    )
                    .show_ui(ui, |ui| {
                        for &opt in &DB_SCALE_OPTIONS {
                            if ui
                                .selectable_label(
                                    (data.scale_db - opt).abs() < 0.5,
                                    RichText::new(format!("±{:.0} dB", opt))
                                        .font(FontId::proportional(10.0)),
                                )
                                .clicked()
                            {
                                events.set_scale_db = Some(opt);
                            }
                        }
                    });

                // Right-aligned badge + Peak/LUFS mini-readout (padded safely inside right edge)
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("PHANTOM DSP")
                            .font(FontId::proportional(9.5))
                            .strong()
                            .color(TEXT_DIMMED),
                    );
                    ui.separator();

                    let peak_col = if data.peak_db >= -0.3 {
                        ACCENT_RED
                    } else if data.peak_db >= -6.0 {
                        ACCENT_AMBER
                    } else {
                        TEXT_MUTED
                    };
                    let peak_str = if data.peak_db.is_finite() && data.peak_db > -80.0 {
                        format!("{:+.1} dB", data.peak_db)
                    } else {
                        "-inf dB".to_string()
                    };
                    let lufs_str = if data.momentary_lufs.is_finite() && data.momentary_lufs > -69.5
                    {
                        format!("{:.1} LUFS", data.momentary_lufs)
                    } else {
                        "--- LUFS".to_string()
                    };
                    ui.label(
                        RichText::new(format!("{lufs_str}  •  {peak_str}"))
                            .font(FontId::proportional(10.0))
                            .color(peak_col),
                    );
                });
            });
        });

    events
}

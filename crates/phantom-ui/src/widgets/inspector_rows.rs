//! Band Inspector Row Helpers (Band Tabs, Core EQ Sliders, Dynamics Ballistics)
//! Split from `band_inspector.rs` to keep every file cleanly modular and <= 300 lines.

use crate::theme::{band_color, ACCENT_AMBER, ACCENT_CYAN, TEXT_DIMMED, TEXT_MUTED};
use crate::widgets::band_inspector::{BandInspectorData, BandInspectorEvents};
use crate::widgets::inspector_controls::{compact_slider, draw_live_gr_pill};
use nih_plug_egui::egui::{Color32, FontId, RichText, Ui};

pub fn draw_band_selector(ui: &mut Ui, data: &BandInspectorData, events: &mut BandInspectorEvents) {
    let mut active_indices = [0usize; 32];
    let mut count = 0usize;
    let max_n = data.num_bands.min(32);
    for i in 0..max_n {
        if data.used_bands_mask == 0 || (data.used_bands_mask & (1u32 << i)) != 0 {
            active_indices[count] = i;
            count += 1;
        }
    }

    if count <= 10 {
        for &i in &active_indices[..count] {
            let is_active = data.selected_idx == i;
            let is_tab_solo = data.soloed_band == Some(i);
            let label = if is_tab_solo {
                format!("{}•", i + 1)
            } else {
                format!("{}", i + 1)
            };
            let col = if is_tab_solo {
                ACCENT_AMBER
            } else if is_active {
                Color32::WHITE
            } else {
                band_color(i)
            };
            if ui
                .selectable_label(
                    is_active,
                    RichText::new(label)
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(col),
                )
                .clicked()
            {
                events.select_band = Some(i);
            }
        }
    } else {
        let pos = active_indices[..count]
            .iter()
            .position(|&x| x == data.selected_idx)
            .unwrap_or(0);
        if ui
            .small_button("‹")
            .on_hover_text("Previous active band")
            .clicked()
        {
            events.select_band = Some(active_indices[(pos + count - 1) % count]);
        }
        ui.label(
            RichText::new(format!("B{}/{}", data.selected_idx + 1, count))
                .font(FontId::proportional(10.5))
                .strong()
                .color(band_color(data.selected_idx)),
        );
        if ui
            .small_button("›")
            .on_hover_text("Next active band")
            .clicked()
        {
            events.select_band = Some(active_indices[(pos + 1) % count]);
        }
    }
}

pub fn draw_core_sliders_row(
    ui: &mut Ui,
    data: &BandInspectorData,
    events: &mut BandInspectorEvents,
) {
    ui.horizontal(|ui| {
        let mut freq = data.freq;
        if compact_slider(
            ui,
            "Freq:",
            &mut freq,
            10.0..=30000.0,
            true,
            true,
            |v, _| {
                if v >= 1000.0 {
                    format!("{:.2} kHz", v / 1000.0)
                } else {
                    format!("{:.1} Hz", v)
                }
            },
        )
        .changed()
        {
            events.set_freq = Some(freq);
        }

        let mut gain = data.gain;
        if compact_slider(
            ui,
            "Gain:",
            &mut gain,
            -30.0..=30.0,
            false,
            false,
            |v, _| format!("{:+.1} dB", v),
        )
        .changed()
        {
            events.set_gain = Some(gain);
        }

        let mut q = data.q;
        if compact_slider(ui, "Q:", &mut q, 0.1..=30.0, true, false, |v, _| {
            format!("{:.2}", v)
        })
        .changed()
        {
            events.set_q = Some(q);
        }

        ui.separator();

        let mut dyn_range = data.dyn_range;
        if compact_slider(
            ui,
            "Dyn:",
            &mut dyn_range,
            -24.0..=24.0,
            false,
            false,
            |v, _| {
                if v.abs() < 0.1 {
                    "OFF".to_string()
                } else {
                    format!("{:+.1} dB", v)
                }
            },
        )
        .changed()
        {
            events.set_dyn_range = Some(dyn_range);
        }

        ui.add_enabled_ui(!data.dyn_auto_thresh, |ui| {
            let mut thresh = data.dyn_threshold;
            if compact_slider(
                ui,
                "Thresh:",
                &mut thresh,
                -60.0..=0.0,
                false,
                false,
                |v, _| format!("{:.1} dB", v),
            )
            .changed()
            {
                events.set_dyn_threshold = Some(thresh);
            }
        });
    });
}

pub fn draw_dynamics_row(ui: &mut Ui, data: &BandInspectorData, events: &mut BandInspectorEvents) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("DYN:")
                .font(FontId::proportional(10.0))
                .strong()
                .color(TEXT_DIMMED),
        );
        if ui
            .selectable_label(
                data.dyn_auto_thresh,
                RichText::new("AUTO")
                    .font(FontId::proportional(10.0))
                    .strong()
                    .color(if data.dyn_auto_thresh {
                        ACCENT_CYAN
                    } else {
                        TEXT_MUTED
                    }),
            )
            .on_hover_text("Adaptive program-dependent automatic dynamic threshold")
            .clicked()
        {
            events.toggle_dyn_auto_thresh = true;
        }
        if ui
            .selectable_label(
                data.dyn_ext_sc,
                RichText::new("EXT SC")
                    .font(FontId::proportional(10.0))
                    .strong()
                    .color(if data.dyn_ext_sc {
                        ACCENT_AMBER
                    } else {
                        TEXT_MUTED
                    }),
            )
            .on_hover_text("Trigger dynamic EQ from external sidechain input")
            .clicked()
        {
            events.toggle_dyn_ext_sc = true;
        }

        ui.separator();

        let mut atk = data.dyn_attack_ms;
        if compact_slider(ui, "Atk:", &mut atk, 0.5..=250.0, true, false, |v, _| {
            format!("{:.1} ms", v)
        })
        .changed()
        {
            events.set_dyn_attack_ms = Some(atk);
        }

        let mut rel = data.dyn_release_ms;
        if compact_slider(ui, "Rel:", &mut rel, 10.0..=2000.0, true, false, |v, _| {
            format!("{:.0} ms", v)
        })
        .changed()
        {
            events.set_dyn_release_ms = Some(rel);
        }

        ui.separator();
        draw_live_gr_pill(ui, data.live_gr_db);
    });
}

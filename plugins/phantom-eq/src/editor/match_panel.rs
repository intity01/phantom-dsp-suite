//! Pro-Q 4 Reference Spectrum EQ Match Workflow Bar
//! Allows capturing or recording a Reference Spectrum (or snapshotting Pre-EQ), recording the
//! Current Track spectrum, adjusting Band Count (1..12) and Match Amount (10..100%), and applying
//! the fitted parametric curve directly into unused EQ band slots via `ParamSetter`.

use super::EditorState;
use crate::params::{
    ChannelPlacementParam, FilterSlopeParam, FilterTypeParam, PhantomEqParams, NUM_BANDS,
};
use crate::state::EqPresetSnapshot;
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, FontId, Margin, RichText, Ui};
use phantom_dsp::FilterType;
use phantom_ui::{
    ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_RED, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED,
    TEXT_MUTED,
};

pub fn draw_eq_match_bar(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    egui::Frame::NONE
        .fill(BG_CARD)
        .inner_margin(Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("EQ MATCH")
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(ACCENT_AMBER),
                );
                ui.separator();

                // 1. `[Record Ref]` toggle
                let ref_frames = state.eq_match_engine.reference_frame_count();
                let ref_lbl = if state.match_rec_ref {
                    format!("REC REF ({ref_frames})")
                } else if ref_frames > 0 {
                    format!("Record Ref ({ref_frames})")
                } else {
                    String::from("Record Ref")
                };
                if ui
                    .selectable_label(
                        state.match_rec_ref,
                        RichText::new(ref_lbl)
                            .font(FontId::proportional(9.5))
                            .strong()
                            .color(if state.match_rec_ref {
                                ACCENT_RED
                            } else if ref_frames > 0 {
                                ACCENT_AMBER
                            } else {
                                TEXT_BRIGHT
                            }),
                    )
                    .on_hover_text("Accumulate live input spectrum as the EQ Match Reference target")
                    .clicked()
                {
                    state.match_rec_ref = !state.match_rec_ref;
                    if state.match_rec_ref {
                        state.match_rec_track = false;
                    }
                }

                // 2. `[Use Pre as Ref]` instant snapshot button
                if ui
                    .selectable_label(
                        false,
                        RichText::new("Use Pre as Ref")
                            .font(FontId::proportional(9.5))
                            .color(ACCENT_AMBER),
                    )
                    .on_hover_text("Snapshot current Pre-EQ spectrum as the Reference curve")
                    .clicked()
                {
                    state.match_rec_ref = false;
                    state
                        .eq_match_engine
                        .set_reference_snapshot(state.spectrum_processor.pre_bins_db());
                }

                // 3. `[Record Track]` toggle
                let cur_frames = state.eq_match_engine.current_frame_count();
                let trk_lbl = if state.match_rec_track {
                    format!("REC TRACK ({cur_frames})")
                } else if cur_frames > 0 {
                    format!("Record Track ({cur_frames})")
                } else {
                    String::from("Record Track")
                };
                if ui
                    .selectable_label(
                        state.match_rec_track,
                        RichText::new(trk_lbl)
                            .font(FontId::proportional(9.5))
                            .strong()
                            .color(if state.match_rec_track {
                                ACCENT_RED
                            } else if cur_frames > 0 {
                                ACCENT_CYAN
                            } else {
                                TEXT_BRIGHT
                            }),
                    )
                    .on_hover_text("Accumulate Post-EQ track spectrum to compare against Reference")
                    .clicked()
                {
                    state.match_rec_track = !state.match_rec_track;
                    if state.match_rec_track {
                        state.match_rec_ref = false;
                    }
                }

                // 4. `[Clear]` button
                if ui
                    .selectable_label(
                        false,
                        RichText::new("Clear")
                            .font(FontId::proportional(9.5))
                            .color(TEXT_MUTED),
                    )
                    .on_hover_text("Clear recorded Reference and Track spectrum profiles")
                    .clicked()
                {
                    state.match_rec_ref = false;
                    state.match_rec_track = false;
                    state.eq_match_engine.clear_reference();
                    state.eq_match_engine.clear_current();
                }

                ui.separator();

                // 5. `Bands (1..12)` slider
                ui.label(
                    RichText::new("Bands:")
                        .font(FontId::proportional(9.5))
                        .color(TEXT_MUTED),
                );
                let mut bands_i32 = state.match_max_bands as i32;
                if ui
                    .add(
                        egui::Slider::new(&mut bands_i32, 1..=12)
                            .show_value(true)
                            .clamping(egui::SliderClamping::Always),
                    )
                    .changed()
                {
                    state.match_max_bands = (bands_i32 as usize).clamp(1, 12);
                }

                // 6. `Amount (10..100%)` slider
                ui.label(
                    RichText::new("Amount:")
                        .font(FontId::proportional(9.5))
                        .color(TEXT_MUTED),
                );
                ui.add(
                    egui::Slider::new(&mut state.match_amount_pct, 10.0..=100.0)
                        .suffix("%")
                        .max_decimals(0)
                        .clamping(egui::SliderClamping::Always),
                );

                ui.separator();

                // 7. `[APPLY MATCH]` button
                let ready = state.eq_match_engine.has_reference();
                if ui
                    .selectable_label(
                        ready,
                        RichText::new("APPLY MATCH")
                            .font(FontId::proportional(10.0))
                            .strong()
                            .color(if ready { ACCENT_GREEN } else { TEXT_DIMMED }),
                    )
                    .on_hover_text(
                        "Fit parametric EQ bands to match Reference spectrum and write into unused slots",
                    )
                    .clicked()
                    && ready
                {
                    state.match_rec_ref = false;
                    state.match_rec_track = false;
                    if !state.eq_match_engine.has_current() {
                        state
                            .eq_match_engine
                            .set_current_snapshot(state.spectrum_processor.post_bins_db());
                    }
                    apply_matched_bands_to_params(setter, params, state);
                }
            });
        });
}

fn apply_matched_bands_to_params(
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
) {
    let (specs, count) = state
        .eq_match_engine
        .compute_matched_bands(state.match_max_bands, state.match_amount_pct);
    if count == 0 {
        return;
    }

    let mut last_created: Option<usize> = None;
    for spec in &specs[..count] {
        let Some(slot_idx) = (0..NUM_BANDS).find(|&i| !params.bands[i].used.value()) else {
            break;
        };
        let shape = match spec.filter_type {
            FilterType::LowShelf => FilterTypeParam::LowShelf,
            FilterType::HighShelf => FilterTypeParam::HighShelf,
            FilterType::LowCut => FilterTypeParam::LowCut,
            FilterType::HighCut => FilterTypeParam::HighCut,
            _ => FilterTypeParam::Bell,
        };
        let band = &params.bands[slot_idx];

        setter.begin_set_parameter(&band.used);
        setter.set_parameter(&band.used, true);
        setter.end_set_parameter(&band.used);

        setter.begin_set_parameter(&band.enabled);
        setter.set_parameter(&band.enabled, true);
        setter.end_set_parameter(&band.enabled);

        setter.begin_set_parameter(&band.filter_type);
        setter.set_parameter(&band.filter_type, shape);
        setter.end_set_parameter(&band.filter_type);

        setter.begin_set_parameter(&band.slope);
        setter.set_parameter(&band.slope, FilterSlopeParam::Db12);
        setter.end_set_parameter(&band.slope);

        setter.begin_set_parameter(&band.placement);
        setter.set_parameter(&band.placement, ChannelPlacementParam::Stereo);
        setter.end_set_parameter(&band.placement);

        setter.begin_set_parameter(&band.frequency);
        setter.set_parameter(&band.frequency, spec.freq_hz.clamp(10.0, 30000.0));
        setter.end_set_parameter(&band.frequency);

        setter.begin_set_parameter(&band.gain);
        setter.set_parameter(&band.gain, spec.gain_db.clamp(-30.0, 30.0));
        setter.end_set_parameter(&band.gain);

        setter.begin_set_parameter(&band.q);
        setter.set_parameter(&band.q, spec.q.clamp(0.1, 30.0));
        setter.end_set_parameter(&band.q);

        setter.begin_set_parameter(&band.dynamic_range);
        setter.set_parameter(&band.dynamic_range, 0.0);
        setter.end_set_parameter(&band.dynamic_range);

        setter.begin_set_parameter(&band.spectral_dynamics);
        setter.set_parameter(&band.spectral_dynamics, false);
        setter.end_set_parameter(&band.spectral_dynamics);

        last_created = Some(slot_idx);
    }

    if let Some(sel) = last_created {
        state.selected_band = Some(sel);
        state
            .history
            .record_if_changed(EqPresetSnapshot::capture_from(params), false);
    }
}

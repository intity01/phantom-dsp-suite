use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, Ui, Vec2};
use phantom_vox_dsp::ConfidenceLevel;

use crate::project::SourceFileStatus;
use crate::state::StudioApp;
use crate::theme::{
    confidence_color, layer_color, pill_badge, toggle_chip, with_alpha, ACCENT_AMBER, ACCENT_CYAN,
    ACCENT_RED, BG_CARD, BORDER_SUBTLE, TEXT_BRIGHT, TEXT_MUTED,
};
use crate::timeline_interact::handle_lane_interaction;
use crate::timeline_paint::{paint_track_lane, LaneView};

const HEADER_W: f32 = 224.0;

pub fn draw_track_row(ui: &mut Ui, app: &mut StudioApp, idx: usize, height: f32, view: &LaneView) {
    let total_w = ui.available_width();
    let lane_w = (total_w - HEADER_W - 6.0).max(120.0);
    let track_id = app.tracks[idx].id;
    let is_selected = app.selected_track_id == Some(track_id);
    let is_guide = app.tracks[idx].is_guide;

    let mut mix_dirty = false;
    let mut make_guide = false;
    let mut remove_me = false;

    ui.horizontal(|ui| {
        let accent = if is_guide {
            ACCENT_CYAN
        } else {
            layer_color(app.tracks[idx].color_idx)
        };
        let border = if is_selected {
            Stroke::new(1.4_f32, accent)
        } else {
            Stroke::new(1.0_f32, BORDER_SUBTLE)
        };

        // Left track header card.
        egui::Frame::NONE
            .fill(if is_selected {
                Color32::from_rgb(26, 30, 42)
            } else {
                BG_CARD
            })
            .stroke(border)
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(HEADER_W - 14.0);
                ui.set_min_height(height - 14.0);
                let tr = &mut app.tracks[idx];
                ui.horizontal(|ui| {
                    let lbl = if tr.is_guide { "GUIDE" } else { "DUB" };
                    pill_badge(ui, lbl, accent, with_alpha(accent, 36));
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new(&tr.name)
                                    .strong()
                                    .size(11.5)
                                    .color(TEXT_BRIGHT),
                            )
                            .truncate()
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        app.selected_track_id = Some(track_id);
                    }
                });

                ui.horizontal(|ui| {
                    if tr.processing {
                        pill_badge(
                            ui,
                            "Aligning...",
                            ACCENT_AMBER,
                            with_alpha(ACCENT_AMBER, 34),
                        );
                    } else if let Some(al) = &tr.alignment {
                        let pct = (al.overall_confidence * 100.0).round() as u32;
                        let lvl = if al.overall_confidence >= 0.72 {
                            ConfidenceLevel::High
                        } else if al.overall_confidence >= 0.45 {
                            ConfidenceLevel::Medium
                        } else {
                            ConfidenceLevel::Low
                        };
                        let c = confidence_color(lvl);
                        pill_badge(ui, &format!("{pct}%"), c, with_alpha(c, 34));
                    }
                    match &tr.file_status {
                        SourceFileStatus::Missing => {
                            pill_badge(ui, "MISSING", ACCENT_RED, with_alpha(ACCENT_RED, 40));
                        }
                        SourceFileStatus::Modified(_) => {
                            pill_badge(ui, "MODIFIED", ACCENT_AMBER, with_alpha(ACCENT_AMBER, 40));
                        }
                        _ => {}
                    }
                });

                ui.horizontal(|ui| {
                    if toggle_chip(ui, "M", tr.muted, ACCENT_RED).clicked() {
                        tr.muted = !tr.muted;
                        mix_dirty = true;
                    }
                    if toggle_chip(ui, "S", tr.solo, ACCENT_AMBER).clicked() {
                        tr.solo = !tr.solo;
                        mix_dirty = true;
                    }
                    if !tr.is_guide
                        && ui
                            .small_button("Ref")
                            .on_hover_text("Set as Guide")
                            .clicked()
                    {
                        make_guide = true;
                    }
                    if ui.small_button("×").on_hover_text("Remove track").clicked() {
                        remove_me = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Vol").size(10.0).color(TEXT_MUTED));
                    if ui
                        .add(egui::Slider::new(&mut tr.gain, 0.0..=1.5).show_value(false))
                        .changed()
                    {
                        mix_dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Pan").size(10.0).color(TEXT_MUTED));
                    if ui
                        .add(egui::Slider::new(&mut tr.pan, -1.0..=1.0).show_value(false))
                        .changed()
                    {
                        mix_dirty = true;
                    }
                });
            });

        // Right interactive waveform lane.
        let (lane_rect, resp) =
            ui.allocate_exact_size(Vec2::new(lane_w, height), Sense::click_and_drag());
        let guide_al = app.tracks.iter().find_map(|t| t.alignment.as_deref());
        let pending_g = app
            .pending_anchor
            .filter(|p| p.track_id == track_id)
            .map(|p| p.guide_sec);
        let protect_prev = app
            .protect_drag
            .filter(|(tid, _, _)| *tid == track_id)
            .map(|(_, a, b)| (a, b));

        paint_track_lane(
            ui.painter(),
            lane_rect,
            &app.tracks[idx],
            guide_al,
            view,
            pending_g,
            protect_prev,
        );

        // Phrase confidence bar hover tooltip.
        if let Some(hover) = resp.hover_pos() {
            if !is_guide && hover.y >= lane_rect.bottom() - 16.0 {
                let sec = view.x_to_sec(lane_rect, hover.x);
                if let Some(al) = &app.tracks[idx].alignment {
                    if let Some(phr) = al
                        .phrases
                        .iter()
                        .find(|p| sec >= p.start_sec && sec <= p.end_sec)
                    {
                        let tip = match &phr.warning {
                            Some(w) => format!(
                                "Phrase {:.2}s–{:.2}s | Confidence {:.0}%\nWarning: {}",
                                phr.start_sec,
                                phr.end_sec,
                                phr.confidence * 100.0,
                                w
                            ),
                            None => format!(
                                "Phrase {:.2}s–{:.2}s | Confidence {:.0}% (Healthy)",
                                phr.start_sec,
                                phr.end_sec,
                                phr.confidence * 100.0
                            ),
                        };
                        egui::show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            egui::Id::new(("conf_tip", track_id)),
                            |ui| {
                                ui.label(tip);
                            },
                        );
                    }
                }
            }
        }

        handle_lane_interaction(ui, app, idx, lane_rect, &resp, view);
    });

    if mix_dirty {
        app.sync_audio_mix();
    }
    if make_guide {
        app.set_track_as_guide(idx);
    } else if remove_me {
        app.remove_track(idx);
    }
}

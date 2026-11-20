//! Multi-track Timeline UI: pinned Guide on top, scrollable Layer tracks below,
//! playhead ruler, zoom/pan, interactive Sync Anchors, and Protected Region drag-painting.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};

use crate::state::StudioApp;
use crate::theme::{ACCENT_CYAN, BG_CARD, BG_HEADER, BORDER_SUBTLE, TEXT_DIMMED, TEXT_MUTED};
use crate::timeline_paint::LaneView;
use crate::timeline_row::draw_track_row;

const HEADER_W: f32 = 224.0;
const RULER_H: f32 = 26.0;
const GUIDE_H: f32 = 104.0;
const LAYER_H: f32 = 116.0;

pub fn draw_timeline(ui: &mut Ui, app: &mut StudioApp) {
    if app.tracks.is_empty() {
        draw_empty_timeline(ui, app);
        return;
    }

    // Handle Ctrl+Wheel zoom and horizontal scroll over timeline.
    let (scroll_delta, ctrl_down) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.ctrl));
    if ui.rect_contains_pointer(ui.max_rect()) && scroll_delta.y.abs() > 0.1 {
        if ctrl_down {
            let factor = (1.0 + scroll_delta.y * 0.0025).clamp(0.75, 1.35);
            app.zoom_px_per_sec = (app.zoom_px_per_sec * factor).clamp(40.0, 900.0);
        } else if scroll_delta.x.abs() > 0.1 {
            app.scroll_sec = (app.scroll_sec - scroll_delta.x / app.zoom_px_per_sec).max(0.0);
        }
    }

    let view = LaneView {
        scroll_sec: app.scroll_sec,
        px_per_sec: app.zoom_px_per_sec,
        audition_b: app.audio.audition_b(),
        show_segmentation: app.show_segmentation,
        show_pitch: app.show_pitch,
        playhead_sec: app.audio.playhead_sec(),
        loop_enabled: app.audio.loop_enabled(),
        loop_start_sec: app.loop_start_sec,
        loop_end_sec: app.loop_end_sec,
    };

    draw_ruler_row(ui, app, &view);
    ui.add_space(4.0);

    // Pinned Guide track on top in Cyan.
    if let Some(g_idx) = app.guide_index() {
        draw_track_row(ui, app, g_idx, GUIDE_H, &view);
        ui.add_space(5.0);
    }

    // Scrollable Layer tracks below.
    egui::ScrollArea::vertical()
        .id_salt("vox_timeline_layers")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let indices: Vec<usize> = (0..app.tracks.len())
                .filter(|&i| !app.tracks[i].is_guide)
                .collect();
            for idx in indices {
                if idx < app.tracks.len() {
                    draw_track_row(ui, app, idx, LAYER_H, &view);
                    ui.add_space(4.0);
                }
            }
        });
}

fn draw_empty_timeline(ui: &mut Ui, app: &mut StudioApp) {
    ui.centered_and_justified(|ui| {
        egui::Frame::NONE
            .fill(BG_CARD)
            .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(28))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("PHANTOM VOX STUDIO")
                            .size(18.0)
                            .strong()
                            .color(ACCENT_CYAN),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "100% Offline Multi-Track Vocal Alignment, Harmony-Safe Pitch Match & Doubler",
                        )
                        .color(TEXT_MUTED),
                    );
                    ui.add_space(14.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Load Demo Vocal Stack (Guide + L/R + 3rd Harmony)")
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(ACCENT_CYAN)
                            .corner_radius(CornerRadius::same(8))
                            .min_size(Vec2::new(320.0, 34.0)),
                        )
                        .clicked()
                    {
                        app.load_demo_stack();
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Or drag & drop .wav / .aif / .pvox files directly onto this window.",
                        )
                        .size(11.5)
                        .color(TEXT_DIMMED),
                    );
                });
            });
    });
}

fn draw_ruler_row(ui: &mut Ui, app: &mut StudioApp, view: &LaneView) {
    let total_w = ui.available_width();
    let lane_w = (total_w - HEADER_W - 6.0).max(120.0);
    ui.horizontal(|ui| {
        let (h_rect, _) = ui.allocate_exact_size(Vec2::new(HEADER_W, RULER_H), Sense::hover());
        ui.painter()
            .rect_filled(h_rect, CornerRadius::same(4), BG_HEADER);
        ui.painter().text(
            Pos2::new(h_rect.left() + 8.0, h_rect.center().y),
            Align2::LEFT_CENTER,
            format!("TRACKS ({})  •  {} Hz", app.tracks.len(), app.sample_rate),
            FontId::proportional(11.0),
            TEXT_MUTED,
        );

        let (r_rect, resp) =
            ui.allocate_exact_size(Vec2::new(lane_w, RULER_H), Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(r_rect);
        p.rect_filled(r_rect, CornerRadius::same(4), BG_HEADER);

        // Loop bar highlight in ruler.
        if app.audio.loop_enabled() && app.loop_end_sec > app.loop_start_sec + 0.05 {
            let x0 = view
                .sec_to_x(r_rect, app.loop_start_sec)
                .clamp(r_rect.left(), r_rect.right());
            let x1 = view
                .sec_to_x(r_rect, app.loop_end_sec)
                .clamp(r_rect.left(), r_rect.right());
            if x1 > x0 {
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x0, r_rect.top()),
                        Pos2::new(x1, r_rect.top() + 5.0),
                    ),
                    CornerRadius::ZERO,
                    ACCENT_CYAN,
                );
            }
        }

        // Ruler ticks.
        let step = if view.px_per_sec >= 240.0 {
            0.25
        } else if view.px_per_sec >= 100.0 {
            0.5
        } else {
            1.0
        };
        let mut t = (view.scroll_sec / step).floor().max(0.0) * step;
        let end_t = view.x_to_sec(r_rect, r_rect.right()) + step;
        while t <= end_t {
            let x = view.sec_to_x(r_rect, t);
            p.line_segment(
                [
                    Pos2::new(x, r_rect.bottom() - 7.0),
                    Pos2::new(x, r_rect.bottom()),
                ],
                Stroke::new(1.0_f32, TEXT_DIMMED),
            );
            p.text(
                Pos2::new(x + 3.0, r_rect.center().y + 1.0),
                Align2::LEFT_CENTER,
                format!("{t:.1}s"),
                FontId::proportional(10.0),
                TEXT_MUTED,
            );
            t += step;
        }

        // Playhead triangle.
        let ph_x = view.sec_to_x(r_rect, view.playhead_sec);
        if ph_x >= r_rect.left() && ph_x <= r_rect.right() {
            p.circle_filled(Pos2::new(ph_x, r_rect.bottom() - 4.0), 4.0, ACCENT_CYAN);
        }

        // Click/drag to scrub; Shift+drag to set loop range.
        if let Some(pos) = resp.interact_pointer_pos() {
            let sec = view.x_to_sec(r_rect, pos.x);
            let shift = ui.input(|i| i.modifiers.shift);
            if shift && resp.drag_started() {
                app.loop_start_sec = sec;
                app.loop_end_sec = sec;
            } else if shift && resp.dragged() {
                app.loop_end_sec = sec.max(app.loop_start_sec + 0.1);
                app.audio
                    .set_loop_range_sec(app.loop_start_sec, app.loop_end_sec);
            } else if resp.clicked() || resp.dragged() {
                app.audio.seek_sec(sec);
            }
        }
    });
}

use eframe::egui::{self, Rect, Ui};
use phantom_vox_dsp::{ProtectedRegion, SyncAnchor};

use crate::state::{PendingAnchor, StudioApp, TimelineTool};
use crate::timeline_paint::LaneView;

pub fn handle_lane_interaction(
    _ui: &mut Ui,
    app: &mut StudioApp,
    idx: usize,
    rect: Rect,
    resp: &egui::Response,
    view: &LaneView,
) {
    let track_id = app.tracks[idx].id;
    let is_guide = app.tracks[idx].is_guide;

    if resp.clicked() || resp.drag_started() {
        app.selected_track_id = Some(track_id);
    }

    // Right-click deletes nearby Anchor or ProtectedRegion.
    if resp.secondary_clicked() && !is_guide {
        if let Some(pos) = resp.interact_pointer_pos() {
            let sec = view.x_to_sec(rect, pos.x);
            let mut mutated = false;
            if let Some(a_idx) = app.tracks[idx]
                .anchors
                .iter()
                .position(|a| (a.dub_sec - sec).abs() < 0.12 || (a.guide_sec - sec).abs() < 0.12)
            {
                app.push_undo();
                app.tracks[idx].anchors.remove(a_idx);
                mutated = true;
            } else if let Some(p_idx) = app.tracks[idx]
                .protected
                .iter()
                .position(|r| sec >= r.start_sec && sec <= r.end_sec)
            {
                app.push_undo();
                app.tracks[idx].protected.remove(p_idx);
                mutated = true;
            }
            if mutated {
                app.queue_track_analysis(idx);
            }
        }
        return;
    }

    match app.tool {
        TimelineTool::Scrub => {
            if (resp.clicked() || resp.dragged()) && resp.interact_pointer_pos().is_some() {
                let sec = view.x_to_sec(rect, resp.interact_pointer_pos().unwrap().x);
                app.audio.seek_sec(sec);
            }
        }
        TimelineTool::Anchor => {
            if is_guide {
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let g_sec = view.x_to_sec(rect, pos.x);
                        let target_id = app
                            .selected_track_id
                            .filter(|&id| id != track_id)
                            .or_else(|| app.tracks.iter().find(|t| !t.is_guide).map(|t| t.id));
                        if let Some(tid) = target_id {
                            app.pending_anchor = Some(PendingAnchor {
                                track_id: tid,
                                guide_sec: g_sec,
                            });
                            app.status_line = format!(
                                "Anchor Guide={g_sec:.2}s — click Dub lane to pin matching point."
                            );
                        }
                    }
                }
                return;
            }
            // Drag existing anchor dub_sec, or 2-click pin pair.
            if resp.drag_started() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let sec = view.x_to_sec(rect, pos.x);
                    if let Some(ai) = app.tracks[idx]
                        .anchors
                        .iter()
                        .position(|a| (a.dub_sec - sec).abs() < 0.1)
                    {
                        app.push_undo();
                        app.dragging_anchor = Some((track_id, ai));
                    }
                }
            } else if resp.dragged() {
                if let Some((tid, ai)) = app.dragging_anchor {
                    if tid == track_id {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            if let Some(anc) = app.tracks[idx].anchors.get_mut(ai) {
                                anc.dub_sec = view.x_to_sec(rect, pos.x);
                            }
                        }
                    }
                }
            } else if resp.drag_stopped() {
                if let Some((tid, _)) = app.dragging_anchor.take() {
                    if tid == track_id {
                        app.queue_track_analysis(idx);
                    }
                }
            } else if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let sec = view.x_to_sec(rect, pos.x);
                    if let Some(pend) = app.pending_anchor.take() {
                        if pend.track_id == track_id {
                            app.push_undo();
                            app.tracks[idx].anchors.push(SyncAnchor {
                                guide_sec: pend.guide_sec,
                                dub_sec: sec,
                            });
                            app.queue_track_analysis(idx);
                            return;
                        }
                    }
                    app.pending_anchor = Some(PendingAnchor {
                        track_id,
                        guide_sec: sec,
                    });
                    app.status_line =
                        format!("Anchor Guide={sec:.2}s — click Dub point to pin pair.");
                }
            }
        }
        TimelineTool::Protect => {
            if is_guide {
                return;
            }
            if resp.drag_started() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let s = view.x_to_sec(rect, pos.x);
                    app.protect_drag = Some((track_id, s, s));
                }
            } else if resp.dragged() {
                if let Some((tid, _start, ref mut end)) = app.protect_drag {
                    if tid == track_id {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            *end = view.x_to_sec(rect, pos.x);
                        }
                    }
                }
            } else if resp.drag_stopped() {
                if let Some((tid, a, b)) = app.protect_drag.take() {
                    if tid == track_id && (b - a).abs() >= 0.05 {
                        app.push_undo();
                        app.tracks[idx].protected.push(ProtectedRegion {
                            start_sec: a.min(b),
                            end_sec: a.max(b),
                        });
                        app.queue_track_analysis(idx);
                    }
                }
            }
        }
    }
}

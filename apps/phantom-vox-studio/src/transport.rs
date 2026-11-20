//! Top Transport & Studio Action Bar: Play/Stop, instant A/B toggle, Mono Check, Loop,
//! Tools (Scrub / Anchor / Protect), Overlays, Undo/Redo, Demo, Import, Batch Process, Export, .pvox.

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Ui, Vec2};

use crate::file_dialog::{
    pick_export_directory, pick_open_audio_multi, pick_open_audio_single, pick_open_project,
    pick_save_project, PathModal, PathModalKind,
};
use crate::project::ExportFormat;
use crate::state::{StudioApp, TimelineTool};
use crate::theme::{
    pill_badge, toggle_chip, with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, ACCENT_PINK,
    ACCENT_PURPLE, BORDER_SUBTLE, TEXT_BRIGHT, TEXT_MUTED,
};

pub fn draw_transport_bar(ui: &mut Ui, app: &mut StudioApp) {
    // Row 1: Brand + Transport + Instant A/B + Mono Check + Loop + Undo/Redo + File Actions.
    ui.horizontal_wrapped(|ui| {
        pill_badge(
            ui,
            "PHANTOM VOX STUDIO",
            ACCENT_CYAN,
            with_alpha(ACCENT_CYAN, 32),
        );
        ui.separator();

        let playing = app.audio.is_playing();
        let play_lbl = if playing { "⏸ Pause" } else { "▶ Play" };
        let play_accent = if playing { ACCENT_GREEN } else { ACCENT_CYAN };
        if toggle_chip(ui, play_lbl, playing, play_accent)
            .on_hover_text("Play / Pause (Space)")
            .clicked()
        {
            app.audio.set_playing(!playing);
        }
        if ui.button("■ Stop").clicked() {
            app.audio.stop();
        }

        let pos = app.audio.playhead_sec();
        let dur = app.total_duration_sec();
        ui.label(
            RichText::new(format!("{pos:05.2}s / {dur:05.2}s"))
                .monospace()
                .strong()
                .size(12.0)
                .color(TEXT_BRIGHT),
        );

        ui.separator();
        let is_b = app.audio.audition_b();
        if toggle_chip(ui, "A (Orig)", !is_b, ACCENT_AMBER)
            .on_hover_text("Audition Original Dubs (B key toggles A/B)")
            .clicked()
        {
            app.audio.set_audition_b(false);
        }
        if toggle_chip(ui, "B (Aligned)", is_b, ACCENT_GREEN)
            .on_hover_text("Audition Aligned & Pitch-Matched Dubs")
            .clicked()
        {
            app.audio.set_audition_b(true);
        }

        let mono = app.audio.mono_check();
        if toggle_chip(ui, "Mono", mono, ACCENT_PURPLE)
            .on_hover_text("Fold stereo mix to mono to check phase cancellation (M)")
            .clicked()
        {
            app.audio.set_mono_check(!mono);
        }

        let lp = app.audio.loop_enabled();
        if toggle_chip(ui, "Loop", lp, ACCENT_CYAN)
            .on_hover_text("Toggle loop playback (Shift+drag timeline ruler to set range)")
            .clicked()
        {
            app.audio.set_loop_enabled(!lp);
        }

        ui.separator();
        if ui
            .add_enabled(app.history.can_undo(), egui::Button::new("↶ Undo"))
            .clicked()
        {
            app.undo();
        }
        if ui
            .add_enabled(app.history.can_redo(), egui::Button::new("↷ Redo"))
            .clicked()
        {
            app.redo();
        }
    });

    ui.add_space(4.0);

    // Row 2: Tools, Overlays, Zoom, and Workflow buttons.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Tool:").size(11.0).color(TEXT_MUTED));
        if toggle_chip(ui, "Scrub", app.tool == TimelineTool::Scrub, ACCENT_CYAN).clicked() {
            app.tool = TimelineTool::Scrub;
            app.pending_anchor = None;
        }
        if toggle_chip(
            ui,
            "Pin Anchor",
            app.tool == TimelineTool::Anchor,
            ACCENT_AMBER,
        )
        .on_hover_text(
            "Click Guide then Dub to pin Sync Anchor; drag to adjust; right-click to delete",
        )
        .clicked()
        {
            app.tool = TimelineTool::Anchor;
        }
        if toggle_chip(
            ui,
            "Protect Region",
            app.tool == TimelineTool::Protect,
            ACCENT_PURPLE,
        )
        .on_hover_text(
            "Drag-paint locked purple regions untouched by alignment; right-click to remove",
        )
        .clicked()
        {
            app.tool = TimelineTool::Protect;
            app.pending_anchor = None;
        }

        ui.separator();
        if toggle_chip(ui, "Seg Badges", app.show_segmentation, ACCENT_AMBER).clicked() {
            app.show_segmentation = !app.show_segmentation;
        }
        if toggle_chip(ui, "YIN Pitch", app.show_pitch, ACCENT_PINK).clicked() {
            app.show_pitch = !app.show_pitch;
        }

        if ui.small_button("−").on_hover_text("Zoom Out").clicked() {
            app.zoom_px_per_sec = (app.zoom_px_per_sec * 0.78).clamp(40.0, 900.0);
        }
        if ui.small_button("+").on_hover_text("Zoom In").clicked() {
            app.zoom_px_per_sec = (app.zoom_px_per_sec * 1.28).clamp(40.0, 900.0);
        }
        if ui
            .small_button("Fit")
            .on_hover_text("Fit timeline")
            .clicked()
        {
            let dur = app.total_duration_sec().max(1.0);
            app.zoom_px_per_sec = (720.0 / dur).clamp(40.0, 400.0);
            app.scroll_sec = 0.0;
        }

        ui.separator();
        if ui
            .add(
                egui::Button::new(RichText::new("Load Demo").strong().color(Color32::BLACK))
                    .fill(ACCENT_CYAN)
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(0.0, 24.0)),
            )
            .clicked()
        {
            app.load_demo_stack();
        }
        if ui.button("Import Guide").clicked() {
            if let Some(p) = pick_open_audio_single() {
                app.import_guide_file(p);
            } else if cfg!(not(windows)) {
                app.path_modal = Some(PathModal::new(PathModalKind::ImportGuide, ""));
            }
        }
        if ui.button("Add Dub(s)").clicked() {
            if let Some(ps) = pick_open_audio_multi() {
                app.add_dub_files(ps);
            } else if cfg!(not(windows)) {
                app.path_modal = Some(PathModal::new(PathModalKind::AddDub, ""));
            }
        }
        if ui
            .add(
                egui::Button::new(
                    RichText::new("Batch Process All")
                        .strong()
                        .color(ACCENT_GREEN),
                )
                .fill(with_alpha(ACCENT_GREEN, 30))
                .stroke(Stroke::new(1.0_f32, ACCENT_GREEN))
                .corner_radius(CornerRadius::same(6))
                .min_size(Vec2::new(0.0, 24.0)),
            )
            .clicked()
        {
            app.batch_process_all();
        }

        let fmt_lbl = match app.export_format {
            ExportFormat::Wav24 => "24-bit",
            ExportFormat::Wav32F => "32f",
        };
        if ui
            .small_button(fmt_lbl)
            .on_hover_text("Toggle WAV export format (24-bit PCM / 32-bit Float)")
            .clicked()
        {
            app.export_format = match app.export_format {
                ExportFormat::Wav24 => ExportFormat::Wav32F,
                ExportFormat::Wav32F => ExportFormat::Wav24,
            };
        }
        if ui
            .add(
                egui::Button::new(RichText::new("Export Stems").strong().color(TEXT_BRIGHT))
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(0.0, 24.0)),
            )
            .clicked()
        {
            if let Some(dir) = pick_export_directory() {
                app.export_stems_to(&dir);
            } else if cfg!(not(windows)) {
                app.path_modal = Some(PathModal::new(PathModalKind::ExportDir, "./exported_stems"));
            }
        }

        if ui.button("Open .pvox").clicked() {
            if let Some(p) = pick_open_project() {
                app.open_project_from(p);
            } else if cfg!(not(windows)) {
                app.path_modal = Some(PathModal::new(PathModalKind::OpenProject, "session.pvox"));
            }
        }
        if ui.button("Save .pvox").clicked() {
            if let Some(p) = pick_save_project() {
                app.save_project_to(p);
            } else if cfg!(not(windows)) {
                let def = app
                    .project_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "session.pvox".to_owned());
                app.path_modal = Some(PathModal::new(PathModalKind::SaveProject, &def));
            }
        }
    });
}

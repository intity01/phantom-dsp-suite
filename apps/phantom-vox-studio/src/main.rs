//! Phantom Vox Studio — Standalone 100% Offline Multi-Track Vocal Alignment Timeline App.
//! Zero telemetry, zero external runtime dependencies, Dark Obsidian native desktop GUI.

mod actions;
mod actions_demo;
mod actions_project;
mod audio_out;
#[cfg(windows)]
mod audio_out_win32;
mod export;
mod file_dialog;
#[cfg(windows)]
mod file_dialog_win32;
mod inspector;
mod phrase_report;
mod playback_mix;
mod project;
mod sha256;
mod state;
mod state_jobs;
mod theme;
mod timeline;
mod timeline_interact;
mod timeline_overlays;
mod timeline_paint;
mod timeline_row;
mod track;
mod transport;
mod worker;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::{self, Margin, RichText, Stroke};

use file_dialog::{draw_path_modal, parse_cli_args, CliArgs, PathModalAction, PathModalKind};
use inspector::draw_inspector;
use state::StudioApp;
use theme::{
    apply_obsidian_theme, ACCENT_AMBER, ACCENT_CYAN, ACCENT_PINK, ACCENT_PURPLE, BG_CARD, BG_PANEL,
    BORDER_SUBTLE, TEXT_DIMMED, TEXT_MUTED,
};
use timeline::draw_timeline;
use transport::draw_transport_bar;

struct VoxStudioEframe {
    app: StudioApp,
}

impl VoxStudioEframe {
    fn new(cc: &eframe::CreationContext<'_>, cli: CliArgs) -> Self {
        apply_obsidian_theme(&cc.egui_ctx);
        let mut app = StudioApp::new();
        if let Some(proj) = cli.project {
            app.open_project_from(proj);
        } else {
            if let Some(guide) = cli.guide {
                app.import_guide_file(guide);
            }
            if !cli.dubs.is_empty() {
                app.add_dub_files(cli.dubs);
            }
            if cli.demo {
                app.load_demo_stack();
            }
        }
        Self { app }
    }
}

impl eframe::App for VoxStudioEframe {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.audio.tick_ui();
        let worker_updated = self.app.poll_worker();
        self.app.maybe_autosave();

        if self.app.audio.is_playing() || self.app.any_processing() || worker_updated {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // Drag-and-drop WAV / AIFF / .pvox files directly onto the studio window.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.app.handle_dropped_paths(dropped);
        }

        // Global keyboard shortcuts (guarded when typing in text inputs).
        if !ctx.wants_keyboard_input() && self.app.path_modal.is_none() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    let next = !self.app.audio.is_playing();
                    self.app.audio.set_playing(next);
                }
                if i.key_pressed(egui::Key::B) {
                    let next = !self.app.audio.audition_b();
                    self.app.audio.set_audition_b(next);
                }
                if i.key_pressed(egui::Key::M) {
                    let next = !self.app.audio.mono_check();
                    self.app.audio.set_mono_check(next);
                }
                if i.modifiers.ctrl && i.key_pressed(egui::Key::Z) {
                    self.app.undo();
                }
                if i.modifiers.ctrl && i.key_pressed(egui::Key::Y) {
                    self.app.redo();
                }
            });
        }

        // Top Transport & Action Bar.
        egui::TopBottomPanel::top("vox_transport_panel")
            .frame(
                egui::Frame::NONE
                    .fill(BG_CARD)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(10, 7)),
            )
            .show(ctx, |ui| {
                draw_transport_bar(ui, &mut self.app);
            });

        // Bottom Status & Legend Bar.
        egui::TopBottomPanel::bottom("vox_status_panel")
            .frame(
                egui::Frame::NONE
                    .fill(BG_CARD)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.app.status_line)
                            .size(11.0)
                            .color(TEXT_MUTED),
                    );
                    if !self.app.warnings.is_empty() {
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "⚠ {} warning(s): {}",
                                self.app.warnings.len(),
                                self.app.warnings.last().map(String::as_str).unwrap_or("")
                            ))
                            .size(11.0)
                            .color(ACCENT_AMBER),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("100% Offline").size(10.5).color(TEXT_DIMMED));
                        ui.separator();
                        ui.label(RichText::new("■ Transient").size(10.0).color(ACCENT_PINK));
                        ui.label(RichText::new("■ Breath").size(10.0).color(ACCENT_PURPLE));
                        ui.label(RichText::new("■ Sibilance").size(10.0).color(ACCENT_AMBER));
                        ui.label(RichText::new("■ Voiced").size(10.0).color(ACCENT_CYAN));
                    });
                });
            });

        // Right Inspector Panel.
        egui::SidePanel::right("vox_inspector_panel")
            .resizable(true)
            .default_width(300.0)
            .min_width(250.0)
            .max_width(400.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| {
                draw_inspector(ui, &mut self.app);
            });

        // Center Unified Multi-Track Timeline.
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::same(8)),
            )
            .show(ctx, |ui| {
                draw_timeline(ui, &mut self.app);
            });

        // Optional non-Windows file path prompt modal.
        if let Some(mut modal) = self.app.path_modal.take() {
            match draw_path_modal(ctx, &mut modal) {
                PathModalAction::None => {
                    self.app.path_modal = Some(modal);
                }
                PathModalAction::Cancel => {}
                PathModalAction::Submit(kind, raw) => {
                    if !raw.is_empty() {
                        match kind {
                            PathModalKind::ImportGuide => {
                                self.app.import_guide_file(PathBuf::from(raw));
                            }
                            PathModalKind::AddDub => {
                                let paths: Vec<PathBuf> = raw
                                    .split(',')
                                    .map(|s| PathBuf::from(s.trim()))
                                    .filter(|p| !p.as_os_str().is_empty())
                                    .collect();
                                self.app.add_dub_files(paths);
                            }
                            PathModalKind::OpenProject => {
                                self.app.open_project_from(PathBuf::from(raw));
                            }
                            PathModalKind::SaveProject => {
                                self.app.save_project_to(PathBuf::from(raw));
                            }
                            PathModalKind::ExportDir => {
                                self.app.export_stems_to(&PathBuf::from(raw));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let cli = parse_cli_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phantom Vox Studio — Offline Vocal Alignment Timeline")
            .with_inner_size([1340.0, 820.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };

    if let Err(err) = eframe::run_native(
        "Phantom Vox Studio",
        options,
        Box::new(move |cc| Ok(Box::new(VoxStudioEframe::new(cc, cli)))),
    ) {
        eprintln!("[ERROR] Failed to launch Phantom Vox Studio: {err}");
        std::process::exit(1);
    }
}

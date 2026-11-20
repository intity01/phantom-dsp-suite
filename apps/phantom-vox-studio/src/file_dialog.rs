//! Zero-dependency native Win32 file pickers (`comdlg32.dll` / `shell32.dll`), CLI flags,
//! and clean non-Windows path prompt modal.

use std::path::PathBuf;

use eframe::egui::{self, RichText};

use crate::theme::{ACCENT_CYAN, TEXT_BRIGHT, TEXT_MUTED};

/// Parsed command-line launch arguments (`--guide`, `--dub`, `--project`, `--demo`).
#[derive(Debug, Default, Clone)]
pub struct CliArgs {
    pub guide: Option<PathBuf>,
    pub dubs: Vec<PathBuf>,
    pub project: Option<PathBuf>,
    pub demo: bool,
}

pub fn parse_cli_args() -> CliArgs {
    let mut out = CliArgs::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--guide" => {
                if let Some(p) = it.next() {
                    out.guide = Some(PathBuf::from(p));
                }
            }
            "--dub" => {
                if let Some(p) = it.next() {
                    out.dubs.push(PathBuf::from(p));
                }
            }
            "--project" => {
                if let Some(p) = it.next() {
                    out.project = Some(PathBuf::from(p));
                }
            }
            "--demo" => out.demo = true,
            other => {
                let lower = other.to_ascii_lowercase();
                if lower.ends_with(".pvox") {
                    out.project = Some(PathBuf::from(other));
                } else if lower.ends_with(".wav")
                    || lower.ends_with(".aif")
                    || lower.ends_with(".aiff")
                {
                    if out.guide.is_none() {
                        out.guide = Some(PathBuf::from(other));
                    } else {
                        out.dubs.push(PathBuf::from(other));
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathModalKind {
    ImportGuide,
    AddDub,
    OpenProject,
    SaveProject,
    ExportDir,
}

#[derive(Debug, Clone)]
pub struct PathModal {
    pub kind: PathModalKind,
    pub input: String,
}

impl PathModal {
    pub fn new(kind: PathModalKind, initial: &str) -> Self {
        Self {
            kind,
            input: initial.to_owned(),
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            PathModalKind::ImportGuide => "Import Guide Audio (.wav / .aif)",
            PathModalKind::AddDub => "Add Dub Track(s) (comma-separated paths)",
            PathModalKind::OpenProject => "Open Project (.pvox)",
            PathModalKind::SaveProject => "Save Project (.pvox)",
            PathModalKind::ExportDir => "Export Aligned Stems Folder",
        }
    }
}

pub enum PathModalAction {
    None,
    Cancel,
    Submit(PathModalKind, String),
}

pub fn draw_path_modal(ctx: &egui::Context, modal: &mut PathModal) -> PathModalAction {
    let mut action = PathModalAction::None;
    egui::Window::new(modal.title())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([500.0, 120.0])
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Enter path (or drag & drop files directly onto the timeline):")
                    .color(TEXT_MUTED),
            );
            ui.add_space(4.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut modal.input)
                    .desired_width(470.0)
                    .hint_text("/path/to/vocal.wav"),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                action = PathModalAction::Submit(modal.kind, modal.input.trim().to_owned());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Confirm").strong().color(ACCENT_CYAN))
                    .clicked()
                {
                    action = PathModalAction::Submit(modal.kind, modal.input.trim().to_owned());
                }
                if ui
                    .button(RichText::new("Cancel").color(TEXT_BRIGHT))
                    .clicked()
                {
                    action = PathModalAction::Cancel;
                }
            });
        });
    action
}

pub fn pick_open_audio_single() -> Option<PathBuf> {
    win_open_files(
        "Select Guide Audio",
        "Audio Files (*.wav;*.aif;*.aiff)\0*.wav;*.aif;*.aiff\0All Files (*.*)\0*.*\0\0",
        false,
    )
    .and_then(|mut v| v.pop())
}

pub fn pick_open_audio_multi() -> Option<Vec<PathBuf>> {
    win_open_files(
        "Select Dub Track(s)",
        "Audio Files (*.wav;*.aif;*.aiff)\0*.wav;*.aif;*.aiff\0All Files (*.*)\0*.*\0\0",
        true,
    )
}

pub fn pick_open_project() -> Option<PathBuf> {
    win_open_files(
        "Open Phantom Vox Project",
        "Phantom Vox Project (*.pvox)\0*.pvox\0All Files (*.*)\0*.*\0\0",
        false,
    )
    .and_then(|mut v| v.pop())
}

pub fn pick_save_project() -> Option<PathBuf> {
    win_save_file(
        "Save Phantom Vox Project",
        "Phantom Vox Project (*.pvox)\0*.pvox\0All Files (*.*)\0*.*\0\0",
        "pvox",
    )
}

pub fn pick_export_directory() -> Option<PathBuf> {
    win_browse_folder("Choose Output Folder for Aligned WAV Stems")
}

#[cfg(not(windows))]
fn win_open_files(_title: &str, _filter: &str, _multi: bool) -> Option<Vec<PathBuf>> {
    None
}

#[cfg(not(windows))]
fn win_save_file(_title: &str, _filter: &str, _def_ext: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(windows))]
fn win_browse_folder(_title: &str) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn win_open_files(title: &str, filter: &str, multi: bool) -> Option<Vec<PathBuf>> {
    crate::file_dialog_win32::open_files(title, filter, multi)
}

#[cfg(windows)]
fn win_save_file(title: &str, filter: &str, def_ext: &str) -> Option<PathBuf> {
    crate::file_dialog_win32::save_file(title, filter, def_ext)
}

#[cfg(windows)]
fn win_browse_folder(title: &str) -> Option<PathBuf> {
    crate::file_dialog_win32::browse_folder(title)
}

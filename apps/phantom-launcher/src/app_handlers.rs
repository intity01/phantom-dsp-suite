//! Event handlers, keyboard shortcuts, drag-and-drop archive install, and plugin list import.

use crate::app::PhantomLauncherApp;
use crate::app_actions::{execute_verify, execute_zip_install};
use crate::hosts::detect_hosts;
use crate::manifest::{find_plugin, PluginManifest, PluginStatus};
use crate::platform::{open_folder, open_url, utc_hms_now};
use crate::store::{export_installed_list, parse_exported_list, plugin_list_path};
use crate::ui_grid::GridEvents;
use crate::ui_inspector::{ConfirmAction, InspectorEvents};
use crate::ui_log::LogLevel;
use crate::ui_settings::SettingsEvents;
use crate::ui_sidebar::AppView;
use eframe::egui::{self, Key};
use std::path::Path;

impl PhantomLauncherApp {
    pub(crate) fn handle_dropped_file(&mut self, path: &Path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "zip" {
            let paths = self.current_paths();
            let out = execute_zip_install(path, &paths, self.opts);
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("archive.zip");
            self.apply_outcome(out, Some(&format!("Installed from {fname}")));
        } else if ext == "json" {
            self.import_list_from_path(path);
        } else {
            self.notify("Drop a .zip plugin archive or .json plugin list");
        }
    }

    pub(crate) fn import_list_from_path(&mut self, path: &Path) {
        let Ok(raw) = std::fs::read_to_string(path) else {
            self.notify(format!("Could not read {}", path.display()));
            return;
        };
        let list = match parse_exported_list(&raw) {
            Ok(l) => l,
            Err(e) => {
                self.notify(e);
                return;
            }
        };
        let mut count = 0usize;
        for entry in list.plugins {
            if let Some(p) = find_plugin(&entry.id) {
                if p.status == PluginStatus::Ready {
                    self.run_guarded(p.id, ConfirmAction::Install, true);
                    count += 1;
                }
            }
        }
        self.notify(format!("Imported list: processed {count} ready plugin(s)"));
    }

    pub(crate) fn handle_shortcuts(
        &mut self,
        ctx: &egui::Context,
        visible: &[&'static PluginManifest],
    ) {
        let typing = ctx.wants_keyboard_input();
        ctx.input(|i| {
            let mod_k = i.modifiers.command && i.key_pressed(Key::K);
            let slash = !typing && i.key_pressed(Key::Slash);
            if mod_k || slash {
                self.view = AppView::Browse;
                ctx.memory_mut(|m| m.request_focus(self.search_id));
            }
            if i.key_pressed(Key::Escape) {
                if self.pending_confirm.is_some() {
                    self.pending_confirm = None;
                } else if !self.query.is_empty() {
                    self.query.clear();
                }
            }
            if !typing && self.view == AppView::Browse && !visible.is_empty() {
                let delta: isize = if i.key_pressed(Key::ArrowLeft) {
                    -1
                } else if i.key_pressed(Key::ArrowRight) {
                    1
                } else if i.key_pressed(Key::ArrowUp) {
                    -(self.last_grid_cols as isize)
                } else if i.key_pressed(Key::ArrowDown) {
                    self.last_grid_cols as isize
                } else {
                    0
                };
                if delta != 0 {
                    let cur = visible
                        .iter()
                        .position(|p| Some(p.id) == self.selected_id)
                        .unwrap_or(0) as isize;
                    let next = (cur + delta).clamp(0, visible.len() as isize - 1) as usize;
                    self.selected_id = Some(visible[next].id);
                    self.pending_confirm = None;
                }
            }
        });
    }

    pub(crate) fn apply_inspector_events(&mut self, ctx: &egui::Context, ev: InspectorEvents) {
        if let Some(id) = ev.install_id {
            self.run_guarded(id, ConfirmAction::Install, false);
        }
        if let Some(id) = ev.update_id {
            self.run_guarded(id, ConfirmAction::Update, false);
        }
        if let Some(id) = ev.uninstall_id {
            self.run_guarded(id, ConfirmAction::Remove, false);
        }
        if let Some(id) = ev.verify_id {
            let paths = self.current_paths();
            let out = execute_verify(id, &paths);
            self.apply_outcome(out, Some("SHA-256 verification logged"));
        }
        if let Some(h) = ev.copy_hash {
            ctx.copy_text(h.clone());
            self.notify("Copied SHA-256 to clipboard");
            self.log(LogLevel::Info, format!("Copied SHA-256: {h}"));
        }
        if let Some(url) = ev.open_repo {
            let _ = open_url(&url);
        }
        if ev.confirm_continue {
            if let Some(c) = self.pending_confirm.take() {
                self.run_guarded(c.id, c.action, true);
            }
        }
        if ev.confirm_cancel {
            self.pending_confirm = None;
        }
        if ev.open_log {
            self.view = AppView::Log;
        }
    }

    pub(crate) fn apply_grid_events(&mut self, gev: GridEvents) {
        self.last_grid_cols = gev.cols.max(1);
        if let Some(id) = gev.select_id {
            self.selected_id = Some(id);
            self.pending_confirm = None;
        }
        if let Some(id) = gev.install_id {
            self.run_guarded(id, ConfirmAction::Install, false);
        }
        if let Some(id) = gev.update_id {
            self.run_guarded(id, ConfirmAction::Update, false);
        }
        if let Some(id) = gev.uninstall_id {
            self.run_guarded(id, ConfirmAction::Remove, false);
        }
        if let Some(h) = gev.select_host {
            self.target_host_id = h;
        }
        if gev.rescan_hosts {
            self.hosts = detect_hosts();
            self.notify("Updated DAW detection");
        }
    }

    pub(crate) fn apply_settings_events(&mut self, ctx: &egui::Context, sev: SettingsEvents) {
        let paths = self.current_paths();
        if sev.changed {
            self.persist_settings();
            self.rescan_all();
        }
        if sev.open_vst3 {
            let _ = open_folder(&paths.vst3_dir);
        }
        if sev.open_clap {
            let _ = open_folder(&paths.clap_dir);
        }
        if let Some(url) = sev.open_releases {
            let _ = open_url(&url);
        }
        if let Some(url) = sev.copy_releases {
            ctx.copy_text(url);
            self.notify("Copied releases URL");
        }
        if sev.export_list {
            match export_installed_list(&self.scans, utc_hms_now()) {
                Ok((p, _)) => self.notify(format!("Exported to {}", p.display())),
                Err(e) => self.notify(e),
            }
        }
        if sev.import_list {
            let p = plugin_list_path();
            self.import_list_from_path(&p);
        }
    }
}

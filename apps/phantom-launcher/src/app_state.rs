//! Application state initialization, catalog filtering, and guarded install/remove actions.

use crate::app::PhantomLauncherApp;
use crate::app_actions::{execute_install, execute_uninstall, ActionOutcome};
use crate::brand::BrandAssets;
use crate::discovery::scan_plugin;
use crate::hosts::{detect_hosts, running_hosts_for_plugin};
use crate::installer::InstallOptions;
use crate::manifest::{find_plugin, BrowseGroup, PluginManifest, AVAILABLE_PLUGINS};
use crate::paths::{
    default_user_clap_dir, default_user_vst3_dir, resolve_install_paths, InstallPaths,
};
use crate::platform::utc_hms_now;
use crate::store::{load_settings, save_settings, PersistedSettings};
use crate::theme::apply_suite_theme;
use crate::ui_art::ArtCache;
use crate::ui_inspector::{ConfirmAction, PendingConfirm};
use crate::ui_log::{LogEntry, LogLevel};
use crate::ui_sidebar::{AppView, BrowseScope};
use eframe::egui::Id;
use std::collections::HashMap;
use std::time::Instant;

impl PhantomLauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_suite_theme(&cc.egui_ctx);
        let saved = load_settings();
        let hosts = detect_hosts();
        let target_host_id = hosts.iter().find(|h| h.installed).map(|h| h.id);
        let mut app = Self {
            view: AppView::Browse,
            scope: BrowseScope::All,
            query: String::new(),
            selected_id: AVAILABLE_PLUGINS.first().map(|p| p.id),
            install_scope: saved.install_scope(),
            custom_vst3: if saved.custom_vst3.is_empty() {
                default_user_vst3_dir().to_string_lossy().into_owned()
            } else {
                saved.custom_vst3
            },
            custom_clap: if saved.custom_clap.is_empty() {
                default_user_clap_dir().to_string_lossy().into_owned()
            } else {
                saved.custom_clap
            },
            opts: InstallOptions {
                install_vst3: saved.install_vst3,
                install_clap: saved.install_clap,
            },
            auto_rescan_daws: saved.auto_rescan_daws,
            github_repo: saved.github_repo,
            scans: HashMap::new(),
            hosts,
            target_host_id,
            pending_confirm: None,
            logs: Vec::new(),
            brand: BrandAssets::load(&cc.egui_ctx),
            art_cache: ArtCache::new(&cc.egui_ctx),
            last_grid_cols: 3,
            search_id: Id::new("phantom_search_input"),
            toast: None,
        };
        app.log(
            LogLevel::Info,
            "Phantom Launcher ready — 100% offline, zero telemetry, native egui.",
        );
        app.rescan_all();
        app
    }

    pub(crate) fn current_paths(&self) -> InstallPaths {
        resolve_install_paths(self.install_scope, &self.custom_vst3, &self.custom_clap)
    }

    pub(crate) fn persist_settings(&self) {
        save_settings(&PersistedSettings {
            scope: PersistedSettings::scope_str(self.install_scope).into(),
            custom_vst3: self.custom_vst3.clone(),
            custom_clap: self.custom_clap.clone(),
            install_vst3: self.opts.install_vst3,
            install_clap: self.opts.install_clap,
            auto_rescan_daws: self.auto_rescan_daws,
            github_repo: self.github_repo.clone(),
        });
    }

    pub(crate) fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    pub(crate) fn log(&mut self, level: LogLevel, msg: impl Into<String>) {
        self.logs.push(LogEntry {
            time: utc_hms_now(),
            level,
            message: msg.into(),
        });
    }

    pub(crate) fn rescan_all(&mut self) {
        let paths = self.current_paths();
        self.scans.clear();
        for p in AVAILABLE_PLUGINS {
            self.scans.insert(p.id, scan_plugin(p, &paths, None));
        }
        self.hosts = detect_hosts();
        if self
            .target_host_id
            .is_some_and(|id| !self.hosts.iter().any(|h| h.id == id && h.installed))
        {
            self.target_host_id = self.hosts.iter().find(|h| h.installed).map(|h| h.id);
        }
        self.log(
            LogLevel::Info,
            format!(
                "Scanned VST3 ({}) & CLAP ({})",
                paths.vst3_dir.display(),
                paths.clap_dir.display()
            ),
        );
    }

    pub(crate) fn apply_outcome(&mut self, outcome: ActionOutcome, toast_ok: Option<&str>) {
        let mut had_err = None;
        for (lvl, msg) in outcome.messages {
            if matches!(lvl, LogLevel::Error) && had_err.is_none() {
                had_err = Some(msg.clone());
            }
            self.log(lvl, msg);
        }
        if let Some((id, scan)) = outcome.updated_scan {
            self.scans.insert(id, scan);
        }
        if outcome.needs_rescan {
            self.rescan_all();
        } else if self.auto_rescan_daws {
            self.hosts = detect_hosts();
        }
        if let Some(err) = had_err {
            self.notify(err);
        } else if let Some(ok) = toast_ok {
            self.notify(ok);
        }
    }

    pub(crate) fn run_guarded(&mut self, id: &'static str, action: ConfirmAction, bypass: bool) {
        let Some(plugin) = find_plugin(id) else {
            return;
        };
        if !bypass && matches!(action, ConfirmAction::Update | ConfirmAction::Remove) {
            let open = running_hosts_for_plugin(&self.hosts, plugin);
            if !open.is_empty() {
                self.selected_id = Some(id);
                self.pending_confirm = Some(PendingConfirm {
                    id,
                    action,
                    hosts: open,
                });
                return;
            }
        }
        self.pending_confirm = None;
        let paths = self.current_paths();
        match action {
            ConfirmAction::Install => {
                let out = execute_install(id, &paths, self.opts);
                self.apply_outcome(out, Some(&format!("{} installed & verified", plugin.name)));
            }
            ConfirmAction::Update => {
                let out = execute_install(id, &paths, self.opts);
                self.apply_outcome(out, Some(&format!("{} updated & verified", plugin.name)));
            }
            ConfirmAction::Remove => {
                let out = execute_uninstall(id, &paths);
                self.apply_outcome(out, Some(&format!("{} removed", plugin.name)));
            }
        }
    }

    pub(crate) fn visible_plugins(&self) -> Vec<&'static PluginManifest> {
        let q = self.query.trim().to_ascii_lowercase();
        AVAILABLE_PLUGINS
            .iter()
            .filter(|p| {
                let in_scope = match self.scope {
                    BrowseScope::All => true,
                    BrowseScope::Equalizer => p.browse_group == BrowseGroup::Equalizer,
                    BrowseScope::Vocal => p.browse_group == BrowseGroup::Vocal,
                    BrowseScope::Dynamics => p.browse_group == BrowseGroup::Dynamics,
                    BrowseScope::Mastering => p.browse_group == BrowseGroup::Mastering,
                    BrowseScope::Installed => {
                        self.scans.get(p.id).is_some_and(|s| s.is_any_installed())
                    }
                    BrowseScope::Updates => self.scans.get(p.id).is_some_and(|s| s.has_update()),
                };
                in_scope
                    && (q.is_empty()
                        || p.name.to_ascii_lowercase().contains(&q)
                        || p.kind.to_ascii_lowercase().contains(&q)
                        || p.category.to_ascii_lowercase().contains(&q))
            })
            .collect()
    }
}

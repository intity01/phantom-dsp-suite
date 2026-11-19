//! Main eframe desktop GUI state and panel layout for Phantom Launcher.

use crate::app_actions::{execute_install, execute_uninstall, execute_verify, ActionOutcome};
use crate::discovery::{scan_plugin, InstallScanResult};
use crate::installer::InstallOptions;
use crate::manifest::{github_releases_url, PluginStatus, AVAILABLE_PLUGINS, DEFAULT_GITHUB_REPO};
use crate::paths::{
    default_user_clap_dir, default_user_vst3_dir, host_dist_os_dir, resolve_install_paths,
    InstallPaths, InstallScope,
};
use crate::platform::{detect_running_cubase, open_folder, open_url, utc_hms_now};
use crate::theme::{apply_obsidian_theme, BG_PANEL, TEXT_DIMMED, TEXT_MUTED};
use crate::ui_cards::{render_plugin_card, CardAction};
use crate::ui_log::{render_activity_log, LogEntry, LogLevel};
use crate::ui_panels::{
    render_cubase_banner, render_header, render_release_panel, render_settings_bar,
};
use eframe::egui::{self, FontId, Margin, RichText};
use std::collections::HashMap;

pub struct PhantomLauncherApp {
    scope: InstallScope,
    custom_vst3: String,
    custom_clap: String,
    opts: InstallOptions,
    github_repo: String,
    show_release_panel: bool,
    scans: HashMap<&'static str, InstallScanResult>,
    running_cubase: Option<String>,
    logs: Vec<LogEntry>,
}

impl PhantomLauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_obsidian_theme(&cc.egui_ctx);
        let mut app = Self {
            scope: InstallScope::User,
            custom_vst3: default_user_vst3_dir().to_string_lossy().into_owned(),
            custom_clap: default_user_clap_dir().to_string_lossy().into_owned(),
            opts: InstallOptions::default(),
            github_repo: DEFAULT_GITHUB_REPO.to_string(),
            show_release_panel: false,
            scans: HashMap::new(),
            running_cubase: detect_running_cubase(),
            logs: Vec::new(),
        };
        app.log(
            LogLevel::Info,
            "Phantom Launcher started in 100% offline mode (zero network requests, zero telemetry).",
        );
        if let Some(proc_name) = app.running_cubase.clone() {
            app.log(
                LogLevel::Warn,
                format!(
                    "{proc_name} detected running — close Cubase before overwriting locked plugins."
                ),
            );
        }
        app.rescan_all();
        app
    }

    fn current_paths(&self) -> InstallPaths {
        resolve_install_paths(self.scope, &self.custom_vst3, &self.custom_clap)
    }

    fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        self.logs.push(LogEntry {
            time: utc_hms_now(),
            level,
            message: message.into(),
        });
    }

    fn rescan_all(&mut self) {
        let paths = self.current_paths();
        self.scans.clear();
        for plugin in AVAILABLE_PLUGINS {
            self.scans
                .insert(plugin.id, scan_plugin(plugin, &paths, None));
        }
        self.running_cubase = detect_running_cubase();
        if let Some(eq_scan) = self.scans.get("phantom-eq") {
            let src_desc = eq_scan
                .source
                .detected_root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| format!("not found (expected dist/{})", host_dist_os_dir()));
            self.log(
                LogLevel::Info,
                format!(
                    "Scanned disk — VST3: {} | Source: {src_desc}",
                    paths.vst3_dir.display()
                ),
            );
        }
    }

    fn apply_outcome(&mut self, outcome: ActionOutcome) {
        for (lvl, msg) in outcome.messages {
            self.log(lvl, msg);
        }
        if let Some((id, scan)) = outcome.updated_scan {
            self.scans.insert(id, scan);
        }
        if outcome.needs_rescan {
            self.rescan_all();
        }
    }

    fn handle_card_action(&mut self, ctx: &egui::Context, action: CardAction) {
        let paths = self.current_paths();
        match action {
            CardAction::Install(id) => self.apply_outcome(execute_install(id, &paths, self.opts)),
            CardAction::Uninstall(id) => self.apply_outcome(execute_uninstall(id, &paths)),
            CardAction::Verify(id) => self.apply_outcome(execute_verify(id, &paths)),
            CardAction::CopyHash(hash) => {
                ctx.copy_text(hash.clone());
                self.log(
                    LogLevel::Info,
                    format!("Copied SHA-256 to clipboard: {hash}"),
                );
            }
        }
    }
}

impl eframe::App for PhantomLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header_panel")
            .frame(egui::Frame::NONE.fill(BG_PANEL))
            .show(ctx, |ui| {
                let ev = render_header(ui, self.show_release_panel);
                if ev.rescan {
                    self.rescan_all();
                }
                if ev.toggle_releases {
                    self.show_release_panel = !self.show_release_panel;
                    if self.show_release_panel {
                        let url = github_releases_url(&self.github_repo);
                        self.log(
                            LogLevel::Info,
                            format!("Manual release check opened (offline-first). Target: {url}"),
                        );
                    }
                }
            });

        egui::TopBottomPanel::bottom("log_panel")
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| render_activity_log(ctx, ui, &mut self.logs));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                if self.show_release_panel {
                    let ev = render_release_panel(ui, &mut self.github_repo);
                    if let Some(url) = ev.open_url {
                        match open_url(&url) {
                            Ok(()) => self.log(LogLevel::Info, format!("Opened browser -> {url}")),
                            Err(e) => self.log(LogLevel::Error, e),
                        }
                    }
                    if let Some(url) = ev.copy_url {
                        ctx.copy_text(url.clone());
                        self.log(LogLevel::Info, format!("Copied releases URL: {url}"));
                    }
                    ui.add_space(8.0);
                }

                if let Some(daw) = self.running_cubase.clone() {
                    if render_cubase_banner(ui, &daw) {
                        self.running_cubase = detect_running_cubase();
                    }
                    ui.add_space(8.0);
                }

                let paths = self.current_paths();
                let set_ev = render_settings_bar(
                    ui,
                    &mut self.scope,
                    &mut self.custom_vst3,
                    &mut self.custom_clap,
                    &mut self.opts,
                    &paths,
                );
                if set_ev.changed {
                    self.rescan_all();
                }
                if set_ev.open_vst3 {
                    if let Err(e) = open_folder(&paths.vst3_dir) {
                        self.log(LogLevel::Error, e);
                    }
                }
                if set_ev.open_clap {
                    if let Err(e) = open_folder(&paths.clap_dir) {
                        self.log(LogLevel::Error, e);
                    }
                }

                ui.add_space(10.0);
                let ready = AVAILABLE_PLUGINS
                    .iter()
                    .filter(|p| p.status == PluginStatus::Ready)
                    .count();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PHANTOM DSP SUITE")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(TEXT_MUTED),
                    );
                    ui.label(
                        RichText::new(format!(
                            "({ready} Ready • {} Planned)",
                            AVAILABLE_PLUGINS.len() - ready
                        ))
                        .font(FontId::proportional(11.0))
                        .color(TEXT_DIMMED),
                    );
                });
                ui.add_space(6.0);

                let mut pending = None;
                egui::ScrollArea::vertical()
                    .id_salt("plugins_scroll")
                    .show(ui, |ui| {
                        for plugin in AVAILABLE_PLUGINS {
                            if let Some(scan) = self.scans.get(plugin.id) {
                                if let Some(act) = render_plugin_card(ui, plugin, scan, self.opts) {
                                    pending = Some(act);
                                }
                                ui.add_space(8.0);
                            }
                        }
                    });
                if let Some(act) = pending {
                    self.handle_card_action(ctx, act);
                }
            });
    }
}

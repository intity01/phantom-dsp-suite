//! Main 3-pane native eframe/egui application for Phantom Launcher (100% offline, zero WebView).

use crate::app_overlays::{render_drop_overlay, render_toast};
use crate::brand::BrandAssets;
use crate::discovery::InstallScanResult;
use crate::hosts::HostInfo;
use crate::installer::InstallOptions;
use crate::manifest::{find_plugin, github_releases_url, BrowseGroup, AVAILABLE_PLUGINS};
use crate::paths::InstallScope;
use crate::platform::open_url;
use crate::theme::{paint_window_backdrop, BG_SURFACE, BORDER_SUBTLE};
use crate::ui_about::render_about_page;
use crate::ui_art::ArtCache;
use crate::ui_grid::render_browse_grid;
use crate::ui_inspector::{render_inspector, PendingConfirm};
use crate::ui_log::{render_activity_log_page, LogEntry, LogLevel};
use crate::ui_settings::render_settings_page;
use crate::ui_sidebar::{render_sidebar, AppView, BrowseScope, SidebarCounts};
use crate::ui_toolbar::render_toolbar;
use eframe::egui::{self, CornerRadius, Id, Margin, Stroke};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct PhantomLauncherApp {
    pub(crate) view: AppView,
    pub(crate) scope: BrowseScope,
    pub(crate) query: String,
    pub(crate) selected_id: Option<&'static str>,
    pub(crate) install_scope: InstallScope,
    pub(crate) custom_vst3: String,
    pub(crate) custom_clap: String,
    pub(crate) opts: InstallOptions,
    pub(crate) auto_rescan_daws: bool,
    pub(crate) github_repo: String,
    pub(crate) scans: HashMap<&'static str, InstallScanResult>,
    pub(crate) hosts: Vec<HostInfo>,
    pub(crate) target_host_id: Option<&'static str>,
    pub(crate) pending_confirm: Option<PendingConfirm>,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) brand: BrandAssets,
    pub(crate) art_cache: ArtCache,
    pub(crate) last_grid_cols: usize,
    pub(crate) search_id: Id,
    pub(crate) toast: Option<(String, Instant)>,
}

impl eframe::App for PhantomLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        paint_window_backdrop(ctx);
        let visible = self.visible_plugins();
        self.handle_shortcuts(ctx, &visible);

        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(p) = f.path {
                self.handle_dropped_file(&p);
            }
        }
        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());

        let counts = SidebarCounts {
            all: AVAILABLE_PLUGINS.len(),
            eq: AVAILABLE_PLUGINS
                .iter()
                .filter(|p| p.browse_group == BrowseGroup::Equalizer)
                .count(),
            vocal: AVAILABLE_PLUGINS
                .iter()
                .filter(|p| p.browse_group == BrowseGroup::Vocal)
                .count(),
            dynamics: AVAILABLE_PLUGINS
                .iter()
                .filter(|p| p.browse_group == BrowseGroup::Dynamics)
                .count(),
            mastering: AVAILABLE_PLUGINS
                .iter()
                .filter(|p| p.browse_group == BrowseGroup::Mastering)
                .count(),
            installed: self.scans.values().filter(|s| s.is_any_installed()).count(),
            updates: self.scans.values().filter(|s| s.has_update()).count(),
            logs: self.logs.len(),
        };

        egui::SidePanel::left("sidebar")
            .exact_width(236.0)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(10, 8)),
            )
            .show(ctx, |ui| {
                render_sidebar(ui, &self.brand, &mut self.view, &mut self.scope, &counts);
            });

        let has_local_src = self.scans.values().any(|s| s.has_any_source());
        egui::TopBottomPanel::top("titlebar")
            .exact_height(52.0)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(10, 10)),
            )
            .show(ctx, |ui| {
                let tb = render_toolbar(ui, &mut self.query, &mut self.view, has_local_src);
                self.search_id = tb.search_id;
                if tb.rescan {
                    self.rescan_all();
                    self.notify("Rescanned plugins and DAWs");
                }
                if tb.open_releases {
                    let url = github_releases_url(&self.github_repo);
                    if let Err(e) = open_url(&url) {
                        self.log(LogLevel::Error, e);
                    }
                }
            });

        let selected_plugin = self
            .selected_id
            .and_then(find_plugin)
            .or_else(|| visible.first().copied())
            .or_else(|| AVAILABLE_PLUGINS.first());
        let active_host = self
            .target_host_id
            .and_then(|id| self.hosts.iter().find(|h| h.id == id && h.installed))
            .cloned();

        let mut ins_ev = None;
        egui::SidePanel::right("inspector")
            .exact_width(340.0)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(Margin {
                        left: 4,
                        right: 10,
                        top: 0,
                        bottom: 10,
                    }),
            )
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(BG_SURFACE)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_height(ui.available_height());
                        let scan = selected_plugin.and_then(|p| self.scans.get(p.id));
                        ins_ev = Some(render_inspector(
                            ui,
                            selected_plugin,
                            scan,
                            active_host.as_ref(),
                            self.opts,
                            self.pending_confirm.as_ref(),
                            self.logs.last(),
                            &self.art_cache,
                        ));
                    });
            });
        if let Some(ev) = ins_ev {
            self.apply_inspector_events(ctx, ev);
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(Margin {
                        left: 4,
                        right: 6,
                        top: 0,
                        bottom: 10,
                    }),
            )
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(BG_SURFACE)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(24, 20))
                    .show(ui, |ui| {
                        ui.set_height(ui.available_height());
                        match self.view {
                            AppView::Browse => {
                                let gev = render_browse_grid(
                                    ui,
                                    self.scope,
                                    &visible,
                                    AVAILABLE_PLUGINS,
                                    &self.scans,
                                    &self.hosts,
                                    self.target_host_id,
                                    selected_plugin.map(|p| p.id),
                                    &self.art_cache,
                                );
                                self.apply_grid_events(gev);
                            }
                            AppView::Log => {
                                if let Some(msg) = render_activity_log_page(ctx, ui, &mut self.logs)
                                {
                                    self.notify(msg);
                                }
                            }
                            AppView::Settings => {
                                let paths = self.current_paths();
                                let sev = render_settings_page(
                                    ui,
                                    &mut self.install_scope,
                                    &mut self.custom_vst3,
                                    &mut self.custom_clap,
                                    &mut self.opts,
                                    &mut self.auto_rescan_daws,
                                    &mut self.github_repo,
                                    &paths,
                                );
                                self.apply_settings_events(ctx, sev);
                            }
                            AppView::About => render_about_page(ui, &self.brand),
                        }
                    });
            });

        if hovering_files {
            render_drop_overlay(ctx);
        }
        if let Some((msg, at)) = &self.toast {
            if at.elapsed() < Duration::from_secs_f32(3.2) {
                render_toast(ctx, msg);
                ctx.request_repaint_after(Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }
    }
}

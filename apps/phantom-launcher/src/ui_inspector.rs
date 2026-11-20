//! Right 340px Plugin Inspector pane with artwork, release notes, and recent activity.

use crate::discovery::InstallScanResult;
use crate::hosts::HostInfo;
use crate::installer::InstallOptions;
use crate::manifest::PluginManifest;
use crate::ui_art::ArtCache;
use crate::ui_inspector_parts::{render_actions, render_confirm_guard, render_facts_box};
use crate::ui_log::LogEntry;
use eframe::egui::{self, CornerRadius, FontId, Margin, RichText, Ui, Vec2};
use phantom_kit::prelude::{show_icon, IconName};
use phantom_kit::prelude::{
    ACCENT_SOFT, ACCENT_TEXT, FILL_1, FILL_2, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Install,
    Update,
    Remove,
}

pub struct PendingConfirm {
    pub id: &'static str,
    pub action: ConfirmAction,
    pub hosts: Vec<String>,
}

#[derive(Default)]
pub struct InspectorEvents {
    pub install_id: Option<&'static str>,
    pub update_id: Option<&'static str>,
    pub uninstall_id: Option<&'static str>,
    pub verify_id: Option<&'static str>,
    pub copy_hash: Option<String>,
    pub open_repo: Option<String>,
    pub confirm_continue: bool,
    pub confirm_cancel: bool,
    pub open_log: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_inspector(
    ui: &mut Ui,
    plugin: Option<&'static PluginManifest>,
    scan: Option<&InstallScanResult>,
    active_host: Option<&HostInfo>,
    opts: InstallOptions,
    pending_confirm: Option<&PendingConfirm>,
    latest_log: Option<&LogEntry>,
    art_cache: &ArtCache,
) -> InspectorEvents {
    let mut ev = InspectorEvents::default();
    let Some(plugin) = plugin else {
        ui.label(RichText::new("Select a plugin").color(TEXT_TERTIARY));
        return ev;
    };

    egui::ScrollArea::vertical()
        .id_salt("inspector_scroll")
        .show(ui, |ui| {
            let art_w = ui.available_width().max(240.0);
            art_cache.draw_art(ui, plugin.id, Vec2::new(art_w, 192.0), 14, false);

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(plugin.name)
                        .font(FontId::proportional(21.0))
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("v{}", plugin.version))
                            .font(FontId::proportional(12.5))
                            .color(TEXT_TERTIARY),
                    );
                });
            });
            ui.label(
                RichText::new(format!("{} · {}", plugin.kind, plugin.category))
                    .font(FontId::proportional(13.0))
                    .color(TEXT_SECONDARY),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(plugin.author)
                        .font(FontId::proportional(12.0))
                        .color(TEXT_SECONDARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::NONE
                        .fill(FILL_2)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(plugin.license)
                                    .font(FontId::proportional(11.0))
                                    .color(TEXT_SECONDARY),
                            );
                        });
                });
            });

            ui.add_space(10.0);
            ui.label(
                RichText::new(plugin.description)
                    .font(FontId::proportional(13.0))
                    .color(TEXT_SECONDARY),
            );

            ui.add_space(14.0);
            render_facts_box(ui, plugin, scan, active_host, &mut ev);

            ui.add_space(12.0);
            egui::Frame::NONE
                .fill(ACCENT_SOFT)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(14, 11))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(format!("Highlights in v{}", plugin.version))
                            .font(FontId::proportional(12.5))
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(plugin.release_notes)
                            .font(FontId::proportional(12.5))
                            .color(TEXT_SECONDARY),
                    );
                });

            if let Some(conf) = pending_confirm.filter(|c| c.id == plugin.id) {
                ui.add_space(12.0);
                render_confirm_guard(ui, conf, &mut ev);
            }

            ui.add_space(14.0);
            render_actions(ui, plugin, scan, opts, &mut ev);

            ui.add_space(18.0);
            ui.label(
                RichText::new("RECENT ACTIVITY")
                    .font(FontId::proportional(10.5))
                    .strong()
                    .color(TEXT_TERTIARY),
            );
            ui.add_space(4.0);
            egui::Frame::NONE
                .fill(FILL_1)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if let Some(entry) = latest_log {
                        ui.label(
                            RichText::new(format!("[{}] {}", entry.time, entry.message))
                                .font(FontId::monospace(10.5))
                                .color(TEXT_SECONDARY),
                        );
                    } else {
                        ui.label(
                            RichText::new("No activity yet.")
                                .font(FontId::proportional(12.0))
                                .color(TEXT_TERTIARY),
                        );
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        show_icon(ui, IconName::Terminal, 12.5, ACCENT_TEXT);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Open activity & SHA-256 log")
                                        .font(FontId::proportional(11.5))
                                        .color(ACCENT_TEXT),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            ev.open_log = true;
                        }
                    });
                });
        });

    ev
}

//! Modular UI chrome panels: header, offline GitHub release bar, DAW banner, and settings bar.

use crate::installer::InstallOptions;
use crate::manifest::{github_releases_url, DEFAULT_GITHUB_REPO};
use crate::paths::{InstallPaths, InstallScope};
use crate::theme::{
    draw_pill, subtle_button, ACCENT_AMBER, ACCENT_AMBER_BG, ACCENT_CYAN, ACCENT_CYAN_BG,
    ACCENT_GREEN, ACCENT_GREEN_BG, BG_CARD, BG_DISPLAY, BORDER_LIGHT, BORDER_SUBTLE, TEXT_BRIGHT,
    TEXT_DIMMED, TEXT_MUTED,
};
use eframe::egui::{self, CornerRadius, FontId, Margin, RichText, Stroke};

#[derive(Default)]
pub struct HeaderEvents {
    pub rescan: bool,
    pub toggle_releases: bool,
}

pub fn render_header(ui: &mut egui::Ui, release_panel_open: bool) -> HeaderEvents {
    let mut ev = HeaderEvents::default();
    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PHANTOM LAUNCHER")
                                .font(FontId::proportional(18.0))
                                .strong()
                                .color(TEXT_BRIGHT),
                        );
                        draw_pill(
                            ui,
                            "100% OFFLINE • ZERO TELEMETRY",
                            ACCENT_GREEN,
                            ACCENT_GREEN_BG,
                            ACCENT_GREEN,
                        );
                    });
                    ui.label(
                        RichText::new(
                            "Open-Source Audio Plugin Hub • Verifiable SHA-256 Offline Installer",
                        )
                        .font(FontId::proportional(11.5))
                        .color(TEXT_MUTED),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if subtle_button(ui, "Rescan Disk", TEXT_BRIGHT, BORDER_LIGHT).clicked() {
                        ev.rescan = true;
                    }
                    let border = if release_panel_open {
                        ACCENT_CYAN
                    } else {
                        BORDER_LIGHT
                    };
                    if subtle_button(ui, "Check GitHub Releases", ACCENT_CYAN, border)
                        .on_hover_text(
                            "Manual offline release check — never connects automatically",
                        )
                        .clicked()
                    {
                        ev.toggle_releases = true;
                    }
                });
            });
        });
    ev
}

#[derive(Default)]
pub struct ReleaseEvents {
    pub open_url: Option<String>,
    pub copy_url: Option<String>,
}

pub fn render_release_panel(ui: &mut egui::Ui, github_repo: &mut String) -> ReleaseEvents {
    let mut ev = ReleaseEvents::default();
    let url = github_releases_url(github_repo);
    egui::Frame::NONE
        .fill(BG_DISPLAY)
        .stroke(Stroke::new(1.0_f32, ACCENT_CYAN))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("GitHub Repository:").strong().color(ACCENT_CYAN));
                ui.add(
                    egui::TextEdit::singleline(github_repo)
                        .desired_width(240.0)
                        .hint_text(DEFAULT_GITHUB_REPO),
                );
                if subtle_button(ui, "Open Releases in Browser", TEXT_BRIGHT, ACCENT_CYAN).clicked() {
                    ev.open_url = Some(url.clone());
                }
                if subtle_button(ui, "Copy URL", TEXT_MUTED, BORDER_LIGHT).clicked() {
                    ev.copy_url = Some(url.clone());
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Offline-First: Zero background connections are ever made. Releases URL: {url} — compare SHA256SUMS.txt with card checksums."
                ))
                .font(FontId::proportional(11.0))
                .color(TEXT_MUTED),
            );
        });
    ev
}

pub fn render_cubase_banner(ui: &mut egui::Ui, proc_name: &str) -> bool {
    let mut recheck = false;
    egui::Frame::NONE
        .fill(ACCENT_AMBER_BG)
        .stroke(Stroke::new(1.0_f32, ACCENT_AMBER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Warning: {proc_name} is currently running. Close Cubase if plugin replacement is locked."
                    ))
                    .color(ACCENT_AMBER),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Recheck").clicked() {
                        recheck = true;
                    }
                });
            });
        });
    recheck
}

#[derive(Default)]
pub struct SettingsEvents {
    pub changed: bool,
    pub open_vst3: bool,
    pub open_clap: bool,
}

pub fn render_settings_bar(
    ui: &mut egui::Ui,
    scope: &mut InstallScope,
    custom_vst3: &mut String,
    custom_clap: &mut String,
    opts: &mut InstallOptions,
    paths: &InstallPaths,
) -> SettingsEvents {
    let mut ev = SettingsEvents::default();
    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Install Scope:").strong().color(TEXT_MUTED));
                for (sc, label) in [
                    (InstallScope::User, "User (Recommended)"),
                    (InstallScope::System, "System"),
                    (InstallScope::Custom, "Custom"),
                ] {
                    let active = *scope == sc;
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(label)
                                .font(FontId::proportional(11.5))
                                .color(if active { ACCENT_CYAN } else { TEXT_MUTED }),
                        )
                        .fill(if active { ACCENT_CYAN_BG } else { BG_DISPLAY })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if active { ACCENT_CYAN } else { BORDER_SUBTLE },
                        ))
                        .corner_radius(CornerRadius::same(6)),
                    );
                    if resp.clicked() && *scope != sc {
                        *scope = sc;
                        ev.changed = true;
                    }
                }

                ui.separator();
                ui.checkbox(
                    &mut opts.install_vst3,
                    RichText::new("VST3").color(TEXT_BRIGHT),
                );
                ui.checkbox(
                    &mut opts.install_clap,
                    RichText::new("CLAP").color(TEXT_BRIGHT),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if subtle_button(ui, "Open CLAP Folder", TEXT_MUTED, BORDER_SUBTLE).clicked() {
                        ev.open_clap = true;
                    }
                    if subtle_button(ui, "Open VST3 Folder", TEXT_BRIGHT, BORDER_LIGHT).clicked() {
                        ev.open_vst3 = true;
                    }
                });
            });

            ui.add_space(6.0);
            if *scope == InstallScope::Custom {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("VST3:")
                            .font(FontId::monospace(11.0))
                            .color(TEXT_MUTED),
                    );
                    let r1 = ui.add(egui::TextEdit::singleline(custom_vst3).desired_width(310.0));
                    ui.label(
                        RichText::new("CLAP:")
                            .font(FontId::monospace(11.0))
                            .color(TEXT_MUTED),
                    );
                    let r2 = ui.add(egui::TextEdit::singleline(custom_clap).desired_width(310.0));
                    if r1.lost_focus() || r2.lost_focus() {
                        ev.changed = true;
                    }
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("VST3: {}", paths.vst3_dir.display()))
                            .font(FontId::monospace(11.0))
                            .color(TEXT_DIMMED),
                    );
                    ui.label(RichText::new("•").color(BORDER_LIGHT));
                    ui.label(
                        RichText::new(format!("CLAP: {}", paths.clap_dir.display()))
                            .font(FontId::monospace(11.0))
                            .color(TEXT_DIMMED),
                    );
                });
            }
        });
    ev
}

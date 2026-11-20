//! Settings center view with grouped macOS-style rows for plugin paths, formats, and export.

use crate::installer::InstallOptions;
use crate::manifest::{github_releases_url, DEFAULT_GITHUB_REPO};
use crate::paths::{InstallPaths, InstallScope};
use crate::theme::{
    draw_pill, draw_toggle_switch, quiet_button, FILL_1, FILL_3, GREEN, GREEN_SOFT, TEXT_PRIMARY,
    TEXT_SECONDARY, TEXT_TERTIARY,
};
use crate::ui_icons::{icon_text_button, IconName};
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, TextEdit, Ui};

#[derive(Default)]
pub struct SettingsEvents {
    pub changed: bool,
    pub open_vst3: bool,
    pub open_clap: bool,
    pub open_releases: Option<String>,
    pub copy_releases: Option<String>,
    pub export_list: bool,
    pub import_list: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_settings_page(
    ui: &mut Ui,
    scope: &mut InstallScope,
    custom_vst3: &mut String,
    custom_clap: &mut String,
    opts: &mut InstallOptions,
    auto_rescan_daws: &mut bool,
    github_repo: &mut String,
    paths: &InstallPaths,
) -> SettingsEvents {
    let mut ev = SettingsEvents::default();
    egui::ScrollArea::vertical()
        .id_salt("settings_scroll")
        .show(ui, |ui| {
            ui.set_max_width(720.0);
            ui.label(
                RichText::new("Settings")
                    .font(FontId::proportional(25.0))
                    .strong()
                    .color(TEXT_PRIMARY),
            );

            group_head(
                ui,
                "Install locations",
                Some("Per-user folders need no admin rights and every modern DAW scans them."),
            );
            setting_row(ui, |ui| {
                row_copy(ui, "Install scope", "Choose where VST3 and CLAP bundles are placed");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::NONE
                        .fill(FILL_1)
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::same(2))
                        .show(ui, |ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    for (sc, label) in [
                                        (InstallScope::User, "User (Recommended)"),
                                        (InstallScope::System, "System"),
                                        (InstallScope::Custom, "Custom"),
                                    ] {
                                        let active = *scope == sc;
                                        let btn = ui.add(
                                            egui::Button::new(
                                                RichText::new(label)
                                                    .font(FontId::proportional(12.0))
                                                    .color(if active {
                                                        TEXT_PRIMARY
                                                    } else {
                                                        TEXT_SECONDARY
                                                    }),
                                            )
                                            .fill(if active {
                                                FILL_3
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .stroke(Stroke::NONE)
                                            .corner_radius(CornerRadius::same(7)),
                                        );
                                        if btn.clicked() && *scope != sc {
                                            *scope = sc;
                                            ev.changed = true;
                                        }
                                    }
                                },
                            );
                        });
                });
            });

            path_row(
                ui,
                "VST3",
                *scope == InstallScope::Custom,
                custom_vst3,
                &paths.vst3_dir.display().to_string(),
                &mut ev.changed,
                &mut ev.open_vst3,
            );
            path_row(
                ui,
                "CLAP",
                *scope == InstallScope::Custom,
                custom_clap,
                &paths.clap_dir.display().to_string(),
                &mut ev.changed,
                &mut ev.open_clap,
            );

            group_head(ui, "Formats & behaviour", None);
            setting_row(ui, |ui| {
                row_copy(ui, "Install VST3 bundle", "Standard plugin format for Cubase, Ableton Live, FL Studio, REAPER");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if draw_toggle_switch(ui, &mut opts.install_vst3).changed() {
                        ev.changed = true;
                    }
                });
            });
            setting_row(ui, |ui| {
                row_copy(ui, "Install CLAP plugin", "Modern open standard supported by Bitwig Studio, REAPER & FL Studio");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if draw_toggle_switch(ui, &mut opts.install_clap).changed() {
                        ev.changed = true;
                    }
                });
            });
            setting_row(ui, |ui| {
                row_copy(ui, "Check DAWs after install", "Rescan running DAWs and remind you to rescan inside open hosts");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if draw_toggle_switch(ui, auto_rescan_daws).changed() {
                        ev.changed = true;
                    }
                });
            });
            setting_row(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("GitHub Releases repository").font(FontId::proportional(13.5)).color(TEXT_PRIMARY));
                    ui.label(
                        RichText::new("Manual offline link only — Phantom Launcher never connects automatically.")
                            .font(FontId::proportional(12.0))
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        let te = ui.add(
                            TextEdit::singleline(github_repo)
                                .desired_width(250.0)
                                .hint_text(DEFAULT_GITHUB_REPO),
                        );
                        if te.lost_focus() {
                            ev.changed = true;
                        }
                        let url = github_releases_url(github_repo);
                        if quiet_button(ui, "Open Releases").clicked() {
                            ev.open_releases = Some(url.clone());
                        }
                        if quiet_button(ui, "Copy URL").clicked() {
                            ev.copy_releases = Some(url);
                        }
                    });
                });
            });

            group_head(ui, "Move to another machine", None);
            setting_row(ui, |ui| {
                row_copy(
                    ui,
                    "Plugin list",
                    "Export installed state JSON or import phantom-plugin-list.json (drop any .json onto window)",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_text_button(ui, IconName::Import, "Import", TEXT_PRIMARY).clicked() {
                        ev.import_list = true;
                    }
                    if icon_text_button(ui, IconName::Export, "Export", TEXT_PRIMARY).clicked() {
                        ev.export_list = true;
                    }
                });
            });

            group_head(ui, "Privacy", None);
            setting_row(ui, |ui| {
                row_copy(
                    ui,
                    "Telemetry & analytics",
                    "Not built in. Phantom has no accounts and makes zero background network connections.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    draw_pill(ui, "Off forever", GREEN, GREEN_SOFT);
                });
            });
            ui.add_space(16.0);
        });
    ev
}

pub fn group_head(ui: &mut Ui, title: &str, note: Option<&str>) {
    ui.add_space(22.0);
    ui.label(
        RichText::new(title)
            .font(FontId::proportional(13.0))
            .strong()
            .color(TEXT_SECONDARY),
    );
    if let Some(n) = note {
        ui.label(
            RichText::new(n)
                .font(FontId::proportional(12.0))
                .color(TEXT_TERTIARY),
        );
    }
    ui.add_space(6.0);
}

pub fn setting_row(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(FILL_1)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 11))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(add_contents);
        });
    ui.add_space(2.0);
}

fn row_copy(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.set_max_width(470.0);
        ui.label(
            RichText::new(title)
                .font(FontId::proportional(13.5))
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new(subtitle)
                .font(FontId::proportional(12.0))
                .color(TEXT_SECONDARY),
        );
    });
}

fn path_row(
    ui: &mut Ui,
    fmt: &str,
    editable: bool,
    custom_buf: &mut String,
    resolved: &str,
    changed: &mut bool,
    open_clicked: &mut bool,
) {
    setting_row(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(fmt)
                    .font(FontId::proportional(13.5))
                    .color(TEXT_PRIMARY),
            );
            if editable {
                let te = ui.add(TextEdit::singleline(custom_buf).desired_width(460.0));
                if te.lost_focus() {
                    *changed = true;
                }
            } else {
                ui.label(
                    RichText::new(resolved)
                        .font(FontId::monospace(11.5))
                        .color(TEXT_TERTIARY),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_text_button(ui, IconName::Folder, "Open", TEXT_PRIMARY).clicked() {
                *open_clicked = true;
            }
        });
    });
}

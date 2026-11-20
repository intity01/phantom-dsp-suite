//! Inspector sub-sections: facts box, running-DAW confirm guard, and action buttons.

use crate::discovery::InstallScanResult;
use crate::hosts::{host_supports_plugin, HostInfo};
use crate::installer::InstallOptions;
use crate::integrity::{format_file_size, short_fingerprint, VerificationStatus};
use crate::manifest::{PluginManifest, PluginStatus, DEFAULT_GITHUB_REPO};
use crate::ui_inspector::{InspectorEvents, PendingConfirm};
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Ui, Vec2};
use phantom_kit::prelude::{icon_text_button, show_icon, IconName};
use phantom_kit::prelude::{
    primary_button, quiet_button, ACCENT_SOFT, ACCENT_TEXT, FILL_1, FILL_2, FILL_3, GREEN, ORANGE,
    ORANGE_SOFT, RED, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};

pub fn render_facts_box(
    ui: &mut Ui,
    plugin: &PluginManifest,
    scan: Option<&InstallScanResult>,
    active_host: Option<&HostInfo>,
    ev: &mut InspectorEvents,
) {
    let (status_txt, status_col) = if plugin.status == PluginStatus::Planned {
        ("Planned (Roadmap)", TEXT_TERTIARY)
    } else if let Some(s) = scan {
        if s.has_update() {
            ("Update Available", ORANGE)
        } else if s.is_verified_match() {
            ("Installed · Verified", GREEN)
        } else if s.is_any_installed() {
            ("Installed", GREEN)
        } else if s.has_any_source() {
            ("Ready to Install", ACCENT_TEXT)
        } else {
            ("Binary Missing", ORANGE)
        }
    } else {
        ("Unknown", TEXT_TERTIARY)
    };

    let binary_sz = if plugin.status == PluginStatus::Planned {
        "Roadmap".to_string()
    } else {
        scan.and_then(|s| s.vst3.display_size().or(s.clap.display_size()))
            .map(format_file_size)
            .unwrap_or_else(|| "No local dist/ build".into())
    };

    egui::Frame::NONE
        .fill(FILL_1)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            fact_row(ui, "Status", status_txt, status_col);
            fact_row(ui, "Binary", &binary_sz, TEXT_PRIMARY);
            fact_row(ui, "Released", plugin.released, TEXT_PRIMARY);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Formats")
                        .font(FontId::proportional(12.5))
                        .color(TEXT_SECONDARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for &fmt in plugin.formats.iter().rev() {
                        let fit = active_host
                            .map(|h| h.formats.iter().any(|hf| hf.eq_ignore_ascii_case(fmt)))
                            .unwrap_or_else(|| host_supports_plugin(None, plugin));
                        let inst = scan.is_some_and(|s| match fmt {
                            "VST3" => s.vst3.status != VerificationStatus::NotInstalled,
                            "CLAP" => s.clap.status != VerificationStatus::NotInstalled,
                            _ => false,
                        });
                        egui::Frame::NONE
                            .fill(if fit { ACCENT_SOFT } else { FILL_2 })
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(7, 2))
                            .show(ui, |ui| {
                                // Parent is right-to-left: add the check first so it ends up
                                // after the label. A nested left-to-right layout here would
                                // stretch the chip across the row and stack chips on top of
                                // each other.
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if inst {
                                    show_icon(ui, IconName::Check, 10.5, GREEN);
                                }
                                ui.label(
                                    RichText::new(fmt)
                                        .font(FontId::proportional(11.0))
                                        .strong()
                                        .color(if fit { ACCENT_TEXT } else { TEXT_TERTIARY }),
                                );
                            });
                    }
                });
            });
            ui.add_space(4.0);

            if let Some(hash) = scan.and_then(|s| s.primary_hash()) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("SHA-256")
                            .font(FontId::proportional(12.5))
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let chip = icon_text_button(
                            ui,
                            IconName::Copy,
                            &short_fingerprint(hash),
                            TEXT_PRIMARY,
                        )
                        .on_hover_text(format!("Click to copy full SHA-256:\n{hash}"));
                        if chip.clicked() {
                            ev.copy_hash = Some(hash.to_string());
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
}

fn fact_row(ui: &mut Ui, key: &str, val: &str, val_color: Color32) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .font(FontId::proportional(12.5))
                .color(TEXT_SECONDARY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(val)
                    .font(FontId::proportional(12.5))
                    .color(val_color),
            );
        });
    });
    ui.add_space(2.0);
}

pub fn render_confirm_guard(ui: &mut Ui, conf: &PendingConfirm, ev: &mut InspectorEvents) {
    let verb = match conf.action {
        crate::ui_inspector::ConfirmAction::Install => {
            "Installing overwrites files it may have loaded."
        }
        crate::ui_inspector::ConfirmAction::Update => "Updating replaces files it may have loaded.",
        crate::ui_inspector::ConfirmAction::Remove => "Removing deletes files it may have loaded.",
    };
    egui::Frame::NONE
        .fill(ORANGE_SOFT)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                show_icon(ui, IconName::Alert, 16.0, ORANGE);
                ui.label(
                    RichText::new(format!(
                        "{} {} open",
                        conf.hosts.join(", "),
                        if conf.hosts.len() > 1 { "are" } else { "is" }
                    ))
                    .font(FontId::proportional(13.0))
                    .strong()
                    .color(TEXT_PRIMARY),
                );
            });
            ui.add_space(3.0);
            ui.label(
                RichText::new(format!(
                    "{verb} Close the DAW first, or continue and rescan afterwards."
                ))
                .font(FontId::proportional(12.0))
                .color(TEXT_SECONDARY),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Continue anyway", false).clicked() {
                    ev.confirm_continue = true;
                }
                if quiet_button(ui, "Not now").clicked() {
                    ev.confirm_cancel = true;
                }
            });
        });
}

pub fn render_actions(
    ui: &mut Ui,
    plugin: &'static PluginManifest,
    scan: Option<&InstallScanResult>,
    opts: InstallOptions,
    ev: &mut InspectorEvents,
) {
    let repo = format!("https://github.com/{DEFAULT_GITHUB_REPO}");
    if plugin.status == PluginStatus::Planned {
        ui.add_enabled_ui(false, |ui| {
            let _ = primary_button(ui, "Planned (Roadmap)", true);
        });
        ui.add_space(6.0);
        if icon_text_button(ui, IconName::Code, "View Source Repository", TEXT_PRIMARY).clicked() {
            ev.open_repo = Some(repo);
        }
        return;
    }

    let Some(s) = scan else { return };
    let can_install = s.has_any_source() && (opts.install_vst3 || opts.install_clap);
    let primary_label = if s.has_update() {
        "Update"
    } else if s.is_any_installed() {
        "Reinstall"
    } else {
        "Install"
    };
    ui.add_enabled_ui(can_install, |ui| {
        if primary_button(ui, primary_label, true).clicked() {
            if s.has_update() {
                ev.update_id = Some(plugin.id);
            } else {
                ev.install_id = Some(plugin.id);
            }
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if quiet_button(ui, "Verify")
            .on_hover_text("Recompute SHA-256 checksums on disk")
            .clicked()
        {
            ev.verify_id = Some(plugin.id);
        }
        if s.is_any_installed()
            && ui
                .add(
                    egui::Button::new(
                        RichText::new("Remove")
                            .font(FontId::proportional(12.5))
                            .strong()
                            .color(RED),
                    )
                    .fill(FILL_3)
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::same(8))
                    .min_size(Vec2::new(0.0, 30.0)),
                )
                .clicked()
        {
            ev.uninstall_id = Some(plugin.id);
        }
        if icon_text_button(ui, IconName::Code, "Source", TEXT_PRIMARY).clicked() {
            ev.open_repo = Some(repo);
        }
    });
}

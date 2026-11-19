//! Sleek Dark Obsidian plugin suite cards with SHA-256 previews and 1-Click Install controls.

use crate::discovery::InstallScanResult;
use crate::installer::InstallOptions;
use crate::integrity::{format_file_size, short_fingerprint, HashComparison, VerificationStatus};
use crate::manifest::{PluginManifest, PluginStatus};
use crate::theme::{
    draw_pill, primary_button, subtle_button, ACCENT_AMBER, ACCENT_AMBER_BG, ACCENT_CYAN,
    ACCENT_CYAN_BG, ACCENT_GREEN, ACCENT_GREEN_BG, ACCENT_RED, BG_CARD, BG_DISPLAY, BG_ELEVATED,
    BORDER_LIGHT, BORDER_SUBTLE, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Ui};

pub enum CardAction {
    Install(&'static str),
    Uninstall(&'static str),
    Verify(&'static str),
    CopyHash(String),
}

pub fn render_plugin_card(
    ui: &mut Ui,
    plugin: &'static PluginManifest,
    scan: &InstallScanResult,
    opts: InstallOptions,
) -> Option<CardAction> {
    let mut action = None;
    let is_planned = plugin.status == PluginStatus::Planned;
    let accent = Color32::from_rgb(
        plugin.accent_rgb[0],
        plugin.accent_rgb[1],
        plugin.accent_rgb[2],
    );
    let border = if scan.is_verified_match() {
        Color32::from_rgb(24, 92, 72)
    } else if !is_planned {
        BORDER_LIGHT
    } else {
        BORDER_SUBTLE
    };

    let card_frame = egui::Frame::NONE
        .fill(if is_planned {
            Color32::from_rgb(18, 20, 27)
        } else {
            BG_CARD
        })
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(16, 13));

    let resp = card_frame.show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Top row: identity + status badge
        ui.horizontal(|ui| {
            let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(
                dot_rect.center(),
                4.0,
                if is_planned { TEXT_DIMMED } else { accent },
            );
            ui.label(
                RichText::new(plugin.name)
                    .font(FontId::proportional(16.0))
                    .strong()
                    .color(if is_planned { TEXT_MUTED } else { TEXT_BRIGHT }),
            );
            draw_pill(
                ui,
                &format!("v{}", plugin.version),
                TEXT_MUTED,
                BG_ELEVATED,
                BORDER_SUBTLE,
            );
            draw_pill(
                ui,
                plugin.category,
                if is_planned { TEXT_DIMMED } else { accent },
                BG_DISPLAY,
                BORDER_SUBTLE,
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                render_status_badge(ui, plugin, scan);
            });
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new(plugin.description)
                .font(FontId::proportional(12.0))
                .color(if is_planned { TEXT_DIMMED } else { TEXT_MUTED }),
        );

        ui.add_space(10.0);

        // Bottom row: format badges + SHA-256 preview + action buttons
        ui.horizontal_wrapped(|ui| {
            render_format_chip(ui, "VST3", &scan.vst3, scan.source.vst3_binary.is_some());
            render_format_chip(ui, "CLAP", &scan.clap, scan.source.clap_file.is_some());

            if let Some(hash) = scan.primary_hash() {
                let chip = ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("SHA-256: {}", short_fingerprint(hash)))
                                .font(FontId::monospace(10.5))
                                .color(TEXT_MUTED),
                        )
                        .fill(BG_DISPLAY)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .corner_radius(CornerRadius::same(6)),
                    )
                    .on_hover_text(build_hash_tooltip(scan));
                if chip.clicked() {
                    action = Some(CardAction::CopyHash(hash.to_string()));
                }
            } else if !is_planned {
                draw_pill(
                    ui,
                    "No local dist/ binary found",
                    ACCENT_AMBER,
                    ACCENT_AMBER_BG,
                    ACCENT_AMBER,
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if is_planned {
                    ui.add_enabled(
                        false,
                        egui::Button::new(
                            RichText::new("Roadmap")
                                .font(FontId::proportional(11.5))
                                .color(TEXT_DIMMED),
                        )
                        .fill(BG_ELEVATED)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .corner_radius(CornerRadius::same(6))
                        .min_size(egui::vec2(94.0, 28.0)),
                    );
                    return;
                }

                // Uninstall button
                if scan.is_any_installed() {
                    if subtle_button(ui, "Uninstall", ACCENT_RED, Color32::from_rgb(120, 40, 48))
                        .clicked()
                    {
                        action = Some(CardAction::Uninstall(plugin.id));
                    }
                }

                // Verify button
                if subtle_button(ui, "Verify", TEXT_MUTED, BORDER_LIGHT)
                    .on_hover_text("Recompute SHA-256 checksums for source & installed binaries")
                    .clicked()
                {
                    action = Some(CardAction::Verify(plugin.id));
                }

                // 1-Click Install / Update / Reinstall button
                let (btn_label, btn_bg, btn_fg) = if scan.has_update() {
                    ("Update", ACCENT_AMBER, Color32::from_rgb(16, 17, 23))
                } else if scan.is_any_installed() {
                    ("Reinstall", ACCENT_CYAN_BG, ACCENT_CYAN)
                } else {
                    ("Install", ACCENT_CYAN, Color32::from_rgb(10, 14, 20))
                };

                let can_install = scan.has_any_source() && (opts.install_vst3 || opts.install_clap);
                let resp = ui.add_enabled_ui(can_install, |ui| {
                    primary_button(ui, btn_label, btn_bg, btn_fg)
                });
                if resp.inner.clicked() {
                    action = Some(CardAction::Install(plugin.id));
                }
            });
        });
    });

    // Subtle left accent bar inside card edge
    if !is_planned {
        let r = resp.response.rect;
        let bar = egui::Rect::from_min_max(
            egui::pos2(r.left() + 1.0, r.top() + 8.0),
            egui::pos2(r.left() + 4.0, r.bottom() - 8.0),
        );
        ui.painter().rect_filled(bar, CornerRadius::same(2), accent);
    }

    action
}

fn render_status_badge(ui: &mut Ui, plugin: &PluginManifest, scan: &InstallScanResult) {
    if plugin.status == PluginStatus::Planned {
        draw_pill(ui, "PLANNED", TEXT_DIMMED, BG_ELEVATED, BORDER_SUBTLE);
        return;
    }
    if scan.has_update() {
        draw_pill(
            ui,
            "UPDATE AVAILABLE",
            ACCENT_AMBER,
            ACCENT_AMBER_BG,
            ACCENT_AMBER,
        );
    } else if scan.is_verified_match() {
        draw_pill(
            ui,
            "INSTALLED • VERIFIED",
            ACCENT_GREEN,
            ACCENT_GREEN_BG,
            ACCENT_GREEN,
        );
    } else if scan.is_any_installed() {
        draw_pill(ui, "INSTALLED", ACCENT_GREEN, ACCENT_GREEN_BG, ACCENT_GREEN);
    } else if scan.has_any_source() {
        draw_pill(
            ui,
            "READY TO INSTALL",
            ACCENT_CYAN,
            ACCENT_CYAN_BG,
            ACCENT_CYAN,
        );
    } else {
        draw_pill(
            ui,
            "BINARY MISSING",
            ACCENT_AMBER,
            ACCENT_AMBER_BG,
            ACCENT_AMBER,
        );
    }
}

fn render_format_chip(ui: &mut Ui, name: &str, cmp: &HashComparison, has_source: bool) {
    let size_suffix = cmp
        .display_size()
        .map(|s| format!(" · {}", format_file_size(s)))
        .unwrap_or_default();
    let (fg, bg, border, label) = match cmp.status {
        VerificationStatus::VerifiedMatch => (
            ACCENT_GREEN,
            ACCENT_GREEN_BG,
            ACCENT_GREEN,
            format!("{name} ✓{size_suffix}"),
        ),
        VerificationStatus::UpdateAvailable => (
            ACCENT_AMBER,
            ACCENT_AMBER_BG,
            ACCENT_AMBER,
            format!("{name} ↑{size_suffix}"),
        ),
        VerificationStatus::InstalledLocalOnly => (
            ACCENT_GREEN,
            ACCENT_GREEN_BG,
            BORDER_LIGHT,
            format!("{name} ✓{size_suffix}"),
        ),
        VerificationStatus::NotInstalled if has_source => (
            ACCENT_CYAN,
            BG_DISPLAY,
            BORDER_LIGHT,
            format!("{name}{size_suffix}"),
        ),
        VerificationStatus::NotInstalled => {
            (TEXT_DIMMED, BG_DISPLAY, BORDER_SUBTLE, name.to_string())
        }
    };
    draw_pill(ui, &label, fg, bg, border);
}

fn build_hash_tooltip(scan: &InstallScanResult) -> String {
    let mut lines = vec!["Click to copy primary SHA-256 digest:".to_string()];
    for (fmt, cmp) in [("VST3", &scan.vst3), ("CLAP", &scan.clap)] {
        if let Some(h) = &cmp.source_hash {
            lines.push(format!("{fmt} Source   : {h}"));
        }
        if let Some(h) = &cmp.installed_hash {
            lines.push(format!("{fmt} Installed: {h}"));
        }
    }
    lines.join("\n")
}

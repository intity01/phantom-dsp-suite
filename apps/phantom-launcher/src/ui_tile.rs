//! Individual plugin tile rendering with procedural artwork, status line, and capsule CTA.

use crate::discovery::InstallScanResult;
use crate::hosts::HostInfo;
use crate::integrity::format_file_size;
use crate::manifest::{PluginManifest, PluginStatus};
use crate::theme::{
    capsule_button, ACCENT, ACCENT_TEXT, BG_CARD, BG_CARD_HOVER, BG_CARD_SELECTED, GREEN, ORANGE,
    TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};
use crate::ui_art::ArtCache;
use crate::ui_grid::GridEvents;
use crate::ui_icons::{icon_widget, IconName};
use eframe::egui::{
    self, CornerRadius, FontId, Margin, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};

#[allow(clippy::too_many_arguments)]
pub fn render_tile(
    ui: &mut Ui,
    plugin: &'static PluginManifest,
    scan: Option<&InstallScanResult>,
    active_host: Option<&HostInfo>,
    fits: bool,
    selected: bool,
    card_w: f32,
    art_cache: &ArtCache,
    ev: &mut GridEvents,
) {
    let is_planned = plugin.status == PluginStatus::Planned;
    let id = ui.make_persistent_id(("tile_box", plugin.id));
    let prev_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    let bg = if selected {
        BG_CARD_SELECTED
    } else if prev_hovered {
        BG_CARD_HOVER
    } else {
        BG_CARD
    };

    let frame = egui::Frame::NONE
        .fill(bg)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            let inner_w = card_w - 16.0;
            ui.vertical(|ui| {
                ui.set_width(inner_w);
                let ver_label = if is_planned {
                    format!("v{}", plugin.version.trim_end_matches("-planned"))
                } else {
                    format!("v{}", plugin.version)
                };

                let top_scope = ui.scope(|ui| {
                    art_cache.draw_art(
                        ui,
                        plugin.id,
                        Vec2::new(inner_w, 124.0),
                        11,
                        !fits || is_planned,
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.set_width(inner_w - 12.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(plugin.name)
                                        .font(FontId::proportional(15.5))
                                        .strong()
                                        .color(if fits && !is_planned {
                                            TEXT_PRIMARY
                                        } else {
                                            TEXT_SECONDARY
                                        }),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(ver_label)
                                                .font(FontId::proportional(11.5))
                                                .color(TEXT_TERTIARY),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                RichText::new(plugin.kind)
                                    .font(FontId::proportional(12.5))
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(4.0);
                            tile_status_line(ui, plugin, scan, active_host, fits);
                        });
                    });
                });
                if ui
                    .interact(top_scope.response.rect, id.with("top_hit"), Sense::click())
                    .clicked()
                {
                    ev.select_id = Some(plugin.id);
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.set_width(inner_w);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        tile_action_btn(ui, plugin, scan, ev);
                        let rest_w = ui.available_width();
                        if rest_w > 8.0 {
                            let (_, blank_resp) =
                                ui.allocate_exact_size(Vec2::new(rest_w, 28.0), Sense::click());
                            if blank_resp.clicked() {
                                ev.select_id = Some(plugin.id);
                            }
                        }
                    });
                });
                ui.add_space(2.0);
            });
        });

    let hovered = ui.rect_contains_pointer(frame.response.rect);
    ui.ctx().data_mut(|d| d.insert_temp(id, hovered));
    if selected {
        ui.painter().rect_stroke(
            frame.response.rect,
            CornerRadius::same(16),
            Stroke::new(2.0_f32, ACCENT),
            StrokeKind::Inside,
        );
    }
}

fn tile_status_line(
    ui: &mut Ui,
    plugin: &PluginManifest,
    scan: Option<&InstallScanResult>,
    active_host: Option<&HostInfo>,
    fits: bool,
) {
    ui.horizontal_wrapped(|ui| {
        if plugin.status == PluginStatus::Planned {
            ui.label(
                RichText::new("Planned · Roadmap")
                    .font(FontId::proportional(11.5))
                    .color(TEXT_TERTIARY),
            );
        } else if let Some(s) = scan {
            if s.has_update() {
                ui.label(
                    RichText::new("Update available")
                        .font(FontId::proportional(11.5))
                        .color(ACCENT_TEXT),
                );
            } else if s.is_any_installed() {
                icon_widget(ui, IconName::Check, 12.0, GREEN);
                ui.label(
                    RichText::new("Installed")
                        .font(FontId::proportional(11.5))
                        .color(GREEN),
                );
            } else if let Some(sz) = s.vst3.display_size().or(s.clap.display_size()) {
                ui.label(
                    RichText::new(format_file_size(sz))
                        .font(FontId::proportional(11.5))
                        .color(TEXT_SECONDARY),
                );
            } else {
                ui.label(
                    RichText::new("No local dist/ build")
                        .font(FontId::proportional(11.5))
                        .color(ORANGE),
                );
            }
        }
        if !fits {
            if let Some(h) = active_host {
                ui.label(
                    RichText::new(format!("Not for {}", h.name))
                        .font(FontId::proportional(11.5))
                        .color(ORANGE),
                );
            }
        }
    });
}

fn tile_action_btn(
    ui: &mut Ui,
    plugin: &'static PluginManifest,
    scan: Option<&InstallScanResult>,
    ev: &mut GridEvents,
) {
    if plugin.status == PluginStatus::Planned {
        ui.add_enabled_ui(false, |ui| {
            let _ = capsule_button(ui, "Roadmap", false);
        });
        return;
    }
    let Some(s) = scan else { return };
    if s.has_update() {
        if capsule_button(ui, "Update", true).clicked() {
            ev.update_id = Some(plugin.id);
        }
    } else if s.is_any_installed() {
        if capsule_button(ui, "Remove", false).clicked() {
            ev.uninstall_id = Some(plugin.id);
        }
    } else {
        ui.add_enabled_ui(s.has_any_source(), |ui| {
            if capsule_button(ui, "Install", true).clicked() {
                ev.install_id = Some(plugin.id);
            }
        });
    }
}

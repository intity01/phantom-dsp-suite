//! Center Browse view: Target DAW segmented bar, responsive card grid, and drop hint.

use crate::discovery::InstallScanResult;
use crate::hosts::{host_supports_plugin, HostInfo};
use crate::integrity::format_file_size;
use crate::manifest::{PluginManifest, PluginStatus};
use crate::theme::{FILL_1, FILL_3, GREEN, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY};
use crate::ui_art::ArtCache;
use crate::ui_icons::{draw_icon, icon_widget, IconName};
use crate::ui_sidebar::BrowseScope;
use crate::ui_tile::render_tile;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, Pos2, RichText, Sense, Stroke, Ui, Vec2,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct GridEvents {
    pub select_id: Option<&'static str>,
    pub install_id: Option<&'static str>,
    pub update_id: Option<&'static str>,
    pub uninstall_id: Option<&'static str>,
    pub select_host: Option<Option<&'static str>>,
    pub rescan_hosts: bool,
    pub cols: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn render_browse_grid(
    ui: &mut Ui,
    scope: BrowseScope,
    visible: &[&'static PluginManifest],
    all_plugins: &[PluginManifest],
    scans: &HashMap<&'static str, InstallScanResult>,
    hosts: &[HostInfo],
    target_host_id: Option<&'static str>,
    selected_id: Option<&'static str>,
    art_cache: &ArtCache,
) -> GridEvents {
    let mut ev = GridEvents::default();
    let active_host =
        target_host_id.and_then(|id| hosts.iter().find(|h| h.id == id && h.installed));

    let total_bytes: u64 = all_plugins
        .iter()
        .filter_map(|p| scans.get(p.id))
        .map(|s| s.vst3.installed_size.unwrap_or(0) + s.clap.installed_size.unwrap_or(0))
        .sum();

    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(scope.title())
                    .font(FontId::proportional(25.0))
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            let planned = visible
                .iter()
                .filter(|p| p.status == PluginStatus::Planned)
                .count();
            let ready = visible.len() - planned;
            let mix = if planned == 0 {
                format!("{ready} available")
            } else {
                format!("{ready} available · {planned} coming soon")
            };
            ui.label(
                RichText::new(format!(
                    "{mix} · {} installed on disk",
                    format_file_size(total_bytes)
                ))
                .font(FontId::proportional(12.5))
                .color(TEXT_SECONDARY),
            );
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (sync_r, sync_resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
            if sync_resp.hovered() {
                ui.painter()
                    .rect_filled(sync_r, CornerRadius::same(7), FILL_1);
            }
            draw_icon(
                ui.painter(),
                sync_r.center(),
                15.0,
                IconName::Sync,
                if sync_resp.hovered() {
                    TEXT_PRIMARY
                } else {
                    TEXT_SECONDARY
                },
            );
            if sync_resp
                .on_hover_text("Detect installed & running DAWs again")
                .clicked()
            {
                ev.rescan_hosts = true;
            }

            let detected: Vec<&HostInfo> = hosts.iter().filter(|h| h.installed).collect();
            egui::Frame::NONE
                .fill(FILL_1)
                .corner_radius(CornerRadius::same(9))
                .inner_margin(Margin::same(2))
                .show(ui, |ui| {
                    // Parent is right-to-left, so add segments in reverse to read
                    // "All · DAW 1 · DAW 2" and keep the bar sized to its content.
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for h in detected.into_iter().rev() {
                        let compat = all_plugins
                            .iter()
                            .filter(|p| {
                                p.status == PluginStatus::Ready && host_supports_plugin(Some(h), p)
                            })
                            .count();
                        let have = all_plugins
                            .iter()
                            .filter(|p| {
                                host_supports_plugin(Some(h), p)
                                    && scans.get(p.id).is_some_and(|s| s.is_any_installed())
                            })
                            .count();
                        let is_on = target_host_id == Some(h.id);
                        if daw_seg_btn(ui, h.name, Some((have, compat)), is_on, h.running) {
                            ev.select_host = Some(Some(h.id));
                        }
                    }
                    if daw_seg_btn(ui, "All", None, target_host_id.is_none(), false) {
                        ev.select_host = Some(None);
                    }
                });
            ui.label(
                RichText::new("Target DAW")
                    .font(FontId::proportional(12.0))
                    .color(TEXT_SECONDARY),
            );
        });
    });

    ui.add_space(16.0);
    let avail_w = ui.available_width();
    let gap = 16.0_f32;
    let cols = (((avail_w + gap) / (230.0 + gap)).floor() as usize).clamp(1, 4);
    ev.cols = cols;
    let card_w = ((avail_w - gap * (cols as f32 - 1.0)) / cols as f32).max(180.0);

    egui::ScrollArea::vertical()
        .id_salt("browse_grid_scroll")
        .show(ui, |ui| {
            if visible.is_empty() {
                ui.add_space(56.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No plugins match this view.")
                            .font(FontId::proportional(14.0))
                            .color(TEXT_SECONDARY),
                    );
                });
            } else {
                for row in visible.chunks(cols) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for &plugin in row {
                            let scan = scans.get(plugin.id);
                            let fits = host_supports_plugin(active_host, plugin);
                            let is_sel = selected_id == Some(plugin.id);
                            ui.allocate_ui_with_layout(
                                Vec2::new(card_w, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.set_width(card_w);
                                    render_tile(
                                        ui,
                                        plugin,
                                        scan,
                                        active_host,
                                        fits,
                                        is_sel,
                                        card_w,
                                        art_cache,
                                        &mut ev,
                                    );
                                },
                            );
                        }
                    });
                    ui.add_space(gap);
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                icon_widget(ui, IconName::Zip, 15.0, TEXT_TERTIARY);
                ui.label(
                    RichText::new(
                        "Have a plugin .zip already? Drop it anywhere on this window to install offline.",
                    )
                    .font(FontId::proportional(12.0))
                    .color(TEXT_TERTIARY),
                );
            });
        });

    ev
}

fn daw_seg_btn(
    ui: &mut Ui,
    name: &str,
    counts: Option<(usize, usize)>,
    active: bool,
    running: bool,
) -> bool {
    let label = match counts {
        Some((h, t)) => format!("{name}  {h}/{t}"),
        None => name.to_string(),
    };
    let tip = match counts {
        Some((h, t)) => format!(
            "{h} of {t} available plugins for {name} are installed{}",
            if running { " · open now" } else { "" }
        ),
        None => "Show plugins for every DAW".to_string(),
    };
    let resp = ui
        .add(
            egui::Button::new(
                RichText::new(label)
                    .font(FontId::proportional(12.0))
                    .color(if active { TEXT_PRIMARY } else { TEXT_SECONDARY }),
            )
            .fill(if active { FILL_3 } else { Color32::TRANSPARENT })
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(7))
            .min_size(Vec2::new(0.0, 26.0)),
        )
        .on_hover_text(tip);
    if running {
        let dot = Pos2::new(resp.rect.left() + 5.0, resp.rect.center().y);
        ui.painter().circle_filled(dot, 2.8, GREEN);
    }
    resp.clicked()
}

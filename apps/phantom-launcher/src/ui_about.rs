//! About center view featuring the brand sheet, icon scale showcase, and build info.

use crate::brand::BrandAssets;
use crate::paths::host_dist_os_dir;
use crate::theme::{ACCENT, ACCENT_SOFT, FILL_1, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY};
use crate::ui_settings::{group_head, setting_row};
use eframe::egui::{self, CornerRadius, FontId, Margin, RichText, Ui};

pub fn render_about_page(ui: &mut Ui, brand: &BrandAssets) {
    egui::ScrollArea::vertical()
        .id_salt("about_scroll")
        .show(ui, |ui| {
            ui.set_max_width(720.0);
            ui.label(
                RichText::new("About")
                    .font(FontId::proportional(25.0))
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.add_space(16.0);

            egui::Frame::NONE
                .fill(ACCENT_SOFT)
                .corner_radius(CornerRadius::same(20))
                .inner_margin(Margin::same(26))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        brand.draw_logo_icon(ui, 132.0);
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            brand.draw_wordmark(ui, 186.0, TEXT_PRIMARY, ACCENT);
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("dsp suite · v0.1.0 · MIT / Apache-2.0")
                                    .font(FontId::proportional(12.5))
                                    .color(TEXT_TERTIARY),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(
                                    "A glass orb, the phantom, floats above its own shadow and carries a sound wave drawn in the same round, single-weight stroke as the wordmark. The orbit ring and its satellite are the signal moving through your plugin chain.",
                                )
                                .font(FontId::proportional(13.5))
                                .color(TEXT_SECONDARY),
                            );
                        });
                    });
                });

            group_head(ui, "Icon sizes", None);
            egui::Frame::NONE
                .fill(FILL_1)
                .corner_radius(CornerRadius::same(16))
                .inner_margin(Margin::same(20))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 28.0;
                        for sz in [128u32, 64, 32, 16] {
                            ui.vertical(|ui| {
                                brand.draw_logo_icon(ui, sz as f32);
                                ui.label(
                                    RichText::new(format!("{sz}px"))
                                        .font(FontId::proportional(11.5))
                                        .color(TEXT_TERTIARY),
                                );
                            });
                        }
                    });
                });

            group_head(ui, "This build", None);
            let facts = [
                ("Engine", "Pure Rust · eframe / egui (Native, Zero WebView)"),
                ("Platform", host_dist_os_dir()),
                ("Security", "100% Offline · Streaming 64 KiB SHA-256"),
                ("Shortcuts", "Ctrl+K or / search · Arrow keys move · Esc clears"),
            ];
            for (k, v) in facts {
                setting_row(ui, |ui| {
                    ui.label(
                        RichText::new(k)
                            .font(FontId::proportional(13.0))
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(v)
                                .font(FontId::proportional(13.0))
                                .color(TEXT_PRIMARY),
                        );
                    });
                });
            }
            ui.add_space(16.0);
        });
}

//! Foreground drag-and-drop overlay and floating toast notification capsule.

use crate::theme::{ACCENT, ACCENT_TEXT, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};
use crate::ui_icons::{icon_widget, IconName};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Margin, Order, RichText, Stroke,
};

pub fn render_drop_overlay(ctx: &egui::Context) {
    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("drop_overlay"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            ui.painter().rect_filled(
                screen,
                CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(10, 10, 12, 165),
            );
            ui.centered_and_justified(|ui| {
                egui::Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(40, 40, 44, 235))
                    .stroke(Stroke::new(3.0_f32, ACCENT))
                    .corner_radius(CornerRadius::same(22))
                    .inner_margin(Margin::same(36))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            icon_widget(ui, IconName::Zip, 34.0, ACCENT_TEXT);
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Drop to install")
                                    .font(FontId::proportional(20.0))
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(
                                    "Verified via SHA-256 & zip-slip guard before files are copied.",
                                )
                                .font(FontId::proportional(13.0))
                                .color(TEXT_SECONDARY),
                            );
                        });
                    });
            });
        });
}

pub fn render_toast(ctx: &egui::Context, msg: &str) {
    egui::Area::new(Id::new("toast_overlay"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -22.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(48, 48, 54, 240))
                .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(msg)
                            .font(FontId::proportional(13.0))
                            .color(TEXT_PRIMARY),
                    );
                });
        });
}

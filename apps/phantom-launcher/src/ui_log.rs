//! Activity & SHA-256 Verification Log console view inside the center surface.

use crate::theme::{
    quiet_button, BORDER_SUBTLE, FILL_1, GREEN, ORANGE, RED, TEXT_PRIMARY, TEXT_SECONDARY,
    TEXT_TERTIARY,
};
use eframe::egui::{self, CornerRadius, FontId, Margin, RichText, Stroke};

#[derive(Clone, Copy)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

pub fn render_activity_log_page(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    logs: &mut Vec<LogEntry>,
) -> Option<String> {
    let mut toast_msg = None;

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Activity & SHA-256 Log")
                    .font(FontId::proportional(24.0))
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.label(
                RichText::new(format!(
                    "{} recorded events · 100% offline verification history",
                    logs.len()
                ))
                .font(FontId::proportional(13.0))
                .color(TEXT_SECONDARY),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if quiet_button(ui, "Clear").clicked() {
                logs.clear();
            }
            if quiet_button(ui, "Copy Log").clicked() {
                let dump = logs
                    .iter()
                    .map(|e| format!("[{}] {}", e.time, e.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.copy_text(dump);
                toast_msg = Some("Copied activity log to clipboard".into());
            }
        });
    });

    ui.add_space(16.0);
    egui::Frame::NONE
        .fill(FILL_1)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("activity_log_page_scroll")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if logs.is_empty() {
                        ui.label(
                            RichText::new("No log entries recorded yet.")
                                .font(FontId::proportional(13.0))
                                .color(TEXT_TERTIARY),
                        );
                    }
                    for entry in logs.iter() {
                        let color = match entry.level {
                            LogLevel::Info => TEXT_SECONDARY,
                            LogLevel::Success => GREEN,
                            LogLevel::Warn => ORANGE,
                            LogLevel::Error => RED,
                        };
                        ui.label(
                            RichText::new(format!("[{}] {}", entry.time, entry.message))
                                .font(FontId::monospace(11.5))
                                .color(color),
                        );
                        ui.add_space(3.0);
                    }
                });
        });

    toast_msg
}

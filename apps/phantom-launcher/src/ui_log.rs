//! Timestamped Activity & SHA-256 Verification Log console panel.

use crate::theme::{ACCENT_AMBER, ACCENT_GREEN, ACCENT_RED, BG_DISPLAY, BORDER_SUBTLE, TEXT_MUTED};
use eframe::egui::{self, FontId, Margin, RichText, Stroke};

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

pub fn render_activity_log(ctx: &egui::Context, ui: &mut egui::Ui, logs: &mut Vec<LogEntry>) {
    egui::Frame::NONE
        .fill(BG_DISPLAY)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("ACTIVITY & SHA-256 VERIFICATION LOG")
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(TEXT_MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Clear").clicked() {
                        logs.clear();
                    }
                    if ui.small_button("Copy Log").clicked() {
                        let dump = logs
                            .iter()
                            .map(|e| format!("[{}] {}", e.time, e.message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        ctx.copy_text(dump);
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("activity_log_scroll")
                .max_height(125.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for entry in logs.iter() {
                        let color = match entry.level {
                            LogLevel::Info => TEXT_MUTED,
                            LogLevel::Success => ACCENT_GREEN,
                            LogLevel::Warn => ACCENT_AMBER,
                            LogLevel::Error => ACCENT_RED,
                        };
                        ui.label(
                            RichText::new(format!("[{}] {}", entry.time, entry.message))
                                .font(FontId::monospace(11.0))
                                .color(color),
                        );
                    }
                });
        });
}

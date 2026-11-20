//! Top 52px titlebar with Ctrl+K / `/` search pill, offline catalog indicator, and quick actions.

use crate::ui_sidebar::AppView;
use eframe::egui::{
    self, CornerRadius, FontId, Id, Margin, RichText, Sense, Stroke, TextEdit, Ui, Vec2,
};
use phantom_kit::prelude::{icon_button, icon_text_button, show_icon, IconName};
use phantom_kit::prelude::{
    ACCENT, FILL_2, FILL_3, GREEN, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};

pub struct ToolbarEvents {
    pub rescan: bool,
    pub open_releases: bool,
    pub search_id: Id,
}

pub fn render_toolbar(
    ui: &mut Ui,
    query: &mut String,
    view: &mut AppView,
    has_local_source: bool,
) -> ToolbarEvents {
    let mut rescan = false;
    let mut open_releases = false;
    let search_id = ui.make_persistent_id("phantom_search_input");

    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let has_focus = ui.ctx().memory(|m| m.has_focus(search_id));
        let pill_w = (ui.available_width() - 370.0).clamp(180.0, 420.0);

        egui::Frame::NONE
            .fill(if has_focus { FILL_3 } else { FILL_2 })
            .stroke(if has_focus {
                Stroke::new(1.5_f32, ACCENT)
            } else {
                Stroke::NONE
            })
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.set_width(pill_w);
                ui.set_height(22.0);
                ui.horizontal(|ui| {
                    show_icon(
                        ui,
                        IconName::Search,
                        15.0,
                        if has_focus {
                            TEXT_SECONDARY
                        } else {
                            TEXT_TERTIARY
                        },
                    );
                    let input_w = (pill_w - 76.0).max(80.0);
                    let resp = ui.add(
                        TextEdit::singleline(query)
                            .id(search_id)
                            .frame(false)
                            .desired_width(input_w)
                            .hint_text(
                                RichText::new("Search plugins")
                                    .font(FontId::proportional(13.0))
                                    .color(TEXT_TERTIARY),
                            )
                            .font(FontId::proportional(13.0))
                            .text_color(TEXT_PRIMARY),
                    );
                    if resp.changed() {
                        *view = AppView::Browse;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if query.is_empty() {
                            let shortcut = if cfg!(target_os = "macos") {
                                "Cmd K"
                            } else {
                                "Ctrl K"
                            };
                            egui::Frame::NONE
                                .fill(FILL_2)
                                .corner_radius(CornerRadius::same(5))
                                .inner_margin(Margin::symmetric(6, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(shortcut)
                                            .font(FontId::proportional(10.5))
                                            .color(TEXT_TERTIARY),
                                    );
                                });
                        } else {
                            if icon_button(ui, IconName::X, 13.0, TEXT_SECONDARY)
                                .on_hover_text("Clear search")
                                .clicked()
                            {
                                query.clear();
                            }
                        }
                    });
                });
            });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if icon_text_button(ui, IconName::Code, "Releases", TEXT_PRIMARY)
                .on_hover_text("Open GitHub Releases in browser (manual offline check)")
                .clicked()
            {
                open_releases = true;
            }
            if icon_text_button(ui, IconName::Sync, "Rescan", TEXT_PRIMARY)
                .on_hover_text("Rescan dist/ binaries, installed plugins, and running DAWs")
                .clicked()
            {
                rescan = true;
            }
            ui.add_space(6.0);
            let status_txt = if has_local_source {
                "Offline · Local dist/ catalog"
            } else {
                "Offline · No local dist/ build"
            };
            ui.label(
                RichText::new(status_txt)
                    .font(FontId::proportional(12.0))
                    .color(TEXT_SECONDARY),
            );
            let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            let dot_col = if has_local_source {
                GREEN
            } else {
                TEXT_TERTIARY
            };
            ui.painter().circle_filled(dot_rect.center(), 3.5, dot_col);
        });
    });

    ToolbarEvents {
        rescan,
        open_releases,
        search_id,
    }
}

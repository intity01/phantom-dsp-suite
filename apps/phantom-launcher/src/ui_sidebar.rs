//! Left macOS-inspired blended navigation sidebar with brand mark, scopes, and privacy badge.

use crate::brand::{draw_wordmark, BrandAssets};
use crate::theme::{
    ACCENT, ACCENT_TEXT, BG_WINDOW, FILL_1, FILL_3, GREEN, TEXT_PRIMARY, TEXT_SECONDARY,
    TEXT_TERTIARY,
};
use crate::ui_icons::{draw_icon, icon_widget, IconName};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Sense, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Browse,
    Log,
    Settings,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseScope {
    All,
    Equalizer,
    Dynamics,
    Mastering,
    Installed,
    Updates,
}

impl BrowseScope {
    pub fn title(self) -> &'static str {
        match self {
            Self::All => "All plugins",
            Self::Equalizer => "Equalizer",
            Self::Dynamics => "Dynamics",
            Self::Mastering => "Mastering",
            Self::Installed => "Installed",
            Self::Updates => "Updates",
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct SidebarCounts {
    pub all: usize,
    pub eq: usize,
    pub dynamics: usize,
    pub mastering: usize,
    pub installed: usize,
    pub updates: usize,
    pub logs: usize,
}

pub fn render_sidebar(
    ui: &mut Ui,
    brand: &BrandAssets,
    view: &mut AppView,
    scope: &mut BrowseScope,
    counts: &SidebarCounts,
) {
    ui.painter()
        .rect_filled(ui.max_rect(), CornerRadius::ZERO, BG_WINDOW);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        brand.draw_logo_icon(ui, 36.0);
        ui.add_space(4.0);
        draw_wordmark(ui, 94.0, TEXT_PRIMARY, ACCENT);
    });
    ui.add_space(12.0);

    section_label(ui, "Browse");
    let browse_items = [
        (BrowseScope::All, "All plugins", IconName::Grid, counts.all),
        (
            BrowseScope::Equalizer,
            "Equalizer",
            IconName::Wave,
            counts.eq,
        ),
        (
            BrowseScope::Dynamics,
            "Dynamics",
            IconName::Meter,
            counts.dynamics,
        ),
        (
            BrowseScope::Mastering,
            "Mastering",
            IconName::Keys,
            counts.mastering,
        ),
    ];
    for (sc, label, icon, count) in browse_items {
        let active = *view == AppView::Browse && *scope == sc;
        if nav_row(ui, icon, label, Some(count), false, active) {
            *view = AppView::Browse;
            *scope = sc;
        }
    }

    ui.add_space(12.0);
    section_label(ui, "This machine");
    if nav_row(
        ui,
        IconName::Disk,
        "Installed",
        Some(counts.installed),
        false,
        *view == AppView::Browse && *scope == BrowseScope::Installed,
    ) {
        *view = AppView::Browse;
        *scope = BrowseScope::Installed;
    }
    if nav_row(
        ui,
        IconName::Up,
        "Updates",
        Some(counts.updates),
        counts.updates > 0,
        *view == AppView::Browse && *scope == BrowseScope::Updates,
    ) {
        *view = AppView::Browse;
        *scope = BrowseScope::Updates;
    }
    if nav_row(
        ui,
        IconName::Terminal,
        "Activity Log",
        Some(counts.logs),
        false,
        *view == AppView::Log,
    ) {
        *view = AppView::Log;
    }

    let bottom_h = 122.0_f32;
    let gap = (ui.available_height() - bottom_h).max(16.0);
    ui.add_space(gap);

    if nav_row(
        ui,
        IconName::Settings,
        "Settings",
        None,
        false,
        *view == AppView::Settings,
    ) {
        *view = AppView::Settings;
    }
    if nav_row(
        ui,
        IconName::Info,
        "About",
        None,
        false,
        *view == AppView::About,
    ) {
        *view = AppView::About;
    }

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        icon_widget(ui, IconName::Shield, 15.0, GREEN);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                egui::RichText::new("Offline & open source.")
                    .font(FontId::proportional(11.5))
                    .color(TEXT_TERTIARY),
            );
            ui.label(
                egui::RichText::new("No account, no telemetry.")
                    .font(FontId::proportional(11.5))
                    .color(TEXT_TERTIARY),
            );
        });
    });
}

fn section_label(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(text)
                .font(FontId::proportional(11.5))
                .strong()
                .color(TEXT_TERTIARY),
        );
    });
    ui.add_space(2.0);
}

fn nav_row(
    ui: &mut Ui,
    icon: IconName,
    label: &str,
    badge: Option<usize>,
    hot_badge: bool,
    active: bool,
) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if active {
            p.rect_filled(rect, CornerRadius::same(8), FILL_3);
        } else if resp.hovered() {
            p.rect_filled(rect, CornerRadius::same(8), FILL_1);
        }
        draw_icon(
            p,
            Pos2::new(rect.left() + 18.0, rect.center().y),
            16.5,
            icon,
            ACCENT_TEXT,
        );
        p.text(
            Pos2::new(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.5),
            TEXT_PRIMARY,
        );
        if let Some(count) = badge {
            let txt = count.to_string();
            let right_x = rect.right() - 10.0;
            if hot_badge {
                let bw = (txt.len() as f32 * 7.5 + 12.0).max(22.0);
                let brect = egui::Rect::from_center_size(
                    Pos2::new(right_x - bw * 0.5, rect.center().y),
                    Vec2::new(bw, 18.0),
                );
                p.rect_filled(brect, CornerRadius::same(9), ACCENT);
                p.text(
                    brect.center(),
                    Align2::CENTER_CENTER,
                    txt,
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            } else {
                p.text(
                    Pos2::new(right_x, rect.center().y),
                    Align2::RIGHT_CENTER,
                    txt,
                    FontId::proportional(12.0),
                    if active {
                        TEXT_SECONDARY
                    } else {
                        TEXT_TERTIARY
                    },
                );
            }
        }
    }
    resp.clicked()
}

//! Slim Bottom Status & Toggle Bar for Phantom Vox
//!
//! Layout: `Sib-Safe` toggle | `Mono Check` toggle | Guide Sidechain status hint
//! (`Sidechain Connected` / `No Sidechain -> Doubler Active`) | `PHANTOM DSP`.

use crate::live_mirror::LiveSetter;
use crate::params::PhantomVoxParams;
use crate::telemetry::VoxTelemetrySnapshot;
use nih_plug_egui::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use phantom_ui::{
    with_alpha, ACCENT_AMBER, ACCENT_CYAN, ACCENT_GREEN, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED,
    TEXT_MUTED,
};

fn toggle_pill(ui: &mut Ui, label: &str, active: bool, accent: Color32, w: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    let hov = resp.hovered();
    let fill = if active {
        with_alpha(accent, 42)
    } else if hov {
        Color32::from_rgba_unmultiplied(255, 255, 255, 20)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 9)
    };
    let border = if active {
        with_alpha(accent, 180)
    } else if hov {
        with_alpha(accent, 100)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    };
    let fg = if active {
        accent
    } else if hov {
        TEXT_BRIGHT
    } else {
        TEXT_MUTED
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.2),
        fg,
    );
    resp.clicked()
}

pub fn draw_bottom_bar(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomVoxParams,
    snap: &VoxTelemetrySnapshot,
) {
    let sib_safe = setter.read(&params.sibilance_safe);
    let mono_check = setter.read(&params.mono_check);

    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        ))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.horizontal(|ui| {
                if toggle_pill(ui, "Sib-Safe", sib_safe, ACCENT_CYAN, 64.0) {
                    setter.set_once(&params.sibilance_safe, !sib_safe);
                }
                if toggle_pill(ui, "Mono Check", mono_check, ACCENT_AMBER, 78.0) {
                    setter.set_once(&params.mono_check, !mono_check);
                }

                ui.separator();

                let (dot_col, hint) = if snap.sidechain_active {
                    (ACCENT_GREEN, "Sidechain Connected")
                } else {
                    (ACCENT_AMBER, "No Sidechain -> Doubler Active")
                };
                let (dot_r, _) = ui.allocate_exact_size(Vec2::new(8.0, 12.0), Sense::hover());
                ui.painter().circle_filled(dot_r.center(), 3.2, dot_col);
                ui.label(RichText::new(hint).font(FontId::proportional(9.5)).color(
                    if snap.sidechain_active {
                        TEXT_BRIGHT
                    } else {
                        TEXT_MUTED
                    },
                ));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("PHANTOM DSP")
                            .font(FontId::proportional(9.2))
                            .strong()
                            .color(TEXT_DIMMED),
                    );
                });
            });
        });
}

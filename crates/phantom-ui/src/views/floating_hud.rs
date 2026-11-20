//! Pro-Q 4 Floating Band Popover HUD
//! Compact dark-glass floating card (`368 x 98/114 px`) positioned automatically near the selected
//! EQ node with 4 precision Rotary Knobs, Headphone Solo Audition, Shape/Slope/Routing pills,
//! Spectral Dynamics toggle, and Dynamic EQ ballistics micro-strip.

use crate::theme::{band_color, with_alpha};
use crate::views::curve_plot::EqCanvasMapping;
use crate::views::floating_hud_parts::{draw_dyn_micro_strip, draw_top_strip};
use crate::widgets::band_inspector::{draw_hud_knobs_row, BandInspectorData, BandInspectorEvents};
use nih_plug_egui::egui::{
    Align, Color32, CornerRadius, Layout, Pos2, Rect, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};

pub const FLOATING_HUD_WIDTH: f32 = 368.0;
pub const FLOATING_HUD_HEIGHT: f32 = 98.0;
const HUD_NODE_GAP_PX: f32 = 20.0;

/// Renders the Pro-Q 4 Floating Band HUD Popover near `node_pos`, clamped inside `map.rect`.
/// Returns `(BandInspectorEvents, hud_rect)` so canvas interaction ignores clicks inside the HUD.
pub fn draw_floating_band_hud(
    ui: &mut Ui,
    map: &EqCanvasMapping,
    node_pos: Pos2,
    data: &BandInspectorData,
    filter_names: &[&'static str],
    slope_names: &[&'static str],
    routing_names: &[&'static str],
) -> (BandInspectorEvents, Rect) {
    let mut events = BandInspectorEvents::default();
    let accent = band_color(data.selected_idx);
    let has_dyn = data.dyn_range.abs() > 0.05;
    let hud_h = if has_dyn { 114.0 } else { FLOATING_HUD_HEIGHT };

    let min_x = map.rect.min.x + 8.0;
    let max_x = (map.rect.max.x - FLOATING_HUD_WIDTH - 8.0).max(min_x);
    let hud_x = (node_pos.x - FLOATING_HUD_WIDTH * 0.5).clamp(min_x, max_x);
    let min_y = map.rect.min.y + 6.0;
    let max_y = (map.rect.max.y - hud_h - 6.0).max(min_y);
    let below = (node_pos.y - hud_h - HUD_NODE_GAP_PX) < min_y;
    let raw_y = if below {
        node_pos.y + HUD_NODE_GAP_PX
    } else {
        node_pos.y - hud_h - HUD_NODE_GAP_PX
    };
    let hud_rect = Rect::from_min_size(
        Pos2::new(hud_x, raw_y.clamp(min_y, max_y)),
        Vec2::new(FLOATING_HUD_WIDTH, hud_h),
    );

    let p = ui.painter();
    let ay = if below {
        hud_rect.min.y
    } else {
        hud_rect.max.y
    };
    let ax = node_pos
        .x
        .clamp(hud_rect.min.x + 16.0, hud_rect.max.x - 16.0);
    let r8 = CornerRadius::same(8);
    p.line_segment(
        [node_pos, Pos2::new(ax, ay)],
        Stroke::new(1.2_f32, with_alpha(accent, 95)),
    );
    p.rect_filled(
        hud_rect.translate(Vec2::new(0.0, 3.0)),
        r8,
        Color32::from_rgba_unmultiplied(0, 0, 0, 135),
    );
    p.rect_filled(
        hud_rect,
        r8,
        Color32::from_rgba_unmultiplied(18, 20, 29, 244),
    );
    p.rect_stroke(
        hud_rect,
        r8,
        Stroke::new(1.2_f32, with_alpha(accent, 185)),
        StrokeKind::Inside,
    );

    let inner = hud_rect.shrink2(Vec2::new(6.0, 4.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::top_down(Align::LEFT)),
    );
    child.set_clip_rect(hud_rect);
    child.spacing_mut().item_spacing.y = 2.0;
    draw_top_strip(
        &mut child,
        data,
        accent,
        (filter_names, slope_names, routing_names),
        &mut events,
    );
    child.add_space(3.0);
    let shape = filter_names
        .get(data.filter_shape_idx)
        .copied()
        .unwrap_or("Bell");
    draw_hud_knobs_row(&mut child, data, accent, shape, &mut events);
    if has_dyn {
        child.add_space(1.0);
        draw_dyn_micro_strip(&mut child, data, &mut events);
    }

    (events, hud_rect)
}

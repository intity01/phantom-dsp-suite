//! Interactive Filter Node Handles
//! Draggable circular nodes with dynamic range stems, real-time GR rings/dots, and solo badges.

use crate::icons::draw_headphones_icon;
use crate::theme::{band_color, with_alpha, ACCENT_AMBER, TEXT_DIMMED};
use crate::views::curve_plot::EqCanvasMapping;
use nih_plug_egui::egui::{
    Align2, Color32, FontId, Painter, Pos2, Response, RichText, Shape, Stroke,
};
use std::f32::consts::{FRAC_PI_2, PI};

/// Hit radius for the optional floating headphone solo badge on a soloed node.
pub const SOLO_BADGE_RADIUS: f32 = 8.0;

pub struct NodeRenderData {
    pub index: usize,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub dyn_range: f32,
    /// Real-time dynamic EQ gain offset in dB (negative = compression, positive = expansion).
    pub live_gr_db: f32,
    pub enabled: bool,
    pub is_soloed: bool,
    pub filter_name: &'static str,
    pub target_name: &'static str,
}

/// Returns the screen center of the floating headphone badge attached to a soloed node.
#[inline]
pub fn solo_badge_center(node_pos: Pos2, node_radius: f32) -> Pos2 {
    Pos2::new(
        node_pos.x + node_radius + 4.5,
        node_pos.y - node_radius - 3.5,
    )
}

fn draw_gr_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    live_gr_db: f32,
    dyn_range: f32,
    color: Color32,
) {
    let scale = dyn_range.abs().max(6.0);
    let frac = (live_gr_db.abs() / scale).clamp(0.05, 1.0);
    let sweep = frac * (1.5 * PI) * if live_gr_db >= 0.0 { 1.0 } else { -1.0 };
    let segments = 18;
    let mut pts = Vec::with_capacity(segments + 1);
    let start = -FRAC_PI_2;
    for i in 0..=segments {
        let a = start + sweep * (i as f32 / segments as f32);
        pts.push(Pos2::new(
            center.x + radius * a.cos(),
            center.y + radius * a.sin(),
        ));
    }
    painter.add(Shape::line(pts, Stroke::new(2.2_f32, color)));
}

pub fn draw_node(
    painter: &Painter,
    map: &EqCanvasMapping,
    node: &NodeRenderData,
    is_hovered: bool,
    is_selected: bool,
) -> Pos2 {
    let node_pos = Pos2::new(map.freq_to_x(node.freq), map.db_to_y(node.gain));
    let color = band_color(node.index);

    // Static dynamic range guide stem & target marker
    if node.dyn_range.abs() > 0.1 && node.enabled {
        let dyn_target_y = map.db_to_y(node.gain + node.dyn_range);
        let dyn_col = if node.dyn_range > 0.0 {
            Color32::from_rgb(52, 211, 153)
        } else {
            Color32::from_rgb(248, 113, 113)
        };
        painter.line_segment(
            [node_pos, Pos2::new(node_pos.x, dyn_target_y)],
            Stroke::new(1.4_f32, with_alpha(dyn_col, 130)),
        );
        painter.circle_stroke(
            Pos2::new(node_pos.x, dyn_target_y),
            3.2_f32,
            Stroke::new(1.4_f32, with_alpha(dyn_col, 190)),
        );
    }

    // Live real-time Dynamic Gain Reduction deflection bar & glowing dot
    let has_live_gr = node.enabled && node.live_gr_db.abs() > 0.05;
    let gr_color = if node.live_gr_db >= 0.0 {
        Color32::from_rgb(52, 211, 153)
    } else {
        Color32::from_rgb(251, 113, 133)
    };
    if has_live_gr {
        let live_y = map.db_to_y(node.gain + node.live_gr_db);
        let live_pt = Pos2::new(node_pos.x, live_y);
        painter.line_segment(
            [node_pos, live_pt],
            Stroke::new(2.4_f32, with_alpha(gr_color, 215)),
        );
        painter.circle_filled(live_pt, 5.5_f32, with_alpha(gr_color, 85));
        painter.circle_filled(live_pt, 3.2_f32, gr_color);
    }

    let node_radius = if is_hovered || is_selected || node.is_soloed {
        11.0_f32
    } else {
        9.0_f32
    };
    let fill_color = if node.is_soloed {
        Color32::from_rgb(54, 36, 12)
    } else if node.enabled {
        Color32::from_rgb(22, 25, 34)
    } else {
        Color32::from_rgba_unmultiplied(38, 41, 52, 165)
    };
    let stroke_color = if node.is_soloed {
        ACCENT_AMBER
    } else if node.enabled {
        color
    } else {
        Color32::from_rgba_unmultiplied(100, 116, 139, 135)
    };

    // Animated GR circumference arc
    if has_live_gr {
        draw_gr_arc(
            painter,
            node_pos,
            node_radius + 3.2,
            node.live_gr_db,
            node.dyn_range,
            with_alpha(gr_color, 220),
        );
    }

    // Solo audition ring + floating headphone badge
    if node.is_soloed {
        painter.circle_stroke(
            node_pos,
            node_radius + 6.0_f32,
            Stroke::new(1.6_f32, Color32::from_rgba_unmultiplied(245, 158, 11, 140)),
        );
        let badge_pos = solo_badge_center(node_pos, node_radius);
        painter.circle_filled(badge_pos, SOLO_BADGE_RADIUS, Color32::from_rgb(46, 31, 10));
        painter.circle_stroke(
            badge_pos,
            SOLO_BADGE_RADIUS,
            Stroke::new(1.2_f32, ACCENT_AMBER),
        );
        draw_headphones_icon(painter, badge_pos, 4.4, ACCENT_AMBER, 1.3);
    }

    // Selection outer halo
    if is_selected {
        painter.circle_stroke(
            node_pos,
            node_radius + 3.2_f32,
            Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 125)),
        );
    }

    painter.circle_filled(node_pos, node_radius, fill_color);
    painter.circle_stroke(node_pos, node_radius, Stroke::new(2.0_f32, stroke_color));

    painter.text(
        node_pos,
        Align2::CENTER_CENTER,
        format!("{}", node.index + 1),
        FontId::proportional(10.0),
        if node.is_soloed {
            ACCENT_AMBER
        } else if node.enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(148, 163, 184)
        },
    );

    node_pos
}

pub fn show_node_tooltip(response: &Response, node: &NodeRenderData) {
    let color = band_color(node.index);
    response.clone().on_hover_ui_at_pointer(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Band {}", node.index + 1))
                    .strong()
                    .color(color),
            );
            ui.label(node.filter_name);
            if !node.enabled {
                ui.label(RichText::new("[OFF]").color(TEXT_DIMMED));
            }
            if node.is_soloed {
                ui.label(RichText::new("[SOLO]").strong().color(ACCENT_AMBER));
            }
        });
        if node.freq >= 1000.0 {
            ui.label(format!("Freq: {:.2} kHz", node.freq / 1000.0));
        } else {
            ui.label(format!("Freq: {:.1} Hz", node.freq));
        }
        ui.label(format!("Gain: {:+.1} dB", node.gain));
        ui.label(format!("Q: {:.2}", node.q));
        if node.dyn_range.abs() > 0.1 {
            ui.label(format!("Dyn Range: {:+.1} dB", node.dyn_range));
        }
        if node.live_gr_db.abs() > 0.05 {
            ui.label(format!("Live GR: {:+.1} dB", node.live_gr_db));
        }
        ui.label(format!("Routing: {}", node.target_name));
        ui.separator();
        ui.label(
            RichText::new(
                "Double-click: 0 dB  •  Right-click: On/Off  •  Scroll: Q  •  Alt+Drag: Solo",
            )
            .font(FontId::proportional(9.5))
            .color(TEXT_DIMMED),
        );
    });
}

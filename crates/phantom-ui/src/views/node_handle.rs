//! Interactive Filter Node Handles
//! Draggable circular nodes with dynamic range indicators, hover states, solo rings, and halos.

use crate::theme::{ACCENT_AMBER, BAND_COLORS, TEXT_DIMMED};
use crate::views::curve_plot::EqCanvasMapping;
use nih_plug_egui::egui::{Align2, Color32, FontId, Painter, Pos2, Response, RichText, Stroke};

pub struct NodeRenderData {
    pub index: usize,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub dyn_range: f32,
    pub enabled: bool,
    pub is_soloed: bool,
    pub filter_name: &'static str,
    pub target_name: &'static str,
}

pub fn draw_node(
    painter: &Painter,
    map: &EqCanvasMapping,
    node: &NodeRenderData,
    is_hovered: bool,
    is_selected: bool,
) -> Pos2 {
    let node_pos = Pos2::new(map.freq_to_x(node.freq), map.db_to_y(node.gain));
    let band_color = BAND_COLORS[node.index % 8];

    // Dynamic Range Indicator line and endpoint dot
    if node.dyn_range.abs() > 0.1 && node.enabled {
        let dyn_target_y = map.db_to_y(node.gain + node.dyn_range);
        let dyn_stroke = Stroke::new(
            1.5_f32,
            if node.dyn_range > 0.0 {
                Color32::from_rgba_unmultiplied(52, 211, 153, 190)
            } else {
                Color32::from_rgba_unmultiplied(248, 113, 113, 190)
            },
        );
        painter.line_segment([node_pos, Pos2::new(node_pos.x, dyn_target_y)], dyn_stroke);
        painter.circle_filled(
            Pos2::new(node_pos.x, dyn_target_y),
            3.5_f32,
            if node.dyn_range > 0.0 {
                Color32::from_rgb(52, 211, 153)
            } else {
                Color32::from_rgb(248, 113, 113)
            },
        );
    }

    // Node body
    let node_radius = if is_hovered || is_selected || node.is_soloed {
        11.0_f32
    } else {
        9.0_f32
    };
    let fill_color = if node.is_soloed {
        Color32::from_rgb(56, 38, 12)
    } else if node.enabled {
        Color32::from_rgb(24, 27, 36)
    } else {
        Color32::from_rgba_unmultiplied(40, 42, 50, 160)
    };

    let stroke_color = if node.is_soloed {
        ACCENT_AMBER
    } else if node.enabled {
        band_color
    } else {
        Color32::from_rgba_unmultiplied(100, 116, 139, 140)
    };

    // Solo outer audition ring
    if node.is_soloed {
        painter.circle_stroke(
            node_pos,
            node_radius + 6.5_f32,
            Stroke::new(1.8_f32, Color32::from_rgba_unmultiplied(245, 158, 11, 195)),
        );
    }

    // Selection outer halo
    if is_selected {
        painter.circle_stroke(
            node_pos,
            node_radius + 3.5_f32,
            Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
        );
    }

    painter.circle_filled(node_pos, node_radius, fill_color);
    painter.circle_stroke(node_pos, node_radius, Stroke::new(2.2_f32, stroke_color));

    // Band number text inside node
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
    let band_color = BAND_COLORS[node.index % 8];
    response.clone().on_hover_ui_at_pointer(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Band {}", node.index + 1))
                    .strong()
                    .color(band_color),
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
        ui.label(format!("Routing: {}", node.target_name));
        ui.separator();
        ui.label(
            RichText::new("Double-click: 0 dB  •  Right-click: On/Off  •  Scroll: Q")
                .font(FontId::proportional(9.5))
                .color(TEXT_DIMMED),
        );
    });
}

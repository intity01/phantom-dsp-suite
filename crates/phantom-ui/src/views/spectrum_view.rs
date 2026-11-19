//! Real-Time FFT Audio Spectrum Analyzer View
//! Smooth reactive glowing RTA frequency contour.

use crate::views::curve_plot::EqCanvasMapping;
use nih_plug_egui::egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke};
use phantom_dsp::{SharedSpectrum, NUM_DISPLAY_BINS};
use std::sync::Arc;

pub fn draw_spectrum(
    painter: &Painter,
    map: &EqCanvasMapping,
    shared_spectrum: &Arc<SharedSpectrum>,
) {
    if let Ok(disp) = shared_spectrum.display_data.try_lock() {
        let mut points = Vec::with_capacity(NUM_DISPLAY_BINS);
        for i in 0..NUM_DISPLAY_BINS {
            let norm = i as f32 / (NUM_DISPLAY_BINS - 1) as f32;
            let x = map.rect.min.x + norm * map.rect.width();

            let db = disp.bins_db[i];
            let db_norm = ((db + 90.0) / 100.0).clamp(0.0, 1.0);
            let y = map.rect.max.y - db_norm * map.rect.height();
            points.push(Pos2::new(x, y));
        }

        if points.len() > 1 {
            // Smooth gradient triangle-strip fill under spectrum contour
            let top_col = Color32::from_rgba_unmultiplied(6, 182, 212, 34);
            let bot_col = Color32::from_rgba_unmultiplied(6, 182, 212, 4);
            let bottom_y = map.rect.max.y;

            let mut mesh = Mesh::default();
            for w in points.windows(2) {
                let idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(w[0], top_col);
                mesh.colored_vertex(Pos2::new(w[0].x, bottom_y), bot_col);
                mesh.colored_vertex(w[1], top_col);
                mesh.colored_vertex(Pos2::new(w[1].x, bottom_y), bot_col);
                mesh.add_triangle(idx, idx + 1, idx + 2);
                mesh.add_triangle(idx + 1, idx + 3, idx + 2);
            }
            painter.add(Shape::mesh(mesh));

            // Top contour line
            painter.add(Shape::line(
                points,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(34, 211, 238, 95)),
            ));
        }
    }
}

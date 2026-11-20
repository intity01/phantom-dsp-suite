//! Embedded Phantom Orb icon, Windows viewport IconData, and the connected wordmark,
//! both drawn from pixel-exact textures (see `brand_raster`).

use crate::brand_raster::{rasterize_wordmark, resample_rgba, wordmark_height_px};
use eframe::egui::{
    self, Color32, IconData, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2,
};
use std::cell::RefCell;
use std::collections::HashMap;

const ICON_256_PNG: &[u8] = include_bytes!("../assets/icons/icon-256.png");

fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let buf_size = reader.output_buffer_size()?;
    let mut raw = vec![0u8; buf_size];
    let info = reader.next_frame(&mut raw).ok()?;
    let data = &raw[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((rgba, info.width, info.height))
}

/// Decodes the embedded 256x256 PNG icon into `egui::IconData` for the native OS window.
pub fn load_window_icon() -> Option<IconData> {
    let (rgba, width, height) = decode_png_rgba(ICON_256_PNG)?;
    Some(IconData {
        rgba,
        width,
        height,
    })
}

pub struct BrandAssets {
    icon: (Vec<u8>, usize, usize),
    /// Textures rendered at an exact pixel size, keyed by what was drawn and how big.
    cache: RefCell<HashMap<String, TextureHandle>>,
}

impl BrandAssets {
    pub fn load(_ctx: &egui::Context) -> Self {
        let (rgba, w, h) =
            decode_png_rgba(ICON_256_PNG).unwrap_or_else(|| (vec![10, 132, 255, 255], 1, 1));
        Self {
            icon: (rgba, w as usize, h as usize),
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn draw_logo_icon(&self, ui: &mut Ui, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let ppp = ui.ctx().pixels_per_point();
        let px = (size * ppp).round().max(1.0) as usize;
        let tex = self.texture(ui.ctx(), format!("logo:{px}"), || {
            let (rgba, w, h) = &self.icon;
            resample_rgba(rgba, *w, *h, px, px)
        });
        paint_pixel_exact(ui, rect.min, &tex, ppp);
    }

    /// Draws the connected monoline `phantom` wordmark `width` points wide.
    pub fn draw_wordmark(&self, ui: &mut Ui, width: f32, text: Color32, accent: Color32) {
        let ppp = ui.ctx().pixels_per_point();
        let px = (width * ppp).round().max(1.0) as usize;
        let size = Vec2::new(px as f32, wordmark_height_px(px) as f32) / ppp;
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let key = format!(
            "wordmark:{px}:{:?}:{:?}",
            text.to_array(),
            accent.to_array()
        );
        let tex = self.texture(ui.ctx(), key, || rasterize_wordmark(px, text, accent));
        paint_pixel_exact(ui, rect.min, &tex, ppp);
    }

    fn texture(
        &self,
        ctx: &egui::Context,
        key: String,
        make: impl FnOnce() -> egui::ColorImage,
    ) -> TextureHandle {
        self.cache
            .borrow_mut()
            .entry(key.clone())
            .or_insert_with(|| ctx.load_texture(key, make(), TextureOptions::NEAREST))
            .clone()
    }
}

/// Paints a texture 1:1 with screen pixels, snapping its corner to the pixel grid so it is
/// never resampled (which is what blurs or distorts small marks).
fn paint_pixel_exact(ui: &Ui, min: Pos2, tex: &TextureHandle, ppp: f32) {
    let snapped = Pos2::new((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp);
    let [w, h] = tex.size();
    let rect = Rect::from_min_size(snapped, Vec2::new(w as f32, h as f32) / ppp);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
}

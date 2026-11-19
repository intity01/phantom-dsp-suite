//! Fixed Constant Window Size Presets (`S / M / L / XL`) with Proportional UI Zoom
//!
//! Why not `ResizableWindow`: it calls `set_requested_size` on every corner-drag frame, and
//! `nih_plug_egui` swaps `requested_size` to `None` *before* `request_resize()`, so Cubase's
//! `IPlugView::getSize()` sees the OLD size and the outer VST3 frame desyncs from the baseview
//! viewport. Here a size change happens exactly once per click:
//! 1. the new size is persisted into `EguiState` (so `Editor::size()` already reports it),
//! 2. the host is asked once to resize its frame (`GuiContext::request_resize`),
//! 3. only if the host accepted, the baseview window is resized (`ViewportCommand::InnerSize`).
//!
//! The whole UI is drawn in a single layer scaled by the preset zoom (`TSTransform`), so every
//! font, knob, node and panel keeps constant proportions at every preset.

use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{
    self, emath::TSTransform, Id, LayerId, Order, Pos2, Rect, Ui, UiBuilder, Vec2, ViewportCommand,
};
use nih_plug_egui::EguiState;
use phantom_ui::BG_PANEL;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizePreset {
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub zoom: f32,
}

pub const SIZE_PRESETS: [SizePreset; 4] = [
    SizePreset {
        label: "S",
        width: 960,
        height: 560,
        zoom: 0.90,
    },
    SizePreset {
        label: "M",
        width: 1100,
        height: 640,
        zoom: 1.00,
    },
    SizePreset {
        label: "L",
        width: 1260,
        height: 720,
        zoom: 1.12,
    },
    SizePreset {
        label: "XL",
        width: 1420,
        height: 800,
        zoom: 1.25,
    },
];

pub const DEFAULT_SIZE_PRESET: usize = 1;

/// Index of the preset closest to the persisted window size (exact match for preset sizes).
pub fn preset_index_for_size(size: (u32, u32)) -> usize {
    SIZE_PRESETS
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| p.width.abs_diff(size.0) + p.height.abs_diff(size.1))
        .map(|(i, _)| i)
        .unwrap_or(DEFAULT_SIZE_PRESET)
}

pub fn current_preset(editor_state: &EguiState) -> SizePreset {
    SIZE_PRESETS[preset_index_for_size(editor_state.size())]
}

fn persist_size(editor_state: &Arc<EguiState>, size: (u32, u32)) {
    if let Ok(new_state) = Arc::try_unwrap(EguiState::from_size(size.0, size.1)) {
        PersistentField::set(editor_state, new_state);
    }
}

/// Switches to a fixed size preset with a single, host-approved resize (no per-frame fighting).
pub fn apply_size_preset(
    ctx: &egui::Context,
    host: &ParamSetter,
    editor_state: &Arc<EguiState>,
    idx: usize,
) {
    let Some(preset) = SIZE_PRESETS.get(idx).copied() else {
        return;
    };
    let old = editor_state.size();
    let target = (preset.width, preset.height);
    if old == target {
        return;
    }
    persist_size(editor_state, target);
    if host.raw_context.request_resize() {
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
            preset.width as f32,
            preset.height as f32,
        )));
    } else {
        persist_size(editor_state, old);
    }
}

/// Draws `add_contents` into one full-window layer scaled by `zoom` (logical size = window/zoom).
pub fn show_zoomed_root(ctx: &egui::Context, zoom: f32, add_contents: impl FnOnce(&mut Ui)) {
    let zoom = zoom.clamp(0.5, 2.0);
    let screen = ctx.screen_rect();
    ctx.layer_painter(LayerId::background())
        .rect_filled(screen, 0.0, BG_PANEL);
    let logical = Rect::from_min_size(Pos2::ZERO, screen.size() / zoom);
    let area = egui::Area::new(Id::new("phantom-eq-zoom-root"))
        .order(Order::Background)
        .fixed_pos(Pos2::ZERO)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(logical);
            ui.scope_builder(UiBuilder::new().max_rect(logical), |ui| {
                ui.set_min_size(logical.size());
                add_contents(ui);
            });
        });
    let transform = if (zoom - 1.0).abs() < 1e-3 {
        TSTransform::IDENTITY
    } else {
        TSTransform::from_scaling(zoom)
    };
    ctx.set_transform_layer(area.response.layer_id, transform);
}

/// Pointer position converted into the (possibly zoomed) layer space of `ui`.
pub fn local_pointer(ui: &Ui, global: Option<Pos2>) -> Option<Pos2> {
    let pos = global?;
    Some(match ui.ctx().layer_transform_from_global(ui.layer_id()) {
        Some(from_global) => from_global * pos,
        None => pos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_size_presets_match_spec() {
        assert_eq!(SIZE_PRESETS.len(), 4);
        assert_eq!(
            (
                SIZE_PRESETS[0].label,
                SIZE_PRESETS[0].width,
                SIZE_PRESETS[0].height
            ),
            ("S", 960, 560)
        );
        assert_eq!(
            (
                SIZE_PRESETS[1].label,
                SIZE_PRESETS[1].width,
                SIZE_PRESETS[1].height
            ),
            ("M", 1100, 640)
        );
        assert_eq!(
            (
                SIZE_PRESETS[2].label,
                SIZE_PRESETS[2].width,
                SIZE_PRESETS[2].height
            ),
            ("L", 1260, 720)
        );
        assert_eq!(
            (
                SIZE_PRESETS[3].label,
                SIZE_PRESETS[3].width,
                SIZE_PRESETS[3].height
            ),
            ("XL", 1420, 800)
        );
        for (idx, p) in SIZE_PRESETS.iter().enumerate() {
            assert_eq!(preset_index_for_size((p.width, p.height)), idx);
        }
    }
}

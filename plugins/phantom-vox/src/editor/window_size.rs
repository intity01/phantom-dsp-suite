//! Fixed Constant Window Size Presets (`S / M / L`) with Proportional UI Zoom
//!
//! Uses a single host-approved resize per click (persisting `EguiState` before calling
//! `request_resize()` + `ViewportCommand::InnerSize`) and scales the entire UI in one
//! `TSTransform` layer so proportions remain constant across Cubase 15 DPI configurations.

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

pub const SIZE_PRESETS: [SizePreset; 3] = [
    SizePreset {
        label: "S",
        width: 780,
        height: 480,
        zoom: 0.92,
    },
    SizePreset {
        label: "M",
        width: 860,
        height: 520,
        zoom: 1.00,
    },
    SizePreset {
        label: "L",
        width: 1020,
        height: 600,
        zoom: 1.15,
    },
];

pub const DEFAULT_SIZE_PRESET: usize = 1;

/// Index of the preset closest to the persisted window size.
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

/// Switches to a fixed size preset with a single host-approved resize.
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

/// Draws `add_contents` into one full-window layer scaled by `zoom`.
pub fn show_zoomed_root(ctx: &egui::Context, zoom: f32, add_contents: impl FnOnce(&mut Ui)) {
    let zoom = zoom.clamp(0.5, 2.0);
    let screen = ctx.screen_rect();
    ctx.layer_painter(LayerId::background())
        .rect_filled(screen, 0.0, BG_PANEL);
    let logical = Rect::from_min_size(Pos2::ZERO, screen.size() / zoom);
    let area = egui::Area::new(Id::new("phantom-vox-zoom-root"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_size_presets_match_spec() {
        assert_eq!(SIZE_PRESETS.len(), 3);
        assert_eq!(
            (
                SIZE_PRESETS[0].label,
                SIZE_PRESETS[0].width,
                SIZE_PRESETS[0].height
            ),
            ("S", 780, 480)
        );
        assert_eq!(
            (
                SIZE_PRESETS[1].label,
                SIZE_PRESETS[1].width,
                SIZE_PRESETS[1].height
            ),
            ("M", 860, 520)
        );
        assert_eq!(
            (
                SIZE_PRESETS[2].label,
                SIZE_PRESETS[2].width,
                SIZE_PRESETS[2].height
            ),
            ("L", 1020, 600)
        );
        for (idx, p) in SIZE_PRESETS.iter().enumerate() {
            assert_eq!(preset_index_for_size((p.width, p.height)), idx);
        }
    }
}

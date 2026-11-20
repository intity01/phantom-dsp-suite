//! Runs the editor's draw code headlessly, so a panic or hang in the GUI shows up in tests
//! instead of as a blank plugin window in the DAW.

use nih_plug::prelude::*;
use nih_plug::wrapper::state::PluginState;
use nih_plug_egui::egui;
use phantom_dsp::SharedMeters;
use phantom_vox::editor::{draw_editor, EditorShared, EditorState};
use phantom_vox::live_mirror::LiveParamMirror;
use phantom_vox::params::PhantomVoxParams;
use phantom_vox::telemetry::SharedVoxTelemetry;
use std::sync::Arc;

struct NullGui;

impl GuiContext for NullGui {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Vst3
    }
    fn request_resize(&self) -> bool {
        true
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        unimplemented!()
    }
    fn set_state(&self, _: PluginState) {}
}

#[test]
fn editor_draws_several_frames() {
    let params = Arc::new(PhantomVoxParams::default());
    let shared = EditorShared {
        live: Arc::new(LiveParamMirror::new(params.clone())),
        meters: Arc::new(SharedMeters::default()),
        telemetry: Arc::new(SharedVoxTelemetry::new()),
    };
    let gui = NullGui;
    let setter = ParamSetter::new(&gui);
    let mut state = EditorState::default();
    let ctx = egui::Context::default();
    for frame in 0..5 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(860.0, 520.0),
            )),
            time: Some(frame as f64 / 60.0),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            draw_editor(ctx, &setter, &mut state, &params, &shared)
        });
        // Nothing may be painted past the window edge: in a fixed-size plugin window that
        // means a card or row grew wider than its slot and pushed content out of view.
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(860.0, 520.0));
        for clipped in &out.shapes {
            let r = clipped.shape.visual_bounding_rect();
            if r.is_positive() {
                assert!(
                    screen.expand(1.0).contains_rect(r),
                    "frame {frame}: shape at {r:?} is drawn outside the {screen:?} window"
                );
            }
        }
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        assert!(!prims.is_empty(), "frame {frame} painted nothing");
    }
}

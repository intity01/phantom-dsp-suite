//! Phantom Vox Plugin Editor Window & State Orchestration

mod align_view;
mod align_view_meters;
mod bottom_bar;
mod compare;
mod field_view;
mod focus_strip;
mod header;
mod knobs;
mod phase_meter;
mod window_size;

use crate::live_mirror::{LiveParamMirror, LiveSetter};
use crate::params::PhantomVoxParams;
use crate::telemetry::{SharedVoxTelemetry, WAVE_BINS};
use field_view::FieldViewParams;
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Margin, Vec2};
use phantom_dsp::SharedMeters;
use phantom_ui::{configure_visuals, BG_PANEL};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use window_size::{current_preset, show_zoomed_root};

/// Thread-safe handles shared between `PhantomVox` DSP and the 60 fps editor.
pub struct EditorShared {
    pub live: Arc<LiveParamMirror>,
    pub meters: Arc<SharedMeters>,
    pub telemetry: Arc<SharedVoxTelemetry>,
    /// Held while the user presses "Hold: Original".
    pub audition: Arc<AtomicBool>,
}

/// Per-window mutable GUI state (waveform scratch buffers avoid per-frame allocations).
pub struct EditorState {
    pub guide_wave: [f32; WAVE_BINS],
    pub dub_wave: [f32; WAVE_BINS],
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            guide_wave: [0.0; WAVE_BINS],
            dub_wave: [0.0; WAVE_BINS],
        }
    }
}

pub fn draw_editor(
    ctx: &egui::Context,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomVoxParams,
    shared: &EditorShared,
) {
    shared.live.reconcile();
    let live = shared.live.as_ref();
    let live_setter = LiveSetter::new(setter, live);

    let snap = shared.telemetry.load();
    let _meter_snap = shared.meters.load();
    shared
        .telemetry
        .load_waveforms(&mut state.guide_wave, &mut state.dub_wave);

    configure_visuals(ctx);

    let field_params = FieldViewParams {
        mode: live.read(&params.mode),
        voices: live.read(&params.voices),
        timing_ms: live.read(&params.timing_ms),
        detune_cents: live.read(&params.detune_cents),
        formant_st: live.read(&params.formant_st),
        width_pct: live.read(&params.width_pct),
        mono_check: live.read(&params.mono_check),
        mix_pct: live.read(&params.mix_pct),
        bypassed: live.read(&params.bypass),
    };

    let zoom = current_preset(&params.editor_state).zoom;
    show_zoomed_root(ctx, zoom, |ui| {
        egui::Frame::NONE.fill(BG_PANEL).show(ui, |ui| {
            header::draw_header(ui, &live_setter, params, &shared.audition);

            egui::Frame::NONE
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    let knobs_and_footer_reserve = 148.0;
                    let vis_h = (ui.available_height() - knobs_and_footer_reserve).max(190.0);
                    let card_w = ((ui.available_width() - 10.0) * 0.5).max(240.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        field_view::draw_field_card(
                            ui,
                            Vec2::new(card_w, vis_h),
                            &field_params,
                            &snap,
                            (&live_setter, params),
                        );
                        align_view::draw_align_card(
                            ui,
                            Vec2::new(card_w, vis_h),
                            field_params.mode,
                            &snap,
                            &state.guide_wave,
                            &state.dub_wave,
                        );
                    });

                    ui.add_space(8.0);
                    knobs::draw_knobs_row(ui, &live_setter, params);
                });

            let remaining = (ui.available_height() - 32.0).max(0.0);
            if remaining > 0.0 {
                ui.add_space(remaining);
            }
            bottom_bar::draw_bottom_bar(ui, &live_setter, params, &snap);
        });
    });
}

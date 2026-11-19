//! Phantom EQ Plugin GUI Window & State Management
//! Modular Pro-Q style editor assembled from phantom-ui components.

mod display;
mod display_helpers;
mod header;
mod inspector;

use crate::params::{PhantomEqParams, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Vec2};
use phantom_dsp::{SharedSpectrum, SpectrumProcessor};
use phantom_ui::{configure_visuals, draw_master_bar, BG_PANEL};
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// Sentinel value indicating no band is currently soloed.
pub const NO_SOLO: u8 = u8::MAX;

/// Lock-free state shared between the audio processor and the GUI editor.
pub struct EditorShared {
    pub spectrum: Arc<SharedSpectrum>,
    pub solo_band: Arc<AtomicU8>,
    pub sample_rate_bits: Arc<AtomicU32>,
    pub latency_samples: Arc<AtomicU32>,
}

impl EditorShared {
    pub fn sample_rate(&self) -> f32 {
        let sr = f32::from_bits(self.sample_rate_bits.load(Ordering::Relaxed));
        if sr.is_finite() && sr >= 8000.0 {
            sr
        } else {
            44100.0
        }
    }

    pub fn solo_band(&self) -> Option<usize> {
        let v = self.solo_band.load(Ordering::Relaxed) as usize;
        if v < NUM_BANDS {
            Some(v)
        } else {
            None
        }
    }

    pub fn toggle_solo(&self, band_idx: usize) {
        let target = band_idx as u8;
        let current = self.solo_band.load(Ordering::Relaxed);
        let next = if current == target { NO_SOLO } else { target };
        self.solo_band.store(next, Ordering::Relaxed);
    }

    pub fn clear_solo(&self) {
        self.solo_band.store(NO_SOLO, Ordering::Relaxed);
    }

    pub fn latency_samples(&self) -> u32 {
        self.latency_samples.load(Ordering::Relaxed)
    }
}

pub struct EditorState {
    pub selected_band: usize,
    pub dragging_band: Option<usize>,
    pub spectrum_processor: SpectrumProcessor,
    pub show_spectrum: bool,
    pub sample_rate: f32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected_band: 0,
            dragging_band: None,
            spectrum_processor: SpectrumProcessor::new(),
            show_spectrum: true,
            sample_rate: 44100.0,
        }
    }
}

pub fn draw_editor(
    ctx: &egui::Context,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
) {
    let sample_rate = shared.sample_rate();
    state.sample_rate = sample_rate;
    if state.show_spectrum {
        state
            .spectrum_processor
            .update(&shared.spectrum, sample_rate);
    }

    configure_visuals(ctx);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_PANEL))
        .show(ctx, |ui| {
            header::draw_header(ui, setter, params, state, shared);
            ui.add_space(4.0);

            let avail_h = ui.available_height() - 150.0;
            let display_size = Vec2::new(ui.available_width(), avail_h.max(260.0));
            display::draw_main_display(ui, setter, state, params, shared, display_size);
            ui.add_space(6.0);

            inspector::draw_inspector_section(ui, setter, state, params, shared);
            ui.add_space(4.0);

            let master_events =
                draw_master_bar(ui, params.phase_invert.value(), params.output_gain.value());
            if master_events.toggle_phase_invert {
                setter.begin_set_parameter(&params.phase_invert);
                setter.set_parameter(&params.phase_invert, !params.phase_invert.value());
                setter.end_set_parameter(&params.phase_invert);
            }
            if let Some(gain) = master_events.set_output_gain {
                setter.begin_set_parameter(&params.output_gain);
                setter.set_parameter(&params.output_gain, gain);
                setter.end_set_parameter(&params.output_gain);
            }
        });
}

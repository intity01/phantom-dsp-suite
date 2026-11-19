//! Phantom EQ Plugin GUI Window & State Management
//! Modular Pro-Q 4 style editor assembled from `phantom-ui` components.

mod display;
mod display_helpers;
mod gestures;
mod header;
mod inspector;

use crate::dsp_state::SharedBandGr;
use crate::params::{CharacterModeParam, PhantomEqParams, NUM_BANDS};
use display_helpers::delete_band;
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Key, Pos2, Vec2};
use phantom_dsp::{SharedMeters, SharedSpectrum, SpectrumProcessor};
use phantom_ui::{
    configure_visuals, draw_master_bar, draw_output_meter, MasterBarData, OutputMeterData,
    BG_PANEL, OUTPUT_METER_WIDTH,
};
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// Sentinel value indicating no band is currently soloed.
pub const NO_SOLO: u8 = u8::MAX;

/// Lock-free state shared between the audio processor and the GUI editor.
pub struct EditorShared {
    pub spectrum: Arc<SharedSpectrum>,
    pub meters: Arc<SharedMeters>,
    pub band_gr: Arc<SharedBandGr>,
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

    pub fn set_solo(&self, band: Option<usize>) {
        let raw = band
            .filter(|&i| i < NUM_BANDS)
            .map(|i| i as u8)
            .unwrap_or(NO_SOLO);
        self.solo_band.store(raw, Ordering::Relaxed);
    }

    pub fn clear_solo(&self) {
        self.solo_band.store(NO_SOLO, Ordering::Relaxed);
    }

    pub fn latency_samples(&self) -> u32 {
        self.latency_samples.load(Ordering::Relaxed)
    }
}

pub struct EditorState {
    pub selected_band: Option<usize>,
    pub dragging_band: Option<usize>,
    pub drag_created_band: Option<usize>,
    pub pending_curve_press: Option<Pos2>,
    pub latched_solo: Option<usize>,
    pub solo_hold_band: Option<usize>,
    pub solo_press_start: Option<f64>,
    pub solo_was_latched_at_press: bool,
    pub alt_solo_band: Option<usize>,
    pub scale_db: f32,
    pub spectrum_processor: SpectrumProcessor,
    pub show_pre_spectrum: bool,
    pub show_post_spectrum: bool,
    pub show_peak_hold: bool,
    pub sample_rate: f32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected_band: None,
            dragging_band: None,
            drag_created_band: None,
            pending_curve_press: None,
            latched_solo: None,
            solo_hold_band: None,
            solo_press_start: None,
            solo_was_latched_at_press: false,
            alt_solo_band: None,
            scale_db: 12.0,
            spectrum_processor: SpectrumProcessor::new(),
            show_pre_spectrum: true,
            show_post_spectrum: true,
            show_peak_hold: true,
            sample_rate: 44100.0,
        }
    }
}

impl EditorState {
    pub fn effective_solo(&self, params: &PhantomEqParams) -> Option<usize> {
        self.alt_solo_band
            .or(self.solo_hold_band)
            .or(self.latched_solo)
            .filter(|&i| i < NUM_BANDS && params.bands[i].used.value())
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

    // Ensure selected_band always points to an active (`used`) band, or None
    if let Some(sel) = state.selected_band {
        if sel >= NUM_BANDS || !params.bands[sel].used.value() {
            state.selected_band = (0..NUM_BANDS).find(|&i| params.bands[i].used.value());
        }
    } else if state.dragging_band.is_none() {
        // Keep None when empty; select first used band only if bands were loaded via preset
        if let Some(first_used) = (0..NUM_BANDS).find(|&i| params.bands[i].used.value()) {
            state.selected_band = Some(first_used);
        }
    }

    // Delete / Backspace removes the selected band (when not typing into a slider box)
    if !ctx.wants_keyboard_input()
        && ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace))
    {
        if let Some(sel) = state.selected_band {
            delete_band(setter, params, state, shared, sel);
        }
    }

    if state.show_pre_spectrum || state.show_post_spectrum {
        state
            .spectrum_processor
            .update(&shared.spectrum, sample_rate);
    }

    let meter_snap = shared.meters.load();
    let makeup_db = shared.meters.load_makeup_db();

    configure_visuals(ctx);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_PANEL))
        .show(ctx, |ui| {
            header::draw_header(ui, setter, params, state, shared);
            ui.add_space(4.0);

            let inspector_reserve = if state.selected_band.is_some() {
                148.0
            } else {
                84.0
            };
            let display_h = (ui.available_height() - inspector_reserve).max(240.0);
            let display_w = (ui.available_width() - OUTPUT_METER_WIDTH - 10.0).max(320.0);

            ui.horizontal(|ui| {
                ui.add_space(4.0);
                display::draw_main_display(
                    ui,
                    setter,
                    state,
                    params,
                    shared,
                    Vec2::new(display_w, display_h),
                );
                ui.add_space(4.0);
                let meter_data = OutputMeterData {
                    peak_l_db: meter_snap.peak_l_db,
                    peak_r_db: meter_snap.peak_r_db,
                    hold_l_db: meter_snap.hold_l_db,
                    hold_r_db: meter_snap.hold_r_db,
                    momentary_lufs: meter_snap.momentary_lufs,
                    short_term_lufs: meter_snap.short_term_lufs,
                };
                draw_output_meter(ui, &meter_data, display_h);
            });

            ui.add_space(5.0);
            inspector::draw_inspector_section(ui, setter, state, params, shared);
            ui.add_space(4.0);

            let master_data = MasterBarData {
                phase_inverted: params.phase_invert.value(),
                output_gain_db: params.output_gain.value(),
                auto_gain: params.auto_gain.value(),
                auto_gain_makeup_db: makeup_db,
                character_idx: params.character.value().to_idx(),
                scale_db: state.scale_db,
                peak_db: meter_snap.peak_l_db.max(meter_snap.peak_r_db),
                momentary_lufs: meter_snap.momentary_lufs,
            };
            let master_events = draw_master_bar(ui, &master_data);
            apply_master_events(setter, params, state, master_events);
        });

    shared.set_solo(state.effective_solo(params));
}

fn apply_master_events(
    setter: &ParamSetter,
    params: &PhantomEqParams,
    state: &mut EditorState,
    events: phantom_ui::MasterBarEvents,
) {
    if events.toggle_phase_invert {
        setter.begin_set_parameter(&params.phase_invert);
        setter.set_parameter(&params.phase_invert, !params.phase_invert.value());
        setter.end_set_parameter(&params.phase_invert);
    }
    if let Some(gain) = events.set_output_gain {
        setter.begin_set_parameter(&params.output_gain);
        setter.set_parameter(&params.output_gain, gain);
        setter.end_set_parameter(&params.output_gain);
    }
    if events.toggle_auto_gain {
        setter.begin_set_parameter(&params.auto_gain);
        setter.set_parameter(&params.auto_gain, !params.auto_gain.value());
        setter.end_set_parameter(&params.auto_gain);
    }
    if let Some(idx) = events.set_character {
        setter.begin_set_parameter(&params.character);
        setter.set_parameter(&params.character, CharacterModeParam::from_idx(idx));
        setter.end_set_parameter(&params.character);
    }
    if let Some(scale) = events.set_scale_db {
        state.scale_db = scale;
    }
}

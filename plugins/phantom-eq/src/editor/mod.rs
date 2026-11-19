//! Phantom EQ Plugin GUI Window & State Management
//! Modular Pro-Q 4+ style editor with ResizableWindow, Floating Band HUD Popover,
//! Piano Roll, Spectrum Grab, EQ Match, Collision Detection, Presets, A/B, and Undo/Redo.

mod display;
mod display_helpers;
mod gestures;
mod header;
mod inspector;
mod match_panel;
mod preset_bar;

use crate::dsp_state::SharedBandGr;
use crate::params::{PhantomEqParams, NUM_BANDS};
use crate::state::{factory_presets, load_user_presets, AbHistoryManager, EqPresetSnapshot};
use display_helpers::{apply_master_events, delete_band};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Key, Pos2, Rect, Vec2};
use nih_plug_egui::resizable_window::ResizableWindow;
use phantom_dsp::{
    CollisionRegistry, EqMatchEngine, SharedMeters, SharedSpectrum, SpectrumProcessor,
    NUM_DISPLAY_BINS,
};
use phantom_ui::{
    configure_visuals, draw_master_bar, draw_output_meter, MasterBarData, OutputMeterData,
    BG_PANEL, OUTPUT_METER_WIDTH,
};
use preset_bar::sync_selected_band_after_load;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

pub const NO_SOLO: u8 = u8::MAX;

pub struct EditorShared {
    pub spectrum: Arc<SharedSpectrum>,
    pub meters: Arc<SharedMeters>,
    pub band_gr: Arc<SharedBandGr>,
    pub solo_band: Arc<AtomicU8>,
    pub sample_rate_bits: Arc<AtomicU32>,
    pub latency_samples: Arc<AtomicU32>,
    pub collision_slot: usize,
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
        (v < NUM_BANDS).then_some(v)
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
    pub show_floating_hud: bool,
    pub show_docked_inspector: bool,
    pub last_hud_rect: Option<Rect>,
    pub show_piano_roll: bool,
    pub snap_to_notes: bool,
    pub spectrum_grab_enabled: bool,
    pub show_collisions: bool,
    pub collision_bins: [f32; NUM_DISPLAY_BINS],
    pub show_eq_match: bool,
    pub eq_match_engine: EqMatchEngine,
    pub match_rec_ref: bool,
    pub match_rec_track: bool,
    pub match_max_bands: usize,
    pub match_amount_pct: f32,
    pub history: AbHistoryManager,
    pub factory_presets: Vec<EqPresetSnapshot>,
    pub user_presets: Vec<EqPresetSnapshot>,
    pub current_preset_idx: usize,
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
            show_floating_hud: true,
            show_docked_inspector: false,
            last_hud_rect: None,
            show_piano_roll: true,
            snap_to_notes: false,
            spectrum_grab_enabled: true,
            show_collisions: false,
            collision_bins: [0.0; NUM_DISPLAY_BINS],
            show_eq_match: false,
            eq_match_engine: EqMatchEngine::new(),
            match_rec_ref: false,
            match_rec_track: false,
            match_max_bands: 6,
            match_amount_pct: 100.0,
            history: AbHistoryManager::new(),
            factory_presets: factory_presets(),
            user_presets: load_user_presets(),
            current_preset_idx: 0,
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

    let is_busy = state.dragging_band.is_some() || state.solo_press_start.is_some();
    state
        .history
        .record_if_changed(EqPresetSnapshot::capture_from(params), is_busy);

    if state
        .selected_band
        .is_some_and(|s| s >= NUM_BANDS || !params.bands[s].used.value())
        || (state.selected_band.is_none() && state.dragging_band.is_none())
    {
        state.selected_band = (0..NUM_BANDS).find(|&i| params.bands[i].used.value());
    }

    if !ctx.wants_keyboard_input() {
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            if let Some(sel) = state.selected_band {
                delete_band(setter, params, state, shared, sel);
            }
        }
        let (undo_key, redo_key) = ctx.input(|i| {
            let cmd = i.modifiers.ctrl || i.modifiers.command;
            let u = cmd && !i.modifiers.shift && i.key_pressed(Key::Z);
            let r = cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z)));
            (u, r)
        });
        if (undo_key && state.history.undo(params, setter))
            || (redo_key && state.history.redo(params, setter))
        {
            sync_selected_band_after_load(params, state);
        }
    }

    state
        .spectrum_processor
        .update(&shared.spectrum, sample_rate);
    let post_bins = state.spectrum_processor.post_bins_db();
    CollisionRegistry::publish_spectrum(shared.collision_slot, post_bins);
    if state.show_collisions {
        CollisionRegistry::compute_collision_bins(
            shared.collision_slot,
            post_bins,
            &mut state.collision_bins,
        );
    }
    if state.match_rec_ref {
        state
            .eq_match_engine
            .record_reference_frame(state.spectrum_processor.pre_bins_db());
    }
    if state.match_rec_track {
        state
            .eq_match_engine
            .record_current_frame(state.spectrum_processor.post_bins_db());
    }

    let meter_snap = shared.meters.load();
    let makeup_db = shared.meters.load_makeup_db();
    configure_visuals(ctx);

    ResizableWindow::new("phantom-eq-res-win")
        .min_size(Vec2::new(860.0, 500.0))
        .show(ctx, params.editor_state.as_ref(), |ui| {
            egui::Frame::NONE.fill(BG_PANEL).show(ui, |ui| {
                header::draw_header(ui, setter, params, state, shared);
                ui.add_space(4.0);

                let dock_reserve = match (state.show_docked_inspector, state.selected_band) {
                    (false, _) => 44.0,
                    (true, Some(_)) => 148.0,
                    (true, None) => 84.0,
                };
                let display_h = (ui.available_height() - dock_reserve).max(240.0);
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

                if state.show_docked_inspector {
                    ui.add_space(5.0);
                    inspector::draw_inspector_section(ui, setter, state, params, shared);
                }
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
        });

    shared.set_solo(state.effective_solo(params));
}

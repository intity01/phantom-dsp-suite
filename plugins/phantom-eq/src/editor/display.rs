//! Main EQ Frequency Response Display, Pro-Q 4 Overlays, Piano Roll & Floating Band HUD
//! Renders adaptive-scale grid, dual Pre/Post + Peak Hold spectrum, Collision & EQ Match
//! overlays, Spectrum Grab peaks, cascaded band curves, nodes with live GR rings, 20px
//! C1..C8 Piano Roll bar, and the Pro-Q 4 Floating Band HUD Popover.

use super::display_helpers::{
    band_cascaded_coeffs, build_node_render_data, create_band_at, find_hit_used_node,
};
use super::gestures::handle_canvas_gestures;
use super::inspector::draw_floating_hud_for_selected;
use super::{EditorShared, EditorState};
use crate::params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{Pos2, Rect, Sense, Ui, Vec2};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{
    band_color, detect_spectrum_peaks, draw_band_curve_fn, draw_canvas_grid,
    draw_collision_overlay, draw_combined_curve_fn, draw_dual_spectrum, draw_empty_canvas_hint,
    draw_node, draw_piano_roll, draw_reference_spectrum_overlay, draw_spectrum_grab_overlay,
    show_node_tooltip, EqCanvasMapping,
};

const PIANO_ROLL_HEIGHT: f32 = 20.0;

pub fn draw_main_display(
    ui: &mut Ui,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
    size: Vec2,
) {
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let piano_h = if state.show_piano_roll {
        PIANO_ROLL_HEIGHT
    } else {
        0.0
    };
    let canvas_max_y = (response.rect.max.y - piano_h).max(response.rect.min.y + 60.0);
    let canvas_rect = Rect::from_min_max(
        response.rect.min,
        Pos2::new(response.rect.max.x, canvas_max_y),
    );
    let map = EqCanvasMapping::with_db_range(canvas_rect, state.scale_db);
    let soloed_band = state.effective_solo(params);
    let natural_phase = params.mode.value() == ProcessingModeParam::NaturalPhase;
    let sr = state.sample_rate;
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    // 1. Adaptive dB/Hz Grid
    draw_canvas_grid(&painter, &map);

    // 2. Dual Pre-EQ / Post-EQ Spectrum Analyzer with Peak Hold
    if state.show_pre_spectrum || state.show_post_spectrum {
        let peak_slice = if state.show_peak_hold {
            Some(state.spectrum_processor.peak_hold_bins_db().as_slice())
        } else {
            None
        };
        draw_dual_spectrum(
            &painter,
            &map,
            state.spectrum_processor.pre_bins_db(),
            state.spectrum_processor.post_bins_db(),
            peak_slice,
            state.show_pre_spectrum,
            state.show_post_spectrum,
            state.show_peak_hold,
        );
    }

    // 3. Multi-Instance Collision Overlay & EQ Match Reference Trace
    if state.show_collisions {
        draw_collision_overlay(
            &painter,
            &map,
            state.spectrum_processor.post_bins_db(),
            &state.collision_bins,
        );
    }
    if state.show_eq_match && state.eq_match_engine.has_reference() {
        draw_reference_spectrum_overlay(&painter, &map, state.eq_match_engine.reference_bins_db());
    }

    // 4. Spectrum Grab Peak Detection & Interactive Overlay
    let grab_active = (state.spectrum_grab_enabled || state.spectrum_processor.is_frozen())
        && state.dragging_band.is_none();
    let hovered_grab_peak = if grab_active {
        let peaks = detect_spectrum_peaks(&map, state.spectrum_processor.post_bins_db(), 6);
        draw_spectrum_grab_overlay(&painter, &map, &peaks, pointer_pos)
    } else {
        None
    };

    // 5. Collect cascaded coeffs & positions for all active (`used`) bands on stack (zero allocs)
    let mut active_coeffs = [CascadedCoeffs::default(); NUM_BANDS];
    let mut active_count = 0usize;
    let mut used_count = 0usize;
    let mut node_positions: [Option<Pos2>; NUM_BANDS] = [None; NUM_BANDS];
    let mut used_band_freqs = [(0usize, 1000.0_f32); NUM_BANDS];

    for (i, band) in params.bands.iter().enumerate() {
        if !band.used.value() {
            continue;
        }
        let freq = band.frequency.value();
        used_band_freqs[used_count] = (i, freq);
        used_count += 1;
        node_positions[i] = Some(Pos2::new(
            map.freq_to_x(freq),
            map.db_to_y(band.gain.value()),
        ));
        if band.enabled.value() {
            active_coeffs[active_count] = band_cascaded_coeffs(band, sr, natural_phase);
            active_count += 1;
        }
    }

    // 6. Selected & Soloed Band Individual Curve Overlays
    if let Some(sel_idx) = state.selected_band {
        let sel_band = &params.bands[sel_idx];
        if sel_band.used.value() {
            let sel_coeffs = band_cascaded_coeffs(sel_band, sr, natural_phase);
            draw_band_curve_fn(
                &painter,
                &map,
                |freq| sel_coeffs.magnitude_db_at(freq, sr),
                band_color(sel_idx),
                sel_band.enabled.value(),
                soloed_band == Some(sel_idx),
            );
        }
    }
    if let Some(s_idx) = soloed_band {
        if state.selected_band != Some(s_idx) && params.bands[s_idx].used.value() {
            let sb = &params.bands[s_idx];
            let sc = band_cascaded_coeffs(sb, sr, natural_phase);
            draw_band_curve_fn(
                &painter,
                &map,
                |freq| sc.magnitude_db_at(freq, sr),
                band_color(s_idx),
                sb.enabled.value(),
                true,
            );
        }
    }

    // 7. Combined Master EQ Response Curve & Empty Startup Hint
    let active_slice = &active_coeffs[..active_count];
    draw_combined_curve_fn(&painter, &map, |freq| {
        active_slice
            .iter()
            .map(|c| c.magnitude_db_at(freq, sr))
            .sum()
    });
    if used_count == 0 {
        draw_empty_canvas_hint(&painter, &map);
    }

    // 8. Render ONLY `used == true` Nodes (selected node last on top) with Live GR rings
    let hovered_node =
        pointer_pos.and_then(|p| find_hit_used_node(p, &node_positions, state.selected_band));
    let solo_now = state.effective_solo(params);
    for (i, band) in params.bands.iter().enumerate() {
        if !band.used.value() || state.selected_band == Some(i) {
            continue;
        }
        let gr = shared.band_gr.load(i);
        let data = build_node_render_data(params, i, solo_now == Some(i), gr);
        draw_node(&painter, &map, &data, hovered_node == Some(i), false);
    }
    if let Some(sel_idx) = state.selected_band {
        if params.bands[sel_idx].used.value() {
            let gr = shared.band_gr.load(sel_idx);
            let sel_data = build_node_render_data(params, sel_idx, solo_now == Some(sel_idx), gr);
            draw_node(
                &painter,
                &map,
                &sel_data,
                hovered_node == Some(sel_idx),
                true,
            );
        }
    }

    // 9. Piano Roll Frequency Strip (20px at bottom of display) + Click-to-Snap Note
    let mut in_piano = false;
    if state.show_piano_roll {
        let piano_rect = Rect::from_min_max(
            Pos2::new(response.rect.min.x, canvas_max_y),
            response.rect.max,
        );
        let hov_piano_freq = pointer_pos
            .filter(|p| piano_rect.contains(*p))
            .map(|p| map.x_to_freq(p.x));
        in_piano = hov_piano_freq.is_some();
        let snapped_key = draw_piano_roll(
            &painter,
            &map,
            piano_rect,
            &used_band_freqs[..used_count],
            state.selected_band,
            hov_piano_freq,
        );
        if in_piano && ui.input(|i| i.pointer.primary_pressed()) {
            if let Some(note_freq) = snapped_key {
                let f = note_freq.clamp(10.0, 30000.0);
                if let Some(sel) = state
                    .selected_band
                    .filter(|&i| i < NUM_BANDS && params.bands[i].used.value())
                {
                    let band = &params.bands[sel];
                    setter.begin_set_parameter(&band.frequency);
                    setter.set_parameter(&band.frequency, f);
                    setter.end_set_parameter(&band.frequency);
                } else {
                    let _ = create_band_at(setter, params, state, f, 0.0);
                }
            }
        }
    }

    // 10. Pro-Q 4 Floating Band HUD Popover near the selected node
    state.last_hud_rect = None;
    if state.show_floating_hud {
        if let Some(sel_idx) = state
            .selected_band
            .filter(|&i| i < NUM_BANDS && params.bands[i].used.value())
        {
            if let Some(node_pos) = node_positions[sel_idx] {
                let hud_rect = draw_floating_hud_for_selected(
                    ui, &map, node_pos, setter, state, params, shared, sel_idx,
                );
                state.last_hud_rect = Some(hud_rect);
            }
        }
    }

    // 11. Canvas Mouse & Keyboard Gestures (blocked when clicking inside Floating HUD or Piano Roll)
    if !in_piano {
        handle_canvas_gestures(
            ui,
            &response,
            &map,
            setter,
            state,
            params,
            shared,
            &node_positions,
            active_slice,
            hovered_node,
            soloed_band,
            state.last_hud_rect,
            hovered_grab_peak,
        );
    }

    if let Some(h_idx) = hovered_node {
        if params.bands[h_idx].used.value()
            && !state
                .last_hud_rect
                .is_some_and(|r| pointer_pos.is_some_and(|p| r.contains(p)))
        {
            let gr = shared.band_gr.load(h_idx);
            let tip_data = build_node_render_data(params, h_idx, solo_now == Some(h_idx), gr);
            show_node_tooltip(&response, &tip_data);
        }
    }
}

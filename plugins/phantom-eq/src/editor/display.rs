//! Main EQ Frequency Response Display & Pro-Q 4 Empty-Canvas Node Renderer
//! Renders adaptive-scale grid, dual Pre/Post + Peak Hold spectrum, cascaded band curves,
//! combined EQ curve, empty-canvas hint when 0 bands exist, and active nodes with live GR rings.

use super::display_helpers::{band_cascaded_coeffs, build_node_render_data, find_hit_used_node};
use super::gestures::handle_canvas_gestures;
use super::{EditorShared, EditorState};
use crate::params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{Pos2, Sense, Ui, Vec2};
use phantom_dsp::CascadedCoeffs;
use phantom_ui::{
    band_color, draw_band_curve_fn, draw_canvas_grid, draw_combined_curve_fn, draw_dual_spectrum,
    draw_empty_canvas_hint, draw_node, show_node_tooltip, EqCanvasMapping,
};

pub fn draw_main_display(
    ui: &mut Ui,
    setter: &ParamSetter,
    state: &mut EditorState,
    params: &PhantomEqParams,
    shared: &EditorShared,
    size: Vec2,
) {
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let map = EqCanvasMapping::with_db_range(response.rect, state.scale_db);
    let soloed_band = state.effective_solo(params);
    let natural_phase = params.mode.value() == ProcessingModeParam::NaturalPhase;
    let sr = state.sample_rate;

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

    // 3. Collect cascaded coeffs for all active (`used && enabled`) bands on stack (zero allocs)
    let mut active_coeffs = [CascadedCoeffs::default(); NUM_BANDS];
    let mut active_count = 0usize;
    let mut used_count = 0usize;
    let mut node_positions: [Option<Pos2>; NUM_BANDS] = [None; NUM_BANDS];

    for (i, band) in params.bands.iter().enumerate() {
        if !band.used.value() {
            continue;
        }
        used_count += 1;
        node_positions[i] = Some(Pos2::new(
            map.freq_to_x(band.frequency.value()),
            map.db_to_y(band.gain.value()),
        ));
        if band.enabled.value() {
            active_coeffs[active_count] = band_cascaded_coeffs(band, sr, natural_phase);
            active_count += 1;
        }
    }

    // 4. Selected Band Individual Curve Overlay (only if a used band is selected)
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

    // Highlight soloed band curve if different from selected
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

    // 5. Combined Master EQ Response Curve
    let active_slice = &active_coeffs[..active_count];
    draw_combined_curve_fn(&painter, &map, |freq| {
        active_slice
            .iter()
            .map(|c| c.magnitude_db_at(freq, sr))
            .sum()
    });

    // Empty Pro-Q 4 startup hint when 0 bands have been created
    if used_count == 0 {
        draw_empty_canvas_hint(&painter, &map);
    }

    // 6. Pro-Q 4 Mouse & Keyboard Gestures
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let hovered_node =
        pointer_pos.and_then(|p| find_hit_used_node(p, &node_positions, state.selected_band));

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
    );

    // 7. Render ONLY `used == true` Nodes (selected node last on top) with Live GR rings
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

    if let Some(h_idx) = hovered_node {
        if params.bands[h_idx].used.value() {
            let gr = shared.band_gr.load(h_idx);
            let tip_data = build_node_render_data(params, h_idx, solo_now == Some(h_idx), gr);
            show_node_tooltip(&response, &tip_data);
        }
    }
}

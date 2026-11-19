//! Main EQ Frequency Response Display & Pro-Q Interactive Node Canvas
//! Handles grid, spectrum, selected-band overlay curve, combined curve, and Pro-Q mouse gestures:
//! - Drag node: adjust frequency & gain
//! - Scroll wheel: adjust Q
//! - Double-click node: reset gain to 0.0 dB
//! - Double-click empty canvas: activate/move nearest disabled or unused band
//! - Right-click node: toggle band enabled/disabled

use super::display_helpers::{
    band_coeffs, build_node_render_data, find_band_for_canvas_double_click, find_hit_node,
};
use super::{EditorShared, EditorState};
use crate::params::{PhantomEqParams, NUM_BANDS};
use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{Pos2, Sense, Ui, Vec2};
use phantom_dsp::BiquadCoeffs;
use phantom_ui::{
    draw_band_curve, draw_canvas_grid, draw_combined_curve, draw_node, draw_spectrum,
    show_node_tooltip, EqCanvasMapping, BAND_COLORS,
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
    let map = EqCanvasMapping::new(response.rect);
    let soloed_band = shared.solo_band();

    // 1. Grid
    draw_canvas_grid(&painter, &map);

    // 2. Real-time Spectrum Analyzer
    if state.show_spectrum {
        draw_spectrum(&painter, &map, &shared.spectrum);
    }

    // 3. Selected Band Individual Curve Overlay (underneath master curve)
    let sel_idx = state.selected_band.min(NUM_BANDS - 1);
    let sel_band = &params.bands[sel_idx];
    let sel_coeffs = band_coeffs(sel_band, state.sample_rate);
    draw_band_curve(
        &painter,
        &map,
        &sel_coeffs,
        state.sample_rate,
        BAND_COLORS[sel_idx % 8],
        sel_band.enabled.value(),
        soloed_band == Some(sel_idx),
    );

    // Highlight a soloed band's curve if it differs from the selected band
    if let Some(s_idx) = soloed_band {
        if s_idx != sel_idx {
            let sb = &params.bands[s_idx];
            let sc = band_coeffs(sb, state.sample_rate);
            draw_band_curve(
                &painter,
                &map,
                &sc,
                state.sample_rate,
                BAND_COLORS[s_idx % 8],
                sb.enabled.value(),
                true,
            );
        }
    }

    // 4. Master Combined EQ Response Curve (stack-allocated, zero heap allocs)
    let mut active_coeffs = [BiquadCoeffs::default(); NUM_BANDS];
    let mut active_count = 0usize;
    for band in &params.bands {
        if band.enabled.value() {
            active_coeffs[active_count] = band_coeffs(band, state.sample_rate);
            active_count += 1;
        }
    }
    draw_combined_curve(
        &painter,
        &map,
        &active_coeffs[..active_count],
        state.sample_rate,
    );

    // 5. Pro-Q Mouse Interactions
    let node_positions: [Pos2; NUM_BANDS] = std::array::from_fn(|i| {
        let b = &params.bands[i];
        Pos2::new(
            map.freq_to_x(b.frequency.value()),
            map.db_to_y(b.gain.value()),
        )
    });

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let interact_pos = response.interact_pointer_pos().or(pointer_pos);
    let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
    let primary_down = ui.input(|i| i.pointer.primary_down());
    let pointer_moved = ui.input(|i| i.pointer.delta().length_sq() > 0.0);
    let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);

    let hovered_node = pointer_pos.and_then(|p| find_hit_node(p, &node_positions, sel_idx));

    // Active drag continuation / release
    if let Some(drag_idx) = state.dragging_band {
        let band = &params.bands[drag_idx];
        if primary_down {
            if pointer_moved {
                if let Some(pos) = pointer_pos {
                    setter.set_parameter(&band.frequency, map.x_to_freq(pos.x));
                    setter.set_parameter(&band.gain, map.y_to_db(pos.y));
                }
            }
        } else {
            setter.end_set_parameter(&band.frequency);
            setter.end_set_parameter(&band.gain);
            state.dragging_band = None;
        }
    }

    // Double-click: node -> reset gain to 0.0 dB; empty canvas -> activate/move nearest band
    if response.double_clicked() {
        if let Some(drag_idx) = state.dragging_band.take() {
            let drag_band = &params.bands[drag_idx];
            setter.end_set_parameter(&drag_band.frequency);
            setter.end_set_parameter(&drag_band.gain);
        }
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_node(pos, &node_positions, sel_idx) {
                state.selected_band = hit_idx;
                let band = &params.bands[hit_idx];
                setter.begin_set_parameter(&band.gain);
                setter.set_parameter(&band.gain, 0.0);
                setter.end_set_parameter(&band.gain);
            } else if map.rect.contains(pos) {
                let click_freq = map.x_to_freq(pos.x).clamp(10.0, 30000.0);
                let click_gain = map.y_to_db(pos.y).clamp(-24.0, 24.0);
                let target_idx = find_band_for_canvas_double_click(params, click_freq);
                state.selected_band = target_idx;
                let band = &params.bands[target_idx];
                setter.begin_set_parameter(&band.enabled);
                setter.set_parameter(&band.enabled, true);
                setter.end_set_parameter(&band.enabled);
                setter.begin_set_parameter(&band.frequency);
                setter.set_parameter(&band.frequency, click_freq);
                setter.end_set_parameter(&band.frequency);
                setter.begin_set_parameter(&band.gain);
                setter.set_parameter(&band.gain, click_gain);
                setter.end_set_parameter(&band.gain);
            }
        }
    } else if response.secondary_clicked() {
        // Right-click on a node -> toggle Band On/Off
        if let Some(pos) = interact_pos {
            if let Some(hit_idx) = find_hit_node(pos, &node_positions, sel_idx) {
                let band = &params.bands[hit_idx];
                setter.begin_set_parameter(&band.enabled);
                setter.set_parameter(&band.enabled, !band.enabled.value());
                setter.end_set_parameter(&band.enabled);
            }
        }
    } else if primary_pressed && state.dragging_band.is_none() {
        // Primary press on a node -> select & begin drag
        if let Some(hit_idx) = hovered_node {
            let band = &params.bands[hit_idx];
            state.selected_band = hit_idx;
            state.dragging_band = Some(hit_idx);
            setter.begin_set_parameter(&band.frequency);
            setter.begin_set_parameter(&band.gain);
        }
    }

    // Mouse wheel adjusts Q of hovered node (or selected band while pointer is in canvas)
    if scroll_delta.abs() > 0.05 && pointer_pos.is_some_and(|p| map.rect.contains(p)) {
        let q_idx = hovered_node.unwrap_or(state.selected_band.min(NUM_BANDS - 1));
        let band = &params.bands[q_idx];
        let mult = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
        let new_q = (band.q.value() * mult).clamp(0.1, 30.0);
        setter.begin_set_parameter(&band.q);
        setter.set_parameter(&band.q, new_q);
        setter.end_set_parameter(&band.q);
    }

    // 6. Render Nodes (selected node rendered last so it stays crisp on top)
    let active_sel = state.selected_band.min(NUM_BANDS - 1);
    for i in 0..NUM_BANDS {
        if i == active_sel {
            continue;
        }
        let data = build_node_render_data(params, i, soloed_band == Some(i));
        draw_node(&painter, &map, &data, hovered_node == Some(i), false);
    }
    let sel_data = build_node_render_data(params, active_sel, soloed_band == Some(active_sel));
    draw_node(
        &painter,
        &map,
        &sel_data,
        hovered_node == Some(active_sel),
        true,
    );

    if let Some(h_idx) = hovered_node {
        let tip_data = build_node_render_data(params, h_idx, soloed_band == Some(h_idx));
        show_node_tooltip(&response, &tip_data);
    }
}

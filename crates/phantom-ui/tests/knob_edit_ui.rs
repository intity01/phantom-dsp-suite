//! Headless egui integration tests for Rotary Knob inline keyboard editing & Floating HUD layout.

use nih_plug_egui::egui::{
    CentralPanel, Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect,
    Vec2,
};
use phantom_ui::{
    draw_editable_compact_knob, draw_floating_band_hud, freq_to_norm, norm_to_freq,
    BandInspectorData, BandInspectorEvents, EqCanvasMapping, KnobEditKind, FLOATING_HUD_HEIGHT,
    FLOATING_HUD_WIDTH,
};

fn click_events(p: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(p),
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]
}

fn key_press(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn run_freq_knob(ctx: &Context, time: f64, events: Vec<Event>, norm: f32) -> (Option<f32>, Rect) {
    let (mut out, mut r) = (None, Rect::NOTHING);
    let input = RawInput {
        time: Some(time),
        events,
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        CentralPanel::default().show(ctx, |ui| {
            let start = ui.cursor().min;
            out = draw_editable_compact_knob(
                ui,
                "k_freq",
                "FREQ",
                "1.00 kHz",
                norm,
                freq_to_norm(1000.0),
                Color32::WHITE,
                36.0,
                false,
                KnobEditKind::FreqHz,
            );
            r = Rect::from_min_size(start, Vec2::new(62.0, 58.0));
        });
    });
    (out, r)
}

#[test]
fn test_knob_inline_edit_enter_escape_focus_loss_and_dial_double_click() {
    let ctx = Context::default();
    let (_, r) = run_freq_knob(&ctx, 0.0, vec![], freq_to_norm(1000.0));
    let val_pt = Pos2::new(r.center().x, r.max.y - 6.0);
    let dial_pt = Pos2::new(r.center().x, r.min.y + 28.0);

    // 1. Click value strip -> type "A4" + Enter -> commits 440.0 Hz (replaces full selection)
    let _ = run_freq_knob(&ctx, 0.1, click_events(val_pt), freq_to_norm(1000.0));
    let (res, _) = run_freq_knob(
        &ctx,
        0.2,
        vec![Event::Text("A4".into()), key_press(Key::Enter)],
        freq_to_norm(1000.0),
    );
    assert!((norm_to_freq(res.expect("Enter should commit")) - 440.0).abs() < 1e-2);
    // Next idle frame must not re-commit
    assert_eq!(
        run_freq_knob(&ctx, 0.3, vec![], freq_to_norm(440.0)).0,
        None
    );

    // 2. Click value strip -> type "2.5k" + Escape -> cancels cleanly without mutating
    let _ = run_freq_knob(&ctx, 0.5, click_events(val_pt), freq_to_norm(440.0));
    let _ = run_freq_knob(
        &ctx,
        0.6,
        vec![Event::Text("2.5k".into())],
        freq_to_norm(440.0),
    );
    let (esc, _) = run_freq_knob(&ctx, 0.7, vec![key_press(Key::Escape)], freq_to_norm(440.0));
    assert_eq!(esc, None);
    assert_eq!(
        run_freq_knob(&ctx, 0.8, vec![], freq_to_norm(440.0)).0,
        None
    );

    // 3. Click value strip -> type "2.5k" -> click outside -> commits 2500.0 Hz on focus loss
    let _ = run_freq_knob(&ctx, 1.0, click_events(val_pt), freq_to_norm(440.0));
    let _ = run_freq_knob(
        &ctx,
        1.1,
        vec![Event::Text("2.5k".into())],
        freq_to_norm(440.0),
    );
    let (blur, _) = run_freq_knob(
        &ctx,
        1.2,
        click_events(Pos2::new(300.0, 300.0)),
        freq_to_norm(440.0),
    );
    assert!((norm_to_freq(blur.expect("focus loss should commit")) - 2500.0).abs() < 1e-1);

    // 4. Double-click dial circle -> resets to default_normalized (1000.0 Hz)
    let _ = run_freq_knob(&ctx, 2.0, click_events(dial_pt), freq_to_norm(440.0));
    let (rst, _) = run_freq_knob(&ctx, 2.1, click_events(dial_pt), freq_to_norm(440.0));
    assert!((norm_to_freq(rst.expect("dial double-click resets")) - 1000.0).abs() < 1e-1);
}

#[test]
fn test_floating_hud_dimensions_and_all_four_editable_knobs() {
    let ctx = Context::default();
    let map = EqCanvasMapping::new(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 500.0)));
    let data = BandInspectorData {
        selected_idx: 0,
        num_bands: 24,
        used_bands_mask: 1,
        is_enabled: true,
        is_soloed: false,
        soloed_band: None,
        filter_shape_idx: 0,
        slope_idx: 1,
        routing_idx: 0,
        freq: 1000.0,
        gain: 0.0,
        q: 1.0,
        dyn_range: 0.0,
        dyn_threshold: -24.0,
        dyn_attack_ms: 10.0,
        dyn_release_ms: 100.0,
        dyn_auto_thresh: true,
        dyn_ext_sc: false,
        spectral_enabled: false,
        live_gr_db: 0.0,
    };
    let run_hud = |time: f64, events: Vec<Event>| -> (BandInspectorEvents, Rect) {
        let mut res = (BandInspectorEvents::default(), Rect::NOTHING);
        let input = RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 500.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                res = draw_floating_band_hud(
                    ui,
                    &map,
                    Pos2::new(450.0, 260.0),
                    &data,
                    &["Bell"],
                    &["6 dB/oct", "12 dB/oct"],
                    &["Stereo"],
                );
            });
        });
        res
    };

    let (_, hud_rect) = run_hud(0.0, vec![]);
    assert!((hud_rect.width() - FLOATING_HUD_WIDTH).abs() < 1e-3);
    assert!((hud_rect.height() - FLOATING_HUD_HEIGHT).abs() < 1e-3);

    // 4 knobs centered inside inner width (356px): row_w = 4*62 + 3*6 = 266, pad = 45
    // Inner min.x = hud_rect.min.x + 6.0; first knob x = min.x + 51.0, pitch = 68.0
    // Knobs bottom readout center y ~= hud_rect.min.y + 4 + 22 + 2 + 3 + 58 - 6 = hud_rect.min.y + 83
    let base_x = hud_rect.min.x + 51.0 + 31.0;
    let val_y = hud_rect.min.y + 83.0;
    let pts = [
        Pos2::new(base_x, val_y),
        Pos2::new(base_x + 68.0, val_y),
        Pos2::new(base_x + 136.0, val_y),
        Pos2::new(base_x + 204.0, val_y),
    ];

    // FREQ -> "C#4" (~277.18 Hz)
    let _ = run_hud(0.2, click_events(pts[0]));
    let (ev, _) = run_hud(0.3, vec![Event::Text("C#4".into()), key_press(Key::Enter)]);
    assert!((ev.set_freq.expect("FREQ commit") - 277.1826).abs() < 0.2);

    // GAIN -> "-4.5 dB"
    let _ = run_hud(0.5, click_events(pts[1]));
    let (ev, _) = run_hud(
        0.6,
        vec![Event::Text("-4.5 dB".into()), key_press(Key::Enter)],
    );
    assert!((ev.set_gain.expect("GAIN commit") - (-4.5)).abs() < 1e-2);

    // Q -> "3.5"
    let _ = run_hud(0.8, click_events(pts[2]));
    let (ev, _) = run_hud(0.9, vec![Event::Text("3.5".into()), key_press(Key::Enter)]);
    assert!((ev.set_q.expect("Q commit") - 3.5).abs() < 1e-2);

    // DYN -> "-8.0"
    let _ = run_hud(1.1, click_events(pts[3]));
    let (ev, _) = run_hud(1.2, vec![Event::Text("-8.0".into()), key_press(Key::Enter)]);
    assert!((ev.set_dyn_range.expect("DYN commit") - (-8.0)).abs() < 1e-2);
}

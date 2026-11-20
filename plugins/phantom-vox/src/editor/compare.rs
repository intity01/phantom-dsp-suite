//! Compare controls in the header: A/B settings slots, a copy button, and
//! "Hold: Original" to hear the unprocessed vocal at the same loudness.

use crate::ab_slots::{Snapshot, AB_SKIP_IDS};
use crate::live_mirror::LiveSetter;
use crate::params::PhantomVoxParams;
use nih_plug::prelude::Params;
use nih_plug_egui::egui::Ui;
use phantom_ui::widgets::inspector_controls::draw_hud_pill;
use phantom_ui::{ACCENT_AMBER, ACCENT_CYAN, ACCENT_PURPLE};
use std::sync::atomic::{AtomicBool, Ordering};

/// Current settings of every parameter A/B covers.
pub fn capture(params: &PhantomVoxParams) -> Snapshot {
    params
        .param_map()
        .into_iter()
        .filter(|(id, _, _)| !AB_SKIP_IDS.contains(&id.as_str()))
        // SAFETY: the pointers belong to `params`, which outlives this call.
        .map(|(id, ptr, _)| (id, unsafe { ptr.unmodulated_normalized_value() }))
        .collect()
}

/// Loads a snapshot as one host gesture per parameter, so Cubase records it like a user edit.
pub fn apply(setter: &LiveSetter, params: &PhantomVoxParams, snap: &Snapshot) {
    let map = params.param_map();
    let ctx = setter.host.raw_context;
    for (id, norm) in snap {
        if AB_SKIP_IDS.contains(&id.as_str()) {
            continue;
        }
        let Some((_, ptr, _)) = map.iter().find(|(pid, _, _)| pid == id) else {
            continue;
        };
        setter.live.write_normalized(*ptr, *norm);
        // SAFETY: `ptr` belongs to `params`, which outlives this call.
        unsafe {
            ctx.raw_begin_set_parameter(*ptr);
            ctx.raw_set_parameter_normalized(*ptr, *norm);
            ctx.raw_end_set_parameter(*ptr);
        }
    }
}

pub fn draw_compare_controls(
    ui: &mut Ui,
    setter: &LiveSetter,
    params: &PhantomVoxParams,
    audition: &AtomicBool,
) {
    let b_active = params.ab_slots.read().map(|s| s.b_active).unwrap_or(false);
    ui.spacing_mut().item_spacing.x = 4.0;
    let mut switch_to = None;
    for (to_b, name) in [(false, "A"), (true, "B")] {
        let res = draw_hud_pill(ui, 22.0, 18.0, name, b_active == to_b, ACCENT_PURPLE)
            .on_hover_text(format!(
                "Settings slot {name}: click to switch to it\n\
                 Each slot remembers its own settings, so you can flip between two ideas"
            ));
        if res.clicked() {
            switch_to = Some(to_b);
        }
    }
    if let Some(to_b) = switch_to {
        let current = capture(params);
        let next = params
            .ab_slots
            .write()
            .ok()
            .and_then(|mut s| s.switch_to(to_b, current));
        if let Some(snap) = next {
            apply(setter, params, &snap);
        }
    }

    let (from, to) = if b_active { ("B", "A") } else { ("A", "B") };
    let copy = draw_hud_pill(ui, 62.0, 18.0, &format!("Copy to {to}"), false, ACCENT_CYAN)
        .on_hover_text(format!("Copy the current settings ({from}) into slot {to}"));
    if copy.clicked() {
        let current = capture(params);
        if let Ok(mut s) = params.ab_slots.write() {
            s.copy_to_other(current);
        }
    }

    let held = audition.load(Ordering::Relaxed);
    let hold = draw_hud_pill(ui, 86.0, 18.0, "Hold: Original", held, ACCENT_AMBER).on_hover_text(
        "Hold to hear the original vocal, turned to the same loudness as Vox\n\
             Release to hear Vox again",
    );
    audition.store(hold.is_pointer_button_down_on(), Ordering::Relaxed);
}

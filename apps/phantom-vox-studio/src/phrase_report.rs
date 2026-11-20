use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Ui};
use phantom_vox_dsp::{ConfidenceLevel, PitchMatchMode};

use crate::state::StudioApp;
use crate::theme::{
    confidence_color, pill_badge, with_alpha, BG_CARD, TEXT_BRIGHT, TEXT_DIMMED, TEXT_MUTED,
};

pub fn draw_phrase_report(ui: &mut Ui, app: &mut StudioApp, idx: usize) {
    ui.label(
        RichText::new("PHRASE CONFIDENCE REPORT (Click to Jump)")
            .size(10.5)
            .strong()
            .color(TEXT_MUTED),
    );
    ui.add_space(4.0);

    let Some(al) = app.tracks[idx].alignment.clone() else {
        ui.label(RichText::new("Not analyzed yet — click Batch Process All.").color(TEXT_DIMMED));
        return;
    };

    let mut jump_sec: Option<f32> = None;
    egui::ScrollArea::vertical()
        .id_salt("phrase_report_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if al.phrases.is_empty() {
                ui.label(RichText::new("No vocal phrases detected.").color(TEXT_DIMMED));
            }
            for (i, phr) in al.phrases.iter().enumerate() {
                let col = confidence_color(phr.level);
                let lvl_txt = match phr.level {
                    ConfidenceLevel::High => "HIGH",
                    ConfidenceLevel::Medium => "MED",
                    ConfidenceLevel::Low => "LOW",
                };
                let frame_resp = egui::Frame::NONE
                    .fill(BG_CARD)
                    .stroke(Stroke::new(1.0_f32, with_alpha(col, 130)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::same(7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            pill_badge(ui, lvl_txt, col, with_alpha(col, 38));
                            ui.label(
                                RichText::new(format!(
                                    "Phrase #{}  ({:.2}s – {:.2}s)",
                                    i + 1,
                                    phr.start_sec,
                                    phr.end_sec
                                ))
                                .size(11.0)
                                .strong()
                                .color(TEXT_BRIGHT),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("{:.0}%", phr.confidence * 100.0))
                                            .size(11.0)
                                            .color(col),
                                    );
                                },
                            );
                        });
                        if let Some(w) = &phr.warning {
                            ui.label(
                                RichText::new(w)
                                    .size(10.5)
                                    .color(Color32::from_rgb(252, 165, 165)),
                            );
                        }
                    })
                    .response
                    .interact(egui::Sense::click());

                if frame_resp.clicked() {
                    jump_sec = Some(phr.start_sec);
                }
                ui.add_space(4.0);
            }
        });

    if let Some(sec) = jump_sec {
        app.audio.seek_sec(sec);
        app.scroll_sec = (sec - 0.35).max(0.0);
        app.status_line = format!("Jumped to phrase at {sec:.2}s");
    }
}

pub fn pitch_mode_label(mode: PitchMatchMode) -> &'static str {
    match mode {
        PitchMatchMode::Off => "Off",
        PitchMatchMode::MatchAllToGuide => "Match All to Guide",
        PitchMatchMode::MatchUnisonOnly => "Unison Only (Harmony Safe)",
        PitchMatchMode::MatchUnisonAndTuneHarmony => "Unison + Tune Harmony",
    }
}

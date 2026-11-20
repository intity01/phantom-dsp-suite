use phantom_vox_dsp::{generate_demo_vocal_stack, PitchMatchMode};

use crate::state::{StudioApp, StudioTrack};

impl StudioApp {
    /// Loads the 1-click synthetic multi-track vocal stack (Guide + Dub L + Dub R + 3rd Harmony).
    pub fn load_demo_stack(&mut self) {
        self.push_undo();
        self.audio.stop();
        self.tracks.clear();
        self.warnings.clear();
        self.project_path = None;
        self.sample_rate = 48_000;

        let (guide, dub_l, dub_r, harm) = generate_demo_vocal_stack(self.sample_rate);
        let id_g = self.alloc_track_id();
        let id_l = self.alloc_track_id();
        let id_r = self.alloc_track_id();
        let id_h = self.alloc_track_id();

        let tr_g = StudioTrack::new(id_g, "Guide Lead".into(), None, None, true, guide, 0.0, 0);
        let tr_l = StudioTrack::new(
            id_l,
            "Dub Left (Early/Flat)".into(),
            None,
            None,
            false,
            dub_l,
            -0.65,
            0,
        );
        let tr_r = StudioTrack::new(
            id_r,
            "Dub Right (Late/Jitter)".into(),
            None,
            None,
            false,
            dub_r,
            0.65,
            1,
        );
        let mut tr_h = StudioTrack::new(
            id_h,
            "3rd Harmony (+4 st)".into(),
            None,
            None,
            false,
            harm,
            0.35,
            2,
        );
        tr_h.config.pitch_mode = PitchMatchMode::MatchUnisonOnly;

        self.tracks = vec![tr_g, tr_l, tr_r, tr_h];
        self.selected_track_id = Some(id_l);
        self.loop_start_sec = 0.0;
        self.loop_end_sec = self.total_duration_sec();
        self.scroll_sec = 0.0;
        self.sync_audio_mix();
        self.batch_process_all();
        self.status_line =
            "Loaded Demo Vocal Stack (Guide + Dub L + Dub R + 3rd Harmony) — aligning..."
                .to_owned();
    }
}

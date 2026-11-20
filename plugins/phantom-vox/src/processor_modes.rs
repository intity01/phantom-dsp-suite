use crate::params::{VoicesParam, VoxModeParam};
use crate::processor::VoxBlockConfig;
use phantom_dsp::OnePoleSmoother;
use phantom_vox_dsp::{DoublerParams, LiveSidechainAligner, VocalDoublerEngine};

const MIX_SUBBLOCK: usize = 32;

#[allow(clippy::too_many_arguments)]
pub fn run_mode_chunk(
    cfg: &VoxBlockConfig,
    mono_in: bool,
    n: usize,
    dry_l: &[f32],
    dry_r: &[f32],
    g_l: &[f32],
    g_r: &[f32],
    aln_l: &mut [f32],
    aln_r: &mut [f32],
    wet_l: &mut [f32],
    wet_r: &mut [f32],
    has_sidechain_signal: bool,
    aligner: &mut LiveSidechainAligner,
    doubler: &mut VocalDoublerEngine,
    mix_smooth: &mut OnePoleSmoother,
) {
    match cfg.mode {
        VoxModeParam::Doubler => {
            if has_sidechain_signal {
                aligner.process_mono(dry_l, g_l, aln_l, 0.0, 0.0);
            }
            run_doubler(
                cfg, mono_in, dry_l, dry_r, wet_l, wet_r, n, doubler, mix_smooth,
            );
        }
        VoxModeParam::LiveAlign => {
            mix_smooth.skip(n);
            let ms = mix_smooth.current();
            let (t, l) = (
                (cfg.align_tightness * ms).clamp(0.0, 1.0),
                (cfg.level_match * ms).clamp(0.0, 1.0),
            );
            if mono_in {
                aligner.process_mono(dry_l, g_l, wet_l, t, l);
                wet_r[..n].copy_from_slice(&wet_l[..n]);
            } else {
                aligner.process_stereo(dry_l, dry_r, g_l, g_r, wet_l, wet_r, t, l);
            }
            if cfg.mono_check {
                for i in 0..n {
                    let m = 0.5 * (wet_l[i] + wet_r[i]);
                    (wet_l[i], wet_r[i]) = (m, m);
                }
            }
        }
        VoxModeParam::AlignAndDouble => {
            let (t, l) = (
                cfg.align_tightness.clamp(0.0, 1.0),
                cfg.level_match.clamp(0.0, 1.0),
            );
            if mono_in {
                aligner.process_mono(dry_l, g_l, aln_l, t, l);
                run_doubler(
                    cfg, true, aln_l, aln_l, wet_l, wet_r, n, doubler, mix_smooth,
                );
            } else {
                aligner.process_stereo(dry_l, dry_r, g_l, g_r, aln_l, aln_r, t, l);
                run_doubler(
                    cfg, false, aln_l, aln_r, wet_l, wet_r, n, doubler, mix_smooth,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_doubler(
    cfg: &VoxBlockConfig,
    mono_in: bool,
    in_l: &[f32],
    in_r: &[f32],
    out_l: &mut [f32],
    out_r: &mut [f32],
    n: usize,
    doubler: &mut VocalDoublerEngine,
    mix_smooth: &mut OnePoleSmoother,
) {
    let mut dp = DoublerParams {
        timing_ms: cfg.timing_ms,
        detune_cents: cfg.detune_cents,
        formant_st: cfg.formant_st,
        width: cfg.width,
        voices_4: cfg.voices == VoicesParam::FourVoices,
        sibilance_safe: cfg.sibilance_safe,
        mono_check: cfg.mono_check,
        mix: mix_smooth.current(),
    };
    let step = if mix_smooth.is_settled(1e-4) {
        n
    } else {
        MIX_SUBBLOCK
    };
    let mut pos = 0;
    while pos < n {
        let sub = (n - pos).min(step);
        if step < n {
            mix_smooth.skip(sub);
            dp.mix = mix_smooth.current();
        }
        let r = pos..pos + sub;
        if mono_in {
            doubler.process_mono_to_stereo(
                &in_l[r.clone()],
                &mut out_l[r.clone()],
                &mut out_r[r],
                &dp,
            );
        } else {
            doubler.process_stereo(
                &in_l[r.clone()],
                &in_r[r.clone()],
                &mut out_l[r.clone()],
                &mut out_r[r],
                &dp,
            );
        }
        pos += sub;
    }
}

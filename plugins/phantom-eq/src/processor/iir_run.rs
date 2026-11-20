//! Zero-Allocation IIR Sub-Block & Polyphase Oversampled Sample Execution Loop
//! Drives 16-sample smoothed parameter updates, 1x/2x/4x polyphase OS, Dynamic/Spectral EQ,
//! Character, Auto-Gain, Solo ramp, smoothed Output Gain, and 10 ms Bypass Crossfade.

use super::iir::{advance_iir_subblock, prepare_iir_bands, publish_band_gr, ActiveBand};
use super::EqProcessor;
use crate::dsp_state::{process_cascaded_placement, process_spectral_placement, SharedBandGr};
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{
    CascadedCoeffs, CascadedStereoFilter, ChannelTarget, CharacterMode, DynamicDetector,
    OversampleFactor, Oversampler2x, Oversampler4x, SpectralBandProcessor, SMOOTH_SUBBLOCK,
};

impl EqProcessor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn process_iir_block(
        &mut self,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        natural_phase: bool,
        snap_initial: bool,
        char_mode: CharacterMode,
        auto_gain_en: bool,
        solo_target: f32,
        solo_placement: ChannelTarget,
        left: &mut [f32],
        right: &mut [f32],
        sc_left: &[f32],
        sc_right: &[f32],
        shared_gr: &SharedBandGr,
    ) {
        let total = left.len().min(right.len());
        let os_factor: OversampleFactor = live.read(&params.oversampling).into();
        let effective_sr = self.sample_rate * os_factor.multiplier() as f32;
        let cur_gs = self.gain_scale_smoother.current();

        let (mut active, count) = prepare_iir_bands(
            params,
            live,
            &mut self.filters,
            &mut self.dynamic_detectors,
            &mut self.spectral_processors,
            &mut self.band_smoothers,
            &mut self.band_was_active,
            shared_gr,
            effective_sr,
            natural_phase,
            cur_gs,
            snap_initial,
        );

        let mut offset = 0;
        while offset < total {
            let sub_n = (total - offset).min(SMOOTH_SUBBLOCK);
            advance_iir_subblock(
                &mut active[..count],
                &mut self.filters,
                &mut self.dynamic_detectors,
                &mut self.band_smoothers,
                &mut self.gain_scale_smoother,
                sub_n,
                effective_sr,
                natural_phase,
            );
            for i in offset..offset + sub_n {
                let (pre_l, pre_r) = (left[i], right[i]);
                let sc_l = sc_left.get(i).copied().unwrap_or(0.0);
                let sc_r = sc_right.get(i).copied().unwrap_or(0.0);

                let (eq_l, eq_r) = if self.solo.is_steady_full_solo(solo_target) {
                    (pre_l, pre_r)
                } else {
                    let (fl, fr) = process_iir_sample_os(
                        &mut active[..count],
                        &mut self.filters,
                        &mut self.dynamic_detectors,
                        &mut self.spectral_processors,
                        &mut self.oversampler_2x,
                        &mut self.oversampler_4x,
                        os_factor,
                        effective_sr,
                        natural_phase,
                        pre_l,
                        pre_r,
                        sc_l,
                        sc_r,
                    );
                    let (cl, cr) = self.character.process_stereo(fl, fr, char_mode);
                    self.auto_gain
                        .process_stereo(pre_l, pre_r, cl, cr, auto_gain_en)
                };

                let (ml, mr) =
                    self.solo
                        .apply_sample(pre_l, pre_r, eq_l, eq_r, solo_target, solo_placement);
                let g = self.output_gain_smoother.next();
                let wet = self.bypass_xfade.next_wet();
                let inv = 1.0 - wet;
                let out_l = pre_l * inv + (ml * g) * wet;
                let out_r = pre_r * inv + (mr * g) * wet;
                self.output_meter.process_sample(out_l, out_r);
                left[i] = out_l;
                right[i] = out_r;
            }
            offset += sub_n;
        }

        publish_band_gr(
            &active[..count],
            &self.dynamic_detectors,
            &self.spectral_processors,
            shared_gr,
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn process_iir_sample_os(
    active: &mut [ActiveBand],
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
    spectral: &mut [SpectralBandProcessor; NUM_BANDS],
    os2: &mut Oversampler2x,
    os4: &mut Oversampler4x,
    os_factor: OversampleFactor,
    effective_sr: f32,
    natural_phase: bool,
    l: f32,
    r: f32,
    sc_l: f32,
    sc_r: f32,
) -> (f32, f32) {
    if active.is_empty() {
        return (l, r);
    }
    match os_factor {
        OversampleFactor::Off => process_one_pair(
            active,
            filters,
            detectors,
            spectral,
            effective_sr,
            natural_phase,
            l,
            r,
            sc_l,
            sc_r,
        ),
        OversampleFactor::X2 => {
            let [s0, s1] = os2.upsample_2x(l, r);
            let o0 = process_one_pair(
                active,
                filters,
                detectors,
                spectral,
                effective_sr,
                natural_phase,
                s0.0,
                s0.1,
                sc_l,
                sc_r,
            );
            let o1 = process_one_pair(
                active,
                filters,
                detectors,
                spectral,
                effective_sr,
                natural_phase,
                s1.0,
                s1.1,
                sc_l,
                sc_r,
            );
            os2.downsample_2x(o0, o1)
        }
        OversampleFactor::X4 => {
            let subs = os4.upsample_4x(l, r);
            let mut outs = [(0.0_f32, 0.0_f32); 4];
            for (dst, &(sl, sr_s)) in outs.iter_mut().zip(subs.iter()) {
                *dst = process_one_pair(
                    active,
                    filters,
                    detectors,
                    spectral,
                    effective_sr,
                    natural_phase,
                    sl,
                    sr_s,
                    sc_l,
                    sc_r,
                );
            }
            os4.downsample_4x(outs)
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn process_one_pair(
    active: &mut [ActiveBand],
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
    spectral: &mut [SpectralBandProcessor; NUM_BANDS],
    sr: f32,
    natural_phase: bool,
    mut l: f32,
    mut r: f32,
    sc_l: f32,
    sc_r: f32,
) -> (f32, f32) {
    for band in active.iter_mut() {
        let idx = band.index;
        if band.spectral_dyn {
            (l, r) = process_cascaded_placement(&mut filters[idx], band.placement, l, r);
            (l, r) = process_spectral_placement(
                &mut spectral[idx],
                band.placement,
                l,
                r,
                sr,
                band.freq,
                band.q,
                band.dyn_thresh,
                band.dyn_range,
            );
        } else {
            if band.has_dynamics {
                let dyn_offset = detectors[idx].compute_dynamic_gain_ext(
                    l,
                    r,
                    sc_l,
                    sc_r,
                    band.dyn_ext_sc,
                    band.dyn_thresh,
                    band.dyn_auto,
                    band.dyn_range,
                );
                if (dyn_offset - band.last_applied_dyn).abs() > 0.005 {
                    let dyn_coeffs = CascadedCoeffs::calculate(
                        band.ftype,
                        band.slope,
                        sr,
                        band.freq,
                        band.gain + dyn_offset,
                        band.q,
                        natural_phase,
                    );
                    filters[idx].set_coeffs(&dyn_coeffs);
                    band.last_applied_dyn = dyn_offset;
                }
            }
            (l, r) = process_cascaded_placement(&mut filters[idx], band.placement, l, r);
        }
    }
    (l, r)
}

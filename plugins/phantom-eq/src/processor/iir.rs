//! Zero-Latency & Natural-Phase IIR Processing Core (with 2x/4x Polyphase OS + Dynamic EQ)
//! Strictly zero heap allocations: all active band metadata lives in fixed stack arrays.

use crate::dsp_state::{process_cascaded_placement, SharedBandGr};
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{
    CascadedCoeffs, CascadedStereoFilter, ChannelTarget, DynamicDetector, FilterSlope, FilterType,
    OversampleFactor, Oversampler2x, Oversampler4x,
};

#[derive(Clone, Copy)]
pub struct ActiveBand {
    pub index: usize,
    pub ftype: FilterType,
    pub slope: FilterSlope,
    pub placement: ChannelTarget,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub dyn_range: f32,
    pub dyn_thresh: f32,
    pub dyn_auto: bool,
    pub dyn_ext_sc: bool,
    pub has_dynamics: bool,
    pub last_applied_dyn: f32,
}

impl Default for ActiveBand {
    fn default() -> Self {
        Self {
            index: 0,
            ftype: FilterType::Bell,
            slope: FilterSlope::Db12,
            placement: ChannelTarget::Stereo,
            freq: 1000.0,
            gain: 0.0,
            q: 1.0,
            dyn_range: 0.0,
            dyn_thresh: -18.0,
            dyn_auto: true,
            dyn_ext_sc: false,
            has_dynamics: false,
            last_applied_dyn: 0.0,
        }
    }
}

/// Prepares active (`used && enabled`) bands into a stack array and updates base filter coeffs.
pub fn prepare_iir_bands(
    params: &PhantomEqParams,
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
    shared_gr: &SharedBandGr,
    effective_sr: f32,
    natural_phase: bool,
) -> ([ActiveBand; NUM_BANDS], usize) {
    let mut active = [ActiveBand::default(); NUM_BANDS];
    let mut count = 0usize;

    for (i, band) in params.bands.iter().enumerate() {
        if !(band.used.value() && band.enabled.value()) {
            shared_gr.store(i, 0.0);
            continue;
        }
        let ftype: FilterType = band.filter_type.value().into();
        let slope: FilterSlope = band.slope.value().into();
        let placement: ChannelTarget = band.placement.value().into();
        let freq = band.frequency.value();
        let gain = band.gain.value();
        let q = band.q.value();
        let dyn_range = band.dynamic_range.value();
        let has_dynamics = dyn_range.abs() > 0.01;

        let coeffs =
            CascadedCoeffs::calculate(ftype, slope, effective_sr, freq, gain, q, natural_phase);
        filters[i].set_coeffs(&coeffs);

        if has_dynamics {
            let att = band.dynamic_attack.value();
            let rel = band.dynamic_release.value();
            detectors[i].update_ballistics(effective_sr, att, rel);
            detectors[i].update_sidechain(effective_sr, freq, q);
        } else {
            shared_gr.store(i, 0.0);
        }

        active[count] = ActiveBand {
            index: i,
            ftype,
            slope,
            placement,
            freq,
            gain,
            q,
            dyn_range,
            dyn_thresh: band.dynamic_threshold.value(),
            dyn_auto: band.dynamic_auto_thresh.value(),
            dyn_ext_sc: band.dynamic_ext_sc.value(),
            has_dynamics,
            last_applied_dyn: 0.0,
        };
        count += 1;
    }

    (active, count)
}

/// Processes a single stereo sample through the active 24-band IIR cascade with oversampling.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn process_iir_sample_os(
    active: &mut [ActiveBand],
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
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
    sr: f32,
    natural_phase: bool,
    mut l: f32,
    mut r: f32,
    sc_l: f32,
    sc_r: f32,
) -> (f32, f32) {
    for band in active.iter_mut() {
        let idx = band.index;
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
    (l, r)
}

/// Publishes the latest dynamic gain reduction telemetry for all active dynamic bands.
pub fn publish_band_gr(
    active: &[ActiveBand],
    detectors: &[DynamicDetector; NUM_BANDS],
    shared_gr: &SharedBandGr,
) {
    for band in active {
        if band.has_dynamics {
            shared_gr.store(band.index, detectors[band.index].last_gain_offset_db());
        }
    }
}

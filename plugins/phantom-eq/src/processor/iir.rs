//! Zero-Latency & Natural-Phase IIR Band Preparation & 16-Sample Smoother Sub-Block Updates
//! Strictly zero heap allocations: active band metadata lives in fixed stack arrays.

use crate::dsp_state::{
    scale_band_dyn_range, scale_band_gain, shape_uses_gain_scale, SharedBandGr,
};
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, NUM_BANDS};
use phantom_dsp::{
    BandParamSmoother, CascadedCoeffs, CascadedStereoFilter, ChannelTarget, DynamicDetector,
    FilterSlope, FilterType, OnePoleSmoother, SpectralBandProcessor,
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
    pub raw_dyn_range: f32,
    pub raw_spectral_dyn: bool,
    pub dyn_range: f32,
    pub dyn_thresh: f32,
    pub dyn_auto: bool,
    pub dyn_ext_sc: bool,
    pub has_dynamics: bool,
    pub spectral_dyn: bool,
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
            raw_dyn_range: 0.0,
            raw_spectral_dyn: false,
            dyn_range: 0.0,
            dyn_thresh: -18.0,
            dyn_auto: true,
            dyn_ext_sc: false,
            has_dynamics: false,
            spectral_dyn: false,
            last_applied_dyn: 0.0,
        }
    }
}

/// Prepares active (`used && enabled`) bands into a stack array and updates smoother targets.
#[allow(clippy::too_many_arguments)]
pub fn prepare_iir_bands(
    params: &PhantomEqParams,
    live: &impl ParamRead,
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
    spectral: &mut [SpectralBandProcessor; NUM_BANDS],
    smoothers: &mut [BandParamSmoother; NUM_BANDS],
    band_was_active: &mut [bool; NUM_BANDS],
    shared_gr: &SharedBandGr,
    effective_sr: f32,
    natural_phase: bool,
    cur_gain_scale: f32,
    snap_initial: bool,
) -> ([ActiveBand; NUM_BANDS], usize) {
    let mut active = [ActiveBand::default(); NUM_BANDS];
    let mut count = 0usize;

    for (i, band) in params.bands.iter().enumerate() {
        if !(live.read(&band.used) && live.read(&band.enabled)) {
            band_was_active[i] = false;
            shared_gr.store(i, 0.0);
            continue;
        }
        let ftype: FilterType = live.read(&band.filter_type).into();
        let slope: FilterSlope = live.read(&band.slope).into();
        let placement: ChannelTarget = live.read(&band.placement).into();
        let t_freq = live.read(&band.frequency);
        let t_gain = live.read(&band.gain);
        let t_q = live.read(&band.q);

        if snap_initial {
            smoothers[i].reset(t_freq, t_gain, t_q);
            band_was_active[i] = true;
        } else if !band_was_active[i] {
            filters[i].reset();
            if shape_uses_gain_scale(ftype) {
                smoothers[i].reset(t_freq, 0.0, t_q);
                smoothers[i].set_targets(t_freq, t_gain, t_q);
            } else {
                smoothers[i].reset(t_freq, t_gain, t_q);
            }
            band_was_active[i] = true;
        } else {
            smoothers[i].set_targets(t_freq, t_gain, t_q);
        }

        let (freq, raw_gain, q) = smoothers[i].current();
        let gain = scale_band_gain(ftype, raw_gain, cur_gain_scale);
        let raw_dyn_range = live.read(&band.dynamic_range);
        let raw_spectral_dyn = live.read(&band.spectral_dynamics);
        let dyn_range = scale_band_dyn_range(ftype, raw_dyn_range, cur_gain_scale);
        let has_dynamics = dyn_range.abs() > 0.01;
        let spectral_dyn = raw_spectral_dyn && dyn_range.abs() > 0.05;

        let coeffs =
            CascadedCoeffs::calculate(ftype, slope, effective_sr, freq, gain, q, natural_phase);
        filters[i].set_coeffs(&coeffs);

        if spectral_dyn {
            let att = live.read(&band.dynamic_attack);
            let rel = live.read(&band.dynamic_release);
            spectral[i].set_ballistics(effective_sr, att, rel);
        } else if has_dynamics {
            let att = live.read(&band.dynamic_attack);
            let rel = live.read(&band.dynamic_release);
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
            raw_dyn_range,
            raw_spectral_dyn,
            dyn_range,
            dyn_thresh: live.read(&band.dynamic_threshold),
            dyn_auto: live.read(&band.dynamic_auto_thresh),
            dyn_ext_sc: live.read(&band.dynamic_ext_sc),
            has_dynamics,
            spectral_dyn,
            last_applied_dyn: 0.0,
        };
        count += 1;
    }

    (active, count)
}

/// Advances per-band smoothers and global gain-scale smoother for one `SMOOTH_SUBBLOCK` stride.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn advance_iir_subblock(
    active: &mut [ActiveBand],
    filters: &mut [CascadedStereoFilter; NUM_BANDS],
    detectors: &mut [DynamicDetector; NUM_BANDS],
    smoothers: &mut [BandParamSmoother; NUM_BANDS],
    gain_scale_smoother: &mut OnePoleSmoother,
    sub_n: usize,
    effective_sr: f32,
    natural_phase: bool,
) {
    let gs_moving = !gain_scale_smoother.is_settled(1e-4);
    if gs_moving {
        gain_scale_smoother.skip(sub_n);
    }
    let gs = gain_scale_smoother.current();

    for band in active.iter_mut() {
        let idx = band.index;
        let band_moving = !smoothers[idx].is_settled();
        if !band_moving && !(gs_moving && shape_uses_gain_scale(band.ftype)) {
            continue;
        }
        let (freq, raw_gain, q) = if band_moving {
            smoothers[idx].advance_subblock(sub_n)
        } else {
            smoothers[idx].current()
        };
        band.freq = freq;
        band.gain = scale_band_gain(band.ftype, raw_gain, gs);
        band.q = q;
        band.dyn_range = scale_band_dyn_range(band.ftype, band.raw_dyn_range, gs);
        band.has_dynamics = band.dyn_range.abs() > 0.01;
        band.spectral_dyn = band.raw_spectral_dyn && band.dyn_range.abs() > 0.05;

        let eff_gain = band.gain
            + if band.has_dynamics {
                band.last_applied_dyn
            } else {
                0.0
            };
        let coeffs = CascadedCoeffs::calculate(
            band.ftype,
            band.slope,
            effective_sr,
            band.freq,
            eff_gain,
            band.q,
            natural_phase,
        );
        filters[idx].set_coeffs(&coeffs);
        if band.has_dynamics && !band.spectral_dyn {
            detectors[idx].update_sidechain(effective_sr, band.freq, band.q);
        }
    }
}

/// Publishes the latest dynamic gain reduction telemetry for all active dynamic/spectral bands.
pub fn publish_band_gr(
    active: &[ActiveBand],
    detectors: &[DynamicDetector; NUM_BANDS],
    spectral: &[SpectralBandProcessor; NUM_BANDS],
    shared_gr: &SharedBandGr,
) {
    for band in active {
        if band.spectral_dyn {
            shared_gr.store(band.index, spectral[band.index].last_gain_offset_db());
        } else if band.has_dynamics {
            shared_gr.store(band.index, detectors[band.index].last_gain_offset_db());
        }
    }
}

//! Audio Processing Engine for Phantom EQ
//! Encapsulates Zero-Latency IIR (+ 2x Oversampling + Dynamic EQ), Linear Phase FIR,
//! and Per-Band Solo Audition — strictly zero heap allocations on the audio thread.

use crate::dsp_state::{capture_band_snapshots, process_solo_sample, BandSnapshot};
use crate::editor::NO_SOLO;
use crate::params::{PhantomEqParams, ProcessingModeParam, NUM_BANDS};
use phantom_dsp::{
    decode_mid_side, encode_mid_side, BiquadCoeffs, ChannelTarget, DynamicDetector, FilterType,
    LinearPhaseEngine, Oversampler2x, StereoBiquad, StereoCurveDb,
};

pub struct EqProcessor {
    sample_rate: f32,
    filters: [StereoBiquad; NUM_BANDS],
    dynamic_detectors: [DynamicDetector; NUM_BANDS],
    solo_filter: StereoBiquad,
    oversampler: Oversampler2x,
    linear_phase: LinearPhaseEngine,
    last_solo_band: u8,
    last_lp_snapshots: [BandSnapshot; NUM_BANDS],
    last_lp_sample_rate_bits: u32,
    lp_dirty: bool,
}

impl Default for EqProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl EqProcessor {
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            filters: std::array::from_fn(|_| StereoBiquad::new()),
            dynamic_detectors: std::array::from_fn(|_| DynamicDetector::new()),
            solo_filter: StereoBiquad::new(),
            oversampler: Oversampler2x::new(),
            linear_phase: LinearPhaseEngine::new(),
            last_solo_band: NO_SOLO,
            last_lp_snapshots: [BandSnapshot::default(); NUM_BANDS],
            last_lp_sample_rate_bits: 0,
            lp_dirty: true,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
        for det in &mut self.dynamic_detectors {
            det.update_ballistics(sample_rate, 10.0, 100.0);
        }
        self.lp_dirty = true;
    }

    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
        for det in &mut self.dynamic_detectors {
            det.reset();
        }
        self.solo_filter.reset();
        self.oversampler.reset();
        self.linear_phase.reset();
    }

    pub fn on_mode_switched(&mut self, mode: ProcessingModeParam) {
        if mode == ProcessingModeParam::LinearPhase {
            self.linear_phase.reset();
            self.lp_dirty = true;
        }
    }

    #[inline]
    pub fn latency_for_mode(&self, mode: ProcessingModeParam) -> u32 {
        match mode {
            ProcessingModeParam::ZeroLatency => 0,
            ProcessingModeParam::LinearPhase => self.linear_phase.latency_samples() as u32,
        }
    }

    pub fn process_solo(
        &mut self,
        params: &PhantomEqParams,
        solo_raw: u8,
        left: &mut [f32],
        right: &mut [f32],
        final_gain: f32,
    ) {
        if solo_raw != self.last_solo_band {
            self.solo_filter.reset();
            self.last_solo_band = solo_raw;
        }
        let snap = BandSnapshot::from_band(&params.bands[solo_raw as usize]);
        self.solo_filter
            .set_coeffs(snap.solo_coeffs(self.sample_rate));
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (sl, sr) = process_solo_sample(&mut self.solo_filter, snap.placement, *l, *r);
            *l = sl * final_gain;
            *r = sr * final_gain;
        }
    }

    pub fn clear_solo_state(&mut self) {
        if self.last_solo_band != NO_SOLO {
            self.last_solo_band = NO_SOLO;
            self.linear_phase.reset();
        }
    }

    /// Synchronizes the Linear Phase FIR 2x2 transfer matrix if any band or sample rate changed.
    /// Strictly zero heap allocations (uses fixed stack arrays).
    pub fn sync_linear_phase(&mut self, params: &PhantomEqParams) {
        let snapshots = capture_band_snapshots(&params.bands);
        let sr_bits = self.sample_rate.to_bits();
        if !self.lp_dirty
            && sr_bits == self.last_lp_sample_rate_bits
            && snapshots == self.last_lp_snapshots
        {
            return;
        }

        let sr = self.sample_rate;
        let coeffs: [BiquadCoeffs; NUM_BANDS] =
            std::array::from_fn(|i| snapshots[i].biquad_coeffs(sr));

        self.linear_phase.update_stereo_curves(sr, |freq| {
            let mut curve = StereoCurveDb::default();
            for i in 0..NUM_BANDS {
                if !snapshots[i].enabled {
                    continue;
                }
                let db = coeffs[i].magnitude_db_at(freq, sr);
                match snapshots[i].placement {
                    ChannelTarget::Stereo => {
                        curve.left += db;
                        curve.right += db;
                    }
                    ChannelTarget::Left => curve.left += db,
                    ChannelTarget::Right => curve.right += db,
                    ChannelTarget::Mid => curve.mid += db,
                    ChannelTarget::Side => curve.side += db,
                }
            }
            curve
        });

        self.last_lp_snapshots = snapshots;
        self.last_lp_sample_rate_bits = sr_bits;
        self.lp_dirty = false;
    }

    pub fn process_linear_phase(
        &mut self,
        params: &PhantomEqParams,
        left: &mut [f32],
        right: &mut [f32],
        final_gain: f32,
    ) {
        self.sync_linear_phase(params);
        self.linear_phase.process_block(left, right);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l *= final_gain;
            *r *= final_gain;
        }
    }

    pub fn process_zero_latency(
        &mut self,
        params: &PhantomEqParams,
        left: &mut [f32],
        right: &mut [f32],
        final_gain: f32,
    ) {
        let use_os = params.anti_cramping.value();
        let effective_sr = if use_os {
            self.sample_rate * 2.0
        } else {
            self.sample_rate
        };

        for (i, band) in params.bands.iter().enumerate() {
            if band.enabled.value() {
                let ftype: FilterType = band.filter_type.value().into();
                let freq = band.frequency.value();
                let gain = band.gain.value();
                let q = band.q.value();
                let coeffs = BiquadCoeffs::calculate(ftype, effective_sr, freq, gain, q);
                self.filters[i].set_coeffs(coeffs);

                if band.dynamic_range.value().abs() > 0.01 {
                    self.dynamic_detectors[i].update_sidechain(effective_sr, freq, q);
                }
            }
        }

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (pl, pr) = if use_os {
                let [sub0, sub1] = self.oversampler.upsample_2x(*l, *r);
                let out0 = self.process_one_pair(params, sub0.0, sub0.1, effective_sr);
                let out1 = self.process_one_pair(params, sub1.0, sub1.1, effective_sr);
                self.oversampler.downsample_2x(out0, out1)
            } else {
                self.process_one_pair(params, *l, *r, effective_sr)
            };
            *l = pl * final_gain;
            *r = pr * final_gain;
        }
    }

    #[inline(always)]
    fn process_one_pair(
        &mut self,
        params: &PhantomEqParams,
        mut l: f32,
        mut r: f32,
        sr: f32,
    ) -> (f32, f32) {
        for (i, band) in params.bands.iter().enumerate() {
            if !band.enabled.value() {
                continue;
            }
            let dyn_range = band.dynamic_range.value();
            if dyn_range.abs() > 0.01 {
                let thresh = band.dynamic_threshold.value();
                let dyn_offset =
                    self.dynamic_detectors[i].compute_dynamic_gain(l, r, thresh, dyn_range);
                if dyn_offset.abs() > 0.01 {
                    let ftype: FilterType = band.filter_type.value().into();
                    let effective_gain = band.gain.value() + dyn_offset;
                    let dynamic_coeffs = BiquadCoeffs::calculate(
                        ftype,
                        sr,
                        band.frequency.value(),
                        effective_gain,
                        band.q.value(),
                    );
                    self.filters[i].set_coeffs(dynamic_coeffs);
                }
            }

            let placement: ChannelTarget = band.placement.value().into();
            match placement {
                ChannelTarget::Stereo => (l, r) = self.filters[i].process_stereo(l, r),
                ChannelTarget::Left => l = self.filters[i].process_stereo(l, 0.0).0,
                ChannelTarget::Right => r = self.filters[i].process_stereo(0.0, r).1,
                ChannelTarget::Mid => {
                    let (mid, side) = encode_mid_side(l, r);
                    let f_mid = self.filters[i].process_stereo(mid, 0.0).0;
                    (l, r) = decode_mid_side(f_mid, side);
                }
                ChannelTarget::Side => {
                    let (mid, side) = encode_mid_side(l, r);
                    let f_side = self.filters[i].process_stereo(side, 0.0).0;
                    (l, r) = decode_mid_side(mid, f_side);
                }
            }
        }
        (l, r)
    }
}

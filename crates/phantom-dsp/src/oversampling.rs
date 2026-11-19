//! HIIR-Style Polyphase Allpass Half-Band Oversampling Engine (2x and 4x)
//! Elliptic 2-path polyphase allpass IIR filters: >103 dB stopband attenuation,
//! 0.0000 dB passband flatness to 0.455*fs, ultra-low CPU, strictly zero allocations.

/// Stage-1 (1x <-> 2x) elliptic half-band allpass coefficients (tb = 0.045, >103 dB stopband).
const STAGE1_A0: [f64; 4] = [
    0.038_059_051_362_609_13,
    0.285_516_809_582_858_6,
    0.589_937_976_805_150_9,
    0.837_829_978_576_492,
];
const STAGE1_A1: [f64; 4] = [
    0.141_739_219_886_511_03,
    0.441_572_773_445_398_1,
    0.721_787_863_804_350_4,
    0.945_517_473_250_823_7,
];

/// Stage-2 (2x <-> 4x) elliptic half-band allpass coefficients (tb = 0.22, >106 dB stopband).
const STAGE2_A0: [f64; 2] = [0.046_348_300_464_797_654, 0.411_986_516_014_008_1];
const STAGE2_A1: [f64; 2] = [0.183_358_170_316_111_68, 0.757_790_030_715_233_5];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OversampleFactor {
    Off,
    #[default]
    X2,
    X4,
}

impl OversampleFactor {
    #[inline]
    pub fn multiplier(self) -> usize {
        match self {
            Self::Off => 1,
            Self::X2 => 2,
            Self::X4 => 4,
        }
    }
}

/// Cascade of `N` first-order allpass sections operating on stereo channels in f64 precision.
#[derive(Debug, Clone)]
struct StereoAllpassChain<const N: usize> {
    coeffs: [f64; N],
    x_l: [f64; N],
    y_l: [f64; N],
    x_r: [f64; N],
    y_r: [f64; N],
}

impl<const N: usize> StereoAllpassChain<N> {
    fn new(coeffs: [f64; N]) -> Self {
        Self {
            coeffs,
            x_l: [0.0; N],
            y_l: [0.0; N],
            x_r: [0.0; N],
            y_r: [0.0; N],
        }
    }

    fn reset(&mut self) {
        self.x_l = [0.0; N];
        self.y_l = [0.0; N];
        self.x_r = [0.0; N];
        self.y_r = [0.0; N];
    }

    #[inline(always)]
    fn process(&mut self, mut vl: f64, mut vr: f64) -> (f64, f64) {
        for i in 0..N {
            let c = self.coeffs[i];
            let ol = c * (vl - self.y_l[i]) + self.x_l[i];
            self.x_l[i] = vl;
            self.y_l[i] = if ol.abs() < 1e-25 { 0.0 } else { ol };
            vl = ol;

            let or = c * (vr - self.y_r[i]) + self.x_r[i];
            self.x_r[i] = vr;
            self.y_r[i] = if or.abs() < 1e-25 { 0.0 } else { or };
            vr = or;
        }
        (vl, vr)
    }
}

#[derive(Debug, Clone)]
struct HalfBandStage<const N: usize> {
    up_a0: StereoAllpassChain<N>,
    up_a1: StereoAllpassChain<N>,
    down_a0: StereoAllpassChain<N>,
    down_a1: StereoAllpassChain<N>,
}

impl<const N: usize> HalfBandStage<N> {
    fn new(a0: [f64; N], a1: [f64; N]) -> Self {
        Self {
            up_a0: StereoAllpassChain::new(a0),
            up_a1: StereoAllpassChain::new(a1),
            down_a0: StereoAllpassChain::new(a0),
            down_a1: StereoAllpassChain::new(a1),
        }
    }

    fn reset(&mut self) {
        self.up_a0.reset();
        self.up_a1.reset();
        self.down_a0.reset();
        self.down_a1.reset();
    }

    #[inline(always)]
    fn upsample(&mut self, in_l: f32, in_r: f32) -> [(f32, f32); 2] {
        let (xl, xr) = (in_l as f64, in_r as f64);
        let (y0l, y0r) = self.up_a0.process(xl, xr);
        let (y1l, y1r) = self.up_a1.process(xl, xr);
        [(y0l as f32, y0r as f32), (y1l as f32, y1r as f32)]
    }

    #[inline(always)]
    fn downsample(&mut self, s0: (f32, f32), s1: (f32, f32)) -> (f32, f32) {
        let (a0l, a0r) = self.down_a0.process(s1.0 as f64, s1.1 as f64);
        let (a1l, a1r) = self.down_a1.process(s0.0 as f64, s0.1 as f64);
        ((0.5 * (a0l + a1l)) as f32, (0.5 * (a0r + a1r)) as f32)
    }
}

/// 2x Polyphase Allpass Half-Band Oversampler (>103 dB stopband, flat to 0.455*fs).
#[derive(Debug, Clone)]
pub struct Oversampler2x {
    stage: HalfBandStage<4>,
}

impl Default for Oversampler2x {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler2x {
    pub fn new() -> Self {
        Self {
            stage: HalfBandStage::new(STAGE1_A0, STAGE1_A1),
        }
    }

    pub fn reset(&mut self) {
        self.stage.reset();
    }

    /// Upsample 1 stereo pair into 2 consecutive 2x-rate stereo pairs.
    #[inline(always)]
    pub fn upsample_2x(&mut self, in_l: f32, in_r: f32) -> [(f32, f32); 2] {
        self.stage.upsample(in_l, in_r)
    }

    /// Downsample 2 consecutive 2x-rate stereo pairs into 1 base-rate stereo pair.
    #[inline(always)]
    pub fn downsample_2x(&mut self, s0: (f32, f32), s1: (f32, f32)) -> (f32, f32) {
        self.stage.downsample(s0, s1)
    }
}

/// 4x Cascaded Polyphase Half-Band Oversampler (Stage 1: 8-coeff 1x<->2x, Stage 2: 4-coeff 2x<->4x).
#[derive(Debug, Clone)]
pub struct Oversampler4x {
    stage1: HalfBandStage<4>,
    stage2: HalfBandStage<2>,
}

impl Default for Oversampler4x {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler4x {
    pub fn new() -> Self {
        Self {
            stage1: HalfBandStage::new(STAGE1_A0, STAGE1_A1),
            stage2: HalfBandStage::new(STAGE2_A0, STAGE2_A1),
        }
    }

    pub fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
    }

    /// Upsample 1 stereo pair into 4 consecutive 4x-rate stereo pairs.
    #[inline(always)]
    pub fn upsample_4x(&mut self, in_l: f32, in_r: f32) -> [(f32, f32); 4] {
        let [m0, m1] = self.stage1.upsample(in_l, in_r);
        let [s0, s1] = self.stage2.upsample(m0.0, m0.1);
        let [s2, s3] = self.stage2.upsample(m1.0, m1.1);
        [s0, s1, s2, s3]
    }

    /// Downsample 4 consecutive 4x-rate stereo pairs into 1 base-rate stereo pair.
    #[inline(always)]
    pub fn downsample_4x(&mut self, samples: [(f32, f32); 4]) -> (f32, f32) {
        let m0 = self.stage2.downsample(samples[0], samples[1]);
        let m1 = self.stage2.downsample(samples[2], samples[3]);
        self.stage1.downsample(m0, m1)
    }
}

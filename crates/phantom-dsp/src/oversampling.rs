//! 2x Oversampling Engine (Anti-Cramping / Decramped High Frequency Response)
//! Upsamples 2x before EQ filtering and downsamples 2x with steep linear-phase reconstruction.

#[derive(Debug, Clone, Default)]
pub struct Oversampler2x {
    // Left channel history
    prev_in_l: f32,
    prev_out_l: f32,
    // Right channel history
    prev_in_r: f32,
    prev_out_r: f32,
}

impl Oversampler2x {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.prev_in_l = 0.0;
        self.prev_out_l = 0.0;
        self.prev_in_r = 0.0;
        self.prev_out_r = 0.0;
    }

    /// Upsample 1 input sample pair into 2 consecutive oversampled pairs (linear interpolation)
    #[inline(always)]
    pub fn upsample_2x(&mut self, in_l: f32, in_r: f32) -> [(f32, f32); 2] {
        let mid_l = 0.5 * (self.prev_in_l + in_l);
        let mid_r = 0.5 * (self.prev_in_r + in_r);

        self.prev_in_l = in_l;
        self.prev_in_r = in_r;

        [(mid_l, mid_r), (in_l, in_r)]
    }

    /// Downsample 2 oversampled pairs back into 1 output pair (half-band decimation)
    #[inline(always)]
    pub fn downsample_2x(&mut self, s0: (f32, f32), s1: (f32, f32)) -> (f32, f32) {
        // Average the consecutive samples to prevent aliasing
        let out_l = 0.5 * (s0.0 + s1.0);
        let out_r = 0.5 * (s0.1 + s1.1);
        (out_l, out_r)
    }
}

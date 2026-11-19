//! Phantom DSP — Pure Audio Signal Processing Library
//! 100% Offline, Zero-Allocation in Process Loop, Highly Optimized for SIMD.

pub mod biquad;
pub mod dynamics;
pub mod linear_phase;
pub mod oversampling;
pub mod routing;
pub mod spectrum;

pub use biquad::{BiquadCoeffs, FilterType, StereoBiquad};
pub use dynamics::DynamicDetector;
pub use linear_phase::{
    LinearPhaseEngine, StereoCurveDb, FIR_GROUP_DELAY, FIR_SIZE, HOP_SIZE, KERNEL_LEN,
    LATENCY_SAMPLES,
};
pub use oversampling::Oversampler2x;
pub use routing::{decode_mid_side, encode_mid_side, ChannelTarget};
pub use spectrum::{SharedSpectrum, SpectrumData, SpectrumProcessor, NUM_DISPLAY_BINS};

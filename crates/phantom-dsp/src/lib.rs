//! Phantom DSP — Pure Audio Signal Processing Library
//! 100% Offline, Zero-Allocation in Process Loop, Highly Optimized for SIMD.

pub mod auto_gain;
pub mod biquad;
pub mod character;
pub mod dynamics;
pub mod linear_phase;
pub mod metering;
pub mod oversampling;
pub mod routing;
pub mod slope_filter;
pub mod spectrum;

pub use auto_gain::AutoGainCompensator;
pub use biquad::{BiquadCoeffs, FilterType, StereoBiquad};
pub use character::{CharacterMode, CharacterProcessor};
pub use dynamics::DynamicDetector;
pub use linear_phase::{
    LinearPhaseEngine, StereoCurveDb, FIR_GROUP_DELAY, FIR_SIZE, HOP_SIZE, KERNEL_LEN,
    LATENCY_SAMPLES,
};
pub use metering::{
    MeterSnapshot, OutputMeter, SharedMeters, LUFS_FLOOR, MOMENTARY_BLOCKS, PEAK_FLOOR_DB,
    SHORT_TERM_BLOCKS,
};
pub use oversampling::{OversampleFactor, Oversampler2x, Oversampler4x};
pub use routing::{decode_mid_side, encode_mid_side, ChannelTarget};
pub use slope_filter::{CascadedCoeffs, CascadedStereoFilter, FilterSlope, MAX_CASCADE_STAGES};
pub use spectrum::{
    display_bin_freq, SharedSpectrum, SpectrumData, SpectrumProcessor, DEFAULT_TILT_DB_PER_OCT,
    FFT_SIZE, NUM_DISPLAY_BINS, SPECTRUM_FLOOR_DB,
};

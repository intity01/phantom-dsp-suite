//! Zero-Allocation Mono Track Processing Wrapper (256-Sample Stack Scratch Buffers)
//! Routes Mono (`1->1`) DAW tracks cleanly through `EqProcessor::process_block` without heap allocs.

use super::EqProcessor;
use crate::dsp_state::SharedBandGr;
use crate::live_mirror::ParamRead;
use crate::params::{PhantomEqParams, ProcessingModeParam};
use phantom_dsp::{SharedMeters, SharedSpectrum};

const MONO_CHUNK_SIZE: usize = 256;

impl EqProcessor {
    /// Processes a mono audio buffer in fixed 256-sample stack chunks with zero heap allocations.
    #[allow(clippy::too_many_arguments)]
    pub fn process_mono_block(
        &mut self,
        params: &PhantomEqParams,
        live: &impl ParamRead,
        mode: ProcessingModeParam,
        bypassed: bool,
        requested_solo: u8,
        mono: &mut [f32],
        sc_left: &[f32],
        sc_right: &[f32],
        final_gain: f32,
        shared_meters: &SharedMeters,
        shared_gr: &SharedBandGr,
        spectrum: Option<&SharedSpectrum>,
    ) {
        let mut l_buf = [0.0_f32; MONO_CHUNK_SIZE];
        let mut r_buf = [0.0_f32; MONO_CHUNK_SIZE];
        let mut offset = 0;
        while offset < mono.len() {
            let n = (mono.len() - offset).min(MONO_CHUNK_SIZE);
            l_buf[..n].copy_from_slice(&mono[offset..offset + n]);
            r_buf[..n].copy_from_slice(&mono[offset..offset + n]);
            if let Some(sp) = spectrum {
                sp.push_stereo_pre(&l_buf[..n], &r_buf[..n]);
            }
            self.process_block(
                params,
                live,
                mode,
                bypassed,
                requested_solo,
                &mut l_buf[..n],
                &mut r_buf[..n],
                slice_window(sc_left, offset, n),
                slice_window(sc_right, offset, n),
                final_gain,
                shared_meters,
                shared_gr,
            );
            mono[offset..offset + n].copy_from_slice(&l_buf[..n]);
            if let Some(sp) = spectrum {
                sp.push_stereo_post_and_side(&l_buf[..n], &r_buf[..n]);
            }
            offset += n;
        }
    }
}

#[inline]
fn slice_window(src: &[f32], offset: usize, len: usize) -> &[f32] {
    if offset >= src.len() {
        &[]
    } else {
        &src[offset..(offset + len).min(src.len())]
    }
}

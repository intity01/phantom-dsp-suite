#![cfg(windows)]

use std::ptr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::playback_mix::{render_mix_block, TransportShared};

const WAVE_MAPPER: u32 = u32::MAX;
const WAVE_FORMAT_PCM: u16 = 1;
const WHDR_DONE: u32 = 0x0000_0001;
const BUF_FRAMES: usize = 2048;
const NUM_BUFS: usize = 2;

#[repr(C)]
struct WaveFormatEx {
    w_format_tag: u16,
    n_channels: u16,
    n_samples_per_sec: u32,
    n_avg_bytes_per_sec: u32,
    n_block_align: u16,
    w_bits_per_sample: u16,
    cb_size: u16,
}

#[repr(C)]
struct WaveHdr {
    lp_data: *mut i16,
    dw_buffer_length: u32,
    dw_bytes_recorded: u32,
    dw_user: usize,
    dw_flags: u32,
    dw_loops: u32,
    lp_next: *mut WaveHdr,
    reserved: usize,
}

#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(
        phwo: *mut *mut core::ffi::c_void,
        u_device_id: u32,
        pwfx: *const WaveFormatEx,
        dw_callback: usize,
        dw_instance: usize,
        fdw_open: u32,
    ) -> u32;
    fn waveOutPrepareHeader(hwo: *mut core::ffi::c_void, pwh: *mut WaveHdr, cbwh: u32) -> u32;
    fn waveOutUnprepareHeader(hwo: *mut core::ffi::c_void, pwh: *mut WaveHdr, cbwh: u32) -> u32;
    fn waveOutWrite(hwo: *mut core::ffi::c_void, pwh: *mut WaveHdr, cbwh: u32) -> u32;
    fn waveOutReset(hwo: *mut core::ffi::c_void) -> u32;
    fn waveOutClose(hwo: *mut core::ffi::c_void) -> u32;
}

pub fn spawn_waveout_thread(shared: Arc<TransportShared>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut current_sr = 0u32;
        let mut hwo: *mut core::ffi::c_void = ptr::null_mut();
        let mut buffers = vec![vec![0i16; BUF_FRAMES * 2]; NUM_BUFS];
        let mut headers: Vec<WaveHdr> = (0..NUM_BUFS)
            .map(|i| WaveHdr {
                lp_data: buffers[i].as_mut_ptr(),
                dw_buffer_length: (BUF_FRAMES * 2 * std::mem::size_of::<i16>()) as u32,
                dw_bytes_recorded: 0,
                dw_user: i,
                dw_flags: WHDR_DONE,
                dw_loops: 0,
                lp_next: ptr::null_mut(),
                reserved: 0,
            })
            .collect();

        while !shared.stop_flag.load(Ordering::Relaxed) {
            let target_sr = shared.sample_rate.load(Ordering::Relaxed).max(8000);
            if target_sr != current_sr {
                if !hwo.is_null() {
                    unsafe {
                        waveOutReset(hwo);
                        for hdr in &mut headers {
                            if (hdr.dw_flags & WHDR_DONE) == 0 {
                                waveOutUnprepareHeader(
                                    hwo,
                                    hdr as *mut WaveHdr,
                                    std::mem::size_of::<WaveHdr>() as u32,
                                );
                                hdr.dw_flags |= WHDR_DONE;
                            }
                        }
                        waveOutClose(hwo);
                    }
                    hwo = ptr::null_mut();
                }
                let wfx = WaveFormatEx {
                    w_format_tag: WAVE_FORMAT_PCM,
                    n_channels: 2,
                    n_samples_per_sec: target_sr,
                    n_avg_bytes_per_sec: target_sr * 4,
                    n_block_align: 4,
                    w_bits_per_sample: 16,
                    cb_size: 0,
                };
                let res = unsafe { waveOutOpen(&mut hwo, WAVE_MAPPER, &wfx, 0, 0, 0) };
                if res == 0 {
                    current_sr = target_sr;
                } else {
                    hwo = ptr::null_mut();
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }

            if hwo.is_null() {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let mut queued = false;
            for (idx, hdr) in headers.iter_mut().enumerate() {
                if (hdr.dw_flags & WHDR_DONE) != 0 {
                    unsafe {
                        waveOutUnprepareHeader(
                            hwo,
                            hdr as *mut WaveHdr,
                            std::mem::size_of::<WaveHdr>() as u32,
                        );
                    }
                    let mix_arc = shared.mix.lock().map(|m| Arc::clone(&m)).ok();
                    if let Some(m) = mix_arc {
                        render_mix_block(&m, &shared, &mut buffers[idx]);
                    } else {
                        buffers[idx].fill(0);
                    }
                    hdr.lp_data = buffers[idx].as_mut_ptr();
                    hdr.dw_flags = 0;
                    unsafe {
                        waveOutPrepareHeader(
                            hwo,
                            hdr as *mut WaveHdr,
                            std::mem::size_of::<WaveHdr>() as u32,
                        );
                        waveOutWrite(
                            hwo,
                            hdr as *mut WaveHdr,
                            std::mem::size_of::<WaveHdr>() as u32,
                        );
                    }
                    queued = true;
                }
            }
            if !queued {
                thread::sleep(Duration::from_millis(5));
            }
        }

        if !hwo.is_null() {
            unsafe {
                waveOutReset(hwo);
                for hdr in &mut headers {
                    waveOutUnprepareHeader(
                        hwo,
                        hdr as *mut WaveHdr,
                        std::mem::size_of::<WaveHdr>() as u32,
                    );
                }
                waveOutClose(hwo);
            }
        }
    })
}

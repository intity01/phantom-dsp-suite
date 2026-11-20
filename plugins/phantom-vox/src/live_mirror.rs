//! Lock-Free Authoritative Live Parameter Mirror (GUI <-> Audio <-> Host) for Phantom Vox
//!
//! Ensures GUI parameter edits take effect immediately (Frame 0) even when Cubase suspends
//! audio processing, while cleanly reconciling external host changes (automation, host bypass,
//! project state restore) on the audio and GUI threads with zero allocations and zero mutexes.

use crate::params::PhantomVoxParams;
use nih_plug::prelude::{Param, ParamPtr, ParamSetter, Params};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Grace window during which a differing host value is treated as a stale echo of a GUI drag.
pub const GUI_EDIT_GRACE_US: u64 = 150_000;
const NORM_EPS: f32 = 1e-6;

/// Generic read access to plain parameter values.
pub trait ParamRead {
    fn read<P: Param>(&self, param: &P) -> P::Plain;
}

/// Shared (`Arc`) lock-free authoritative parameter mirror used by audio DSP and the GUI.
pub struct LiveParamMirror {
    params: Arc<PhantomVoxParams>,
    ptrs: Box<[ParamPtr]>,
    index: HashMap<ParamPtr, usize>,
    live: Box<[AtomicU32]>,
    host_shadow: Box<[AtomicU32]>,
    gui_edit_us: Box<[AtomicU64]>,
    epoch: Instant,
}

impl LiveParamMirror {
    pub fn new(params: Arc<PhantomVoxParams>) -> Self {
        let ptrs: Box<[ParamPtr]> = params.param_map().into_iter().map(|(_, p, _)| p).collect();
        let len = ptrs.len();
        let index: HashMap<ParamPtr, usize> =
            ptrs.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let live = (0..len)
            .map(|_| AtomicU32::new(0.0_f32.to_bits()))
            .collect();
        let host_shadow = (0..len)
            .map(|_| AtomicU32::new(0.0_f32.to_bits()))
            .collect();
        let gui_edit_us = (0..len).map(|_| AtomicU64::new(0)).collect();
        let mirror = Self {
            params,
            ptrs,
            index,
            live,
            host_shadow,
            gui_edit_us,
            epoch: Instant::now(),
        };
        mirror.resync_all();
        mirror
    }

    #[inline]
    fn now_us(&self) -> u64 {
        (self.epoch.elapsed().as_micros() as u64).saturating_add(1)
    }

    #[inline]
    fn host_normalized(&self, k: usize) -> f32 {
        // SAFETY: every `ParamPtr` belongs to `self.params`, kept alive by `Arc`.
        unsafe { self.ptrs[k].unmodulated_normalized_value() }
    }

    pub fn params(&self) -> &PhantomVoxParams {
        &self.params
    }

    /// Hard re-sync of every mirrored value from the host parameters (construction only).
    pub fn resync_all(&self) {
        for k in 0..self.ptrs.len() {
            let bits = self.host_normalized(k).to_bits();
            self.live[k].store(bits, Ordering::Release);
            self.host_shadow[k].store(bits, Ordering::Release);
            self.gui_edit_us[k].store(0, Ordering::Release);
        }
    }

    /// Lock-free, zero-allocation reconciliation of external host changes into the live mirror.
    pub fn reconcile(&self) {
        let mut cached_now_us: Option<u64> = None;
        for k in 0..self.ptrs.len() {
            let host = self.host_normalized(k);
            let host_bits = host.to_bits();
            let prev_bits = self.host_shadow[k].load(Ordering::Relaxed);
            if host_bits == prev_bits {
                continue;
            }
            let prev = f32::from_bits(prev_bits);
            self.host_shadow[k].store(host_bits, Ordering::Relaxed);
            if (host - prev).abs() <= NORM_EPS {
                continue;
            }

            let cur_live_bits = self.live[k].load(Ordering::Acquire);
            let cur_live = f32::from_bits(cur_live_bits);
            let last_gui = self.gui_edit_us[k].load(Ordering::Acquire);

            if (host - cur_live).abs() <= NORM_EPS {
                if last_gui != 0 {
                    let _ = self.gui_edit_us[k].compare_exchange(
                        last_gui,
                        0,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    );
                }
                continue;
            }

            let now = *cached_now_us.get_or_insert_with(|| self.now_us());
            if last_gui == 0 || now.saturating_sub(last_gui) > GUI_EDIT_GRACE_US {
                let _ = self.live[k].compare_exchange(
                    cur_live_bits,
                    host_bits,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
            }
        }
    }

    /// Lock-free live plain value of `param` (falls back to host value if unindexed).
    #[inline]
    pub fn read<P: Param>(&self, param: &P) -> P::Plain {
        match self.index.get(&param.as_ptr()) {
            Some(&k) => {
                let norm = f32::from_bits(self.live[k].load(Ordering::Acquire));
                param.preview_plain(norm)
            }
            None => param.modulated_plain_value(),
        }
    }

    /// Writes a GUI edit into the mirror immediately (0 ms) and stamps its edit time.
    #[inline]
    pub fn write_normalized(&self, ptr: ParamPtr, normalized: f32) {
        if let Some(&k) = self.index.get(&ptr) {
            let now = self.now_us();
            let clamped = normalized.clamp(0.0, 1.0);
            self.gui_edit_us[k].store(now, Ordering::Release);
            self.live[k].store(clamped.to_bits(), Ordering::Release);
        }
    }

    /// Refreshes the edit stamp while a GUI drag gesture is active.
    #[inline]
    pub fn touch(&self, ptr: ParamPtr) {
        if let Some(&k) = self.index.get(&ptr) {
            self.gui_edit_us[k].store(self.now_us(), Ordering::Release);
        }
    }
}

impl ParamRead for LiveParamMirror {
    #[inline]
    fn read<P: Param>(&self, param: &P) -> P::Plain {
        LiveParamMirror::read(self, param)
    }
}

/// Drop-in editor setter: writes the live mirror immediately (0 ms) AND notifies the host.
pub struct LiveSetter<'a> {
    pub host: &'a ParamSetter<'a>,
    pub live: &'a LiveParamMirror,
}

impl<'a> LiveSetter<'a> {
    pub fn new(host: &'a ParamSetter<'a>, live: &'a LiveParamMirror) -> Self {
        Self { host, live }
    }

    #[inline]
    pub fn begin_set_parameter<P: Param>(&self, param: &P) {
        self.live.touch(param.as_ptr());
        self.host.begin_set_parameter(param);
    }

    #[inline]
    pub fn set_parameter<P: Param>(&self, param: &P, value: P::Plain) {
        let normalized = param.preview_normalized(value);
        self.live.write_normalized(param.as_ptr(), normalized);
        self.host.set_parameter_normalized(param, normalized);
    }

    #[inline]
    pub fn end_set_parameter<P: Param>(&self, param: &P) {
        self.live.touch(param.as_ptr());
        self.host.end_set_parameter(param);
    }

    /// Complete begin/set/end automation gesture for a discrete GUI edit.
    #[inline]
    pub fn set_once<P: Param>(&self, param: &P, value: P::Plain) {
        self.begin_set_parameter(param);
        self.set_parameter(param, value);
        self.end_set_parameter(param);
    }

    /// Live plain value of `param`.
    #[inline]
    pub fn read<P: Param>(&self, param: &P) -> P::Plain {
        self.live.read(param)
    }
}

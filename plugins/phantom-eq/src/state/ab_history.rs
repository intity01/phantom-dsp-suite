//! A/B Comparison Bank & 64-Step Drag-Coalesced Undo/Redo Stack
//! Guarantees dragging an EQ node across 100 frames records one clean undo step on mouse release.

use super::snapshot::EqPresetSnapshot;
use crate::params::PhantomEqParams;
use nih_plug::prelude::ParamSetter;

pub const MAX_UNDO_STEPS: usize = 64;

#[derive(Clone, Debug)]
pub struct AbHistoryManager {
    pub slot_a: EqPresetSnapshot,
    pub slot_b: EqPresetSnapshot,
    pub is_slot_b: bool,
    pub active_snapshot: EqPresetSnapshot,
    undo_stack: Vec<EqPresetSnapshot>,
    redo_stack: Vec<EqPresetSnapshot>,
}

impl Default for AbHistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AbHistoryManager {
    pub fn new() -> Self {
        let init = EqPresetSnapshot::default();
        Self {
            slot_a: init.clone(),
            slot_b: init.clone(),
            is_slot_b: false,
            active_snapshot: init,
            undo_stack: Vec::with_capacity(MAX_UNDO_STEPS),
            redo_stack: Vec::with_capacity(MAX_UNDO_STEPS),
        }
    }

    pub fn is_slot_b(&self) -> bool {
        self.is_slot_b
    }

    pub fn active_slot_label(&self) -> &'static str {
        if self.is_slot_b {
            "B"
        } else {
            "A"
        }
    }

    /// Saves current parameter state into the active slot, switches to the other slot, and applies it.
    pub fn toggle_ab(&mut self, params: &PhantomEqParams, setter: &ParamSetter<'_>) {
        let current = EqPresetSnapshot::capture_from(params);
        if self.is_slot_b {
            self.slot_b = current;
        } else {
            self.slot_a = current;
        }
        self.is_slot_b = !self.is_slot_b;
        let target = if self.is_slot_b {
            self.slot_b.clone()
        } else {
            self.slot_a.clone()
        };
        target.apply_to(params, setter);
        self.active_snapshot = target;
    }

    /// Copies slot A state into slot B (refreshing slot A from live parameters if A is active).
    pub fn copy_a_to_b(&mut self, params: &PhantomEqParams) {
        if !self.is_slot_b {
            self.slot_a = EqPresetSnapshot::capture_from(params);
        }
        self.slot_b = self.slot_a.clone();
    }

    /// Copies slot B state into slot A (refreshing slot B from live parameters if B is active).
    pub fn copy_b_to_a(&mut self, params: &PhantomEqParams) {
        if self.is_slot_b {
            self.slot_b = EqPresetSnapshot::capture_from(params);
        }
        self.slot_a = self.slot_b.clone();
    }

    /// Copies whichever slot is currently active into the opposite slot (`A -> B` or `B -> A`).
    pub fn copy_active_to_other(&mut self, params: &PhantomEqParams) {
        if self.is_slot_b {
            self.copy_b_to_a(params);
        } else {
            self.copy_a_to_b(params);
        }
    }

    /// Commits a new undo entry only when `!is_dragging` and parameters changed beyond `< 1e-4` float noise.
    pub fn record_if_changed(&mut self, current: EqPresetSnapshot, is_dragging: bool) {
        if is_dragging || current.approx_eq(&self.active_snapshot) {
            return;
        }
        Self::push_bounded(&mut self.undo_stack, self.active_snapshot.clone());
        self.redo_stack.clear();
        self.active_snapshot = current.clone();
        if self.is_slot_b {
            self.slot_b = current;
        } else {
            self.slot_a = current;
        }
    }

    /// Applies a preset snapshot directly and records the transition on the undo stack.
    pub fn apply_preset(
        &mut self,
        preset: &EqPresetSnapshot,
        params: &PhantomEqParams,
        setter: &ParamSetter<'_>,
    ) {
        if !preset.approx_eq(&self.active_snapshot) {
            Self::push_bounded(&mut self.undo_stack, self.active_snapshot.clone());
            self.redo_stack.clear();
        }
        preset.apply_to(params, setter);
        self.active_snapshot = preset.clone();
        if self.is_slot_b {
            self.slot_b = preset.clone();
        } else {
            self.slot_a = preset.clone();
        }
    }

    /// Steps backward one state in history and applies it to `params`.
    pub fn undo(&mut self, params: &PhantomEqParams, setter: &ParamSetter<'_>) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        Self::push_bounded(&mut self.redo_stack, self.active_snapshot.clone());
        prev.apply_to(params, setter);
        self.active_snapshot = prev.clone();
        if self.is_slot_b {
            self.slot_b = prev;
        } else {
            self.slot_a = prev;
        }
        true
    }

    /// Steps forward one state in history and applies it to `params`.
    pub fn redo(&mut self, params: &PhantomEqParams, setter: &ParamSetter<'_>) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        Self::push_bounded(&mut self.undo_stack, self.active_snapshot.clone());
        next.apply_to(params, setter);
        self.active_snapshot = next.clone();
        if self.is_slot_b {
            self.slot_b = next;
        } else {
            self.slot_a = next;
        }
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    fn push_bounded(stack: &mut Vec<EqPresetSnapshot>, snap: EqPresetSnapshot) {
        if stack.len() >= MAX_UNDO_STEPS {
            stack.remove(0);
        }
        stack.push(snap);
    }
}

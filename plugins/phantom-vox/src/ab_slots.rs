//! A/B settings slots: two complete sets of Vox settings the user can flip between.
//! Stored with the project; the active slot always mirrors the live parameters.

use serde::{Deserialize, Serialize};

/// One captured set of settings: `(parameter id, normalized value)` pairs.
pub type Snapshot = Vec<(String, f32)>;

/// Parameters that A/B never touches.
pub const AB_SKIP_IDS: &[&str] = &["bypass"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AbSlots {
    /// `false` = A is active, `true` = B is active.
    pub b_active: bool,
    pub a: Snapshot,
    pub b: Snapshot,
}

impl AbSlots {
    fn slot_mut(&mut self, b: bool) -> &mut Snapshot {
        if b {
            &mut self.b
        } else {
            &mut self.a
        }
    }

    /// Switches to slot B (`to_b = true`) or A. Saves `current` into the slot being left and
    /// returns the settings to load. A slot that was never filled starts as a copy of `current`.
    pub fn switch_to(&mut self, to_b: bool, current: Snapshot) -> Option<Snapshot> {
        if to_b == self.b_active {
            return None;
        }
        let leaving = self.b_active;
        *self.slot_mut(leaving) = current.clone();
        self.b_active = to_b;
        let target = self.slot_mut(to_b);
        if target.is_empty() {
            *target = current;
        }
        Some(target.clone())
    }

    /// Copies the active slot's current settings over the other slot.
    pub fn copy_to_other(&mut self, current: Snapshot) {
        let active = self.b_active;
        *self.slot_mut(active) = current.clone();
        *self.slot_mut(!active) = current;
    }
}

//! Phantom EQ State Engine — Presets, A/B Comparison Bank & 64-Step Undo/Redo History
//! 100% offline, zero-telemetry parameter snapshotting and disk `.peq` preset management.

pub mod ab_history;
pub mod presets;
pub mod snapshot;

pub use ab_history::{AbHistoryManager, MAX_UNDO_STEPS};
pub use presets::{
    factory_presets, load_user_presets, parse_peq_preset, save_user_preset, serialize_peq_preset,
    user_presets_dir,
};
pub use snapshot::{BandSnapshotData, CharacterParam, EqPresetSnapshot, OversampleParam};

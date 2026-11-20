//! Phantom EQ State Engine — Presets, A/B Comparison Bank & 64-Step Undo/Redo History
//! 100% offline, zero-telemetry parameter snapshotting and disk `.peq` preset management.

pub mod ab_history;
pub mod factory_presets;
pub mod presets;
pub mod snapshot;

pub use ab_history::{AbHistoryManager, MAX_UNDO_STEPS};
pub use presets::{
    delete_user_preset, delete_user_preset_in, factory_presets, load_user_presets,
    load_user_presets_in, parse_peq_preset, sanitize_preset_stem, save_user_preset,
    save_user_preset_in, serialize_peq_preset, user_presets_dir,
};
pub use snapshot::{BandSnapshotData, CharacterParam, EqPresetSnapshot, OversampleParam};

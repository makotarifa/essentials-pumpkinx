//! The shared persistence pattern behind `state.json`, `economy.json` and
//! `kits.json`.
//!
//! [`FileStore`] formalises the rules every JSON store follows:
//!
//! * [`FileStore::load`] is read-only: a missing or empty file yields
//!   `T::default()`, while anything unreadable or unparseable is an error —
//!   a corrupt file is NEVER silently treated as empty, so a later save can
//!   never destroy it.
//! * Legacy shapes (a `{"<wrapper key>": {...}}` file, or a schema older than
//!   the canonical one) are normalised in memory only: `needs_migration` is
//!   set and the canonical form is written later by the periodic tick, so
//!   loading can never block on file I/O.
//! * [`FileStore::save_atomic`] is the only disk write: temp file + rename.

use serde::{Serialize, de::DeserializeOwned};
use std::{io, path::Path};

/// One JSON file per store, one canonical schema per store.
///
/// The stored type is `Self` (the trait is "generic" over the data it holds):
/// `PersistedState`, `Economy` and `Kits` all implement it.
pub trait FileStore: Default + Serialize + DeserializeOwned {
    /// Top-level key of the legacy wrapper shape (`{"state": {...}}`).
    const WRAPPER_KEY: &'static str;

    /// Canonical schema version written by [`FileStore::save_atomic`].
    fn canonical_schema() -> u32;

    /// Schema version currently held in memory (may be older than canonical).
    fn stored_schema(&self) -> u32;

    /// Raise the in-memory schema to [`FileStore::canonical_schema`].
    fn bump_schema(&mut self);

    /// True when [`FileStore::load`] normalised a legacy shape in memory and
    /// the canonical file still has to be written by the first tick.
    fn needs_migration(&self) -> bool;

    fn set_needs_migration(&mut self, needs: bool);

    /// Read and parse the store file. Never writes to disk.
    fn load(path: &Path) -> Result<Self, String> {
        // Missing/empty file => default. Unreadable/corrupt file => Err.
        let Some(mut value) = super::read_value(path)? else {
            return Ok(Self::default());
        };
        let legacy = unwrap_wrapper(&mut value, Self::WRAPPER_KEY);
        let mut loaded: Self =
            serde_json::from_value(value).map_err(|error| format!("parse {}: {error}", path.display()))?;
        let old_schema = loaded.stored_schema() < Self::canonical_schema();
        if old_schema {
            loaded.bump_schema();
        }
        loaded.set_needs_migration(legacy || old_schema);
        Ok(loaded)
    }

    /// Atomic write (temp file + rename); the only disk write in the pattern.
    fn save_atomic(path: &Path, value: &Self) -> io::Result<()> {
        super::save_json(path, value)
    }
}

/// Flatten a legacy wrapper file (`{"<key>": {...}}`) in memory.
pub(super) fn unwrap_wrapper(value: &mut serde_json::Value, key: &str) -> bool {
    let Some(map) = value.as_object() else { return false };
    if map.len() != 1 { return false; }
    let Some(inner) = map.get(key).cloned() else { return false };
    *value = inner;
    true
}

//! Session metadata persisted in `state.json` inside the plugin data folder.
//!
//! `PersistedState` implements the shared [`FileStore`] pattern
//! (`crate::store::persist`): loading is read-only, legacy shapes are
//! normalised in memory (flagged via `needs_migration`) and the canonical
//! file is written later by the periodic tick, so loading never blocks on
//! file I/O.

use crate::store::{Location, persist::FileStore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, path::Path};

fn schema() -> u32 {
    2
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlyState {
    pub allow_flying: bool,
    pub flying: bool,
    pub fly_speed: f32,
}

impl Default for FlyState {
    fn default() -> Self {
        Self { allow_flying: false, flying: false, fly_speed: 0.05 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AfkState {
    pub active: bool,
    pub reason: String,
    pub since_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModRecord {
    pub until_ms: u64,
    pub reason: String,
    pub by: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MailMessage {
    pub from: String,
    pub from_uuid: String,
    pub body: String,
    pub time_ms: u64,
    pub read: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JailRecord {
    pub jail: String,
    pub until_ms: u64,
    pub by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedState {
    #[serde(default = "schema")]
    pub schema: u32,
    pub back: BTreeMap<String, Location>,
    pub fly: BTreeMap<String, FlyState>,
    pub afk: BTreeMap<String, AfkState>,
    pub tpa_off: BTreeMap<String, bool>,
    pub playtime_ms: BTreeMap<String, u64>,
    pub last_seen_ms: BTreeMap<String, u64>,
    pub last_location: BTreeMap<String, Location>,
    pub tpa_auto: BTreeMap<String, bool>,
    pub names: BTreeMap<String, String>,
    pub msg_off: BTreeMap<String, bool>,
    pub social_spy: BTreeMap<String, bool>,
    pub ignored: BTreeMap<String, Vec<String>>,
    pub nicks: BTreeMap<String, String>,
    pub reply_target: BTreeMap<String, String>,
    pub rtoggle: BTreeMap<String, bool>,
    pub mail: BTreeMap<String, Vec<MailMessage>>,
    pub mutes: BTreeMap<String, ModRecord>,
    pub bans: BTreeMap<String, ModRecord>,
    pub vanished: BTreeMap<String, bool>,
    pub god: BTreeMap<String, bool>,
    pub jailed: BTreeMap<String, JailRecord>,
    /// Iteration 8: true = do not ask for confirmation in /clearinventory and /nuke.
    pub clear_confirm_off: BTreeMap<String, bool>,
    /// Set by `load_state` when a legacy shape was normalised; never serialised.
    #[serde(skip)]
    pub needs_migration: bool,
    /// Dirty: the in-memory state changed since the last write of
    /// `state.json`, so the periodic tick should save it. What counts as
    /// dirty:
    ///
    /// * the tick's playtime accounting — the only tick-driven mutation and
    ///   the only writer that sets this flag;
    /// * nothing else: every command/event that mutates other fields calls
    ///   `save_persisted()` immediately and never relies on this flag.
    ///
    /// Never serialised; resets on every load.
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            schema: schema(),
            back: BTreeMap::new(),
            fly: BTreeMap::new(),
            afk: BTreeMap::new(),
            tpa_off: BTreeMap::new(),
            playtime_ms: BTreeMap::new(),
            last_seen_ms: BTreeMap::new(),
            last_location: BTreeMap::new(),
            tpa_auto: BTreeMap::new(),
            names: BTreeMap::new(),
            msg_off: BTreeMap::new(),
            social_spy: BTreeMap::new(),
            ignored: BTreeMap::new(),
            nicks: BTreeMap::new(),
            reply_target: BTreeMap::new(),
            rtoggle: BTreeMap::new(),
            mail: BTreeMap::new(),
            mutes: BTreeMap::new(),
            bans: BTreeMap::new(),
            vanished: BTreeMap::new(),
            god: BTreeMap::new(),
            jailed: BTreeMap::new(),
            clear_confirm_off: BTreeMap::new(),
            needs_migration: false,
            dirty: false,
        }
    }
}

impl FileStore for PersistedState {
    const WRAPPER_KEY: &'static str = "state";

    fn canonical_schema() -> u32 {
        schema()
    }
    fn stored_schema(&self) -> u32 {
        self.schema
    }
    fn bump_schema(&mut self) {
        self.schema = schema();
    }
    fn needs_migration(&self) -> bool {
        self.needs_migration
    }
    fn set_needs_migration(&mut self, needs: bool) {
        self.needs_migration = needs;
    }
}

/// Read and parse `state.json`. Never writes: migration is deferred to the tick.
pub fn load_state(path: &Path) -> Result<PersistedState, String> {
    PersistedState::load(path)
}

pub fn save_state(path: &Path, state: &PersistedState) -> io::Result<()> {
    PersistedState::save_atomic(path, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pumpkinx-state-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn loc() -> Location {
        Location { world: "world".into(), x: 1.0, y: 64.0, z: 2.0, yaw: 0.0, pitch: 0.0 }
    }

    #[test]
    fn missing_is_default_and_corrupt_is_error() {
        let d = tmpdir("missing");
        let p = d.join("state.json");
        assert_eq!(load_state(&p).unwrap(), PersistedState::default());
        fs::write(&p, "{ broken").unwrap();
        assert!(load_state(&p).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ broken");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn roundtrip_all_sections() {
        let d = tmpdir("roundtrip");
        let p = d.join("state.json");
        let mut state = PersistedState::default();
        state.back.insert("u".into(), loc());
        state.fly.insert("u".into(), FlyState { allow_flying: true, flying: true, fly_speed: 0.25 });
        state.afk.insert("u".into(), AfkState { active: true, reason: "dinner".into(), since_ms: 123 });
        state.tpa_off.insert("u".into(), true);
        state.playtime_ms.insert("u".into(), 3_600_000);
        state.names.insert("u".into(), "Alex".into());
        state.msg_off.insert("u".into(), true);
        state.social_spy.insert("u".into(), true);
        state.ignored.insert("u".into(), vec!["v".into()]);
        state.nicks.insert("u".into(), "Nick".into());
        state.reply_target.insert("u".into(), "v".into());
        state.rtoggle.insert("u".into(), false);
        state.mail.insert("v".into(), vec![MailMessage { from: "u".into(), from_uuid: "u".into(), body: "hi".into(), time_ms: 1, read: false }]);
        state.mutes.insert("u".into(), ModRecord { until_ms: 2, reason: "spam".into(), by: "op".into() });
        state.bans.insert("u".into(), ModRecord { until_ms: 3, reason: "bad".into(), by: "op".into() });
        state.vanished.insert("u".into(), true);
        state.god.insert("u".into(), true);
        state.jailed.insert("u".into(), JailRecord { jail: "main".into(), until_ms: 4, by: "mod".into() });
        state.clear_confirm_off.insert("u".into(), true);
        save_state(&p, &state).unwrap();
        let loaded = load_state(&p).unwrap();
        assert_eq!(loaded, state);
        assert!(!loaded.needs_migration);
        assert!(!d.join("state.json.tmp").exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn legacy_wrapper_is_loaded_in_memory_without_writing() {
        let d = tmpdir("legacy-wrapper");
        let p = d.join("state.json");
        let original = r#"{"state":{"schema":1,"back":{"u":{"world":"world","x":1.0,"y":64.0,"z":2.0,"yaw":0.0,"pitch":0.0}}}}"#;
        fs::write(&p, original).unwrap();
        let state = load_state(&p).unwrap();
        assert!(state.needs_migration);
        assert_eq!(state.back["u"], loc());
        // The load must not touch the file (no blocking write during startup).
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn old_schema_marks_migration_without_writing() {
        let d = tmpdir("legacy-schema");
        let p = d.join("state.json");
        let original = r#"{"schema":1,"names":{"u":"Alex"}}"#;
        fs::write(&p, original).unwrap();
        let state = load_state(&p).unwrap();
        assert!(state.needs_migration);
        assert_eq!(state.schema, schema());
        assert!(state.mutes.is_empty() && state.mail.is_empty());
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        // A deferred write produces the canonical current schema.
        save_state(&p, &state).unwrap();
        let after = load_state(&p).unwrap();
        assert_eq!(after.schema, schema());
        assert!(!after.needs_migration);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn dirty_flag_never_reaches_disk() {
        let d = tmpdir("dirty");
        let p = d.join("state.json");
        let mut state = PersistedState::default();
        state.dirty = true;
        save_state(&p, &state).unwrap();
        assert!(!load_state(&p).unwrap().dirty);
        let _ = fs::remove_dir_all(d);
    }
}

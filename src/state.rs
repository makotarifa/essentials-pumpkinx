//! Session metadata persisted in `state.json` inside the plugin data folder.
//!
//! `load_state` is intentionally read-only: legacy shapes are normalised in
//! memory and a `needs_migration` flag is set. The periodic tick writes the
//! canonical file later, so loading can never block on file I/O.

use crate::store::{self, Location};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io, path::Path};

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
    // --- iteración 5 ---
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
    /// Iteración 8: true = no pedir confirmación en /clearinventory y /nuke.
    pub clear_confirm_off: BTreeMap<String, bool>,
    /// Set by `load_state` when a legacy shape was normalised; never serialised.
    #[serde(skip)]
    pub needs_migration: bool,
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
        }
    }
}

/// A wrapper file (`{"state": {...}}`) is accepted without any disk write.
fn unwrap_wrapper(value: &mut Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    if map.len() != 1 {
        return false;
    }
    let Some(inner) = map.get("state").cloned() else {
        return false;
    };
    *value = inner;
    true
}

/// Read and parse `state.json`. Never writes: migration is deferred to the tick.
pub fn load_state(path: &Path) -> Result<PersistedState, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => return Ok(PersistedState::default()),
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(PersistedState::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut value: Value = serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let legacy = unwrap_wrapper(&mut value);
    let mut state: PersistedState =
        serde_json::from_value(value).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let old_schema = state.schema < schema();
    if old_schema {
        state.schema = schema();
    }
    state.needs_migration = legacy || old_schema;
    Ok(state)
}

pub fn save_state(path: &Path, state: &PersistedState) -> io::Result<()> {
    store::save_json(path, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fusion-state-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
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
        let mut s = PersistedState::default();
        s.back.insert("u".into(), loc());
        s.fly.insert("u".into(), FlyState { allow_flying: true, flying: true, fly_speed: 0.25 });
        s.afk.insert("u".into(), AfkState { active: true, reason: "dinner".into(), since_ms: 123 });
        s.tpa_off.insert("u".into(), true);
        s.playtime_ms.insert("u".into(), 3_600_000);
        s.names.insert("u".into(), "Alex".into());
        s.msg_off.insert("u".into(), true);
        s.social_spy.insert("u".into(), true);
        s.ignored.insert("u".into(), vec!["v".into()]);
        s.nicks.insert("u".into(), "Nick".into());
        s.reply_target.insert("u".into(), "v".into());
        s.rtoggle.insert("u".into(), false);
        s.mail.insert("v".into(), vec![MailMessage { from: "u".into(), from_uuid: "u".into(), body: "hi".into(), time_ms: 1, read: false }]);
        s.mutes.insert("u".into(), ModRecord { until_ms: 2, reason: "spam".into(), by: "op".into() });
        s.bans.insert("u".into(), ModRecord { until_ms: 3, reason: "bad".into(), by: "op".into() });
        s.vanished.insert("u".into(), true);
        s.god.insert("u".into(), true);
        s.jailed.insert("u".into(), JailRecord { jail: "main".into(), until_ms: 4, by: "mod".into() });
        s.clear_confirm_off.insert("u".into(), true);
        save_state(&p, &s).unwrap();
        let loaded = load_state(&p).unwrap();
        assert_eq!(loaded, s);
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
        let s = load_state(&p).unwrap();
        assert!(s.needs_migration);
        assert_eq!(s.back["u"], loc());
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
        let s = load_state(&p).unwrap();
        assert!(s.needs_migration);
        assert_eq!(s.schema, schema());
        assert!(s.mutes.is_empty() && s.mail.is_empty());
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        // A deferred write produces the canonical current schema.
        save_state(&p, &s).unwrap();
        let after = load_state(&p).unwrap();
        assert_eq!(after.schema, schema());
        assert!(!after.needs_migration);
        let _ = fs::remove_dir_all(d);
    }
}

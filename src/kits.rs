//! Kits, cooldowns, powertools and unlimited items in `kits.json`.
//!
//! `Kits` implements the shared [`FileStore`] pattern (`crate::store::persist`):
//! read-only load, legacy formats normalised in memory and writes deferred
//! to the first tick (same pattern as state/economy).

use crate::store::persist::FileStore;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, path::Path};

fn schema() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitItem {
    pub id: String,
    pub count: u8,
}

impl Default for KitItem {
    fn default() -> Self {
        Self { id: "minecraft:stone".into(), count: 1 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kit {
    pub items: Vec<KitItem>,
    pub delay_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kits {
    #[serde(default = "schema")]
    pub schema: u32,
    pub kits: BTreeMap<String, Kit>,
    /// "<uuid>|<kit>" -> Unix ms when the cooldown expires.
    pub cooldowns: BTreeMap<String, u64>,
    /// UUID -> (item key -> commands).
    pub powertools: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// UUID -> powertools enabled.
    pub powertool_toggle: BTreeMap<String, bool>,
    /// UUID -> unlimited item keys.
    pub unlimited: BTreeMap<String, Vec<String>>,
    #[serde(skip)]
    pub needs_migration: bool,
}

impl Default for Kits {
    fn default() -> Self {
        Self {
            schema: schema(),
            kits: BTreeMap::new(),
            cooldowns: BTreeMap::new(),
            powertools: BTreeMap::new(),
            powertool_toggle: BTreeMap::new(),
            unlimited: BTreeMap::new(),
            needs_migration: false,
        }
    }
}

pub fn normalize_item_key(raw: &str) -> String {
    let text = raw.trim().to_ascii_lowercase();
    if text.contains(':') { text } else { format!("minecraft:{text}") }
}

pub fn cooldown_key(uuid: &str, kit: &str) -> String {
    format!("{uuid}|{kit}")
}

pub fn is_ready(kits: &Kits, uuid: &str, kit: &str, now_ms: u64) -> bool {
    kits.cooldowns.get(&cooldown_key(uuid, kit)).is_none_or(|until| *until <= now_ms)
}

pub fn mark_used(kits: &mut Kits, uuid: &str, kit: &str, delay_secs: u64, now_ms: u64) {
    if delay_secs == 0 {
        kits.cooldowns.remove(&cooldown_key(uuid, kit));
    } else {
        kits.cooldowns.insert(
            cooldown_key(uuid, kit),
            now_ms.saturating_add(delay_secs.saturating_mul(1000)),
        );
    }
}

pub fn list_powertools(kits: &Kits, uuid: &str) -> Vec<(String, Vec<String>)> {
    kits.powertools.get(uuid).map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

impl FileStore for Kits {
    const WRAPPER_KEY: &'static str = "kits";

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

/// Read-only load; migration is deferred to the first tick.
pub fn load_kits(path: &Path) -> Result<Kits, String> {
    Kits::load(path)
}

pub fn save_kits(path: &Path, kits: &Kits) -> io::Result<()> {
    Kits::save_atomic(path, kits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pumpkinx-kits-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn item_key_normalisation() {
        assert_eq!(normalize_item_key(" STONE "), "minecraft:stone");
        assert_eq!(normalize_item_key("minecraft:DIAMOND_SWORD"), "minecraft:diamond_sword");
    }

    #[test]
    fn cooldowns_expire_and_reset() {
        let mut k = Kits::default();
        let kit = Kit { items: vec![], delay_secs: 60 };
        k.kits.insert("starter".into(), kit);
        assert!(is_ready(&k, "u", "starter", 1000));
        mark_used(&mut k, "u", "starter", 60, 1000);
        assert!(!is_ready(&k, "u", "starter", 60_999));
        assert!(is_ready(&k, "u", "starter", 61_000));
        mark_used(&mut k, "u", "starter", 0, 1000);
        assert!(is_ready(&k, "u", "starter", 1000));
        assert!(k.cooldowns.is_empty());
    }

    #[test]
    fn legacy_migration_is_deferred() {
        let d = tmpdir("legacy");
        let p = d.join("kits.json");
        let original = r#"{"kits":{"schema":0,"kits":{"starter":{"items":[{"id":"minecraft:stone","count":16}],"delay_secs":60}}}}"#;
        fs::write(&p, original).unwrap();
        let k = load_kits(&p).unwrap();
        assert!(k.needs_migration);
        assert_eq!(k.schema, schema());
        assert_eq!(k.kits["starter"].items[0].count, 16);
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        save_kits(&p, &k).unwrap();
        let after = load_kits(&p).unwrap();
        assert!(!after.needs_migration);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_file_is_error_without_overwrite() {
        let d = tmpdir("corrupt");
        let p = d.join("kits.json");
        fs::write(&p, "{ broken").unwrap();
        assert!(load_kits(&p).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ broken");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn powertool_roundtrip() {
        let mut k = Kits::default();
        k.powertools.insert("u".into(), BTreeMap::from([("minecraft:stone".into(), vec!["home".into()])]));
        assert_eq!(list_powertools(&k, "u")[0].1, vec!["home"]);
        assert!(list_powertools(&k, "other").is_empty());
    }
}

//! Kits, cooldowns, powertools y objetos ilimitados en `kits.json`.
//!
//! Carga de solo lectura: los formatos legacy se normalizan en memoria y se
//! escriben de forma diferida en el primer tick (patrón de state/economy).

use crate::store;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io, path::Path};

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
    /// "<uuid>|<kit>" -> Unix ms de caducidad del cooldown.
    pub cooldowns: BTreeMap<String, u64>,
    /// UUID -> (clave de objeto -> comandos).
    pub powertools: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// UUID -> powertools activados.
    pub powertool_toggle: BTreeMap<String, bool>,
    /// UUID -> claves de objetos ilimitados.
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

fn unwrap_wrapper(value: &mut Value) -> bool {
    let Some(map) = value.as_object() else { return false };
    if map.len() != 1 { return false; }
    let Some(inner) = map.get("kits").cloned() else { return false; };
    *value = inner;
    true
}

pub fn load_kits(path: &Path) -> Result<Kits, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => return Ok(Kits::default()),
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Kits::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut value: Value = serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let legacy = unwrap_wrapper(&mut value);
    let mut kits: Kits = serde_json::from_value(value).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let old_schema = kits.schema < schema();
    if old_schema {
        kits.schema = schema();
    }
    kits.needs_migration = legacy || old_schema;
    Ok(kits)
}

pub fn save_kits(path: &Path, kits: &Kits) -> io::Result<()> {
    store::save_json(path, kits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fusion-kits-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
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

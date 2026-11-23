//! JSON storage for homes and warps. Writes are atomic (temp file + rename).
//! A corrupt file is an error, never silently treated as empty.
//!
//! The canonical shapes are identical to PumpkinEssentials 0.1.0:
//! homes.json: { "<uuid>": { "name_last_seen": "...", "limit": n?, "homes": { name: Location } } }
//! warps.json: { "<name>": { world,x,y,z,yaw,pitch,created_by } }
//!
//! On load, common legacy variants (a `players`/`warps` wrapper,
//! `position`/`rotation` sub-objects, `dimension`/`world_name` instead of
//! `world`, numeric strings) are normalised and written back atomically.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::Path,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Location {
    pub fn pos(&self) -> (f64, f64, f64) {
        (self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerHomes {
    pub name_last_seen: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    pub homes: BTreeMap<String, Location>,
}

/// Keyed by player UUID string.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HomesFile {
    pub players: BTreeMap<String, PlayerHomes>,
}

impl HomesFile {
    /// Find a player's UUID string by the last name we saw them with (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&String> {
        self.players
            .iter()
            .find(|(_, p)| p.name_last_seen.eq_ignore_ascii_case(name))
            .map(|(k, _)| k)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    #[serde(flatten)]
    pub loc: Location,
    pub created_by: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WarpsFile {
    pub warps: BTreeMap<String, Warp>,
}

/// Missing/empty file => default. Unreadable/corrupt file => Err.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(T::default()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    write_atomic(path, text.as_bytes())
}

/// Write `data` to `<path>.tmp`, then rename over `path`.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        let _ = f.sync_all();
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&tmp, path)
        }
        Err(e) => Err(e),
    }
}

// ---- migration ----------------------------------------------------------------

fn take_field(obj: &mut Map<String, Value>, names: &[&str]) -> Option<Value> {
    for name in names {
        if let Some(v) = obj.remove(*name) {
            return Some(v);
        }
    }
    None
}

fn normalise_number(obj: &mut Map<String, Value>, key: &str) -> bool {
    let Some(text) = obj.get(key).and_then(Value::as_str) else {
        return false;
    };
    let Ok(number) = text.parse::<f64>() else {
        return false;
    };
    obj.insert(key.to_string(), Value::from(number));
    true
}

/// Flatten legacy position/rotation sub-objects into a canonical Location object.
fn normalise_location(value: &mut Value) -> bool {
    let Some(obj) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    if let Some(pos) = take_field(obj, &["position", "pos"]) {
        if let Some(pos) = pos.as_object() {
            for key in ["x", "y", "z"] {
                if !obj.contains_key(key) {
                    if let Some(v) = pos.get(key) {
                        obj.insert(key.to_string(), v.clone());
                    }

                }
            }
        }
        changed = true;
    }
    if let Some(rot) = take_field(obj, &["rotation", "rot"]) {
        if let Some(rot) = rot.as_object() {
            for key in ["yaw", "pitch"] {
                if !obj.contains_key(key) {
                    if let Some(v) = rot.get(key) {
                        obj.insert(key.to_string(), v.clone());
                    }

                }
            }
        }
        changed = true;
    }
    if !obj.contains_key("world") {
        if let Some(world) = take_field(obj, &["dimension", "world_name", "worldName"]) {
            obj.insert("world".to_string(), world);
            changed = true;
        }
    }
    for key in ["x", "y", "z", "yaw", "pitch"] {
        changed |= normalise_number(obj, key);
    }
    changed
}

/// A wrapper file has exactly one top-level key (`players` or `warps`).
fn unwrap_wrapper(value: &mut Value, key: &str) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    if map.len() != 1 {
        return false;
    }
    let Some(inner) = map.get(key).cloned() else {
        return false;
    };
    *value = inner;
    true
}

fn normalise_homes(value: &mut Value) -> bool {
    let mut changed = unwrap_wrapper(value, "players");
    let Some(map) = value.as_object_mut() else {
        return changed;
    };
    let keys: Vec<String> = map.keys().cloned().collect();
    for key in keys {
        let Some(player) = map.get_mut(&key) else {
            continue;
        };
        let Some(p) = player.as_object_mut() else {
            continue;
        };
        if !p.contains_key("name_last_seen") {
            p.insert("name_last_seen".to_string(), Value::String(String::new()));
            changed = true;
        }
        if let Some(limit) = p.get("limit").and_then(Value::as_str) {
            if let Ok(n) = limit.parse::<u32>() {
                p.insert("limit".to_string(), Value::from(n));
                changed = true;
            }
        }
        if !p.contains_key("homes") {
            p.insert("homes".to_string(), Value::Object(Map::new()));
            changed = true;
        }
        if let Some(homes) = p.get_mut("homes").and_then(Value::as_object_mut) {
            for loc in homes.values_mut() {
                changed |= normalise_location(loc);
            }
        }
    }
    changed
}

fn normalise_warps(value: &mut Value) -> bool {
    let mut changed = unwrap_wrapper(value, "warps");
    let Some(map) = value.as_object_mut() else {
        return changed;
    };
    for warp in map.values_mut() {
        if let Some(obj) = warp.as_object_mut() {
            if !obj.contains_key("created_by") {
                obj.insert("created_by".to_string(), Value::String("unknown".into()));
                changed = true;
            }
        }
        changed |= normalise_location(warp);
    }
    changed
}

fn read_value(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(None),
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Load homes.json, normalising legacy shapes and migrating on disk if needed.
pub fn load_homes(path: &Path) -> Result<HomesFile, String> {
    let Some(mut value) = read_value(path)? else {
        return Ok(HomesFile::default());
    };
    if normalise_homes(&mut value) {
        let migrated: HomesFile =
            serde_json::from_value(value).map_err(|e| format!("migrate {}: {e}", path.display()))?;
        save_json(path, &migrated).map_err(|e| format!("save migrated {}: {e}", path.display()))?;
        Ok(migrated)
    } else {
        serde_json::from_value(value).map_err(|e| format!("parse {}: {e}", path.display()))
    }
}

/// Load warps.json, normalising legacy shapes and migrating on disk if needed.
pub fn load_warps(path: &Path) -> Result<WarpsFile, String> {
    let Some(mut value) = read_value(path)? else {
        return Ok(WarpsFile::default());
    };
    if normalise_warps(&mut value) {
        let migrated: WarpsFile =
            serde_json::from_value(value).map_err(|e| format!("migrate {}: {e}", path.display()))?;
        save_json(path, &migrated).map_err(|e| format!("save migrated {}: {e}", path.display()))?;
        Ok(migrated)
    } else {
        serde_json::from_value(value).map_err(|e| format!("parse {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn loc(w: &str) -> Location {
        Location { world: w.into(), x: 1.5, y: 64.0, z: -3.25, yaw: 90.0, pitch: -10.0 }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fusion-ess-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn homes_roundtrip_and_no_tmp_left() {
        let d = tmpdir("homes");
        let p = d.join("homes.json");
        let mut f = HomesFile::default();
        let mut ph = PlayerHomes { name_last_seen: "Bob".into(), limit: Some(7), ..Default::default() };
        ph.homes.insert("base".into(), loc("world"));
        f.players.insert("00000000-0000-0000-0000-000000000001".into(), ph);
        save_json(&p, &f).unwrap();
        save_json(&p, &f).unwrap();
        assert_eq!(load_homes(&p).unwrap(), f);
        assert!(!d.join("homes.json.tmp").exists());
        assert_eq!(f.find_by_name("bob").unwrap(), "00000000-0000-0000-0000-000000000001");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn json_shape_matches_pumpkin_essentials() {
        let mut f = HomesFile::default();
        let mut ph = PlayerHomes { name_last_seen: "A".into(), ..Default::default() };
        ph.homes.insert("h".into(), loc("world"));
        f.players.insert("u".into(), ph);
        let v: Value = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(v["u"]["name_last_seen"], "A");
        assert!(v["u"].get("limit").is_none());
        assert_eq!(v["u"]["homes"]["h"]["world"], "world");
        assert_eq!(v["u"]["homes"]["h"]["pitch"], -10.0);
    }

    #[test]
    fn warps_roundtrip_flat() {
        let d = tmpdir("warps");
        let p = d.join("warps.json");
        let mut f = WarpsFile::default();
        f.warps.insert("spawn".into(), Warp { loc: loc("world"), created_by: "Op".into() });
        save_json(&p, &f).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["spawn"]["created_by"], "Op");
        assert_eq!(v["spawn"]["x"], 1.5);
        assert_eq!(load_warps(&p).unwrap(), f);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn missing_is_default_corrupt_is_error() {
        let d = tmpdir("bad");
        let p = d.join("homes.json");
        assert!(load_homes(&p).unwrap().players.is_empty());
        fs::write(&p, "{ not json").unwrap();
        assert!(load_homes(&p).is_err());
        assert!(load_warps(&p).is_err());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn migrates_legacy_homes_in_place() {
        let d = tmpdir("mig-homes");
        let p = d.join("homes.json");
        fs::write(
            &p,
            r#"{"players":{"u":{"limit":"5","homes":{"base":{"dimension":"world","position":{"x":1,"y":2,"z":3},"rotation":{"yaw":4,"pitch":5}}}}}}"#,
        )
        .unwrap();
        let homes = load_homes(&p).unwrap();
        let e = &homes.players["u"];
        assert_eq!(e.limit, Some(5));
        let l = &e.homes["base"];
        assert_eq!((l.world.clone(), l.x, l.y, l.z, l.yaw, l.pitch), ("world".into(), 1.0, 2.0, 3.0, 4.0, 5.0));
        assert_eq!(load_homes(&p).unwrap(), homes);
        let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert!(v.get("players").is_none());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn migrates_legacy_warps_in_place() {
        let d = tmpdir("mig-warps");
        let p = d.join("warps.json");
        fs::write(
            &p,
            r#"{"warps":{"spawn":{"world_name":"world","position":{"x":9,"y":64,"z":8},"rotation":{"yaw":1,"pitch":2}}}}"#,
        )
        .unwrap();
        let warps = load_warps(&p).unwrap();
        let w = &warps.warps["spawn"];
        assert_eq!(w.created_by, "unknown");
        assert_eq!((w.loc.world.clone(), w.loc.x, w.loc.y, w.loc.z), ("world".into(), 9.0, 64.0, 8.0));
        assert_eq!(load_warps(&p).unwrap(), warps);
        let _ = fs::remove_dir_all(d);
    }
}

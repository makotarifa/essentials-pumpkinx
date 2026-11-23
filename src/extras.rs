//! Pure helpers for the VEssentials-style commands: flyspeed validation,
//! AFK tracking and game-mode parsing. Kept host-independent for unit tests.

use crate::i18n;
use crate::logic::{self, Key};
use crate::state::PersistedState;
use pumpkin_plugin_api::common::GameMode;
use std::collections::HashMap;

/// AFK state for one player. `pos` is the position when AFK was set; moving
/// further than 0.5 blocks clears the state.
#[derive(Debug, Clone, PartialEq)]
pub struct AfkInfo {
    pub reason: String,
    pub since_ms: u64,
    pub pos: (f64, f64, f64),
}

#[derive(Debug, Default)]
pub struct Afks {
    map: HashMap<Key, AfkInfo>,
}

impl Afks {
    pub fn is_afk(&self, key: Key) -> bool {
        self.map.contains_key(&key)
    }

    pub fn get(&self, key: Key) -> Option<&AfkInfo> {
        self.map.get(&key)
    }

    pub fn set(&mut self, key: Key, info: AfkInfo) {
        self.map.insert(key, info);
    }

    pub fn remove(&mut self, key: Key) -> Option<AfkInfo> {
        self.map.remove(&key)
    }

    /// Clear AFK when the player moved beyond the warmup-style threshold.
    pub fn on_move(&mut self, key: Key, to: (f64, f64, f64)) -> Option<AfkInfo> {
        let far = self.map.get(&key).is_some_and(|info| logic::moved_beyond(info.pos, to, logic::MAX_MOVE));
        if far { self.map.remove(&key) } else { None }
    }
}

/// Parse the player-facing fly speed. The accepted range is inclusive 0.0-10.0.
pub fn parse_fly_speed(raw: &str) -> Result<f32, String> {
    let value: f32 = raw.parse().map_err(|_| i18n::trf("Velocidad de vuelo inválida: '{}'. Usa un número entre 0.0 y 10.0.", "Invalid fly speed '{}'. Use a number from 0.0 to 10.0.", &[raw]))?;
    if !value.is_finite() || !(0.0..=10.0).contains(&value) {
        return Err(i18n::trf("La velocidad de vuelo debe estar entre 0.0 y 10.0 (se obtuvo {}).", "Fly speed must be between 0.0 and 10.0 (got {}).", &[raw]));
    }
    Ok(value)
}

/// Convert a player-facing 0.0-10.0 value to the engine's ability value.
/// 1.0 maps to the vanilla fly speed (0.05 by default).
pub fn engine_fly_speed(value: f32, unit: f32) -> f32 {
    value * unit
}

/// Names and numeric codes accepted by /gm and its aliases.
pub fn parse_mode(raw: &str) -> Option<GameMode> {
    match raw.to_ascii_lowercase().as_str() {
        "s" | "0" | "survival" => Some(GameMode::Survival),
        "c" | "1" | "creative" => Some(GameMode::Creative),
        "a" | "2" | "adventure" => Some(GameMode::Adventure),
        "sp" | "3" | "spectator" | "spec" => Some(GameMode::Spectator),
        _ => None,
    }
}

pub fn mode_name(mode: &GameMode) -> &'static str {
    match mode {
        GameMode::Survival => "survival",
        GameMode::Creative => "creative",
        GameMode::Adventure => "adventure",
        GameMode::Spectator => "spectator",
    }
}

/// Parse the player-facing walk speed (`/speed`). The accepted range is 0.1-10.0.
pub fn parse_walk_speed(raw: &str) -> Result<f32, String> {
    let value: f32 = raw.parse().map_err(|_| i18n::trf("Velocidad al caminar inválida: '{}'. Usa un número entre 0.1 y 10.0.", "Invalid walk speed '{}'. Use a number from 0.1 to 10.0.", &[raw]))?;
    if !value.is_finite() || !(0.1..=10.0).contains(&value) {
        return Err(i18n::trf("La velocidad al caminar debe estar entre 0.1 y 10.0 (se obtuvo {}).", "Walk speed must be between 0.1 and 10.0 (got {}).", &[raw]));
    }
    Ok(value)
}

/// Convert a `/speed` value to the engine's walk-speed value (1.0 = 0.1 vanilla).
pub fn engine_walk_speed(value: f32) -> f32 {
    value * 0.1
}

/// Parse an experience level for `/exp`; rejects negative values.
pub fn parse_level(raw: &str) -> Result<i32, String> {
    let level: i32 = raw.parse().map_err(|_| i18n::trf("Nivel de experiencia inválido: '{}'.", "Invalid experience level '{}'.", &[raw]))?;
    if level < 0 {
        return Err(i18n::tr("El nivel de experiencia no puede ser negativo.", "Experience level cannot be negative.").into());
    }
    Ok(level)
}

/// `/top` lands one block above the highest solid block in the column.
pub fn top_dest_y(highest_block_y: i32) -> f64 {
    highest_block_y as f64 + 1.0
}

/// Human-readable duration for /playtime, /seen and /whois.
pub fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (hours, rem) = (rem / 3600, rem % 3600);
    let (mins, secs) = (rem / 60, rem % 60);
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Compass direction for a yaw in degrees (EssentialsX 8-point sectors).
pub fn bearing(yaw: f32) -> (&'static str, u16) {
    let deg = ((yaw + 180.0) % 360.0 + 360.0) % 360.0;
    let dir = match deg as u16 {
        0..=22 => "N",
        23..=67 => "NE",
        68..=112 => "E",
        113..=157 => "SE",
        158..=202 => "S",
        203..=247 => "SO",
        248..=292 => "O",
        293..=337 => "NO",
        _ => "N",
    };
    (dir, deg as u16)
}

#[derive(Debug, Clone, PartialEq)]
pub struct NearEntry {
    pub name: String,
    pub distance: f64,
}

pub fn sort_near(mut entries: Vec<NearEntry>) -> Vec<NearEntry> {
    entries.sort_by(|a, b| a.distance.total_cmp(&b.distance).then_with(|| a.name.cmp(&b.name)));
    entries
}

/// Parse an absolute or `~relative` coordinate for /tppos.
pub fn parse_tp_coord(raw: &str, current: f64) -> Result<f64, String> {
    let value = if let Some(rest) = raw.strip_prefix('~') {
        if rest.is_empty() { current } else { current + rest.parse::<f64>().map_err(|_| i18n::trf("Coordenada inválida: '{}'.", "Invalid coordinate '{}'.", &[raw]))? }
    } else {
        raw.parse::<f64>().map_err(|_| i18n::trf("Coordenada inválida: '{}'.", "Invalid coordinate '{}'.", &[raw]))?
    };
    if !value.is_finite() || value.abs() > 30_000_000.0 {
        return Err(i18n::tr("Coordenada fuera de rango.", "Coordinate out of range.").into());
    }
    Ok(value)
}

/// Deterministic pseudo-random offset in `[0, radius)` from a time-based seed.
pub fn random_offset(radius: f64, seed: u128) -> f64 {
    if radius <= 0.0 {
        return 0.0;
    }
    let fraction = (seed % 1_000_000) as f64 / 1_000_000.0;
    radius * fraction
}


/// A random point in an annulus around a centre, for /tpr.
pub fn random_point(cx: f64, cz: f64, min: f64, max: f64, seed: u128) -> (f64, f64) {
    let angle = std::f64::consts::TAU * random_offset(1.0, seed);
    let distance = min + random_offset((max - min).max(0.0), seed.wrapping_add(0x9E37_79B9));
    (cx + angle.cos() * distance, cz + angle.sin() * distance)
}

/// Parse a moderation duration: `10s`, `5m`, `2h`, `1d` or a plain number of seconds.
/// `0` or `perm` means permanent (represented by `0`).
pub fn parse_duration(raw: &str) -> Result<u64, String> {
    let text = raw.trim().to_ascii_lowercase();
    if text == "perm" || text == "permanent" {
        return Ok(0);
    }
    let (num, mult) = if let Some(n) = text.strip_suffix('s') {
        (n, 1u64)
    } else if let Some(n) = text.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = text.strip_suffix('h') {
        (n, 3600)
    } else if let Some(n) = text.strip_suffix('d') {
        (n, 86_400)
    } else {
        (text.as_str(), 1)
    };
    let value: u64 = num.parse().map_err(|_| i18n::trf("Duración inválida: '{}'.", "Invalid duration: '{}'.", &[raw]))?;
    Ok(value.saturating_mul(mult))
}

/// Convert a seconds duration into an absolute Unix-ms expiry (`0` stays permanent).
pub fn expiry_from_now(seconds: u64, now_ms: u64) -> u64 {
    if seconds == 0 { 0 } else { now_ms.saturating_add(seconds.saturating_mul(1000)) }
}
/// Find the stored UUID for a name (used by /seen and /tpoffline with offline players).
pub fn find_uuid_by_name(state: &PersistedState, name: &str) -> Option<String> {
    state
        .names
        .iter()
        .find(|(_, stored)| stored.eq_ignore_ascii_case(name))
        .map(|(uuid, _)| uuid.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_range() {
        assert_eq!(parse_fly_speed("0"), Ok(0.0));
        assert_eq!(parse_fly_speed("1.5"), Ok(1.5));
        assert_eq!(parse_fly_speed("10.0"), Ok(10.0));
        assert!(parse_fly_speed("10.1").is_err());
        assert!(parse_fly_speed("-0.1").is_err());
        assert!(parse_fly_speed("abc").is_err());
        assert!(parse_fly_speed("NaN").is_err());
        assert_eq!(engine_fly_speed(1.0, 0.05), 0.05);
    }

    #[test]
    fn mode_aliases() {
        assert!(matches!(parse_mode("s"), Some(GameMode::Survival)));
        assert!(matches!(parse_mode("1"), Some(GameMode::Creative)));
        assert!(matches!(parse_mode("A"), Some(GameMode::Adventure)));
        assert!(matches!(parse_mode("spectator"), Some(GameMode::Spectator)));
        assert!(parse_mode("nope").is_none());
    }

    #[test]
    fn afk_move_clears_only_after_threshold() {
        let mut afks = Afks::default();
        afks.set(
            7,
            AfkInfo { reason: String::new(), since_ms: 0, pos: (1.0, 64.0, 2.0) },
        );
        assert!(afks.is_afk(7));
        assert!(afks.on_move(7, (1.2, 64.0, 2.0)).is_none());
        assert!(afks.on_move(7, (2.0, 64.0, 2.0)).is_some());
        assert!(!afks.is_afk(7));
    }
    #[test]
    fn walk_speed_range() {
        assert_eq!(parse_walk_speed("0.1"), Ok(0.1));
        assert_eq!(parse_walk_speed("10"), Ok(10.0));
        assert!(parse_walk_speed("0.09").is_err());
        assert!(parse_walk_speed("10.1").is_err());
        assert_eq!(engine_walk_speed(1.0), 0.1);
    }

    #[test]
    fn level_and_top_helpers() {
        assert_eq!(parse_level("0"), Ok(0));
        assert_eq!(parse_level("30"), Ok(30));
        assert!(parse_level("-1").is_err());
        assert!(parse_level("x").is_err());
        assert_eq!(top_dest_y(70), 71.0);
    }
    #[test]
    fn duration_and_bearing() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(65_000), "1m 5s");
        assert_eq!(format_duration(3_661_000), "1h 1m 1s");
        assert_eq!(bearing(0.0), ("S", 180));
        assert_eq!(bearing(180.0), ("N", 0));
        assert_eq!(bearing(90.0).0, "O");
    }

    #[test]
    fn near_sorting() {
        let list = vec![
            NearEntry { name: "B".into(), distance: 9.0 },
            NearEntry { name: "A".into(), distance: 2.0 },
        ];
        assert_eq!(sort_near(list)[0].name, "A");
    }

    #[test]
    fn tppos_coordinates() {
        assert_eq!(parse_tp_coord("10", 5.0), Ok(10.0));
        assert_eq!(parse_tp_coord("~", 5.0), Ok(5.0));
        assert_eq!(parse_tp_coord("~-2.5", 5.0), Ok(2.5));
        assert!(parse_tp_coord("x", 5.0).is_err());
        assert!(parse_tp_coord("40000000", 0.0).is_err());
    }

    #[test]
    fn random_and_name_lookup() {
        assert_eq!(random_offset(100.0, 0), 0.0);
        assert!(random_offset(100.0, 123_456) < 100.0);
        let mut state = PersistedState::default();
        state.names.insert("uuid-1".into(), "Alex".into());
        assert_eq!(find_uuid_by_name(&state, "alex").as_deref(), Some("uuid-1"));
        assert!(find_uuid_by_name(&state, "nobody").is_none());
    }
    #[test]
    fn random_point_in_annulus() {
        let (x, z) = random_point(0.0, 0.0, 10.0, 100.0, 42);
        let d = (x * x + z * z).sqrt();
        assert!((10.0..100.0).contains(&d), "distance was {d}");
    }
    #[test]
    fn moderation_duration() {
        assert_eq!(parse_duration("30"), Ok(30));
        assert_eq!(parse_duration("5m"), Ok(300));
        assert_eq!(parse_duration("2h"), Ok(7200));
        assert_eq!(parse_duration("perm"), Ok(0));
        assert!(parse_duration("abc").is_err());
        assert_eq!(expiry_from_now(0, 1000), 0);
        assert_eq!(expiry_from_now(2, 1000), 3000);
    }
}

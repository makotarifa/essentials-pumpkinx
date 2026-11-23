//! Plugin configuration (`config.json` in the plugin data dir). All fields optional.

use crate::store::Location;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TprArea {
    /// Optional custom centre; `None` uses the player's current world spawn.
    pub center: Option<Location>,
    pub min_range: Option<f64>,
    pub max_range: Option<f64>,
}

impl Default for TprArea {
    fn default() -> Self {
        Self { center: None, min_range: None, max_range: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_max_homes: u32,
    pub ops_bypass_home_limit: bool,
    pub warmup_secs: u64,
    pub cooldown_secs: u64,
    pub tpa_expire_secs: u64,
    pub flyspeed_unit: f32,
    pub afk_broadcast: bool,
    pub afk_auto_unafk_on_move: bool,
    pub afk_tab_name: bool,
    pub restore_fly_on_join: bool,
    pub restore_afk_on_join: bool,
    pub spawns: BTreeMap<String, Location>,
    /// Maximum radius accepted by /near.
    pub near_max_radius: i64,
    /// Default random-teleport ranges (overridable per world with /settpr).
    pub tpr_min_range: f64,
    pub tpr_max_range: f64,
    pub tpr_attempts: u32,
    pub tpr: BTreeMap<String, TprArea>,
    /// Texts shown by /customtext <key>.
    pub custom_texts: BTreeMap<String, Vec<String>>,
    /// Rules shown by /rules.
    pub rules: Vec<String>,
    /// Jails defined with /setjail.
    pub jails: BTreeMap<String, Location>,
    /// Symbol used by /balance, /pay, etc.
    pub currency_symbol: String,
    /// Displayed decimals (internal amounts are fixed cents).
    pub currency_decimals: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_max_homes: 3,
            ops_bypass_home_limit: false,
            warmup_secs: 3,
            cooldown_secs: 30,
            tpa_expire_secs: 60,
            flyspeed_unit: 0.05,
            afk_broadcast: true,
            afk_auto_unafk_on_move: true,
            afk_tab_name: true,
            restore_fly_on_join: true,
            restore_afk_on_join: true,
            spawns: BTreeMap::new(),
            near_max_radius: 200,
            tpr_min_range: 50.0,
            tpr_max_range: 1000.0,
            tpr_attempts: 24,
            tpr: BTreeMap::new(),
            custom_texts: BTreeMap::new(),
            rules: vec!["Respeta a los demás jugadores.".into(), "No uses hacks ni exploits.".into()],
            jails: BTreeMap::new(),
            currency_symbol: "$".into(),
            currency_decimals: 2,
        }
    }
}

impl Config {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let mut cfg: Self = serde_json::from_str(text)?;
        if cfg.tpa_expire_secs == 0 {
            cfg.tpa_expire_secs = Self::default().tpa_expire_secs;
        }
        if !cfg.flyspeed_unit.is_finite() || cfg.flyspeed_unit <= 0.0 {
            cfg.flyspeed_unit = Self::default().flyspeed_unit;
        }
        if cfg.near_max_radius <= 0 {
            cfg.near_max_radius = Self::default().near_max_radius;
        }
        if !cfg.tpr_min_range.is_finite() || cfg.tpr_min_range < 0.0 {
            cfg.tpr_min_range = Self::default().tpr_min_range;
        }
        if !cfg.tpr_max_range.is_finite() || cfg.tpr_max_range <= cfg.tpr_min_range {
            cfg.tpr_max_range = Self::default().tpr_max_range;
        }
        if cfg.tpr_attempts == 0 {
            cfg.tpr_attempts = Self::default().tpr_attempts;
        }
        Ok(cfg)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let config = Config::default();
        assert_eq!((config.default_max_homes, config.warmup_secs, config.cooldown_secs), (3, 3, 30));
        assert_eq!(config.tpa_expire_secs, 60);
        assert_eq!(config.flyspeed_unit, 0.05);
        assert_eq!(config.near_max_radius, 200);
        assert_eq!((config.tpr_min_range, config.tpr_max_range, config.tpr_attempts), (50.0, 1000.0, 24));
        assert!(config.tpr.is_empty());
    }

    #[test]
    fn legacy_json_fills_new_defaults() {
        let config = Config::from_json(
            r#"{"default_max_homes":3,"ops_bypass_home_limit":false,"warmup_secs":3,"cooldown_secs":30,"tpa_expire_secs":60}"#,
        )
        .unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_json_fills_defaults() {
        let config = Config::from_json(r#"{"warmup_secs": 0}"#).unwrap();
        assert_eq!(config.warmup_secs, 0);
        assert_eq!(config.cooldown_secs, 30);
    }

    #[test]
    fn spawn_and_tpr_roundtrip() {
        let mut config = Config::default();
        config.spawns.insert(
            "world".into(),
            Location { world: "world".into(), x: 1.5, y: 64.0, z: -2.5, yaw: 90.0, pitch: 0.0 },
        );
        config.tpr.insert("world".into(), TprArea { center: None, min_range: Some(10.0), max_range: None });
        let text = config.to_json();
        assert_eq!(Config::from_json(&text).unwrap(), config);
    }
}

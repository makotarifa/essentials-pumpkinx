//! Funciones puras para /ptime, /pweather y confirmaciones destructivas.

use crate::state::PersistedState;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherKind {
    Clear,
    Downfall,
    Reset,
}

/// Parse an EssentialsX-style time into world ticks.
pub fn parse_time(raw: &str) -> Option<u64> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "day" | "daytime" => Some(1000),
        "noon" | "midday" => Some(6000),
        "sunset" | "dusk" => Some(12_000),
        "night" | "nighttime" => Some(13_000),
        "midnight" => Some(18_000),
        "sunrise" | "dawn" => Some(23_000),
        value => value.parse::<u64>().ok(),
    }
}

pub fn parse_weather(raw: &str) -> Option<WeatherKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "clear" | "sun" | "sunny" => Some(WeatherKind::Clear),
        "downfall" | "storm" | "rain" | "thunder" => Some(WeatherKind::Downfall),
        "reset" | "off" | "default" => Some(WeatherKind::Reset),
        _ => None,
    }
}

/// True when a destructive command still needs an explicit confirmation.
pub fn needs_confirmation(state: &PersistedState, uuid: &str) -> bool {
    !state.clear_confirm_off.get(uuid).copied().unwrap_or(false)
}

pub fn toggle_confirmation(state: &mut PersistedState, uuid: &str) -> bool {
    let off = !state.clear_confirm_off.get(uuid).copied().unwrap_or(false);
    state.clear_confirm_off.insert(uuid.to_string(), off);
    off
}

/// In-memory two-step confirmation for destructive commands.
pub fn take_or_set(pending: &mut HashMap<u128, String>, key: u128, label: &str) -> bool {
    if pending.remove(&key).is_some_and(|previous| previous == label) {
        true
    } else {
        pending.insert(key, label.to_string());
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_aliases() {
        assert_eq!(parse_time("day"), Some(1000));
        assert_eq!(parse_time("noon"), Some(6000));
        assert_eq!(parse_time("night"), Some(13_000));
        assert_eq!(parse_time("12345"), Some(12_345));
        assert_eq!(parse_time("bogus"), None);
    }

    #[test]
    fn weather_aliases() {
        assert_eq!(parse_weather("sun"), Some(WeatherKind::Clear));
        assert_eq!(parse_weather("rain"), Some(WeatherKind::Downfall));
        assert_eq!(parse_weather("reset"), Some(WeatherKind::Reset));
        assert_eq!(parse_weather("snow"), None);
    }

    #[test]
    fn confirmation_preference_and_two_step() {
        let mut state = PersistedState::default();
        assert!(needs_confirmation(&state, "u"));
        assert!(toggle_confirmation(&mut state, "u"));
        assert!(!needs_confirmation(&state, "u"));
        let mut pending = HashMap::new();
        assert!(!take_or_set(&mut pending, 7, "clear"));
        assert!(take_or_set(&mut pending, 7, "clear"));
        assert!(!take_or_set(&mut pending, 7, "nuke"));
    }
}

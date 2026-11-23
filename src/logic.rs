//! Pure rules: names, home limits, cooldowns, warmups, tpa requests, /back.
//! All times are milliseconds since an arbitrary epoch supplied by the caller.

use crate::{
    config::Config,
    store::{Location, PlayerHomes},
};
use std::collections::HashMap;

pub type Key = u128;

/// Hyphenated UUID string for a key (the form used as the homes.json key).
pub fn uuid_string(key: Key) -> String {
    let hex = format!("{key:032x}");
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    BadChar,
}

impl NameError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "Name cannot be empty.",
            Self::TooLong => "Name is too long (max 32 characters).",
            Self::BadChar => "Name may only contain letters, digits, '_' and '-'.",
        }
    }
}

/// Validate a home/warp name; returns the stored (lowercase) form.
pub fn validate_name(raw: &str) -> Result<String, NameError> {
    if raw.is_empty() {
        return Err(NameError::Empty);
    }
    if raw.chars().count() > 32 {
        return Err(NameError::TooLong);
    }
    if !raw.chars().all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-') {
        return Err(NameError::BadChar);
    }
    Ok(raw.to_ascii_lowercase())
}

/// Effective home limit; `None` = unlimited.
pub fn effective_limit(cfg: &Config, override_limit: Option<u32>, is_op: bool) -> Option<u32> {
    if is_op && cfg.ops_bypass_home_limit {
        return None;
    }
    Some(override_limit.unwrap_or(cfg.default_max_homes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetHome {
    Created,
    Overwritten,
    /// Limit reached; carries the limit.
    LimitReached(u32),
}

/// Decide whether `name` (already validated/lowercase) may be saved.
pub fn check_set_home(cfg: &Config, entry: Option<&PlayerHomes>, name: &str, is_op: bool) -> SetHome {
    if entry.is_some_and(|candidate| candidate.homes.contains_key(name)) {
        return SetHome::Overwritten;
    }
    let count = entry.map_or(0, |candidate| candidate.homes.len());
    match effective_limit(cfg, entry.and_then(|candidate| candidate.limit), is_op) {
        Some(limit) if count >= limit as usize => SetHome::LimitReached(limit),
        _ => SetHome::Created,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CdKind {
    Home,
    Tpa,
}

/// Whole seconds left (rounded up); 0 when over.
pub fn remaining_secs(until_ms: u64, now_ms: u64) -> u64 {
    until_ms.saturating_sub(now_ms).div_ceil(1000)
}

#[derive(Debug, Default)]
pub struct Cooldowns {
    until: HashMap<(Key, CdKind), u64>,
}

impl Cooldowns {
    pub fn start(&mut self, key: Key, kind: CdKind, now_ms: u64, secs: u64) {
        if secs > 0 {
            self.until.insert((key, kind), now_ms + secs * 1000);
        }
    }

    pub fn remaining(&mut self, key: Key, kind: CdKind, now_ms: u64) -> u64 {
        match self.until.get(&(key, kind)).copied() {
            Some(u) if u > now_ms => remaining_secs(u, now_ms),
            Some(_) => {
                self.until.remove(&(key, kind));
                0
            }
            None => 0,
        }
    }
}

/// A player who moves further than this many blocks from the warmup start cancels it.
pub const MAX_MOVE: f64 = 0.5;

pub fn moved_beyond(start: (f64, f64, f64), cur: (f64, f64, f64), max: f64) -> bool {
    let d = (cur.0 - start.0).powi(2) + (cur.1 - start.1).powi(2) + (cur.2 - start.2).powi(2);
    d > max * max
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpKind {
    Home,
    Warp,
    Back,
    Tpa,
    Spawn,
    Top,
    Tphere,
    Tpall,
    Bottom,
    Tppos,
    Tpr,
    World,
    Tpoffline,
    Jump,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Warmup {
    pub start: (f64, f64, f64),
    pub entity_id: i32,
    pub due_ms: u64,
    pub dest: Location,
    pub label: String,
    pub kind: TpKind,
    /// Last whole-seconds-left value shown to the player.
    pub shown: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Warmups {
    map: HashMap<Key, Warmup>,
}

impl Warmups {
    /// Start a warmup; returns true if it replaced (cancelled) an earlier one.
    pub fn begin(&mut self, key: Key, w: Warmup) -> bool {
        self.map.insert(key, w).is_some()
    }
    pub fn cancel(&mut self, key: Key) -> Option<Warmup> {
        self.map.remove(&key)
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    /// Player moved to `to`; cancels (and returns) the warmup if too far.
    pub fn on_move(&mut self, key: Key, to: (f64, f64, f64)) -> Option<Warmup> {
        let far = self.map.get(&key).is_some_and(|w| moved_beyond(w.start, to, MAX_MOVE));
        if far { self.map.remove(&key) } else { None }
    }
    /// Entity took damage; cancels the warmup of the player owning that entity id.
    pub fn on_damage(&mut self, entity_id: i32) -> Option<(Key, Warmup)> {
        let key = self.map.iter().find(|(_, w)| w.entity_id == entity_id).map(|(k, _)| *k)?;
        self.map.remove(&key).map(|w| (key, w))
    }
    /// Remove and return warmups whose time is up.
    pub fn take_due(&mut self, now_ms: u64) -> Vec<(Key, Warmup)> {
        let keys: Vec<Key> = self.map.iter().filter(|(_, w)| w.due_ms <= now_ms).map(|(k, _)| *k).collect();
        keys.into_iter().filter_map(|k| self.map.remove(&k).map(|w| (k, w))).collect()
    }
    /// Players whose displayed countdown changed; marks them as shown.
    pub fn countdown_updates(&mut self, now_ms: u64) -> Vec<(Key, u64)> {
        let mut out = vec![];
        for (k, w) in self.map.iter_mut() {
            let left = remaining_secs(w.due_ms, now_ms);
            if left > 0 && w.shown != Some(left) {
                w.shown = Some(left);
                out.push((*k, left));
            }
        }
        out
    }
}

/// Direction of the teleport once a request is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpaKind {
    /// `/tpa`: the requester moves to the target.
    ToTarget,
    /// `/tpahere`: the accepter moves to the requester.
    ToRequester,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpaRequest {
    pub requester: Key,
    pub requester_name: String,
    pub target: Key,
    pub target_name: String,
    pub expires_ms: u64,
    pub kind: TpaKind,
    seq: u64,
}

#[derive(Debug, Default)]
pub struct Tpas {
    by_requester: HashMap<Key, TpaRequest>,
    next_seq: u64,
}

impl Tpas {
    /// Add a `/tpa` request; replaces any earlier pending request from the same requester.
    pub fn send(&mut self, requester: Key, requester_name: &str, target: Key, target_name: &str, expires_ms: u64) {
        self.send_inner(requester, requester_name, target, target_name, expires_ms, TpaKind::ToTarget);
    }

    /// Add a `/tpahere` request: on accept the accepter moves to the requester.
    pub fn send_here(&mut self, requester: Key, requester_name: &str, target: Key, target_name: &str, expires_ms: u64) {
        self.send_inner(requester, requester_name, target, target_name, expires_ms, TpaKind::ToRequester);
    }

    fn send_inner(
        &mut self,
        requester: Key,
        requester_name: &str,
        target: Key,
        target_name: &str,
        expires_ms: u64,
        kind: TpaKind,
    ) {
        self.next_seq += 1;
        self.by_requester.insert(
            requester,
            TpaRequest {
                requester,
                requester_name: requester_name.to_string(),
                target,
                target_name: target_name.to_string(),
                expires_ms,
                kind,
                seq: self.next_seq,
            },
        );
    }

    pub fn pending_from(&self, requester: Key, now_ms: u64) -> Option<&TpaRequest> {
        self.by_requester.get(&requester).filter(|r| r.expires_ms > now_ms)
    }

    /// Take (accept/deny) a live request for `target`: the named one, or the latest.
    pub fn take(&mut self, target: Key, from: Option<&str>, now_ms: u64) -> Option<TpaRequest> {
        let k = self
            .by_requester
            .values()
            .filter(|r| r.target == target && r.expires_ms > now_ms)
            .filter(|r| from.is_none_or(|n| r.requester_name.eq_ignore_ascii_case(n)))
            .max_by_key(|r| r.seq)
            .map(|r| r.requester)?;
        self.by_requester.remove(&k)
    }

    /// Remove and return expired requests.
    pub fn expire(&mut self, now_ms: u64) -> Vec<TpaRequest> {
        let keys: Vec<Key> = self.by_requester.values().filter(|r| r.expires_ms <= now_ms).map(|r| r.requester).collect();
        keys.into_iter().filter_map(|k| self.by_requester.remove(&k)).collect()
    }

    /// Drop every request to or from `key`; returns them.
    pub fn remove_player(&mut self, key: Key) -> Vec<TpaRequest> {
        let keys: Vec<Key> =
            self.by_requester.values().filter(|r| r.requester == key || r.target == key).map(|r| r.requester).collect();
        keys.into_iter().filter_map(|k| self.by_requester.remove(&k)).collect()
    }

    /// Cancel live requests sent by `requester`; `target` narrows it to one destination.
    pub fn cancel_from(&mut self, requester: Key, target: Option<&str>, now_ms: u64) -> usize {
        let keys: Vec<Key> = self
            .by_requester
            .values()
            .filter(|r| r.requester == requester && r.expires_ms > now_ms)
            .filter(|r| target.is_none_or(|n| r.target_name.eq_ignore_ascii_case(n)))
            .map(|r| r.requester)
            .collect();
        let mut count = 0;
        for key in keys {
            if self.by_requester.remove(&key).is_some() {
                count += 1;
            }
        }
        count
    }

    pub fn len(&self) -> usize {
        self.by_requester.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_requester.is_empty()
    }
}

/// In-memory only. Each record overwrites the previous one, so the newest of
/// "before a plugin teleport" and "death" always wins.
#[derive(Debug, Default)]
pub struct Backs {
    map: HashMap<Key, Location>,
}

impl Backs {
    pub fn record(&mut self, key: Key, loc: Location) {
        self.map.insert(key, loc);
    }
    pub fn get(&self, key: Key) -> Option<&Location> {
        self.map.get(&key)
    }
    pub fn remove(&mut self, key: Key) {
        self.map.remove(&key);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameHome {
    Missing,
    Exists,
    Same,
}

/// Rename a home without changing its location. Names must already be validated.
pub fn rename_home(entry: &mut PlayerHomes, old: &str, new: &str) -> Result<(), RenameHome> {
    if old == new {
        return Err(RenameHome::Same);
    }
    if entry.homes.contains_key(new) {
        return Err(RenameHome::Exists);
    }
    let loc = entry.homes.remove(old).ok_or(RenameHome::Missing)?;
    entry.homes.insert(new.to_string(), loc);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(w: &str, x: f64) -> Location {
        Location { world: w.into(), x, y: 64.0, z: 0.0, yaw: 0.0, pitch: 0.0 }
    }

    fn entry(n: usize, limit: Option<u32>) -> PlayerHomes {
        let mut e = PlayerHomes { limit, ..Default::default() };
        for i in 0..n {
            e.homes.insert(format!("h{i}"), loc("world", 0.0));
        }
        e
    }

    #[test]
    fn uuid_format() {
        assert_eq!(uuid_string(1), "00000000-0000-0000-0000-000000000001");
        assert_eq!(uuid_string(u128::MAX), "ffffffff-ffff-ffff-ffff-ffffffffffff");
    }

    #[test]
    fn names_valid() {
        assert_eq!(validate_name("Base_1-a").unwrap(), "base_1-a");
        assert!(validate_name(&"a".repeat(32)).is_ok());
    }

    #[test]
    fn names_invalid() {
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name(&"a".repeat(33)), Err(NameError::TooLong));
        for bad in ["a b", "a/b", "héllo", "a.b", "§x", "a:b"] {
            assert_eq!(validate_name(bad), Err(NameError::BadChar), "{bad}");
        }
    }

    #[test]
    fn limit_default_and_override() {
        let cfg = Config::default();
        assert_eq!(check_set_home(&cfg, Some(&entry(2, None)), "new", false), SetHome::Created);
        assert_eq!(check_set_home(&cfg, Some(&entry(3, None)), "new", false), SetHome::LimitReached(3));
        assert_eq!(check_set_home(&cfg, Some(&entry(3, Some(5))), "new", false), SetHome::Created);
        assert_eq!(check_set_home(&cfg, Some(&entry(5, Some(5))), "new", false), SetHome::LimitReached(5));
        assert_eq!(check_set_home(&cfg, Some(&entry(1, Some(0))), "new", false), SetHome::LimitReached(0));
        assert_eq!(check_set_home(&cfg, None, "new", false), SetHome::Created);
    }

    #[test]
    fn overwrite_does_not_count() {
        let cfg = Config::default();
        assert_eq!(check_set_home(&cfg, Some(&entry(3, None)), "h1", false), SetHome::Overwritten);
    }

    #[test]
    fn ops_not_exempt_unless_configured() {
        let mut cfg = Config::default();
        assert_eq!(check_set_home(&cfg, Some(&entry(3, None)), "n", true), SetHome::LimitReached(3));
        cfg.ops_bypass_home_limit = true;
        assert_eq!(check_set_home(&cfg, Some(&entry(99, None)), "n", true), SetHome::Created);
        assert_eq!(effective_limit(&cfg, None, false), Some(3));
    }

    #[test]
    fn cooldown_math() {
        assert_eq!(remaining_secs(10_000, 0), 10);
        assert_eq!(remaining_secs(10_001, 0), 11);
        assert_eq!(remaining_secs(10_000, 9_001), 1);
        assert_eq!(remaining_secs(10_000, 10_000), 0);
        assert_eq!(remaining_secs(10_000, 20_000), 0);
        let mut c = Cooldowns::default();
        assert_eq!(c.remaining(1, CdKind::Home, 0), 0);
        c.start(1, CdKind::Home, 1_000, 30);
        assert_eq!(c.remaining(1, CdKind::Home, 1_000), 30);
        assert_eq!(c.remaining(1, CdKind::Home, 11_500), 20);
        assert_eq!(c.remaining(1, CdKind::Tpa, 1_000), 0, "kinds independent");
        assert_eq!(c.remaining(2, CdKind::Home, 1_000), 0, "players independent");
        assert_eq!(c.remaining(1, CdKind::Home, 31_000), 0);
    }

    #[test]
    fn cooldown_zero_secs_is_noop() {
        let mut c = Cooldowns::default();
        c.start(1, CdKind::Home, 0, 0);
        assert_eq!(c.remaining(1, CdKind::Home, 0), 0);
    }

    fn warm(due: u64, eid: i32) -> Warmup {
        Warmup {
            start: (0.0, 64.0, 0.0),
            entity_id: eid,
            due_ms: due,
            dest: loc("world", 5.0),
            label: "x".into(),
            kind: TpKind::Home,
            shown: None,
        }
    }

    #[test]
    fn warmup_cancel_distance() {
        assert!(!moved_beyond((0.0, 0.0, 0.0), (0.5, 0.0, 0.0), MAX_MOVE));
        assert!(moved_beyond((0.0, 0.0, 0.0), (0.51, 0.0, 0.0), MAX_MOVE));
        assert!(moved_beyond((0.0, 0.0, 0.0), (0.0, 0.0, -0.6), MAX_MOVE));
        assert!(!moved_beyond((0.0, 0.0, 0.0), (0.2, 0.2, 0.2), MAX_MOVE));
        assert!(moved_beyond((0.0, 0.0, 0.0), (0.3, 0.3, 0.3), MAX_MOVE));
    }

    #[test]
    fn warmup_move_cancels() {
        let mut w = Warmups::default();
        w.begin(1, warm(3000, 7));
        assert!(w.on_move(1, (0.2, 64.0, 0.0)).is_none());
        assert_eq!(w.len(), 1);
        assert!(w.on_move(2, (99.0, 0.0, 0.0)).is_none());
        assert!(w.on_move(1, (2.0, 64.0, 0.0)).is_some());
        assert!(w.is_empty());
    }

    #[test]
    fn warmup_damage_replace_due() {
        let mut w = Warmups::default();
        assert!(!w.begin(1, warm(3000, 7)));
        assert!(w.begin(1, warm(5000, 7)), "new warmup replaces old");
        assert!(w.on_damage(8).is_none());
        assert_eq!(w.on_damage(7).unwrap().0, 1);
        w.begin(1, warm(3000, 7));
        w.begin(2, warm(9000, 8));
        assert!(w.take_due(2999).is_empty());
        let due = w.take_due(3000);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, 1);
        assert_eq!(w.len(), 1);
        assert!(w.cancel(2).is_some());
    }

    #[test]
    fn warmup_countdown_only_on_change() {
        let mut w = Warmups::default();
        w.begin(1, warm(3000, 7));
        assert_eq!(w.countdown_updates(0), vec![(1, 3)]);
        assert!(w.countdown_updates(100).is_empty());
        assert_eq!(w.countdown_updates(1000), vec![(1, 2)]);
        assert_eq!(w.countdown_updates(2500), vec![(1, 1)]);
        assert!(w.countdown_updates(3000).is_empty());
    }

    #[test]
    fn tpa_send_accept() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        assert!(t.pending_from(1, 0).is_some());
        let r = t.take(2, None, 1000).unwrap();
        assert_eq!((r.requester, r.target), (1, 2));
        assert!(t.is_empty());
        assert!(t.take(2, None, 1000).is_none());
    }

    #[test]
    fn tpa_one_pending_per_requester() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        t.send(1, "A", 3, "C", 60_000);
        assert_eq!(t.len(), 1);
        assert!(t.take(2, None, 0).is_none());
        assert!(t.take(3, None, 0).is_some());
    }

    #[test]
    fn tpa_latest_and_named() {
        let mut t = Tpas::default();
        t.send(1, "Alice", 9, "T", 60_000);
        t.send(2, "Bob", 9, "T", 60_000);
        assert_eq!(t.take(9, Some("alice"), 0).unwrap().requester, 1);
        t.send(1, "Alice", 9, "T", 60_000);
        assert_eq!(t.take(9, None, 0).unwrap().requester, 1, "latest wins");
        assert_eq!(t.take(9, None, 0).unwrap().requester, 2);
    }

    #[test]
    fn tpa_expiry() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        assert!(t.pending_from(1, 60_000).is_none());
        assert!(t.take(2, None, 60_000).is_none(), "expired cannot be accepted");
        assert!(t.expire(59_999).is_empty());
        let e = t.expire(60_000);
        assert_eq!(e.len(), 1);
        assert!(t.is_empty());
    }

    #[test]
    fn tpa_deny_removes() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        assert!(t.take(2, Some("A"), 0).is_some());
        assert!(t.pending_from(1, 0).is_none());
    }

    #[test]
    fn tpa_leave_drops_both_directions() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        t.send(3, "C", 1, "A", 60_000);
        t.send(4, "D", 5, "E", 60_000);
        let gone = t.remove_player(1);
        assert_eq!(gone.len(), 2);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn back_newest_wins() {
        let mut b = Backs::default();
        assert!(b.get(1).is_none());
        b.record(1, loc("world", 1.0)); // before teleport
        b.record(1, loc("world", 2.0)); // later: death
        assert_eq!(b.get(1).unwrap().x, 2.0);
        b.record(1, loc("world", 3.0)); // later: another teleport
        assert_eq!(b.get(1).unwrap().x, 3.0);
        assert!(b.get(2).is_none());
        b.remove(1);
        assert!(b.get(1).is_none());
    }
    #[test]
    fn tpahere_request_kind() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        t.send_here(3, "C", 4, "D", 60_000);
        assert_eq!(t.pending_from(1, 0).unwrap().kind, TpaKind::ToTarget);
        assert_eq!(t.pending_from(3, 0).unwrap().kind, TpaKind::ToRequester);
        assert_eq!(t.take(4, None, 0).unwrap().kind, TpaKind::ToRequester);
    }

    #[test]
    fn rename_home_moves_location() {
        let mut e = entry(1, None);
        e.homes.insert("base".into(), loc("world", 7.0));
        assert_eq!(rename_home(&mut e, "base", "main"), Ok(()));
        assert!(!e.homes.contains_key("base"));
        assert_eq!(e.homes["main"].x, 7.0);
        assert_eq!(rename_home(&mut e, "missing", "other"), Err(RenameHome::Missing));
        assert_eq!(rename_home(&mut e, "main", "other"), Ok(()));
        assert_eq!(rename_home(&mut e, "other", "other"), Err(RenameHome::Same));
    }
    #[test]
    fn cancel_from_requester() {
        let mut t = Tpas::default();
        t.send(1, "A", 2, "B", 60_000);
        t.send(3, "C", 4, "D", 60_000);
        assert_eq!(t.cancel_from(1, Some("B"), 0), 1);
        assert!(t.pending_from(1, 0).is_none());
        assert_eq!(t.cancel_from(1, None, 0), 0);
        assert_eq!(t.cancel_from(3, None, 0), 1);
        assert!(t.is_empty());
    }
}

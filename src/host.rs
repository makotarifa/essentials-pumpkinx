//! Global plugin state and host-facing helpers.

use crate::{
    config::Config,
    extras::Afks,
    economy::Economy,
    kits::Kits,
    state::PersistedState,
    logic::{Backs, Cooldowns, Key, Tpas, Warmups},
    store::{HomesFile, Location, WarpsFile},
};
use pumpkin_plugin_api::{Player, Server, common::NamedColor, text::TextComponent, uuid::Uuid};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

// Adapter over the raw Pumpkin WIT bindings (`host/host_traits.rs`). The
// helpers below talk to the server through the `HostPlayer`/`HostServer`
// traits so all WIT specifics live in one place; the public functions keep
// their concrete signatures so no other module needs to change.
mod host_traits;

use host_traits::{HostPlayer, HostServer};

pub struct State {
    pub cfg: Config,
    pub dir: PathBuf,
    pub homes: HomesFile,
    pub warps: WarpsFile,
    pub cooldowns: Cooldowns,
    pub warmups: Warmups,
    pub tpas: Tpas,
    pub backs: Backs,
    pub afks: Afks,
    pub persisted: PersistedState,
    pub economy: Economy,
    pub kits: Kits,
    pub last_tick_ms: Option<u64>,
    pub pending_confirm: HashMap<Key, String>,
}

impl State {
    pub fn homes_path(&self) -> PathBuf {
        self.dir.join("homes.json")
    }
    pub fn warps_path(&self) -> PathBuf {
        self.dir.join("warps.json")
    }
    pub fn state_path(&self) -> PathBuf {
        self.dir.join("state.json")
    }
    pub fn save_persisted(&self) {
        if let Err(e) = crate::state::save_state(&self.state_path(), &self.persisted) {
            tracing::error!("essentials: could not save state.json: {e}");
        }
    }
    pub fn economy_path(&self) -> PathBuf {
        self.dir.join("economy.json")
    }
    pub fn save_economy(&self) {
        if let Err(e) = crate::economy::save_economy(&self.economy_path(), &self.economy) {
            tracing::error!("essentials: could not save economy.json: {e}");
        }
    }
    pub fn kits_path(&self) -> PathBuf {
        self.dir.join("kits.json")
    }
    pub fn save_kits(&self) {
        if let Err(e) = crate::kits::save_kits(&self.kits_path(), &self.kits) {
            tracing::error!("essentials: could not save kits.json: {e}");
        }
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn install(s: State) {
    *lock() = Some(s);
}

fn lock() -> MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` against the state. Never call host functions inside `f` (re-entrancy).
pub fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    lock().as_mut().map(f)
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn key_of(p: &Player) -> Key {
    key_of_impl(p)
}

fn key_of_impl<P: HostPlayer + ?Sized>(p: &P) -> Key {
    uuid_to_key(&p.id())
}

pub fn uuid_to_key(u: &Uuid) -> Key {
    (u128::from(u.high) << 64) | u128::from(u.low)
}

pub fn key_to_uuid(k: Key) -> Uuid {
    Uuid { high: (k >> 64) as u64, low: k as u64 }
}

pub fn entity_id_of(p: &Player) -> i32 {
    entity_id_impl(p)
}

fn entity_id_impl<P: HostPlayer + ?Sized>(p: &P) -> i32 {
    p.entity_id()
}

/// The player's current location (world name, position, rotation).
pub fn location_of(p: &Player) -> Location {
    location_of_impl(p)
}

fn location_of_impl<P: HostPlayer + ?Sized>(p: &P) -> Location {
    let (x, y, z) = p.position();
    let (yaw, pitch) = p.rotation();
    Location { world: p.world_name(), x, y, z, yaw, pitch }
}

pub fn say(p: &Player, text: &str, color: NamedColor) {
    say_impl(p, text, color)
}

fn say_impl<P: HostPlayer + ?Sized>(p: &P, text: &str, color: NamedColor) {
    p.send_system_message(TextComponent::text(text).color_named(color));
}
pub fn ok(p: &Player, text: &str) {
    say(p, text, NamedColor::Green);
}
pub fn err(p: &Player, text: &str) {
    say(p, text, NamedColor::Red);
}
pub fn info(p: &Player, text: &str) {
    say(p, text, NamedColor::Yellow);
}

/// Move `p` to `loc`. Same-world uses `teleport`, cross-world uses the world-aware call.
pub fn teleport_to(server: &Server, p: &Player, loc: &Location) -> Result<(), String> {
    teleport_to_impl(server, p, loc)
}

fn teleport_to_impl<S, P>(server: &S, p: &P, loc: &Location) -> Result<(), String>
where
    S: HostServer<World = P::World>,
    P: HostPlayer + ?Sized,
{
    let Some(world) = server.world_by_name(&loc.world) else {
        return Err(format!("World '{}' no longer exists.", loc.world));
    };
    if p.world_name() == loc.world {
        p.teleport_same_world(loc.pos(), Some(loc.yaw), Some(loc.pitch), world);
    } else {
        p.teleport_cross_world(world, loc.pos(), Some(loc.yaw), Some(loc.pitch));
    }
    Ok(())
}

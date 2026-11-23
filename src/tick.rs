//! Periodic work: warmup countdowns, tpa expiry, playtime accounting and
//! deferred persistence writes.

use crate::{
    commands,
    host::{self, with_state},
    i18n,
    logic,
    store::persist::FileStore,
};
use pumpkin_plugin_api::{Server, common::NamedColor, scheduler::SchedulerExt, text::TextComponent};
use std::sync::atomic::{AtomicU32, Ordering};

const PERIOD_TICKS: u64 = 5;
static TICKS: AtomicU32 = AtomicU32::new(0);

pub fn start(ctx: &pumpkin_plugin_api::Context) {
    ctx.schedule_repeating_task(PERIOD_TICKS, PERIOD_TICKS, tick);
}

fn tick(server: Server) {
    let now = host::now_ms();
    let Some((due, counts, expired)) =
        with_state(|state| (state.warmups.take_due(now), state.warmups.countdown_updates(now), state.tpas.expire(now)))
    else {
        return;
    };

    // Playtime: add the elapsed time to every online player. This is the
    // only tick-driven mutation of the persisted state and the only writer
    // that relies on the periodic save below (commands and events save their
    // own mutations immediately), so it is also what sets the dirty flag.
    let delta = with_state(|state| {
        let d = state.last_tick_ms.map(|t| now.saturating_sub(t)).unwrap_or(0);
        state.last_tick_ms = Some(now);
        d
    })
    .unwrap_or(0);
    if delta > 0 && delta < 10_000 {
        let players = server.get_all_players();
        if !players.is_empty() {
            with_state(|state| {
                for player in &players {
                    let uuid = logic::uuid_string(host::key_of(player));
                    *state.persisted.playtime_ms.entry(uuid).or_insert(0) += delta;
                }
                state.persisted.dirty = true;
            });
        }
        // state.json keeps the old ~2s cadence, but now only writes when
        // something changed since the last write: an empty server produces
        // no writes at all.
        if TICKS.fetch_add(1, Ordering::Relaxed) % 8 == 0 {
            with_state(|state| {
                if state.persisted.dirty {
                    state.save_persisted();
                    state.persisted.dirty = false;
                }
            });
        }
    }

    // A legacy state.json/economy.json/kits.json was normalised in memory at
    // load time; write the canonical form once, now.
    with_state(|state| {
        if state.persisted.needs_migration() {
            state.save_persisted();
            state.persisted.set_needs_migration(false);
        }
    });
    with_state(|state| {
        if state.economy.needs_migration() {
            state.save_economy();
            state.economy.set_needs_migration(false);
        }
    });
    with_state(|state| {
        if state.kits.needs_migration() {
            state.save_kits();
            state.kits.set_needs_migration(false);
        }
    });

    for (key, left) in counts {
        if let Some(player) = server.get_player_by_uuid(host::key_to_uuid(key)) {
            player.show_actionbar(TextComponent::text(&i18n::trf("Teletransportando en {}s - no te muevas", "Teleporting in {}s - don't move", &[&left.to_string()])).color_named(NamedColor::Gold));
        }
    }
    for (key, w) in due {
        if let Some(player) = server.get_player_by_uuid(host::key_to_uuid(key)) {
            commands::complete_teleport(&server, &player, key, &w.dest, &w.label, w.kind);
        }
    }
    for r in expired {
        if let Some(player) = server.get_player_by_uuid(host::key_to_uuid(r.requester)) {
            host::info(&player, &i18n::trf("Tu solicitud de teletransporte a {} ha caducado.", "Your teleport request to {} expired.", &[&r.target_name]));
        }
    }
}

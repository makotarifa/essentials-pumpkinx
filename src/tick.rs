//! Periodic work: warmup countdowns, tpa expiry, playtime accounting and
//! deferred persistence writes.

use crate::{
    commands,
    host::{self, with_state},
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
        with_state(|s| (s.warmups.take_due(now), s.warmups.countdown_updates(now), s.tpas.expire(now)))
    else {
        return;
    };

    // Playtime: add the elapsed time to every online player. This is the
    // only tick-driven mutation of the persisted state and the only writer
    // that relies on the periodic save below (commands and events save their
    // own mutations immediately), so it is also what sets the dirty flag.
    let delta = with_state(|s| {
        let d = s.last_tick_ms.map(|t| now.saturating_sub(t)).unwrap_or(0);
        s.last_tick_ms = Some(now);
        d
    })
    .unwrap_or(0);
    if delta > 0 && delta < 10_000 {
        let players = server.get_all_players();
        if !players.is_empty() {
            with_state(|s| {
                for p in &players {
                    let uuid = logic::uuid_string(host::key_of(p));
                    *s.persisted.playtime_ms.entry(uuid).or_insert(0) += delta;
                }
                s.persisted.dirty = true;
            });
        }
        // state.json keeps the old ~2s cadence, but now only writes when
        // something changed since the last write: an empty server produces
        // no writes at all.
        if TICKS.fetch_add(1, Ordering::Relaxed) % 8 == 0 {
            with_state(|s| {
                if s.persisted.dirty {
                    s.save_persisted();
                    s.persisted.dirty = false;
                }
            });
        }
    }

    // A legacy state.json/economy.json/kits.json was normalised in memory at
    // load time; write the canonical form once, now.
    with_state(|s| {
        if s.persisted.needs_migration() {
            s.save_persisted();
            s.persisted.set_needs_migration(false);
        }
    });
    with_state(|s| {
        if s.economy.needs_migration() {
            s.save_economy();
            s.economy.set_needs_migration(false);
        }
    });
    with_state(|s| {
        if s.kits.needs_migration() {
            s.save_kits();
            s.kits.set_needs_migration(false);
        }
    });

    for (key, left) in counts {
        if let Some(p) = server.get_player_by_uuid(host::key_to_uuid(key)) {
            p.show_actionbar(TextComponent::text(&format!("Teleporting in {left}s - don't move")).color_named(NamedColor::Gold));
        }
    }
    for (key, w) in due {
        if let Some(p) = server.get_player_by_uuid(host::key_to_uuid(key)) {
            commands::complete_teleport(&server, &p, key, &w.dest, &w.label, w.kind);
        }
    }
    for r in expired {
        if let Some(p) = server.get_player_by_uuid(host::key_to_uuid(r.requester)) {
            host::info(&p, &format!("Your teleport request to {} expired.", r.target_name));
        }
    }
}

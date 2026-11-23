//! Command registration split by domain, plus the shared teleport glue
//! (also used by the tick task).
//!
//! `register_all` keeps its signature so `plugin.rs` is untouched; every
//! domain module exports `pub fn register(ctx: &Context) -> Result<(), String>`.

mod chat;
mod economy;
mod environment;
mod gamemode;
mod homes;
mod info;
mod items;
mod kits;
mod misc;
mod moderation;
mod movement;
mod tpa;
mod warps;

use crate::{
    host::{self, State, with_state},
    logic::{self, CdKind, Warmup, TpKind},
    store::{self, Location},
};
use pumpkin_plugin_api::{
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    Context, Player, Server,
};
use tracing::info;

pub fn register_all(ctx: &Context) -> Result<(), String> {
    // Shared by /setwarp and /delwarp (warps) and /sethomelimit (homes).
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:admin".into(),
        description: "Gestionar warps y límites de casas".into(),
        default: PermissionDefault::Op(PermissionLevel::Three),
        children: vec![],
    })?;
    chat::register(ctx)?;
    economy::register(ctx)?;
    environment::register(ctx)?;
    gamemode::register(ctx)?;
    homes::register(ctx)?;
    info::register(ctx)?;
    items::register(ctx)?;
    kits::register(ctx)?;
    misc::register(ctx)?;
    moderation::register(ctx)?;
    movement::register(ctx)?;
    tpa::register(ctx)?;
    warps::register(ctx)?;
    Ok(())
}

/// Do the teleport now: record /back, move, notify, start the /home cooldown.
pub fn complete_teleport(server: &Server, player: &Player, key: u128, dest: &Location, label: &str, kind: TpKind) {
    let before = host::location_of(player);
    match host::teleport_to(server, player, dest) {
        Ok(()) => {
            let now = host::now_ms();
            with_state(|state| {
                state.backs.record(key, before.clone());
                state.persisted.back.insert(logic::uuid_string(key), before);
                state.save_persisted();
                if kind == TpKind::Home {
                    let secs = state.cfg.cooldown_secs;
                    state.cooldowns.start(key, CdKind::Home, now, secs);
                }
            });
            player.show_actionbar(TextComponent::text(""));
            host::ok(player, &format!("Teletransportado a {label}."));
            info!("essentials: {} teleported ({kind:?}) to {label}", player.get_name());
        }
        Err(error) => host::err(player, &error),
    }
}

/// Begin a teleport: instant if warmup is 0, otherwise a cancellable countdown.
pub(crate) fn start_teleport(server: &Server, player: &Player, dest: Location, label: String, kind: TpKind) {
    let key = host::key_of(player);
    let now = host::now_ms();
    let Some(warmup) = with_state(|state| state.cfg.warmup_secs) else {
        host::err(player, "Estado no disponible.");
        return;
    };
    if warmup == 0 {
        with_state(|state| state.warmups.cancel(key));
        complete_teleport(server, player, key, &dest, &label, kind);
        return;
    }
    let w = Warmup {
        start: player.get_position(),
        entity_id: host::entity_id_of(player),
        due_ms: now + warmup * 1000,
        dest,
        label: label.clone(),
        kind,
        shown: None,
    };
    let replaced = with_state(|state| state.warmups.begin(key, w)).unwrap_or(false);
    if replaced {
        host::info(player, "Teletransporte anterior cancelado.");
    }
    host::info(player, &format!("Teletransportando a {label} en {warmup}s. No te muevas."));
}

/// Persist config.json atomically after a command changes a setting.
pub(crate) fn save_cfg(state: &State) -> Result<(), String> {
    store::save_json(&state.dir.join("config.json"), &state.cfg).map_err(|e| format!("No se pudo guardar config.json: {e}"))
}


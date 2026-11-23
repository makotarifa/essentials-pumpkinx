//! Event handlers: warmups, AFK, moderation enforcement and session persistence.

use crate::{
    extras::AfkInfo,
    host::{self, with_state},
    logic::uuid_string,
};
use pumpkin_plugin_api::{
    common::NamedColor,
    events::{self as ev, EventHandler, EventPriority},
    events_wit as d,
    server::CommandSender as ServerCommandSender,
    player::{BedrockDisconnectReason, BedrockKickOptions, JavaKickOptions, SocketTeardownPolicy},
    text::TextComponent,
    events_wit::InteractAction,
    Context, Player, Result, Server,
};
use tracing::info;

fn cancelled_msg(server: &Server, key: u128, why: &str) {
    if let Some(p) = server.get_player_by_uuid(host::key_to_uuid(key)) {
        host::err(&p, why);
    }
}

/// Update the tab-list name from the stored nickname and AFK state.
pub fn apply_display_name(p: &Player, nick: Option<&str>, afk: bool) {
    let real = p.get_name();
    let label = nick.unwrap_or(&real);
    if afk {
        p.set_tab_list_name(Some(TextComponent::text(&format!("[AFK] {label}")).color_named(NamedColor::Gray)));
    } else if nick.is_some() {
        p.set_tab_list_name(Some(TextComponent::text(label)));
    } else {
        p.set_tab_list_name(None);
    }
}

fn current_nick(key: u128) -> Option<String> {
    with_state(|s| s.persisted.nicks.get(&uuid_string(key)).cloned()).flatten()
}

pub(crate) fn kick_player(p: &Player, reason: &str) {
    if let Some(java) = p.as_java() {
        java.kick(JavaKickOptions {
            reason: TextComponent::text(reason),
            log_to_console: true,
            teardown_policy: SocketTeardownPolicy::Graceful,
        });
    } else if let Some(bedrock) = p.as_bedrock() {
        bedrock.kick(&BedrockKickOptions {
            reason: BedrockDisconnectReason::Kicked,
            message: reason.to_string(),
            skip_message: false,
            filtered_message: String::new(),
            log_to_console: true,
            teardown_policy: SocketTeardownPolicy::Graceful,
        });
    } else {
        p.send_system_message(TextComponent::text(reason), false);
    }
}

struct Move;
impl EventHandler<ev::PlayerMoveEvent> for Move {
    fn handle(&self, server: Server, data: d::PlayerMoveEventData) -> d::PlayerMoveEventData {
        if !data.cancelled {
            let key = host::key_of(&data.player);
            let name = data.player.get_name();
            let (cancelled, afk) = with_state(|s| {
                let cancelled = s.warmups.on_move(key, data.to_position);
                let afk = if s.cfg.afk_auto_unafk_on_move { s.afks.on_move(key, data.to_position) } else { None };
                if afk.is_some() {
                    s.persisted.afk.remove(&uuid_string(key));
                    s.save_persisted();
                }
                (cancelled, afk.is_some())
            })
            .unwrap_or((None, false));
            if cancelled.is_some() {
                cancelled_msg(&server, key, "Teleport cancelled: you moved.");
            }
            if afk {
                apply_display_name(&data.player, current_nick(key).as_deref(), false);
                if with_state(|s| s.cfg.afk_broadcast).unwrap_or(false) {
                    server.broadcast(&format!("* {name} is no longer AFK."));
                }
            }
        }
        data
    }
}

struct Damage;
impl EventHandler<ev::EntityDamageEvent> for Damage {
    fn handle(&self, server: Server, data: d::EntityDamageEventData) -> d::EntityDamageEventData {
        if !data.cancelled {
            if let Some((key, _)) = with_state(|s| s.warmups.on_damage(data.entity_id)).flatten() {
                cancelled_msg(&server, key, "Teleport cancelled: you took damage.");
            }
        }
        data
    }
}

struct Death;
impl EventHandler<ev::PlayerDeathEvent> for Death {
    fn handle(&self, _s: Server, data: d::PlayerDeathEventData) -> d::PlayerDeathEventData {
        let key = host::key_of(&data.player);
        let loc = host::location_of(&data.player);
        with_state(|s| {
            s.warmups.cancel(key);
            s.afks.remove(key);
            s.backs.record(key, loc.clone());
            s.persisted.back.insert(uuid_string(key), loc);
            s.save_persisted();
        });
        apply_display_name(&data.player, current_nick(key).as_deref(), false);
        data
    }
}

struct Chat;
impl EventHandler<ev::PlayerChatEvent> for Chat {
    fn handle(&self, server: Server, mut data: d::PlayerChatEventData) -> d::PlayerChatEventData {
        if data.cancelled {
            return data;
        }
        let key = host::key_of(&data.player);
        let uuid = uuid_string(key);
        let now = host::now_ms();
        let (muted, jailed) = with_state(|s| {
            let mute = s.persisted.mutes.get(&uuid).filter(|r| r.until_ms == 0 || r.until_ms > now);
            let jail = s.persisted.jailed.get(&uuid).filter(|r| r.until_ms == 0 || r.until_ms > now);
            (mute.map(|r| r.reason.clone()), jail.is_some())
        })
        .unwrap_or((None, false));
        if let Some(reason) = muted {
            let why = if reason.is_empty() { "Estás silenciado.".into() } else { format!("Estás silenciado: {reason}") };
            data.player.send_system_message(TextComponent::text(&why), false);
            data.cancelled = true;
            return data;
        }
        if jailed {
            data.player.send_system_message(TextComponent::text("Estás encarcelado; no puedes hablar."), false);
            data.cancelled = true;
            return data;
        }
        let name = data.player.get_name();
        let cleared = with_state(|s| {
            let cleared = s.afks.remove(key).is_some();
            if cleared {
                s.persisted.afk.remove(&uuid);
                s.save_persisted();
            }
            cleared
        })
        .unwrap_or(false);
        if cleared {
            apply_display_name(&data.player, current_nick(key).as_deref(), false);
            if with_state(|s| s.cfg.afk_broadcast).unwrap_or(false) {
                server.broadcast(&format!("* {name} is no longer AFK."));
            }
        }
        data
    }
}

struct Join;
impl EventHandler<ev::PlayerJoinEvent> for Join {
    fn handle(&self, server: Server, data: d::PlayerJoinEventData) -> d::PlayerJoinEventData {
        let key = host::key_of(&data.player);
        let uuid = uuid_string(key);
        let name = data.player.get_name();
        let pos = data.player.get_position();
        let snapshot = with_state(|s| {
            if let Some(e) = s.homes.players.get_mut(&uuid) {
                if e.name_last_seen != name {
                    e.name_last_seen = name.clone();
                    if let Err(e) = crate::store::save_json(&s.homes_path(), &s.homes) {
                        tracing::error!("essentials: could not save homes.json: {e}");
                    }
                }
            }
            s.persisted.names.insert(uuid.clone(), name.clone());
            let ban = s.persisted.bans.get(&uuid).filter(|r| r.until_ms == 0 || r.until_ms > host::now_ms()).cloned();
            let back = s.persisted.back.get(&uuid).cloned();
            let fly = if s.cfg.restore_fly_on_join { s.persisted.fly.get(&uuid).cloned() } else { None };
            let afk = if s.cfg.restore_afk_on_join { s.persisted.afk.get(&uuid).cloned().filter(|a| a.active) } else { None };
            let nick = s.persisted.nicks.get(&uuid).cloned();
            let vanish = s.persisted.vanished.get(&uuid).copied().unwrap_or(false);
            let god = s.persisted.god.get(&uuid).copied().unwrap_or(false);
            let jail = s.persisted.jailed.get(&uuid).filter(|r| r.until_ms == 0 || r.until_ms > host::now_ms()).cloned();
            let jail_loc = jail.as_ref().and_then(|j| s.cfg.jails.get(&j.jail).cloned());
            if let Some(a) = &afk {
                s.afks.set(key, AfkInfo { reason: a.reason.clone(), since_ms: a.since_ms, pos });
            }
            s.save_persisted();
            (ban, back, fly, afk.is_some(), nick, vanish, god, jail_loc)
        })
        .unwrap_or((None, None, None, false, None, false, false, None));

        let (ban, back, fly, afk, nick, vanish, god, jail_loc) = snapshot;
        if let Some(record) = ban {
            let why = if record.reason.is_empty() { "Baneado temporalmente.".into() } else { format!("Baneado: {}", record.reason) };
            kick_player(&data.player, &why);
            return data;
        }
        if let Some(loc) = back {
            with_state(|s| s.backs.record(key, loc));
        }
        if let Some(f) = fly {
            if f.fly_speed > 0.0 {
                data.player.set_fly_speed(f.fly_speed);
            }
            if f.allow_flying {
                data.player.set_allow_flight(true);
            }
            if f.flying {
                data.player.set_flying(true);
            }
        }
        apply_display_name(&data.player, nick.as_deref(), afk);
        if afk {
            host::info(&data.player, "Your AFK status was restored.");
        }
        if god {
            data.player.set_invulnerable(true);
        }
        if vanish {
            for other in server.get_all_players() {
                if host::key_of(&other) != key {
                    let Some(handle) = server.get_player_by_uuid(host::key_to_uuid(key)) else { continue };
                    other.hide_player(handle);
                }
            }
        }
        if let Some(loc) = jail_loc {
            if let Err(e) = host::teleport_to(&server, &data.player, &loc) {
                host::err(&data.player, &e);
            }
        }
        data
    }
}

struct Leave;
impl EventHandler<ev::PlayerLeaveEvent> for Leave {
    fn handle(&self, server: Server, data: d::PlayerLeaveEventData) -> d::PlayerLeaveEventData {
        let key = host::key_of(&data.player);
        let uuid = uuid_string(key);
        let name = data.player.get_name();
        let loc = host::location_of(&data.player);
        let now = host::now_ms();
        let dropped = with_state(|s| {
            s.warmups.cancel(key);
            s.backs.remove(key);
            s.persisted.names.insert(uuid.clone(), name);
            s.persisted.last_seen_ms.insert(uuid.clone(), now);
            s.persisted.last_location.insert(uuid, loc);
            s.save_persisted();
            s.tpas.remove_player(key)
        })
        .unwrap_or_default();
        let me = data.player.get_name();
        for r in dropped {
            let (other, msg) = if r.requester == key {
                (r.target, format!("{me} left; their teleport request was cancelled."))
            } else {
                (r.requester, format!("{me} left; your teleport request was cancelled."))
            };
            if let Some(p) = server.get_player_by_uuid(host::key_to_uuid(other)) {
                host::info(&p, &msg);
            }
        }
        info!("essentials: {me} left, cleared pending teleports");
        data
    }
}

struct Interact;
impl EventHandler<ev::PlayerInteractEvent> for Interact {
    fn handle(&self, server: Server, data: d::PlayerInteractEventData) -> d::PlayerInteractEventData {
        if !data.cancelled && matches!(data.action, InteractAction::RightClickAir | InteractAction::RightClickBlock) {
            let key = host::key_of(&data.player);
            let uuid = uuid_string(key);
            let pinv = data.player.get_inventory();
            let slot = u32::from(pinv.get_selected_slot());
            let item_key = pinv.as_inventory().get_item(slot).map(|item| item.get_registry_key());
            if let Some(item_key) = item_key {
                let (enabled, commands) = with_state(|s| {
                    let enabled = s.kits.powertool_toggle.get(&uuid).copied().unwrap_or(true);
                    let commands = s.kits.powertools.get(&uuid).and_then(|map| map.get(&item_key)).cloned().unwrap_or_default();
                    (enabled, commands)
                })
                .unwrap_or((true, Vec::new()));
                if enabled {
                    for command in commands {
                        if let Some(executor) = server.get_player_by_uuid(host::key_to_uuid(key)) {
                            server.execute_command(&command, ServerCommandSender::Player(executor));
                        }
                    }
                }
            }
        }
        data
    }
}

pub fn register_all(ctx: &Context) -> Result<()> {
    ctx.register_event_handler::<ev::PlayerMoveEvent, _>(Move, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::EntityDamageEvent, _>(Damage, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::PlayerDeathEvent, _>(Death, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::PlayerChatEvent, _>(Chat, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::PlayerJoinEvent, _>(Join, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::PlayerLeaveEvent, _>(Leave, EventPriority::Normal, true)?;
    ctx.register_event_handler::<ev::PlayerInteractEvent, _>(Interact, EventPriority::Normal, true)?;
    Ok(())
}

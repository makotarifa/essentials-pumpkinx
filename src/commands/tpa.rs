//! Peticiones de teletransporte: /tpa, /tpahere, /tpaccept, /tpdeny,
//! /tptoggle, /tpaall, /tpacancel y /tpauto.

use crate::{
    cmdutil::{
        button, cooldown_err, cooldown_left, fail, greedy_arg, greedy_node, has_level, no_arg, one_arg, plain, player_of,
        reg, say, target_of, unavailable, word, CmdResult,
    },
    host::{self, with_state},
    logic::{self, CdKind, TpaKind, TpKind},
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    common::NamedColor,
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Player, Server,
};
use tracing::info;

/// Execute an accepted TPA request. `accepter` is the player who said yes.
fn perform_accept(server: &Server, accepter: &Player, req: logic::TpaRequest) -> CmdResult {
    let requester = server.get_player_by_uuid(host::key_to_uuid(req.requester));
    let Some(r) = requester else {
        return Err(fail(&format!("{} ya no está conectado.", req.requester_name)));
    };
    host::ok(accepter, &format!("Aceptaste la petición de {}.", req.requester_name));
    host::ok(&r, &format!("{} aceptó tu petición.", accepter.get_name()));
    let (mover, destination, label) = match req.kind {
        TpaKind::ToTarget => (&r, accepter, accepter.get_name()),
        TpaKind::ToRequester => (accepter, &r, r.get_name()),
    };
    super::start_teleport(server, mover, host::location_of(destination), label, TpKind::Tpa);
    Ok(1)
}

fn send_tpa_request(sender: &CommandSender, server: &Server, args: &ConsumedArgs, here: bool) -> CmdResult {
    let p = player_of(sender)?;
    let target_name = plain(args.get_value("player"));
    let target = server
        .get_player_by_name(&target_name)
        .ok_or_else(|| fail(&format!("Jugador '{target_name}' no conectado.")))?;
    let (key, tkey) = (host::key_of(&p), host::key_of(&target));
    if key == tkey {
        return Err(fail("No puedes enviarte una petición de teletransporte a ti mismo."));
    }
    let (blocked, auto) = with_state(|s| {
        let uuid = logic::uuid_string(tkey);
        (
            *s.persisted.tpa_off.get(&uuid).unwrap_or(&false),
            *s.persisted.tpa_auto.get(&uuid).unwrap_or(&false),
        )
    })
    .ok_or_else(unavailable)?;
    if blocked {
        return Err(fail("Ese jugador tiene las peticiones de teletransporte desactivadas (/tptoggle)."));
    }
    let left = cooldown_left(key, CdKind::Tpa);
    if left > 0 {
        return Err(cooldown_err(left));
    }
    let now = host::now_ms();
    let (me, them) = (p.get_name(), target.get_name());
    let expire_secs = with_state(|s| {
        let exp = now + s.cfg.tpa_expire_secs * 1000;
        if here {
            s.tpas.send_here(key, &me, tkey, &them, exp);
        } else {
            s.tpas.send(key, &me, tkey, &them, exp);
        }
        let secs = s.cfg.cooldown_secs;
        s.cooldowns.start(key, CdKind::Tpa, now, secs);
        s.cfg.tpa_expire_secs
    })
    .ok_or_else(unavailable)?;
    info!("essentials: {} /{} {them}", me, if here { "tpahere" } else { "tpa" });
    if auto {
        let accepted = with_state(|s| s.tpas.take(tkey, Some(&me), host::now_ms())).ok_or_else(unavailable)?;
        if let Some(req) = accepted {
            host::ok(&target, &format!("Petición de {me} aceptada automáticamente."));
            return perform_accept(server, &target, req);
        }
    }
    host::ok(&p, &format!("Petición enviada a {them}. Caduca en {expire_secs}s."));
    let verb = if here { "quiere que te teletransportes a él" } else { "quiere teletransportarse a ti" };
    let msg = TextComponent::text(&format!("{me} {verb}. "))
        .color_named(NamedColor::Gold)
        .add_child(button("[Aceptar]", NamedColor::Green, &format!("/tpaccept {me}"), "Aceptar la petición"))
        .add_child(TextComponent::text(" "))
        .add_child(button("[Denegar]", NamedColor::Red, &format!("/tpdeny {me}"), "Denegar la petición"));
    target.send_system_message(msg, false);
    Ok(1)
}

struct TpaCmd;
impl CommandHandler for TpaCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        send_tpa_request(&sender, &server, &args, false)
    }
}

struct TpaHereCmd;
impl CommandHandler for TpaHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        send_tpa_request(&sender, &server, &args, true)
    }
}

struct TpReply {
    accept: bool,
    has_arg: bool,
}
impl CommandHandler for TpReply {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let from = if self.has_arg { Some(plain(args.get_value("player"))) } else { None };
        let (tkey, now) = (host::key_of(&p), host::now_ms());
        let req = with_state(|s| s.tpas.take(tkey, from.as_deref(), now))
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail("No tienes peticiones de teletransporte pendientes."))?;
        let verb = if self.accept { "accept" } else { "deny" };
        info!("essentials: {} /tp{verb} {}", p.get_name(), req.requester_name);
        if !self.accept {
            let requester = server.get_player_by_uuid(host::key_to_uuid(req.requester));
            host::ok(&p, &format!("Denegaste la petición de {}.", req.requester_name));
            if let Some(r) = requester {
                host::err(&r, &format!("{} denegó tu petición de teletransporte.", p.get_name()));
            }
            return Ok(1);
        }
        perform_accept(&server, &p, req)
    }
}

struct TpToggleCmd;
impl CommandHandler for TpToggleCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let key = host::key_of(&p);
        let now_blocked = with_state(|s| {
            let uuid = logic::uuid_string(key);
            let blocked = !s.persisted.tpa_off.get(&uuid).copied().unwrap_or(false);
            s.persisted.tpa_off.insert(uuid, blocked);
            s.save_persisted();
            blocked
        })
        .ok_or_else(unavailable)?;
        host::ok(&p, if now_blocked { "Las peticiones de teletransporte están ahora bloqueadas." } else { "Las peticiones de teletransporte están ahora permitidas." });
        Ok(1)
    }
}

struct TpaAllCmd;
impl CommandHandler for TpaAllCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let source = if plain(args.get_value("player")).is_empty() {
            me
        } else {
            target_of(&sender, &server, &me, &args, CmdLevel::Two)?
        };
        let key = host::key_of(&source);
        let now = host::now_ms();
        let mut sent = 0;
        for p in server.get_all_players() {
            let tkey = host::key_of(&p);
            if tkey == key {
                continue;
            }
            let blocked = with_state(|s| *s.persisted.tpa_off.get(&logic::uuid_string(tkey)).unwrap_or(&false)).unwrap_or(false);
            if blocked {
                continue;
            }
            let name = source.get_name();
            let target_name = p.get_name();
            let expiry = with_state(|s| {
                let exp = now + s.cfg.tpa_expire_secs * 1000;
                s.tpas.send_here(key, &name, tkey, &target_name, exp);
                exp
            });
            if expiry.is_none() {
                continue;
            }
            sent += 1;
            let msg = TextComponent::text(&format!("{name} quiere que te teletransportes a él. "))
                .add_child(TextComponent::text("[Aceptar]").click_run_command(&format!("/tpaccept {name}")))
                .add_child(TextComponent::text(" "))
                .add_child(TextComponent::text("[Denegar]").click_run_command(&format!("/tpdeny {name}")));
            p.send_system_message(msg, false);
        }
        say(&sender, &format!("Peticiones enviadas a {sent} jugador(es)."));
        Ok(1)
    }
}

struct TpaCancelCmd;
impl CommandHandler for TpaCancelCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let target = if raw.trim().is_empty() { None } else { Some(raw.trim().to_string()) };
        let count = with_state(|s| s.tpas.cancel_from(host::key_of(&me), target.as_deref(), host::now_ms()))
            .ok_or_else(unavailable)?;
        if count == 0 {
            return Err(fail("No habías enviado esa petición."));
        }
        say(&sender, &format!("Canceladas {count} petición(es)."));
        Ok(1)
    }
}

struct TpAutoCmd;
impl CommandHandler for TpAutoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let as_mode = |s: &str| match s.to_ascii_lowercase().as_str() {
            "on" | "true" => Some(true),
            "off" | "false" => Some(false),
            _ => None,
        };
        let first = tokens.first().copied();
        let (target, requested) = if tokens.len() > 1 || first.and_then(as_mode).is_none() {
            let name = first.ok_or_else(|| fail("Uso: /tpauto [on|off] [jugador]"))?;
            let other = server.get_player_by_name(name).ok_or_else(|| fail("Jugador no conectado."))?;
            if host::key_of(&other) != host::key_of(&me) && !has_level(&sender, CmdLevel::Two) {
                return Err(CommandError::PermissionDenied);
            }
            (other, tokens.get(1).copied().and_then(as_mode))
        } else {
            (me, first.and_then(as_mode))
        };
        let now_on = with_state(|s| {
            let uuid = logic::uuid_string(host::key_of(&target));
            let state = requested.unwrap_or(!s.persisted.tpa_auto.get(&uuid).copied().unwrap_or(false));
            s.persisted.tpa_auto.insert(uuid, state);
            s.save_persisted();
            state
        })
        .ok_or_else(unavailable)?;
        say(
            &sender,
            &format!("Auto-aceptar peticiones para {}: {}.", target.get_name(), if now_on { "activado" } else { "desactivado" }),
        );
        Ok(1)
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    let allow = |node: &str, desc: &str| Permission {
        node: format!("pumpkin_essentials:{node}"),
        description: desc.into(),
        default: PermissionDefault::Allow,
        children: vec![],
    };
    for (n, d) in [
        ("tpa", "Usar /tpa, /tpaccept, /tpdeny"),
        ("tpahere", "Pedir a otro jugador que venga a ti"),
        ("tptoggle", "Bloquear o permitir peticiones entrantes"),
        ("tpacancel", "Cancelar peticiones de teleport enviadas"),
        ("tpauto", "Aceptar peticiones automáticamente"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:tpaall".into(),
        description: "Pedir a todos los jugadores que vengan".into(),
        default: PermissionDefault::Op(PermissionLevel::Two),
        children: vec![],
    })?;

    let tpa = "pumpkin_essentials:tpa";
    reg(ctx, one_arg(&["tpa"], "Pide teletransportarte a un jugador", "player", TpaCmd), tpa)?;
    for (label, accept) in [("tpaccept", true), ("tpdeny", false)] {
        let cmd = no_arg(&[label], "Responde a una petición de teletransporte", TpReply { accept, has_arg: false })
            .then(word("player").execute(TpReply { accept, has_arg: true }));
        reg(ctx, cmd, tpa)?;
    }
    reg(ctx, one_arg(&["tpahere"], "Pide a un jugador que se teletransporte a ti", "player", TpaHereCmd), "pumpkin_essentials:tpahere")?;
    reg(ctx, no_arg(&["tptoggle"], "Bloquea o permite peticiones de teletransporte", TpToggleCmd), "pumpkin_essentials:tptoggle")?;
    reg(
        ctx,
        Command::new(&["tpaall".to_string()], "Pedir a todos los jugadores que vengan").then(greedy_node("player").execute(TpaAllCmd)),
        "pumpkin_essentials:tpaall",
    )?;
    reg(
        ctx,
        Command::new(&["tpacancel".to_string()], "Cancelar peticiones enviadas").then(greedy_node("args").execute(TpaCancelCmd)),
        "pumpkin_essentials:tpacancel",
    )?;
    reg(
        ctx,
        Command::new(&["tpauto".to_string()], "Auto-aceptar peticiones").then(greedy_node("args").execute(TpAutoCmd)),
        "pumpkin_essentials:tpauto",
    )?;
    Ok(())
}

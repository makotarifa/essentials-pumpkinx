//! Iteración 5: chat, moderación y utilidades de staff (EssentialsX adaptado).

use crate::{
    events,
    extras,
    state::{MailMessage, ModRecord},
    host::{self, with_state},
    logic::{self, TpKind},
    state::JailRecord,
    store::Location,
};
use pumpkin_plugin_api::{
    command::{Arg, ArgumentType, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    server::CommandSender as ServerCommandSender,
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Player, Server,
};
use std::{fs, path::PathBuf};
use tracing::info;
use pumpkin_plugin_api::world::ExplosionInteraction;

type CmdResult = Result<i32, CommandError>;

fn fail(msg: &str) -> CommandError {
    CommandError::CommandFailed(TextComponent::text(msg))
}

fn unavailable() -> CommandError {
    fail("Estado no disponible.")
}

fn plain(arg: Arg) -> String {
    match arg {
        Arg::Simple(s) | Arg::Msg(s) => s,
        _ => String::new(),
    }
}

fn greedy(args: &ConsumedArgs, name: &str) -> String {
    plain(args.get_value(name))
}

fn player_of(sender: &CommandSender) -> Result<Player, CommandError> {
    sender.as_player().ok_or_else(|| fail("Solo jugadores."))
}

fn say(sender: &CommandSender, msg: &str) {
    sender.send_message(TextComponent::text(msg));
}

fn word(name: &str) -> CommandNode {
    CommandNode::argument(name, &ArgumentType::String(StringType::SingleWord))
}

fn greedy_node(name: &str) -> CommandNode {
    CommandNode::argument(name, &ArgumentType::String(StringType::Greedy))
}

fn one_arg<H: CommandHandler + 'static>(names: &[&str], desc: &str, arg: &str, h: H) -> Command {
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    Command::new(&names, desc).then(word(arg).execute(h))
}

fn no_arg<H: CommandHandler + 'static>(names: &[&str], desc: &str, h: H) -> Command {
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    Command::new(&names, desc).execute(h)
}

fn reg(ctx: &Context, command: Command, permission: &str) -> Result<(), String> {
    ctx.register_command(command, permission);
    Ok(())
}

fn has_level(sender: &CommandSender, level: CmdLevel) -> bool {
    sender.has_permission_level(level)
}

fn uuid_by_name(server: &Server, name: &str) -> Option<String> {
    if let Some(p) = server.get_player_by_name(name) {
        return Some(logic::uuid_string(host::key_of(&p)));
    }
    with_state(|s| extras::find_uuid_by_name(&s.persisted, name)).flatten()
}

fn target_from_name(sender: &CommandSender, server: &Server, me: &Player, name: &str, level: CmdLevel) -> Result<Player, CommandError> {
    if !has_level(sender, level) {
        return Err(CommandError::PermissionDenied);
    }
    let target = server.get_player_by_name(name).ok_or_else(|| fail(&format!("Jugador '{name}' no conectado.")))?;
    if host::key_of(&target) == host::key_of(me) {
        return Err(fail("No puedes aplicarte esto a ti mismo."));
    }
    Ok(target)
}

fn player_by_uuid_string(server: &Server, uuid: &str) -> Option<Player> {
    let key = u128::from_str_radix(&uuid.replace('-', ""), 16).ok()?;
    server.get_player_by_uuid(host::key_to_uuid(key))
}

fn parse_mode_arg(raw: Option<&str>, current: bool) -> bool {
    match raw.map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "on" || s == "true" => true,
        Some(s) if s == "off" || s == "false" => false,
        _ => !current,
    }
}

// ---- chat y mensajería ----------------------------------------------------------

struct BroadcastWorldCmd;
impl CommandHandler for BroadcastWorldCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(fail("Uso: /broadcastworld [mundo] <mensaje>"));
        }
        let player = sender.as_player();
        let (world_name, body) = if tokens.len() > 1 && server.has_world(tokens[0]) {
            (tokens[0].to_string(), tokens[1..].join(" "))
        } else {
            let Some(p) = player else { return Err(fail("Indica el mundo: /broadcastworld <mundo> <mensaje>")); };
            (p.get_world().get_name(), tokens.join(" "))
        };
        let Some(world) = server.get_world_by_name(&world_name) else {
            return Err(fail(&format!("Mundo '{world_name}' no existe.")));
        };
        let from = sender.get_name();
        for p in server.get_players_in_world(world) {
            p.send_system_message(TextComponent::text(&format!("[Mundo {world_name}] <{from}> {body}")), false);
        }
        Ok(1)
    }
}

struct MsgToggleCmd;
impl CommandHandler for MsgToggleCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 || (tokens.first().is_some() && !matches!(tokens[0].to_ascii_lowercase().as_str(), "on" | "off")) {
            let other = target_from_name(&sender, &server, &me, tokens[0], CmdLevel::Two)?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let now_off = with_state(|s| {
            let current = s.persisted.msg_off.get(&uuid).copied().unwrap_or(false);
            let next = parse_mode_arg(mode_token, current);
            s.persisted.msg_off.insert(uuid, next);
            s.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Mensajes privados para {label}: {}.", if now_off { "desactivados" } else { "activados" }));
        Ok(1)
    }
}

struct SocialSpyCmd;
impl CommandHandler for SocialSpyCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|s| {
            let current = s.persisted.social_spy.get(&uuid).copied().unwrap_or(false);
            let next = parse_mode_arg(mode_token, current);
            s.persisted.social_spy.insert(uuid, next);
            s.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("SocialSpy de {label}: {}.", if on { "activado" } else { "desactivado" }));
        Ok(1)
    }
}

struct IgnoreCmd;
impl CommandHandler for IgnoreCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let uuid = logic::uuid_string(host::key_of(&me));
        let name = greedy(&args, "args").trim().to_string();
        if name.is_empty() {
            let list = with_state(|s| {
                let ids = s.persisted.ignored.get(&uuid).cloned().unwrap_or_default();
                ids.iter().map(|id| s.persisted.names.get(id).cloned().unwrap_or_else(|| id.clone())).collect::<Vec<_>>()
            })
            .ok_or_else(unavailable)?;
            say(&sender, &format!("Ignorados: {}", if list.is_empty() { "nadie".into() } else { list.join(", ") }));
            return Ok(1);
        }
        let target_uuid = uuid_by_name(&server, &name).ok_or_else(|| fail("Jugador desconocido."))?;
        if target_uuid == uuid {
            return Err(fail("No puedes ignorarte a ti mismo."));
        }
        let now_ignored = with_state(|s| {
            let list = s.persisted.ignored.entry(uuid.clone()).or_default();
            if let Some(pos) = list.iter().position(|id| id == &target_uuid) {
                list.remove(pos);
                false
            } else {
                list.push(target_uuid);
                true
            }
        })
        .ok_or_else(unavailable)?;
        with_state(|s| s.save_persisted());
        say(&sender, &format!("{name} {}.", if now_ignored { "ignorado" } else { "dejado de ignorar" }));
        Ok(1)
    }
}

struct NickCmd;
impl CommandHandler for NickCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(fail("Uso: /nick [jugador] <nick|off>"));
        }
        let (target, nick_raw) = if tokens.len() > 1 {
            (target_from_name(&sender, &server, &me, tokens[0], CmdLevel::Two)?, tokens[1..].join(" "))
        } else {
            (me, tokens[0].to_string())
        };
        let uuid = logic::uuid_string(host::key_of(&target));
        let clearing = nick_raw.eq_ignore_ascii_case("off");
        let nick = if clearing { None } else { Some(nick_raw.clone()) };
        with_state(|s| {
            if clearing {
                s.persisted.nicks.remove(&uuid);
            } else {
                s.persisted.nicks.insert(uuid, nick_raw.clone());
            }
            s.save_persisted();
        })
        .ok_or_else(unavailable)?;
        let afk = with_state(|s| s.afks.is_afk(host::key_of(&target))).unwrap_or(false);
        events::apply_display_name(&target, nick.as_deref(), afk);
        say(&sender, &format!("Apodo de {}: {}.", target.get_name(), if clearing { "eliminado".into() } else { nick_raw }));
        Ok(1)
    }
}

struct RealNameCmd;
impl CommandHandler for RealNameCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let lookup = greedy(&args, "args").trim().to_ascii_lowercase();
        if lookup.is_empty() {
            return Err(fail("Uso: /realname <apodo>"));
        }
        let found = with_state(|s| {
            s.persisted.nicks.iter().find(|(_, nick)| nick.to_ascii_lowercase() == lookup).and_then(|(uuid, nick)| {
                s.persisted.names.get(uuid).map(|real| (nick.clone(), real.clone()))
            })
        })
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail("No se encontró ese apodo."))?;
        say(&sender, &format!("{} es {} en realidad.", found.0, found.1));
        Ok(1)
    }
}

struct MailCmd;
impl CommandHandler for MailCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let uuid = logic::uuid_string(host::key_of(&me));
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        match tokens.first().copied() {
            None | Some("read") => {
                let mails = with_state(|s| s.persisted.mail.get(&uuid).cloned()).ok_or_else(unavailable)?.unwrap_or_default();
                if mails.is_empty() {
                    return Err(fail("No tienes correos."));
                }
                for m in &mails {
                    say(&sender, &format!("[Correo de {}] {}", m.from, m.body));
                }
                with_state(|s| {
                    if let Some(list) = s.persisted.mail.get_mut(&uuid) {
                        for m in list.iter_mut() { m.read = true; }
                    }
                    s.save_persisted();
                });
            }
            Some("clear") => {
                with_state(|s| { s.persisted.mail.remove(&uuid); s.save_persisted(); }).ok_or_else(unavailable)?;
                say(&sender, "Correos borrados.");
            }
            Some(target) => {
                if tokens.len() < 2 {
                    return Err(fail("Uso: /mail <jugador> <mensaje>"));
                }
                let to_uuid = uuid_by_name(&server, target).ok_or_else(|| fail("Destinatario desconocido."))?;
                let body = tokens[1..].join(" ");
                with_state(|s| {
                    let msg = MailMessage {
                        from: me.get_name(), from_uuid: uuid.clone(), body: body.clone(),
                        time_ms: host::now_ms(), read: false,
                    };
                    s.persisted.mail.entry(to_uuid.clone()).or_default().push(msg);
                    // El destinatario podrá responder con /r.
                    s.persisted.reply_target.insert(to_uuid, uuid.clone());
                    s.save_persisted();
                })
                .ok_or_else(unavailable)?;
                say(&sender, &format!("Correo enviado a {target}."));
            }
        }
        Ok(1)
    }
}

struct ReplyCmd;
impl CommandHandler for ReplyCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let uuid = logic::uuid_string(host::key_of(&me));
        let body = greedy(&args, "args").trim().to_string();
        if body.is_empty() {
            return Err(fail("Uso: /r <mensaje>"));
        }
        let target_uuid = with_state(|s| s.persisted.reply_target.get(&uuid).cloned()).ok_or_else(unavailable)?
            .ok_or_else(|| fail("No tienes a quién responder."))?;
        let (ignored, blocked, spy) = with_state(|s| {
            let ignored = s.persisted.ignored.get(&target_uuid).is_some_and(|l| l.contains(&uuid));
            let blocked = s.persisted.msg_off.get(&target_uuid).copied().unwrap_or(false)
                || s.persisted.rtoggle.get(&target_uuid).copied().unwrap_or(true) == false;
            let spy: Vec<String> = s.persisted.social_spy.iter().filter(|(_, on)| **on).map(|(id, _)| id.clone()).collect();
            (ignored, blocked, spy)
        })
        .ok_or_else(unavailable)?;
        if ignored || blocked {
            return Err(fail("Ese jugador no accepta respuestas ahora."));
        }
        let target = player_by_uuid_string(&_server, &target_uuid);
        let target_name = target.as_ref().map_or_else(|| "(desconectado)".to_string(), |p| p.get_name());
        let text = format!("{} -> {}: {}", me.get_name(), target_name, body);
        if let Some(p) = &target {
            p.send_system_message(TextComponent::text(&text), false);
        }
        for id in spy {
            if let Some(p) = player_by_uuid_string(&_server, &id) {
                p.send_system_message(TextComponent::text(&format!("[SocialSpy] {text}")), false);
            }
        }
        with_state(|s| { s.persisted.reply_target.insert(target_uuid, uuid); s.save_persisted(); });
        say(&sender, &format!("Respuesta enviada: {body}"));
        Ok(1)
    }
}

struct RToggleCmd;
impl CommandHandler for RToggleCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|s| {
            let current = s.persisted.rtoggle.get(&uuid).copied().unwrap_or(true);
            let next = parse_mode_arg(mode_token, current);
            s.persisted.rtoggle.insert(uuid, next);
            s.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Respuestas de {label}: {}.", if on { "activadas" } else { "desactivadas" }));
        Ok(1)
    }
}

struct HelpOpCmd;
impl CommandHandler for HelpOpCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let body = greedy(&args, "args").trim().to_string();
        if body.is_empty() {
            return Err(fail("Uso: /helpop <mensaje>"));
        }
        let from = sender.get_name();
        let mut count = 0;
        for p in server.get_all_players() {
            if p.has_permission("pumpkin_essentials:helpop.receive") {
                p.send_system_message(TextComponent::text(&format!("[HelpOp] <{from}> {body}")), false);
                count += 1;
            }
        }
        say(&sender, &format!("Solicitud enviada a {count} miembro(s) del staff."));
        Ok(1)
    }
}

// ---- texto, info y backup --------------------------------------------------------

struct CustomTextCmd;
impl CommandHandler for CustomTextCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let key = greedy(&args, "args").trim().to_string();
        let (keys, lines) = with_state(|s| {
            (s.cfg.custom_texts.keys().cloned().collect::<Vec<_>>(), s.cfg.custom_texts.get(&key).cloned())
        })
        .ok_or_else(unavailable)?;
        if key.is_empty() {
            say(&sender, &format!("Textos: {}", if keys.is_empty() { "ninguno".into() } else { keys.join(", ") }));
            return Ok(1);
        }
        let Some(lines) = lines else { return Err(fail(&format!("Texto '{key}' no definido."))); };
        for line in lines { say(&sender, &line); }
        Ok(1)
    }
}

struct RulesCmd;
impl CommandHandler for RulesCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let rules = with_state(|s| s.cfg.rules.clone()).ok_or_else(unavailable)?;
        say(&sender, "--- Reglas ---");
        for (i, rule) in rules.iter().enumerate() { say(&sender, &format!("{}. {}", i + 1, rule)); }
        Ok(1)
    }
}

struct BackupCmd;
impl CommandHandler for BackupCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let dir = with_state(|s| s.dir.clone()).ok_or_else(unavailable)?;
        let target: PathBuf = dir.join("backups").join(host::now_ms().to_string());
        fs::create_dir_all(&target).map_err(|e| fail(&format!("No se pudo crear la carpeta de backup: {e}")))?;
        let mut copied = 0;
        for name in ["config.json", "homes.json", "warps.json", "state.json"] {
            let src = dir.join(name);
            if src.exists() {
                if fs::copy(&src, target.join(name)).is_ok() { copied += 1; }
            }
        }
        say(&sender, &format!("Backup creado ({copied} archivos) en {}", target.display()));
        Ok(1)
    }
}

struct EssentialsCmd;
impl CommandHandler for EssentialsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let sub = greedy(&args, "args").trim().to_ascii_lowercase();
        if sub.is_empty() || sub == "version" {
            say(&sender, &format!("EssentialsFusion {} — GPL-3.0 — 86 comandos — Pumpkin 0.2.2", env!("CARGO_PKG_VERSION")));
        } else {
            say(&sender, "Subcomandos disponibles: version.");
        }
        Ok(1)
    }
}

struct ExtCmd;
impl CommandHandler for ExtCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args").trim().to_string();
        let target = if raw.is_empty() { me } else { target_from_name(&sender, &server, &me, raw.trim(), CmdLevel::Two)? };
        target.as_entity().set_fire_ticks(0);
        say(&sender, &format!("Fuego apagado de {}.", target.get_name()));
        Ok(1)
    }
}

struct AntiochCmd;
impl CommandHandler for AntiochCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        if !greedy(&args, "args").trim().is_empty() {
            _server.broadcast(&format!("{} lanza la Granada de Antioquía...", p.get_name()));
        }
        let (x, y, z) = p.get_position();
        let yaw = p.get_yaw().to_radians() as f64;
        let pitch = p.get_pitch().to_radians() as f64;
        let pos = (
            x - yaw.sin() * 6.0 * pitch.cos(),
            y - pitch.sin() * 6.0,
            z + yaw.cos() * 6.0 * pitch.cos(),
        );
        let world = p.get_world();
        world.create_explosion(pos, 2.0, false, ExplosionInteraction::Tnt);
        Ok(1)
    }
}

// ---- moderación ------------------------------------------------------------------

struct MuteCmd;
impl CommandHandler for MuteCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() { return Err(fail("Uso: /mute <jugador> [duración] [razón]")); }
        let name = tokens[0];
        let uuid = uuid_by_name(&server, name).ok_or_else(|| fail("Jugador desconocido."))?;
        let secs = extras::parse_duration(tokens.get(1).copied().unwrap_or("0")).map_err(|e| fail(&e))?;
        let reason = if tokens.len() > 2 { tokens[2..].join(" ") } else { String::new() };
        let now = host::now_ms();
        let by = sender.get_name();
        with_state(|s| {
            s.persisted.mutes.insert(uuid.clone(), ModRecord { until_ms: extras::expiry_from_now(secs, now), reason: reason.clone(), by: by.clone() });
            s.save_persisted();
        })
        .ok_or_else(unavailable)?;
        if let Some(p) = server.get_player_by_name(name) {
            p.send_system_message(TextComponent::text("Has sido silenciado."), false);
        }
        say(&sender, &format!("{name} silenciado."));
        Ok(1)
    }
}

struct UnmuteCmd;
impl CommandHandler for UnmuteCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let name = greedy(&args, "args").trim().to_string();
        let uuid = uuid_by_name(&server, &name).ok_or_else(|| fail("Jugador desconocido."))?;
        with_state(|s| { s.persisted.mutes.remove(&uuid); s.save_persisted(); }).ok_or_else(unavailable)?;
        say(&sender, &format!("{name} ya no está silenciado."));
        Ok(1)
    }
}

struct TempBanCmd;
impl CommandHandler for TempBanCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 { return Err(fail("Uso: /tempban <jugador> <duración> [razón]")); }
        let name = tokens[0];
        let uuid = uuid_by_name(&server, name).ok_or_else(|| fail("Jugador desconocido."))?;
        let secs = extras::parse_duration(tokens[1]).map_err(|e| fail(&e))?;
        let reason = if tokens.len() > 2 { tokens[2..].join(" ") } else { String::new() };
        let by = sender.get_name();
        with_state(|s| {
            s.persisted.bans.insert(uuid.clone(), ModRecord { until_ms: extras::expiry_from_now(secs, host::now_ms()), reason: reason.clone(), by: by.clone() });
            s.save_persisted();
        })
        .ok_or_else(unavailable)?;
        if let Some(p) = server.get_player_by_name(name) {
            events::kick_player(&p, &format!("Baneado temporalmente: {reason}"));
        }
        say(&sender, &format!("{name} baneado."));
        Ok(1)
    }
}

struct KickAllCmd;
impl CommandHandler for KickAllCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let reason = greedy(&args, "args").trim().to_string();
        let reason = if reason.is_empty() { "Kick por administración.".into() } else { reason };
        let me_key = player_of(&sender).ok().map(|p| host::key_of(&p));
        let mut count = 0;
        for p in server.get_all_players() {
            if me_key.is_some_and(|k| k == host::key_of(&p)) { continue; }
            events::kick_player(&p, &reason);
            count += 1;
        }
        say(&sender, &format!("Expulsados {count} jugador(es)."));
        Ok(1)
    }
}

struct VanishCmd;
impl CommandHandler for VanishCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|s| {
            let current = s.persisted.vanished.get(&uuid).copied().unwrap_or(false);
            let next = parse_mode_arg(mode_token, current);
            s.persisted.vanished.insert(uuid, next);
            s.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        let target_key = server.get_player_by_name(&label).map(|p| host::key_of(&p));
        if let Some(target_key) = target_key {
            for other in server.get_all_players() {
                if host::key_of(&other) != target_key {
                    let Some(handle) = server.get_player_by_name(&label) else { continue };
                    if on { other.hide_player(handle); } else { other.show_player(handle); }
                }
            }
        }
        say(&sender, &format!("Vanish de {label}: {}.", if on { "activado" } else { "desactivado" }));
        Ok(1)
    }
}

struct GodCmd;
impl CommandHandler for GodCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|s| {
            let current = s.persisted.god.get(&uuid).copied().unwrap_or(false);
            let next = parse_mode_arg(mode_token, current);
            s.persisted.god.insert(uuid, next);
            s.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        if let Some(target) = server.get_player_by_name(&label) {
            target.set_invulnerable(on);
            if on {
                target.set_health(target.get_max_health());
                target.set_food_level(20);
            }
        }
        say(&sender, &format!("God de {label}: {}.", if on { "activado" } else { "desactivado" }));
        Ok(1)
    }
}

struct SudoCmd;
impl CommandHandler for SudoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let mut tokens = raw.splitn(2, char::is_whitespace);
        let name = tokens.next().unwrap_or("").trim();
        let command = tokens.next().unwrap_or("").trim().trim_start_matches('/').to_string();
        if name.is_empty() || command.is_empty() {
            return Err(fail("Uso: /sudo <jugador> <comando>"));
        }
        if host::key_of(&me) == server.get_player_by_name(name).map(|p| host::key_of(&p)).unwrap_or(0) {
            return Err(fail("No puedes hacerte sudo a ti mismo."));
        }
        let lower = command.to_ascii_lowercase();
        for blocked in ["sudo", "op ", "deop ", "perms", "lp ", "ban ", "tempban", "kickall", "backup"] {
            if lower.starts_with(blocked) {
                return Err(fail("Ese comando no puede ejecutarse con /sudo."));
            }
        }
        let target = server.get_player_by_name(name).ok_or_else(|| fail("Jugador no conectado."))?;
        let full = format!("/{command}");
        server.execute_command(&full, ServerCommandSender::Player(target));
        info!("essentials: sudo {} -> {}", sender.get_name(), full);
        say(&sender, &format!("Ejecutado como {name}: {full}"));
        Ok(1)
    }
}

// ---- cárcel ----------------------------------------------------------------------

struct JailInfoCmd;
impl CommandHandler for JailInfoCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let list = with_state(|s| {
            let now = host::now_ms();
            s.persisted.jailed.iter()
                .filter(|(_, r)| r.until_ms == 0 || r.until_ms > now)
                .map(|(uuid, _)| s.persisted.names.get(uuid).cloned().unwrap_or_else(|| uuid.clone()).into())
                .collect::<Vec<String>>()
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Encarcelados: {}", if list.is_empty() { "nadie".into() } else { list.join(", ") }));
        Ok(1)
    }
}

fn jail_player(server: &Server, sender: &CommandSender, args: &ConsumedArgs, release: bool) -> CmdResult {
    if !has_level(sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
    let raw = plain(args.get_value("player"));
    let uuid = uuid_by_name(server, &raw).ok_or_else(|| fail("Jugador desconocido."))?;
    let binding = plain(args.get_value("player"));
    let tokens: Vec<&str> = binding.split_whitespace().collect();
    let by = sender.get_name();
    let (jail_name, until, message) = if release {
        (String::new(), 0u64, format!("{raw} liberado."))
    } else {
        let jails = with_state(|s| s.cfg.jails.keys().cloned().collect::<Vec<_>>()).ok_or_else(unavailable)?;
        if jails.is_empty() { return Err(fail("No hay cárceles definidas (/setjail).")); }
        let name = tokens.get(1).copied().filter(|n| jails.iter().any(|j| j == n)).unwrap_or(&jails[0]);
        let secs = tokens.get(2).map(|s| extras::parse_duration(s)).transpose().map_err(|e| fail(&e))?.unwrap_or(0);
        (name.to_string(), extras::expiry_from_now(secs, host::now_ms()), format!("{raw} encarcelado en '{name}'."))
    };
    let jail_loc = if release { None } else { with_state(|s| s.cfg.jails.get(&jail_name).cloned()).ok_or_else(unavailable)? };
    with_state(|s| {
        if release {
            s.persisted.jailed.remove(&uuid);
        } else {
            s.persisted.jailed.insert(uuid.clone(), JailRecord { jail: jail_name.clone(), until_ms: until, by: by.clone() });
        }
        s.save_persisted();
    })
    .ok_or_else(unavailable)?;
    if let (Some(loc), Some(p)) = (jail_loc, server.get_player_by_name(&raw)) {
        crate::commands::start_teleport(server, &p, loc, "la cárcel".into(), TpKind::Tphere);
    }
    say(sender, &message);
    Ok(1)
}

struct JailCmd;
impl CommandHandler for JailCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        jail_player(&server, &sender, &args, false)
    }
}

struct ToggleJailCmd;
impl CommandHandler for ToggleJailCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let uuid = uuid_by_name(&server, &plain(args.get_value("player"))).ok_or_else(|| fail("Jugador desconocido."))?;
        let jailed = with_state(|s| s.persisted.jailed.contains_key(&uuid)).ok_or_else(unavailable)?;
        jail_player(&server, &sender, &args, jailed)
    }
}

struct SetJailCmd;
impl CommandHandler for SetJailCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let p = player_of(&sender)?;
        let name = plain(args.get_value("name"));
        if name.trim().is_empty() { return Err(fail("Uso: /setjail <nombre>")); }
        let loc: Location = host::location_of(&p);
        with_state(|s| { s.cfg.jails.insert(name.clone(), loc); crate::commands::save_cfg(s) })
            .ok_or_else(unavailable)?
            .map_err(|e| fail(&e))?;
        say(&sender, &format!("Cárcel '{name}' guardada."));
        Ok(1)
    }
}

struct DelJailCmd;
impl CommandHandler for DelJailCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let name = plain(args.get_value("name"));
        let removed = with_state(|s| {
            let gone = s.cfg.jails.remove(&name).is_some();
            if gone { let _ = crate::commands::save_cfg(s); }
            gone
        })
        .ok_or_else(unavailable)?;
        if !removed { return Err(fail("Esa cárcel no existe.")); }
        say(&sender, &format!("Cárcel '{name}' eliminada."));
        Ok(1)
    }
}

// ---- registro --------------------------------------------------------------------

pub fn register_all(ctx: &Context) -> Result<(), String> {
    let perm = |node: &str, desc: &str, level: PermissionLevel| Permission {
        node: format!("pumpkin_essentials:{node}"),
        description: desc.into(),
        default: if level == PermissionLevel::Zero { PermissionDefault::Allow } else { PermissionDefault::Op(level) },
        children: vec![],
    };
    let two = PermissionLevel::Two;
    let three = PermissionLevel::Three;
    let zero = PermissionLevel::Zero;
    for (n, d, l) in [
        ("msgtoggle", "Alternar mensajes privados", zero),
        ("socialspy", "Espiar mensajes privados", two),
        ("ignore", "Ignorar jugadores", zero),
        ("nick", "Cambiar apodos", zero),
        ("realname", "Ver el nombre real tras un apodo", zero),
        ("mail", "Correo interno", zero),
        ("r", "Responder mensajes", zero),
        ("rtoggle", "Alternar respuestas", zero),
        ("customtext", "Mostrar textos configurados", zero),
        ("rules", "Mostrar reglas", zero),
        ("helpop", "Pedir ayuda al staff", zero),
        ("mute", "Silenciar jugadores", two),
        ("unmute", "Quitar silencios", two),
        ("vanish", "Ocultarse", two),
        ("god", "Modo dios", two),
        ("jail", "Encarcelar", two),
        ("togglejail", "Alternar cárcel", two),
        ("jailedplayers", "Listar encarcelados", two),
        ("broadcastworld", "Anunciar en un mundo", two),
        ("ext", "Apagar fuego", two),
        ("antioch", "Granada de Antioquía", two),
        ("essentials", "Información del plugin", two),
        ("tempban", "Ban temporal", three),
        ("kickall", "Expulsar a todos", three),
        ("sudo", "Ejecutar como otro jugador", three),
        ("backup", "Copiar datos del plugin", three),
        ("setjail", "Definir cárcel", three),
        ("deljail", "Eliminar cárcel", three),
        ("helpop.receive", "Recibir HelpOp", two),
    ] {
        ctx.register_permission(&perm(n, d, l))?;
    }

    reg(ctx, greedy_cmd("broadcastworld", "Anunciar en un mundo", BroadcastWorldCmd), "pumpkin_essentials:broadcastworld")?;
    reg(ctx, greedy_cmd("msgtoggle", "Alternar mensajes", MsgToggleCmd), "pumpkin_essentials:msgtoggle")?;
    reg(ctx, greedy_cmd("socialspy", "Espiar mensajes", SocialSpyCmd), "pumpkin_essentials:socialspy")?;
    reg(ctx, greedy_cmd("ignore", "Ignorar jugadores", IgnoreCmd), "pumpkin_essentials:ignore")?;
    reg(ctx, greedy_cmd("nick", "Cambiar apodo", NickCmd), "pumpkin_essentials:nick")?;
    reg(ctx, one_arg(&["realname"], "Nombre real", "name", RealNameCmd), "pumpkin_essentials:realname")?;
    reg(ctx, greedy_cmd("mail", "Correo interno", MailCmd), "pumpkin_essentials:mail")?;
    reg(ctx, greedy_cmd("r", "Responder", ReplyCmd), "pumpkin_essentials:r")?;
    reg(ctx, greedy_cmd("rtoggle", "Alternar respuestas", RToggleCmd), "pumpkin_essentials:rtoggle")?;
    reg(ctx, greedy_cmd("helpop", "Pedir ayuda", HelpOpCmd), "pumpkin_essentials:helpop")?;
    reg(ctx, greedy_cmd("customtext", "Texto configurado", CustomTextCmd), "pumpkin_essentials:customtext")?;
    reg(ctx, no_arg(&["rules"], "Reglas del servidor", RulesCmd), "pumpkin_essentials:rules")?;
    reg(ctx, no_arg(&["backup"], "Copiar datos", BackupCmd), "pumpkin_essentials:backup")?;
    reg(ctx, greedy_cmd("essentials", "Información del plugin", EssentialsCmd), "pumpkin_essentials:essentials")?;
    reg(ctx, greedy_cmd("ext", "Apagar fuego", ExtCmd), "pumpkin_essentials:ext")?;
    reg(ctx, greedy_cmd("antioch", "Granada de Antioquía", AntiochCmd), "pumpkin_essentials:antioch")?;
    reg(ctx, greedy_cmd("mute", "Silenciar", MuteCmd), "pumpkin_essentials:mute")?;
    reg(ctx, one_arg(&["unmute"], "Quitar silencio", "player", UnmuteCmd), "pumpkin_essentials:unmute")?;
    reg(ctx, greedy_cmd("tempban", "Ban temporal", TempBanCmd), "pumpkin_essentials:tempban")?;
    reg(ctx, greedy_cmd("kickall", "Expulsar a todos", KickAllCmd), "pumpkin_essentials:kickall")?;
    reg(ctx, greedy_cmd("vanish", "Ocultarse", VanishCmd), "pumpkin_essentials:vanish")?;
    reg(ctx, greedy_cmd("god", "Modo dios", GodCmd), "pumpkin_essentials:god")?;
    reg(ctx, greedy_cmd("sudo", "Ejecutar como otro", SudoCmd), "pumpkin_essentials:sudo")?;
    reg(ctx, no_arg(&["jailedplayers"], "Listar encarcelados", JailInfoCmd), "pumpkin_essentials:jailedplayers")?;
    reg(ctx, one_arg(&["jail"], "Encarcelar", "player", JailCmd), "pumpkin_essentials:jail")?;
    reg(ctx, one_arg(&["togglejail"], "Alternar cárcel", "player", ToggleJailCmd), "pumpkin_essentials:togglejail")?;
    reg(ctx, one_arg(&["setjail"], "Definir cárcel", "name", SetJailCmd), "pumpkin_essentials:setjail")?;
    reg(ctx, one_arg(&["deljail"], "Eliminar cárcel", "name", DelJailCmd), "pumpkin_essentials:deljail")?;
    Ok(())
}

fn greedy_cmd<H: CommandHandler + 'static>(name: &str, desc: &str, handler: H) -> Command {
    Command::new(&[name.to_string()], desc).then(greedy_node("args").execute(handler))
}

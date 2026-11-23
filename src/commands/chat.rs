//! Chat y mensajería: /broadcastworld, /msgtoggle, /socialspy, /ignore,
//! /nick, /realname, /mail, /r, /rtoggle, /helpop, /customtext y /rules.

use crate::{
    cmdutil::{
        fail, greedy_arg, greedy_cmd, has_level, no_arg, one_arg, parse_mode_arg, player_of, reg, say, target_from_name,
        unavailable, uuid_by_name, CmdResult,
    },
    events,
    host::{self, with_state},
    logic,
    state::MailMessage,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::CommandHandler,
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Player, Server,
};

fn player_by_uuid_string(server: &Server, uuid: &str) -> Option<Player> {
    let key = u128::from_str_radix(&uuid.replace('-', ""), 16).ok()?;
    server.get_player_by_uuid(host::key_to_uuid(key))
}

struct BroadcastWorldCmd;
impl CommandHandler for BroadcastWorldCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy_arg(&args, "args");
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
        let raw = greedy_arg(&args, "args");
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
        let raw = greedy_arg(&args, "args");
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
        let name = greedy_arg(&args, "args").trim().to_string();
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
        let raw = greedy_arg(&args, "args");
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
        let lookup = greedy_arg(&args, "args").trim().to_ascii_lowercase();
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
        let raw = greedy_arg(&args, "args");
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
        let body = greedy_arg(&args, "args").trim().to_string();
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
            return Err(fail("Ese jugador no acepta respuestas ahora."));
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
        let raw = greedy_arg(&args, "args");
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
        let body = greedy_arg(&args, "args").trim().to_string();
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

struct CustomTextCmd;
impl CommandHandler for CustomTextCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let key = greedy_arg(&args, "args").trim().to_string();
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

pub fn register(ctx: &Context) -> Result<(), String> {
    let perm = |node: &str, desc: &str, level: PermissionLevel| Permission {
        node: format!("pumpkin_essentials:{node}"),
        description: desc.into(),
        default: if level == PermissionLevel::Zero { PermissionDefault::Allow } else { PermissionDefault::Op(level) },
        children: vec![],
    };
    let two = PermissionLevel::Two;
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
        ("broadcastworld", "Anunciar en un mundo", two),
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
    Ok(())
}

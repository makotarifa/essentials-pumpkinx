use crate::{
    cmdutil::{
        fail, greedy_arg, greedy_cmd, has_level, no_arg, one_arg, parse_mode_arg, player_of, reg, say, target_from_name,
        unavailable, uuid_by_name, CmdResult,
    },
    events,
    host::{self, with_state},
    i18n,
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
            return Err(fail(i18n::tr("Uso: /broadcastworld [mundo] <mensaje>", "Usage: /broadcastworld [world] <message>")));
        }
        let player = sender.as_player();
        let (world_name, body) = if tokens.len() > 1 && server.has_world(tokens[0]) {
            (tokens[0].to_string(), tokens[1..].join(" "))
        } else {
            let Some(player) = player else { return Err(fail(i18n::tr("Indica el mundo: /broadcastworld <mundo> <mensaje>", "Specify the world: /broadcastworld <world> <message>"))); };
            (player.get_world().get_name(), tokens.join(" "))
        };
        let Some(world) = server.get_world_by_name(&world_name) else {
            return Err(fail(&i18n::trf("Mundo '{}' no existe.", "World '{}' does not exist.", &[&world_name])));
        };
        let from = sender.get_name();
        for player in server.get_players_in_world(world) {
            player.send_system_message(TextComponent::text(&i18n::trf("[Mundo {}] <{}> {}", "[World {}] <{}> {}", &[&world_name, &from, &body])), false);
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
        let mode = if now_off { i18n::tr("desactivados", "off") } else { i18n::tr("activados", "on") };
        say(&sender, &i18n::trf("Mensajes privados para {}: {}.", "Private messages for {}: {}.", &[&label, mode]));
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
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
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
        let mode = if on { i18n::tr("activado", "on") } else { i18n::tr("desactivado", "off") };
        say(&sender, &i18n::trf("SocialSpy de {}: {}.", "SocialSpy for {}: {}.", &[&label, mode]));
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
            let names = if list.is_empty() { i18n::tr("nadie", "nobody").to_string() } else { list.join(", ") };
            say(&sender, &i18n::trf("Ignorados: {}", "Ignored: {}", &[&names]));
            return Ok(1);
        }
        let target_uuid = uuid_by_name(&server, &name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        if target_uuid == uuid {
            return Err(fail(i18n::tr("No puedes ignorarte a ti mismo.", "You cannot ignore yourself.")));
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
        with_state(|state| state.save_persisted());
        let action = if now_ignored { i18n::tr("ignorado", "ignored") } else { i18n::tr("dejado de ignorar", "no longer ignored") };
        say(&sender, &i18n::trf("{} {}.", "{} {}.", &[&name, action]));
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
            return Err(fail(i18n::tr("Uso: /nick [jugador] <nick|off>", "Usage: /nick [player] <nick|off>")));
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
        let afk = with_state(|state| state.afks.is_afk(host::key_of(&target))).unwrap_or(false);
        events::apply_display_name(&target, nick.as_deref(), afk);
        let nick_label = if clearing { i18n::tr("eliminado", "removed").to_string() } else { nick_raw };
        let target_name = target.get_name();
        say(&sender, &i18n::trf("Apodo de {}: {}.", "Nickname for {}: {}.", &[&target_name, &nick_label]));
        Ok(1)
    }
}

struct RealNameCmd;
impl CommandHandler for RealNameCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let lookup = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        if lookup.is_empty() {
            return Err(fail(i18n::tr("Uso: /realname <apodo>", "Usage: /realname <nickname>")));
        }
        let found = with_state(|s| {
            s.persisted.nicks.iter().find(|(_, nick)| nick.to_ascii_lowercase() == lookup).and_then(|(uuid, nick)| {
                s.persisted.names.get(uuid).map(|real| (nick.clone(), real.clone()))
            })
        })
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail(i18n::tr("No se encontró ese apodo.", "That nickname was not found.")))?;
        say(&sender, &i18n::trf("{} es {} en realidad.", "{} is {} in reality.", &[&found.0, &found.1]));
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
                let mails = with_state(|state| state.persisted.mail.get(&uuid).cloned()).ok_or_else(unavailable)?.unwrap_or_default();
                if mails.is_empty() {
                    return Err(fail(i18n::tr("No tienes correos.", "You have no mail.")));
                }
                for m in &mails {
                    say(&sender, &i18n::trf("[Correo de {}] {}", "[Mail from {}] {}", &[&m.from, &m.body]));
                }
                with_state(|s| {
                    if let Some(list) = s.persisted.mail.get_mut(&uuid) {
                        for m in list.iter_mut() { m.read = true; }
                    }
                    s.save_persisted();
                });
            }
            Some("clear") => {
                with_state(|state| { state.persisted.mail.remove(&uuid); state.save_persisted(); }).ok_or_else(unavailable)?;
                say(&sender, i18n::tr("Correos borrados.", "Mail deleted."));
            }
            Some(target) => {
                if tokens.len() < 2 {
                    return Err(fail(i18n::tr("Uso: /mail <jugador> <mensaje>", "Usage: /mail <player> <message>")));
                }
                let to_uuid = uuid_by_name(&server, target).ok_or_else(|| fail(i18n::tr("Destinatario desconocido.", "Unknown recipient.")))?;
                let body = tokens[1..].join(" ");
                with_state(|s| {
                    let msg = MailMessage {
                        from: me.get_name(), from_uuid: uuid.clone(), body: body.clone(),
                        time_ms: host::now_ms(), read: false,
                    };
                    s.persisted.mail.entry(to_uuid.clone()).or_default().push(msg);
                    // The recipient will be able to reply with /r.
                    s.persisted.reply_target.insert(to_uuid, uuid.clone());
                    s.save_persisted();
                })
                .ok_or_else(unavailable)?;
                say(&sender, &i18n::trf("Correo enviado a {}.", "Mail sent to {}.", &[&target]));
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
            return Err(fail(i18n::tr("Uso: /r <mensaje>", "Usage: /r <message>")));
        }
        let target_uuid = with_state(|state| state.persisted.reply_target.get(&uuid).cloned()).ok_or_else(unavailable)?
            .ok_or_else(|| fail(i18n::tr("No tienes a quién responder.", "You have no one to reply to.")))?;
        let (ignored, blocked, spy) = with_state(|s| {
            let ignored = s.persisted.ignored.get(&target_uuid).is_some_and(|l| l.contains(&uuid));
            let blocked = s.persisted.msg_off.get(&target_uuid).copied().unwrap_or(false)
                || s.persisted.rtoggle.get(&target_uuid).copied().unwrap_or(true) == false;
            let spy: Vec<String> = s.persisted.social_spy.iter().filter(|(_, on)| **on).map(|(id, _)| id.clone()).collect();
            (ignored, blocked, spy)
        })
        .ok_or_else(unavailable)?;
        if ignored || blocked {
            return Err(fail(i18n::tr("Ese jugador no acepta respuestas ahora.", "That player is not accepting replies right now.")));
        }
        let target = player_by_uuid_string(&_server, &target_uuid);
        let target_name = target.as_ref().map_or_else(|| i18n::tr("(desconectado)", "(offline)").to_string(), |player| player.get_name());
        let text = format!("{} -> {}: {}", me.get_name(), target_name, body);
        if let Some(player) = &target {
            player.send_system_message(TextComponent::text(&text), false);
        }
        for id in spy {
            if let Some(player) = player_by_uuid_string(&_server, &id) {
                player.send_system_message(TextComponent::text(&format!("[SocialSpy] {text}")), false);
            }
        }
        with_state(|state| { state.persisted.reply_target.insert(target_uuid, uuid); state.save_persisted(); });
        say(&sender, &i18n::trf("Respuesta enviada: {}", "Reply sent: {}", &[&body]));
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
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
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
        let mode = if on { i18n::tr("activadas", "enabled") } else { i18n::tr("desactivadas", "disabled") };
        say(&sender, &i18n::trf("Respuestas de {}: {}.", "Replies for {}: {}.", &[&label, mode]));
        Ok(1)
    }
}

struct HelpOpCmd;
impl CommandHandler for HelpOpCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let body = greedy_arg(&args, "args").trim().to_string();
        if body.is_empty() {
            return Err(fail(i18n::tr("Uso: /helpop <mensaje>", "Usage: /helpop <message>")));
        }
        let from = sender.get_name();
        let mut count = 0;
        for player in server.get_all_players() {
            if player.has_permission("pumpkin_essentials:helpop.receive") {
                player.send_system_message(TextComponent::text(&format!("[HelpOp] <{from}> {body}")), false);
                count += 1;
            }
        }
        let count_label = count.to_string();
        say(&sender, &i18n::trf("Solicitud enviada a {} miembro(s) del staff.", "Request sent to {} staff member(s).", &[&count_label]));
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
            let texts = if keys.is_empty() { i18n::tr("ninguno", "none").to_string() } else { keys.join(", ") };
            say(&sender, &i18n::trf("Textos: {}", "Texts: {}", &[&texts]));
            return Ok(1);
        }
        let Some(lines) = lines else { return Err(fail(&i18n::trf("Texto '{}' no definido.", "Text '{}' is not defined.", &[&key]))); };
        for line in lines { say(&sender, &line); }
        Ok(1)
    }
}

struct RulesCmd;
impl CommandHandler for RulesCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let rules = with_state(|state| state.cfg.rules.clone()).ok_or_else(unavailable)?;
        say(&sender, i18n::tr("--- Reglas ---", "--- Rules ---"));
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

    reg(ctx, greedy_cmd("broadcastworld", i18n::tr("Anunciar en un mundo", "Announce in a world"), BroadcastWorldCmd), "pumpkin_essentials:broadcastworld")?;
    reg(ctx, greedy_cmd("msgtoggle", i18n::tr("Alternar mensajes", "Toggle messages"), MsgToggleCmd), "pumpkin_essentials:msgtoggle")?;
    reg(ctx, greedy_cmd("socialspy", i18n::tr("Espiar mensajes", "Spy on messages"), SocialSpyCmd), "pumpkin_essentials:socialspy")?;
    reg(ctx, greedy_cmd("ignore", i18n::tr("Ignorar jugadores", "Ignore players"), IgnoreCmd), "pumpkin_essentials:ignore")?;
    reg(ctx, greedy_cmd("nick", i18n::tr("Cambiar apodo", "Change nickname"), NickCmd), "pumpkin_essentials:nick")?;
    reg(ctx, one_arg(&["realname"], i18n::tr("Nombre real", "Real name"), "name", RealNameCmd), "pumpkin_essentials:realname")?;
    reg(ctx, greedy_cmd("mail", i18n::tr("Correo interno", "Internal mail"), MailCmd), "pumpkin_essentials:mail")?;
    reg(ctx, greedy_cmd("r", i18n::tr("Responder", "Reply"), ReplyCmd), "pumpkin_essentials:r")?;
    reg(ctx, greedy_cmd("rtoggle", i18n::tr("Alternar respuestas", "Toggle replies"), RToggleCmd), "pumpkin_essentials:rtoggle")?;
    reg(ctx, greedy_cmd("helpop", i18n::tr("Pedir ayuda", "Request help"), HelpOpCmd), "pumpkin_essentials:helpop")?;
    reg(ctx, greedy_cmd("customtext", i18n::tr("Texto configurado", "Configured text"), CustomTextCmd), "pumpkin_essentials:customtext")?;
    reg(ctx, no_arg(&["rules"], i18n::tr("Reglas del servidor", "Server rules"), RulesCmd), "pumpkin_essentials:rules")?;
    Ok(())
}

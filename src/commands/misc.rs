use crate::{
    cmdutil::{fail, greedy_arg, greedy_cmd, greedy_node, no_arg, optional_target, player_of, reg, resolve_target, say, target_of, unavailable, word, CmdResult},
    events,
    extras::{self, AfkInfo},
    host::{self, with_state},
    logic,
    state::AfkState,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    world::ExplosionInteraction,
    Context, Server,
};
use std::{fs, path::PathBuf};
use tracing::info;

struct AfkCmd;
impl CommandHandler for AfkCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (target, reason) = if let Some(first) = tokens.first() {
            match server.get_player_by_name(first) {
                Some(other) => {
                    if host::key_of(&other) != host::key_of(&me) && !sender.has_permission_level(CmdLevel::Two) {
                        return Err(CommandError::PermissionDenied);
                    }
                    (other, tokens[1..].join(" "))
                }
                None => (me, tokens.join(" ")),
            }
        } else {
            (me, String::new())
        };
        let key = host::key_of(&target);
        let name = target.get_name();
        let pos = target.get_position();
        let now = host::now_ms();
        let uuid = logic::uuid_string(key);
        let (now_afk, broadcast, show_tab, nick) = with_state(|state| {
            let was = state.afks.remove(key).is_some();
            if was {
                state.persisted.afk.remove(&uuid);
            } else {
                state.afks.set(key, AfkInfo { reason: reason.clone(), since_ms: now, pos });
                state.persisted.afk.insert(
                    uuid.clone(),
                    AfkState { active: true, reason: reason.clone(), since_ms: now },
                );
            }
            state.save_persisted();
            let nick = state.persisted.nicks.get(&uuid).cloned();
            (!was, state.cfg.afk_broadcast, state.cfg.afk_tab_name, nick)
        })
        .ok_or_else(unavailable)?;
        if show_tab {
            events::apply_display_name(&target, nick.as_deref(), now_afk);
        }
        if now_afk {
            let suffix = if reason.is_empty() { String::new() } else { format!(": {reason}") };
            if broadcast {
                server.broadcast(&format!("* {name} está ahora AFK{suffix}."));
            }
        } else if broadcast {
            server.broadcast(&format!("* {name} ya no está AFK."));
        }
        let word = if now_afk { "AFK" } else { "no longer AFK" };
        host::ok(&target, if now_afk { "Estás ahora AFK." } else { "Ya no estás AFK." });
        info!("essentials: {} is {word}", name);
        Ok(1)
    }
}

struct HealCmd {
    target: bool,
}
impl CommandHandler for HealCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = optional_target(&sender, &server, &me, &args, self.target)?;
        let other = host::key_of(&target) != host::key_of(&me);
        let max = target.get_max_health();
        target.set_health(max);
        target.set_food_level(20);
        target.set_saturation(10.0);
        if other {
            sender.send_message(TextComponent::text(&format!("Curado {}.", target.get_name())));
        } else {
            host::ok(&target, "Has sido curado.");
        }
        info!("essentials: {} healed {}", sender.get_name(), target.get_name());
        Ok(1)
    }
}

struct FeedCmd {
    target: bool,
}
impl CommandHandler for FeedCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = optional_target(&sender, &server, &me, &args, self.target)?;
        let other = host::key_of(&target) != host::key_of(&me);
        target.set_food_level(20);
        target.set_saturation(10.0);
        target.set_exhaustion(0.0);
        if other {
            sender.send_message(TextComponent::text(&format!("Alimentado {}.", target.get_name())));
        } else {
            host::ok(&target, "Has sido alimentado.");
        }
        info!("essentials: {} fed {}", sender.get_name(), target.get_name());
        Ok(1)
    }
}

struct SpeedCmd;
impl CommandHandler for SpeedCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let first = tokens.next().ok_or_else(|| fail("Uso: /speed [jugador] <0.1-10.0>"))?;
        let rest: Vec<&str> = tokens.collect();
        let (target, speed_raw) = if extras::parse_walk_speed(first).is_ok() {
            (me, first)
        } else if rest.len() == 1 && extras::parse_walk_speed(rest[0]).is_ok() {
            (resolve_target(&sender, &server, &me, first)?, rest[0])
        } else {
            return Err(fail("Uso: /speed [jugador] <0.1-10.0>"));
        };
        let value = extras::parse_walk_speed(speed_raw).map_err(|error| fail(&error))?;
        target.set_walk_speed(extras::engine_walk_speed(value));
        host::ok(&target, &format!("Velocidad de caminata fijada en {value:.2}."));
        info!("essentials: {} speed {value:.2}", target.get_name());
        Ok(1)
    }
}

struct ExpCmd;
impl CommandHandler for ExpCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let me_key = host::key_of(&me);
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (target, level) = match tokens.as_slice() {
            [] => (me, None),
            [value] if extras::parse_level(value).is_ok() => (me, Some(extras::parse_level(value).unwrap())),
            [name] => (resolve_target(&sender, &server, &me, name)?, None),
            [name, value] => {
                let level = extras::parse_level(value).map_err(|error| fail(&error))?;
                (resolve_target(&sender, &server, &me, name)?, Some(level))
            }
            _ => return Err(fail("Uso: /exp [jugador] [nivel]")),
        };
        let name = target.get_name();
        match level {
            Some(n) => {
                target.set_experience_level(n);
                let msg = format!("Nivel de experiencia de {name} fijado en {n}.");
                if host::key_of(&target) == me_key {
                    host::ok(&target, &msg);
                } else {
                    sender.send_message(TextComponent::text(&msg));
                }
            }
            None => {
                let current = target.get_experience_level();
                let msg = if host::key_of(&target) == me_key {
                    format!("Tu nivel de experiencia es {current}.")
                } else {
                    format!("El nivel de experiencia de {name} es {current}.")
                };
                sender.send_message(TextComponent::text(&msg));
            }
        }
        Ok(1)
    }
}

struct RestCmd {
    target: bool,
}
impl CommandHandler for RestCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let player = if self.target { target_of(&sender, &server, &me, &args, CmdLevel::Two)? } else { me };
        player.set_exhaustion(0.0);
        player.set_saturation(10.0);
        host::info(&player, "Has descansado.");
        Ok(1)
    }
}

struct BackupCmd;
impl CommandHandler for BackupCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let dir = with_state(|state| state.dir.clone()).ok_or_else(unavailable)?;
        let target: PathBuf = dir.join("backups").join(host::now_ms().to_string());
        fs::create_dir_all(&target).map_err(|error| fail(&format!("No se pudo crear la carpeta de backup: {error}")))?;
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
        let sub = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        if sub.is_empty() || sub == "version" {
            say(&sender, &format!("EssentialsPumpkinX {} — GPL-3.0 — 86 comandos — Pumpkin 0.2.2", env!("CARGO_PKG_VERSION")));
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
        let raw = greedy_arg(&args, "args").trim().to_string();
        let target = if raw.is_empty() { me } else { crate::cmdutil::target_from_name(&sender, &server, &me, raw.trim(), CmdLevel::Two)? };
        target.as_entity().set_fire_ticks(0);
        say(&sender, &format!("Fuego apagado de {}.", target.get_name()));
        Ok(1)
    }
}

struct AntiochCmd;
impl CommandHandler for AntiochCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        if !greedy_arg(&args, "args").trim().is_empty() {
            _server.broadcast(&format!("{} lanza la Granada de Antioquía...", player.get_name()));
        }
        let (x, y, z) = player.get_position();
        let yaw = player.get_yaw().to_radians() as f64;
        let pitch = player.get_pitch().to_radians() as f64;
        let pos = (
            x - yaw.sin() * 6.0 * pitch.cos(),
            y - pitch.sin() * 6.0,
            z + yaw.cos() * 6.0 * pitch.cos(),
        );
        let world = player.get_world();
        world.create_explosion(pos, 2.0, false, ExplosionInteraction::Tnt);
        Ok(1)
    }
}

struct SuicideCmd;
impl CommandHandler for SuicideCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let name = player.get_name();
        player.set_health(0.0);
        _server.broadcast(&format!("{name} se ha quitado la vida."));
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
    for (n, d) in [("afk", "Usar /afk"), ("rest", "Reiniciar exhausción y saturación")] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [
        ("heal", "Usar /heal"),
        ("feed", "Usar /feed"),
        ("speed", "Usar /speed"),
        ("exp", "Usar /exp"),
        ("essentials", "Información del plugin"),
        ("ext", "Apagar fuego"),
        ("antioch", "Granada de Antioquía"),
        ("suicide", "Quitarse la vida"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:backup".into(),
        description: "Copiar datos del plugin".into(),
        default: PermissionDefault::Op(PermissionLevel::Three),
        children: vec![],
    })?;

    reg(
        ctx,
        Command::new(&["afk".to_string()], "Alterna el estado AFK, opcionalmente con motivo").then(greedy_node("args").execute(AfkCmd)),
        "pumpkin_essentials:afk",
    )?;
    reg(
        ctx,
        Command::new(&["heal".to_string()], "Cúrate a ti o a otro jugador")
            .execute(HealCmd { target: false })
            .then(word("player").execute(HealCmd { target: true })),
        "pumpkin_essentials:heal",
    )?;
    reg(
        ctx,
        Command::new(&["feed".to_string()], "Aliméntate a ti o a otro jugador")
            .execute(FeedCmd { target: false })
            .then(word("player").execute(FeedCmd { target: true })),
        "pumpkin_essentials:feed",
    )?;
    reg(
        ctx,
        Command::new(&["speed".to_string()], "Fija la velocidad de caminata 0.1-10.0").then(greedy_node("args").execute(SpeedCmd)),
        "pumpkin_essentials:speed",
    )?;
    reg(
        ctx,
        Command::new(&["exp".to_string()], "Muestra o fija un nivel de experiencia").then(greedy_node("args").execute(ExpCmd)),
        "pumpkin_essentials:exp",
    )?;
    reg(
        ctx,
        Command::new(&["rest".to_string()], "Descansar")
            .execute(RestCmd { target: false })
            .then(word("player").execute(RestCmd { target: true })),
        "pumpkin_essentials:rest",
    )?;
    reg(ctx, no_arg(&["backup"], "Copiar datos", BackupCmd), "pumpkin_essentials:backup")?;
    reg(ctx, greedy_cmd("essentials", "Información del plugin", EssentialsCmd), "pumpkin_essentials:essentials")?;
    reg(ctx, greedy_cmd("ext", "Apagar fuego", ExtCmd), "pumpkin_essentials:ext")?;
    reg(ctx, greedy_cmd("antioch", "Granada de Antioquía", AntiochCmd), "pumpkin_essentials:antioch")?;
    reg(ctx, no_arg(&["suicide"], "Quitarse la vida", SuicideCmd), "pumpkin_essentials:suicide")?;
    Ok(())
}

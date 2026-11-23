use crate::{
    i18n::{self, Lang},
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
            if broadcast {
                if reason.is_empty() {
                    server.broadcast(&i18n::trf("* {} está ahora AFK.", "* {} is now AFK.", &[&name]));
                } else {
                    server.broadcast(&i18n::trf("* {} está ahora AFK: {}.", "* {} is now AFK: {}.", &[&name, &reason]));
                }
            }
        } else if broadcast {
            server.broadcast(&i18n::trf("* {} ya no está AFK.", "* {} is no longer AFK.", &[&name]));
        }
        let word = if now_afk { "AFK" } else { "no longer AFK" };
        host::ok(
            &target,
            if now_afk { i18n::tr("Estás ahora AFK.", "You are now AFK.") } else { i18n::tr("Ya no estás AFK.", "You are no longer AFK.") },
        );
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
            let name = target.get_name();
            sender.send_message(TextComponent::text(&i18n::trf("Curado {}.", "Healed {}.", &[&name])));
        } else {
            host::ok(&target, i18n::tr("Has sido curado.", "You have been healed."));
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
            let name = target.get_name();
            sender.send_message(TextComponent::text(&i18n::trf("Alimentado {}.", "Fed {}.", &[&name])));
        } else {
            host::ok(&target, i18n::tr("Has sido alimentado.", "You have been fed."));
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
        let first = tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /speed [jugador] <0.1-10.0>", "Usage: /speed [player] <0.1-10.0>")))?;
        let rest: Vec<&str> = tokens.collect();
        let (target, speed_raw) = if extras::parse_walk_speed(first).is_ok() {
            (me, first)
        } else if rest.len() == 1 && extras::parse_walk_speed(rest[0]).is_ok() {
            (resolve_target(&sender, &server, &me, first)?, rest[0])
        } else {
            return Err(fail(i18n::tr("Uso: /speed [jugador] <0.1-10.0>", "Usage: /speed [player] <0.1-10.0>")));
        };
        let value = extras::parse_walk_speed(speed_raw).map_err(|error| fail(&error))?;
        target.set_walk_speed(extras::engine_walk_speed(value));
        let value_text = format!("{value:.2}");
        host::ok(&target, &i18n::trf("Velocidad de caminata fijada en {}.", "Walk speed set to {}.", &[&value_text]));
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
            _ => return Err(fail(i18n::tr("Uso: /exp [jugador] [nivel]", "Usage: /exp [player] [level]"))),
        };
        let name = target.get_name();
        match level {
            Some(n) => {
                target.set_experience_level(n);
                let level_text = n.to_string();
                let msg = i18n::trf("Nivel de experiencia de {} fijado en {}.", "Experience level of {} set to {}.", &[&name, &level_text]);
                if host::key_of(&target) == me_key {
                    host::ok(&target, &msg);
                } else {
                    sender.send_message(TextComponent::text(&msg));
                }
            }
            None => {
                let current = target.get_experience_level();
                let current_text = current.to_string();
                let msg = if host::key_of(&target) == me_key {
                    i18n::trf("Tu nivel de experiencia es {}.", "Your experience level is {}.", &[&current_text])
                } else {
                    i18n::trf("El nivel de experiencia de {} es {}.", "The experience level of {} is {}.", &[&name, &current_text])
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
        host::info(&player, i18n::tr("Has descansado.", "You have rested."));
        Ok(1)
    }
}

struct BackupCmd;
impl CommandHandler for BackupCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let dir = with_state(|state| state.dir.clone()).ok_or_else(unavailable)?;
        let target: PathBuf = dir.join("backups").join(host::now_ms().to_string());
        fs::create_dir_all(&target).map_err(|error| {
            let error = error.to_string();
            fail(&i18n::trf("No se pudo crear la carpeta de backup: {}", "Could not create backup folder: {}", &[&error]))
        })?;
        let mut copied = 0;
        for name in ["config.json", "homes.json", "warps.json", "state.json"] {
            let src = dir.join(name);
            if src.exists() {
                if fs::copy(&src, target.join(name)).is_ok() { copied += 1; }
            }
        }
        let count = copied.to_string();
        let path = target.display().to_string();
        say(&sender, &i18n::trf("Backup creado ({} archivos) en {}", "Backup created ({} files) at {}", &[&count, &path]));
        Ok(1)
    }
}

struct EssentialsCmd;
impl CommandHandler for EssentialsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        say(&sender, &format!("EssentialsPumpkinX {}", env!("CARGO_PKG_VERSION")));
        Ok(1)
    }
}

const HELP_GROUPS: &[(&str, &[&str])] = &[
    ("Homes", &["sethome", "home", "delhome", "homes", "listhomes", "renamehome", "sethomelimit"]),
    ("Warps", &["setwarp", "delwarp", "warp", "warps", "warpinfo"]),
    ("TPA", &["tpa", "tpaccept", "tpdeny", "tpahere", "tptoggle", "tpaall", "tpacancel", "tpauto"]),
    ("Movement", &["back", "spawn", "setspawn", "top", "tphere", "tpall", "bottom", "tppos", "tpr", "settpr", "tpo", "tpoffline", "tpohere", "world", "jump", "fly", "flyspeed"]),
    ("Gamemode", &["gm", "gma", "gmc", "gms", "gmsp"]),
    ("Info", &["ping", "playtime", "near", "seen", "whois", "info", "gc", "compass", "depth", "getpos"]),
    ("Chat", &["broadcastworld", "msgtoggle", "socialspy", "ignore", "nick", "realname", "mail", "r", "rtoggle", "helpop", "customtext", "rules"]),
    ("Moderation", &["mute", "unmute", "tempban", "kickall", "vanish", "god", "sudo", "jail", "togglejail", "jailedplayers", "setjail", "deljail", "jails"]),
    ("Economy", &["balance", "bal", "balancetop", "eco", "pay", "payconfirm", "paytoggle", "payconfirmtoggle", "sell", "worth", "setworth"]),
    ("Kits", &["kit", "createkit", "delkit", "kitreset", "showkit"]),
    ("Items", &["item", "itemdb", "itemlore", "itemname", "repair", "unlimited", "hat", "more", "book", "firework", "powertool", "powertoollist", "powertooltoggle", "condense", "invsee", "enderchest", "clearinventory", "clearinventoryconfirmtoggle", "remove", "spawnmob", "spawner", "skull", "kittycannon", "beezooka", "fireball", "potion"]),
    ("Environment", &["anvil", "cartographytable", "grindstone", "loom", "smithingtable", "workbench", "disposal", "editsign", "stonecutter", "ice", "tree", "bigtree", "break", "burn", "lightning", "thunder", "nuke", "ptime", "pweather"]),
    ("Misc", &["afk", "heal", "feed", "speed", "exp", "rest", "backup", "essentials", "ext", "antioch", "suicide"]),
];

struct EssentialsHelpCmd;
impl CommandHandler for EssentialsHelpCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let sub = greedy_arg(&args, "sub").trim().to_ascii_lowercase();
        if sub != "help" {
            say(&sender, i18n::msg(i18n::UNKNOWN_SUBCOMMAND));
            return Ok(1);
        }
        say(&sender, i18n::tr("Comandos de EssentialsPumpkinX (149):", "EssentialsPumpkinX commands (149):"));
        for (group, commands) in HELP_GROUPS {
            let mut line = format!("{group} ({}):", commands.len());
            for command in *commands {
                let piece = format!(" /{command}");
                if line.len() + piece.len() > 240 {
                    say(&sender, &line);
                    line = format!("{group} (cont.):");
                }
                line.push_str(&piece);
            }
            say(&sender, &line);
        }
        say(&sender, i18n::tr("Usa /help para ver el árbol completo de comandos del servidor.", "Use /help for the full server command tree."));
        Ok(1)
    }
}

struct EssentialsLanguageCmd;
impl CommandHandler for EssentialsLanguageCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy_arg(&args, "value").trim().to_ascii_lowercase();
        let lang = match raw.as_str() {
            "en" | "en-us" | "english" => Lang::En,
            "es" | "es-es" | "spanish" => Lang::Es,
            _ => return Err(fail(i18n::msg(i18n::LANGUAGE_USAGE))),
        };
        let saved = with_state(|state| {
            state.cfg.language = lang.as_str().to_string();
            crate::commands::save_cfg(state)
        })
        .ok_or_else(unavailable)?;
        saved.map_err(|error| fail(&error))?;
        i18n::set(lang);
        say(&sender, &i18n::msgf(i18n::LANGUAGE_SET, &[lang.as_str()]));
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
        let name = target.get_name();
        say(&sender, &i18n::trf("Fuego apagado de {}.", "Fire extinguished for {}.", &[&name]));
        Ok(1)
    }
}

struct AntiochCmd;
impl CommandHandler for AntiochCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        if !greedy_arg(&args, "args").trim().is_empty() {
            let name = player.get_name();
            _server.broadcast(&i18n::trf("{} lanza la Granada de Antioquía...", "{} throws the Antioch Grenade...", &[&name]));
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
        _server.broadcast(&i18n::trf("{} se ha quitado la vida.", "{} took their own life.", &[&name]));
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
        Command::new(&["afk".to_string()], i18n::tr("Alterna el estado AFK, opcionalmente con motivo", "Toggles AFK status, optionally with a reason")).then(greedy_node("args").execute(AfkCmd)),
        "pumpkin_essentials:afk",
    )?;
    reg(
        ctx,
        Command::new(&["heal".to_string()], i18n::tr("Cúrate a ti o a otro jugador", "Heal yourself or another player"))
            .execute(HealCmd { target: false })
            .then(word("player").execute(HealCmd { target: true })),
        "pumpkin_essentials:heal",
    )?;
    reg(
        ctx,
        Command::new(&["feed".to_string()], i18n::tr("Aliméntate a ti o a otro jugador", "Feed yourself or another player"))
            .execute(FeedCmd { target: false })
            .then(word("player").execute(FeedCmd { target: true })),
        "pumpkin_essentials:feed",
    )?;
    reg(
        ctx,
        Command::new(&["speed".to_string()], i18n::tr("Fija la velocidad de caminata 0.1-10.0", "Set walk speed 0.1-10.0")).then(greedy_node("args").execute(SpeedCmd)),
        "pumpkin_essentials:speed",
    )?;
    reg(
        ctx,
        Command::new(&["exp".to_string()], i18n::tr("Muestra o fija un nivel de experiencia", "Show or set an experience level")).then(greedy_node("args").execute(ExpCmd)),
        "pumpkin_essentials:exp",
    )?;
    reg(
        ctx,
        Command::new(&["rest".to_string()], i18n::tr("Descansar", "Rest"))
            .execute(RestCmd { target: false })
            .then(word("player").execute(RestCmd { target: true })),
        "pumpkin_essentials:rest",
    )?;
    reg(ctx, no_arg(&["backup"], i18n::tr("Copiar datos", "Copy data"), BackupCmd), "pumpkin_essentials:backup")?;
    reg(ctx, Command::new(&["essentials".to_string()], i18n::tr("Versión del plugin", "Plugin version")).execute(EssentialsCmd).then(word("sub").execute(EssentialsHelpCmd).then(word("value").execute(EssentialsLanguageCmd))), "pumpkin_essentials:essentials")?;
    reg(ctx, greedy_cmd("ext", i18n::tr("Apagar fuego", "Extinguish fire"), ExtCmd), "pumpkin_essentials:ext")?;
    reg(ctx, greedy_cmd("antioch", i18n::tr("Granada de Antioquía", "Antioch Grenade"), AntiochCmd), "pumpkin_essentials:antioch")?;
    reg(ctx, no_arg(&["suicide"], i18n::tr("Quitarse la vida", "Take your own life"), SuicideCmd), "pumpkin_essentials:suicide")?;
    Ok(())
}

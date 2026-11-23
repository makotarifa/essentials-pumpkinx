use crate::{
    cmdutil::{fail, greedy_arg, greedy_node, has_level, no_arg, one_arg, optional_target, player_of, plain, reg, say, target_of, unavailable, word, CmdResult},
    extras::{self, NearEntry},
    host::{self, with_state},
    i18n,
    logic,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Server,
};

struct PingCmd {
    echo: bool,
}
impl CommandHandler for PingCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if self.echo {
            say(&sender, &greedy_arg(&args, "msg"));
        } else {
            let ping = sender.as_player().map_or(0, |p| p.get_ping());
            let ping_s = ping.to_string();
            say(&sender, &i18n::trf("Pong! Latencia: {} ms", "Pong! Latency: {} ms", &[&ping_s]));
        }
        Ok(1)
    }
}

struct PlaytimeCmd {
    target: bool,
}
impl CommandHandler for PlaytimeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = if self.target { target_of(&sender, &server, &me, &args, CmdLevel::Two)? } else { me };
        let uuid = logic::uuid_string(host::key_of(&target));
        let ms = with_state(|s| s.persisted.playtime_ms.get(&uuid).copied())
            .ok_or_else(unavailable)?
            .unwrap_or(0);
        let pname = target.get_name();
        let duration = extras::format_duration(ms);
        say(&sender, &i18n::trf("{} lleva {} de juego.", "{} has {} of playtime.", &[&pname, &duration]));
        Ok(1)
    }
}

struct NearCmd;
impl CommandHandler for NearCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (center, radius_raw) = if tokens.is_empty() {
            (me, None)
        } else if let Ok(r) = tokens[0].parse::<i64>() {
            (me, Some(r))
        } else {
            let other = server
                .get_player_by_name(tokens[0])
                .ok_or_else(|| fail(&i18n::trf("Jugador '{}' no conectado.", "Player '{}' is not online.", &[tokens[0]])))?;
            if host::key_of(&other) != host::key_of(&me) && !has_level(&sender, CmdLevel::Two) {
                return Err(CommandError::PermissionDenied);
            }
            (other, tokens.get(1).and_then(|s| s.parse::<i64>().ok()))
        };
        let max_radius = with_state(|s| s.cfg.near_max_radius).unwrap_or(200);
        let mut radius = radius_raw.unwrap_or(max_radius).abs();
        if radius > max_radius {
            radius = max_radius;
        }
        let (cx, cy, cz) = center.get_position();
        let world = center.get_world().get_name();
        let mut entries: Vec<NearEntry> = Vec::new();
        for p in server.get_all_players() {
            if p.get_world().get_name() != world {
                continue;
            }
            let (x, y, z) = p.get_position();
            let distance = ((x - cx).powi(2) + (y - cy).powi(2) + (z - cz).powi(2)).sqrt();
            if distance <= radius as f64 {
                entries.push(NearEntry { name: p.get_name(), distance });
            }
        }
        let entries = extras::sort_near(entries);
        let list = entries
            .iter()
            .map(|e| format!("{} ({:.1}m)", e.name, e.distance))
            .collect::<Vec<_>>()
            .join(", ");
        let near = if list.is_empty() { i18n::tr("nadie", "nobody").to_string() } else { list };
        let center_name = center.get_name();
        let radius_s = radius.to_string();
        say(
            &sender,
            &i18n::trf("Jugadores cerca de {} (radio {}): {}", "Players near {} (radius {}): {}", &[&center_name, &radius_s, &near]),
        );
        Ok(1)
    }
}

struct SeenCmd;
impl CommandHandler for SeenCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let name = plain(args.get_value("player"));
        if let Some(p) = server.get_player_by_name(&name) {
            let (x, y, z) = p.get_position();
            let pname = p.get_name();
            let pworld = p.get_world().get_name();
            let xs = format!("{x:.1}");
            let ys = format!("{y:.1}");
            let zs = format!("{z:.1}");
            say(
                &sender,
                &i18n::trf("{} está conectado ahora ({}: {}, {}, {}).", "{} is online now ({}: {}, {}, {}).", &[&pname, &pworld, &xs, &ys, &zs]),
            );
            return Ok(1);
        }
        let seen = with_state(|s| {
            let uuid = extras::find_uuid_by_name(&s.persisted, &name)?;
            let ms = s.persisted.last_seen_ms.get(&uuid).copied();
            let loc = s.persisted.last_location.get(&uuid).cloned();
            Some((uuid, ms, loc))
        })
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail(&i18n::trf("No hay datos de '{}'.", "No data for '{}'.", &[&name])))?;
        let (uuid, ms, loc) = seen;
        let when = ms.map_or(i18n::tr("fecha desconocida", "unknown date").to_string(), |t| {
            let ago = extras::format_duration(host::now_ms().saturating_sub(t));
            i18n::trf("hace {}", "{} ago", &[&ago])
        });
        let place = loc.map_or(String::new(), |l| {
            let lx = format!("{:.1}", l.x);
            let ly = format!("{:.1}", l.y);
            let lz = format!("{:.1}", l.z);
            i18n::trf(" — último lugar: {} ({}, {}, {})", " — last location: {} ({}, {}, {})", &[&l.world, &lx, &ly, &lz])
        });
        say(&sender, &i18n::trf("{} ({}): visto por última vez {}{}.", "{} ({}): last seen {}{}.", &[&name, &uuid, &when, &place]));
        Ok(1)
    }
}

struct WhoIsCmd;
impl CommandHandler for WhoIsCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let name = plain(args.get_value("player"));
        let p = server.get_player_by_name(&name).ok_or_else(|| fail(i18n::tr("El jugador debe estar conectado.", "The player must be online.")))?;
        let uuid = logic::uuid_string(host::key_of(&p));
        let (x, y, z) = p.get_position();
        let ms = with_state(|s| s.persisted.playtime_ms.get(&uuid).copied())
            .ok_or_else(unavailable)?
            .unwrap_or(0);
        let afk = with_state(|s| s.afks.is_afk(host::key_of(&p))).unwrap_or(false);
        let pname = p.get_name();
        let mode = extras::mode_name(&p.get_gamemode());
        let op_level = format!("{:?}", p.get_permission_level());
        let ping_s = p.get_ping().to_string();
        let health = format!("{:.1}", p.get_health());
        let food = p.get_food_level().to_string();
        let exp = p.get_experience_level().to_string();
        let world = p.get_world().get_name();
        let xs = format!("{x:.1}");
        let ys = format!("{y:.1}");
        let zs = format!("{z:.1}");
        let playtime = extras::format_duration(ms);
        let flying = p.is_flying().to_string();
        let afk_s = afk.to_string();
        let msg = i18n::trf(
            "--- {} ---\nUUID: {}\nModo: {} | Nivel op: {} | Ping: {} ms\nVida: {} | Comida: {} | Nivel exp: {}\nMundo: {} ({}, {}, {})\nTiempo de juego: {}\nVuelo: {} | AFK: {}",
            "--- {} ---\nUUID: {}\nMode: {} | Op level: {} | Ping: {} ms\nHealth: {} | Food: {} | Exp level: {}\nWorld: {} ({}, {}, {})\nPlaytime: {}\nFlying: {} | AFK: {}",
            &[&pname, &uuid, mode, &op_level, &ping_s, &health, &food, &exp, &world, &xs, &ys, &zs, &playtime, &flying, &afk_s],
        );
        say(&sender, &msg);
        Ok(1)
    }
}

struct InfoCmd;
impl CommandHandler for InfoCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let sys = server.get_sys_info();
        let names: Vec<String> = server.get_all_worlds().iter().map(|w| w.get_name()).collect();
        let tps = format!("{:.1}", server.get_tps());
        let mspt = format!("{:.1}", server.get_mspt());
        let players = server.get_player_count().to_string();
        let max_players = server.get_max_players().to_string();
        let worlds = if names.is_empty() { i18n::tr("ninguno", "none").to_string() } else { names.join(", ") };
        let mut msg = i18n::trf(
            "Pumpkin {} — TPS {} (MSPT {}) — jugadores {}/{} — mundos: {}",
            "Pumpkin {} — TPS {} (MSPT {}) — players {}/{} — worlds: {}",
            &[&sys.pumpkin_version, &tps, &mspt, &players, &max_players, &worlds],
        );
        if let Some(os) = sys.os_name.as_ref() {
            msg.push_str(&i18n::trf("\nSO: {} {}", "\nOS: {} {}", &[os, sys.os_version.as_deref().unwrap_or("")]));
        }
        if let (Some(total), Some(used)) = (sys.total_memory, sys.used_memory) {
            let used_mb = (used / 1_048_576).to_string();
            let total_mb = (total / 1_048_576).to_string();
            msg.push_str(&i18n::trf("\nMemoria: {} / {} MB", "\nMemory: {} / {} MB", &[&used_mb, &total_mb]));
        }
        say(&sender, &msg);
        Ok(1)
    }
}

struct CompassCmd;
impl CommandHandler for CompassCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let (dir, deg) = extras::bearing(p.get_yaw());
        let deg_s = deg.to_string();
        say(&sender, &i18n::trf("Rumbo: {} ({})", "Bearing: {} ({})", &[dir, &deg_s]));
        Ok(1)
    }
}

struct DepthCmd;
impl CommandHandler for DepthCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let (_, y, _) = p.get_position();
        let delta = y.floor() as i32 - p.get_world().get_sea_level();
        let msg = if delta > 0 {
            let d = delta.to_string();
            i18n::trf("{} bloques sobre el nivel del mar.", "{} blocks above sea level.", &[&d])
        } else if delta < 0 {
            let d = (-delta).to_string();
            i18n::trf("{} bloques bajo el nivel del mar.", "{} blocks below sea level.", &[&d])
        } else {
            i18n::tr("Estás al nivel del mar.", "You are at sea level.").to_string()
        };
        host::info(&p, &msg);
        Ok(1)
    }
}

struct GcCmd;
impl CommandHandler for GcCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let sys = server.get_sys_info();
        let cpu = format!("{:?}", sys.cpu_count);
        let players = server.get_player_count().to_string();
        let max_players = server.get_max_players().to_string();
        let tps = format!("{:.1}", server.get_tps());
        let mspt = format!("{:.1}", server.get_mspt());
        let mut msg = i18n::trf(
            "GC — CPU: {} | Jugadores: {}/{} | TPS: {} | MSPT: {}",
            "GC — CPU: {} | Players: {}/{} | TPS: {} | MSPT: {}",
            &[&cpu, &players, &max_players, &tps, &mspt],
        );
        if let (Some(total), Some(used)) = (sys.total_memory, sys.used_memory) {
            let used_mb = (used / 1_048_576).to_string();
            let total_mb = (total / 1_048_576).to_string();
            msg.push_str(&i18n::trf("\nMemoria: {} / {} MB", "\nMemory: {} / {} MB", &[&used_mb, &total_mb]));
        }
        for w in server.get_all_worlds() {
            msg.push_str(&format!("\n- {}: {}", w.get_name(), w.get_dimension()));
        }
        say(&sender, &msg);
        Ok(1)
    }
}

struct GetPosCmd {
    target: bool,
}
impl CommandHandler for GetPosCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = optional_target(&sender, &server, &me, &args, self.target)?;
        let (x, y, z) = target.get_position();
        let yaw = target.get_yaw();
        let pitch = target.get_pitch();
        let world = target.get_world().get_name();
        let pname = target.get_name();
        let xs = format!("{x:.2}");
        let ys = format!("{y:.2}");
        let zs = format!("{z:.2}");
        let yaws = format!("{yaw:.1}");
        let pitchs = format!("{pitch:.1}");
        let mut msg = i18n::trf(
            "{} — mundo: {}, x: {}, y: {}, z: {}, yaw: {}, pitch: {}",
            "{} — world: {}, x: {}, y: {}, z: {}, yaw: {}, pitch: {}",
            &[&pname, &world, &xs, &ys, &zs, &yaws, &pitchs],
        );
        if world == me.get_world().get_name() {
            let (mx, my, mz) = me.get_position();
            let distance = ((x - mx).powi(2) + (y - my).powi(2) + (z - mz).powi(2)).sqrt();
            let dist_s = format!("{distance:.2}");
            msg.push_str(&i18n::trf(" — distancia: {}", " — distance: {}", &[&dist_s]));
        }
        say(&sender, &msg);
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
        ("ping", "Ver latencia o repetir un mensaje"),
        ("playtime", "Ver tiempo de juego"),
        ("near", "Listar jugadores cercanos"),
        ("compass", "Ver rumbo"),
        ("depth", "Ver profundidad"),
        ("info", "Ver información del servidor"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [
        ("seen", "Ver última conexión de un jugador"),
        ("whois", "Ver información detallada de un jugador"),
        ("gc", "Ver uso de memoria y estado del servidor"),
        ("getpos", "Usar /getpos"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }

    let (ping, playtime, near, compass, depth, info) = (
        "pumpkin_essentials:ping",
        "pumpkin_essentials:playtime",
        "pumpkin_essentials:near",
        "pumpkin_essentials:compass",
        "pumpkin_essentials:depth",
        "pumpkin_essentials:info",
    );
    reg(
        ctx,
        Command::new(&["ping".to_string()], i18n::tr("Pong / repetir texto", "Pong / repeat text"))
            .execute(PingCmd { echo: false })
            .then(greedy_node("msg").execute(PingCmd { echo: true })),
        ping,
    )?;
    reg(
        ctx,
        Command::new(&["playtime".to_string()], i18n::tr("Tiempo de juego", "Playtime"))
            .execute(PlaytimeCmd { target: false })
            .then(word("player").execute(PlaytimeCmd { target: true })),
        playtime,
    )?;
    reg(ctx, Command::new(&["near".to_string()], i18n::tr("Jugadores cercanos", "Nearby players")).then(greedy_node("args").execute(NearCmd)), near)?;
    reg(ctx, no_arg(&["compass"], i18n::tr("Rumbo actual", "Current bearing"), CompassCmd), compass)?;
    reg(ctx, no_arg(&["depth"], i18n::tr("Profundidad", "Depth"), DepthCmd), depth)?;
    reg(ctx, no_arg(&["info"], i18n::tr("Información del servidor", "Server information"), InfoCmd), info)?;
    reg(ctx, one_arg(&["seen"], i18n::tr("Última conexión", "Last seen"), "player", SeenCmd), "pumpkin_essentials:seen")?;
    reg(ctx, one_arg(&["whois"], i18n::tr("Información de jugador", "Player information"), "player", WhoIsCmd), "pumpkin_essentials:whois")?;
    reg(ctx, no_arg(&["gc"], i18n::tr("Estado del servidor", "Server status"), GcCmd), "pumpkin_essentials:gc")?;
    reg(
        ctx,
        Command::new(&["getpos".to_string()], i18n::tr("Muestra coordenadas, mundo y rotación", "Show coordinates, world and rotation"))
            .execute(GetPosCmd { target: false })
            .then(word("player").execute(GetPosCmd { target: true })),
        "pumpkin_essentials:getpos",
    )?;
    Ok(())
}

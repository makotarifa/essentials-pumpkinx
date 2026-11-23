use crate::{
    cmdutil::{fail, greedy_arg, greedy_node, has_level, no_arg, one_arg, optional_target, player_of, plain, reg, say, target_of, unavailable, word, CmdResult},
    extras::{self, NearEntry},
    host::{self, with_state},
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
            say(&sender, &format!("Pong! Latencia: {ping} ms"));
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
        say(&sender, &format!("{} lleva {} de juego.", target.get_name(), extras::format_duration(ms)));
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
                .ok_or_else(|| fail(&format!("Jugador '{}' no conectado.", tokens[0])))?;
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
        say(
            &sender,
            &format!("Jugadores cerca de {} (radio {radius}): {}", center.get_name(), if list.is_empty() { "nadie".into() } else { list }),
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
            say(
                &sender,
                &format!("{} está conectado ahora ({}: {:.1}, {:.1}, {:.1}).", p.get_name(), p.get_world().get_name(), x, y, z),
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
        .ok_or_else(|| fail(&format!("No hay datos de '{name}'.")))?;
        let (uuid, ms, loc) = seen;
        let when = ms.map_or("fecha desconocida".into(), |t| {
            format!("hace {}", extras::format_duration(host::now_ms().saturating_sub(t)))
        });
        let place = loc.map_or(String::new(), |l| {
            format!(" — último lugar: {} ({:.1}, {:.1}, {:.1})", l.world, l.x, l.y, l.z)
        });
        say(&sender, &format!("{name} ({uuid}): visto por última vez {when}{place}."));
        Ok(1)
    }
}

struct WhoIsCmd;
impl CommandHandler for WhoIsCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let name = plain(args.get_value("player"));
        let p = server.get_player_by_name(&name).ok_or_else(|| fail("El jugador debe estar conectado."))?;
        let uuid = logic::uuid_string(host::key_of(&p));
        let (x, y, z) = p.get_position();
        let ms = with_state(|s| s.persisted.playtime_ms.get(&uuid).copied())
            .ok_or_else(unavailable)?
            .unwrap_or(0);
        let afk = with_state(|s| s.afks.is_afk(host::key_of(&p))).unwrap_or(false);
        let msg = format!(
            "--- {} ---\nUUID: {}\nModo: {} | Nivel op: {:?} | Ping: {} ms\nVida: {:.1} | Comida: {} | Nivel exp: {}\nMundo: {} ({:.1}, {:.1}, {:.1})\nTiempo de juego: {}\nVuelo: {} | AFK: {}",
            p.get_name(), uuid, extras::mode_name(&p.get_gamemode()), p.get_permission_level(), p.get_ping(),
            p.get_health(), p.get_food_level(), p.get_experience_level(),
            p.get_world().get_name(), x, y, z, extras::format_duration(ms), p.is_flying(), afk
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
        let mut msg = format!(
            "Pumpkin {} — TPS {:.1} (MSPT {:.1}) — jugadores {}/{} — mundos: {}",
            sys.pumpkin_version, server.get_tps(), server.get_mspt(),
            server.get_player_count(), server.get_max_players(),
            if names.is_empty() { "ninguno".into() } else { names.join(", ") }
        );
        if let Some(os) = sys.os_name.as_ref() {
            msg.push_str(&format!("\nSO: {os} {}", sys.os_version.as_deref().unwrap_or("")));
        }
        if let (Some(total), Some(used)) = (sys.total_memory, sys.used_memory) {
            msg.push_str(&format!("\nMemoria: {} / {} MB", used / 1_048_576, total / 1_048_576));
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
        say(&sender, &format!("Rumbo: {dir} ({deg}°)"));
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
            format!("{delta} bloques sobre el nivel del mar.")
        } else if delta < 0 {
            format!("{} bloques bajo el nivel del mar.", -delta)
        } else {
            "Estás al nivel del mar.".into()
        };
        host::info(&p, &msg);
        Ok(1)
    }
}

struct GcCmd;
impl CommandHandler for GcCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let sys = server.get_sys_info();
        let mut msg = format!(
            "GC — CPU: {:?} | Jugadores: {}/{} | TPS: {:.1} | MSPT: {:.1}",
            sys.cpu_count, server.get_player_count(), server.get_max_players(), server.get_tps(), server.get_mspt()
        );
        if let (Some(total), Some(used)) = (sys.total_memory, sys.used_memory) {
            msg.push_str(&format!("\nMemoria: {} / {} MB", used / 1_048_576, total / 1_048_576));
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
        let mut msg = format!(
            "{} — mundo: {}, x: {x:.2}, y: {y:.2}, z: {z:.2}, yaw: {yaw:.1}, pitch: {pitch:.1}",
            target.get_name(),
            world
        );
        if world == me.get_world().get_name() {
            let (mx, my, mz) = me.get_position();
            let distance = ((x - mx).powi(2) + (y - my).powi(2) + (z - mz).powi(2)).sqrt();
            msg.push_str(&format!(" — distancia: {distance:.2}"));
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
        Command::new(&["ping".to_string()], "Pong / repetir texto")
            .execute(PingCmd { echo: false })
            .then(greedy_node("msg").execute(PingCmd { echo: true })),
        ping,
    )?;
    reg(
        ctx,
        Command::new(&["playtime".to_string()], "Tiempo de juego")
            .execute(PlaytimeCmd { target: false })
            .then(word("player").execute(PlaytimeCmd { target: true })),
        playtime,
    )?;
    reg(ctx, Command::new(&["near".to_string()], "Jugadores cercanos").then(greedy_node("args").execute(NearCmd)), near)?;
    reg(ctx, no_arg(&["compass"], "Rumbo actual", CompassCmd), compass)?;
    reg(ctx, no_arg(&["depth"], "Profundidad", DepthCmd), depth)?;
    reg(ctx, no_arg(&["info"], "Información del servidor", InfoCmd), info)?;
    reg(ctx, one_arg(&["seen"], "Última conexión", "player", SeenCmd), "pumpkin_essentials:seen")?;
    reg(ctx, one_arg(&["whois"], "Información de jugador", "player", WhoIsCmd), "pumpkin_essentials:whois")?;
    reg(ctx, no_arg(&["gc"], "Estado del servidor", GcCmd), "pumpkin_essentials:gc")?;
    reg(
        ctx,
        Command::new(&["getpos".to_string()], "Muestra coordenadas, mundo y rotación")
            .execute(GetPosCmd { target: false })
            .then(word("player").execute(GetPosCmd { target: true })),
        "pumpkin_essentials:getpos",
    )?;
    Ok(())
}

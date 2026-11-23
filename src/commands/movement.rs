use crate::{
    cmdutil::{
        fail, forward, greedy_arg, greedy_node, has_level, need_op, no_arg, one_arg, optional_target, player_of, plain, reg,
        resolve_target, say, unavailable, word, CmdResult,
    },
    config::TprArea,
    extras,
    host::{self, with_state},
    logic::{self, TpKind},
    state::FlyState,
    store::Location,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Player, Server,
};
use tracing::info;

struct BackCmd;
impl CommandHandler for BackCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let key = host::key_of(&player);
        let dest = with_state(|state| state.backs.get(key).cloned())
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail("No hay una ubicación anterior a la que volver."))?;
        info!("essentials: {} /back", player.get_name());
        super::start_teleport(&server, &player, dest, "tu ubicación anterior".into(), TpKind::Back);
        Ok(1)
    }
}

struct SpawnCmd;
impl CommandHandler for SpawnCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let world = player.get_world();
        let world_name = world.get_name();
        let spawn = world.get_spawn_location();
        let fallback = Location {
            world: world_name.clone(),
            x: f64::from(spawn.pos.x) + 0.5,
            y: f64::from(spawn.pos.y) + 1.0,
            z: f64::from(spawn.pos.z) + 0.5,
            yaw: spawn.yaw,
            pitch: spawn.pitch,
        };
        let dest = with_state(|state| state.cfg.spawns.get(&world_name).cloned())
            .ok_or_else(unavailable)?
            .unwrap_or(fallback);
        info!("essentials: {} /spawn", player.get_name());
        super::start_teleport(&server, &player, dest, "el spawn".into(), TpKind::Spawn);
        Ok(1)
    }
}

struct SetSpawnCmd;
impl CommandHandler for SetSpawnCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        need_op(&sender)?;
        let player = player_of(&sender)?;
        let loc = host::location_of(&player);
        with_state(|s| {
            s.cfg.spawns.insert(loc.world.clone(), loc.clone());
            super::save_cfg(s)
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        host::ok(&player, &format!("Spawn de '{}' fijado en tu posición.", loc.world));
        info!("essentials: {} /setspawn in {}", player.get_name(), loc.world);
        Ok(1)
    }
}

struct TopCmd {
    target: bool,
}
impl CommandHandler for TopCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = optional_target(&sender, &server, &me, &args, self.target)?;
        let (x, _, z) = target.get_position();
        let yaw = target.get_yaw();
        let pitch = target.get_pitch();
        let world = target.get_world();
        let world_name = world.get_name();
        let highest = world.get_top_block_y(x.floor() as i32, z.floor() as i32);
        if highest < 0 || highest > 319 {
            return Err(fail("No se encontró un bloque seguro en la cima de esta columna."));
        }
        let dest = Location {
            world: world_name,
            x: x.floor() + 0.5,
            y: extras::top_dest_y(highest),
            z: z.floor() + 0.5,
            yaw,
            pitch,
        };
        info!("essentials: {} /top", target.get_name());
        super::start_teleport(&server, &target, dest, "la cima".into(), TpKind::Top);
        Ok(1)
    }
}

struct TpHereCmd;
impl CommandHandler for TpHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Two) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let name = plain(args.get_value("player"));
        let target = server
            .get_player_by_name(&name)
            .ok_or_else(|| fail(&format!("Jugador '{name}' no conectado.")))?;
        super::start_teleport(&server, &target, host::location_of(&me), me.get_name(), TpKind::Tphere);
        host::ok(&me, &format!("{} teletransportado a ti.", target.get_name()));
        Ok(1)
    }
}

struct TpAllCmd;
impl CommandHandler for TpAllCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Three) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let dest = host::location_of(&me);
        let label = me.get_name();
        let key = host::key_of(&me);
        let mut count = 0;
        for other in server.get_all_players() {
            if host::key_of(&other) == key {
                continue;
            }
            super::start_teleport(&server, &other, dest.clone(), label.clone(), TpKind::Tpall);
            count += 1;
        }
        host::ok(&me, &format!("Petición enviada a {count} jugador(es)."));
        Ok(1)
    }
}

struct BottomCmd {
    target: bool,
}
impl CommandHandler for BottomCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let player = if self.target {
            crate::cmdutil::target_of(&sender, &server, &me, &args, CmdLevel::Two)?
        } else {
            me
        };
        let (x, _, z) = player.get_position();
        let world = player.get_world();
        let bottom = world.get_min_y() + 1;
        let dest = Location {
            world: world.get_name(),
            x: x.floor() + 0.5,
            y: bottom as f64,
            z: z.floor() + 0.5,
            yaw: player.get_yaw(),
            pitch: player.get_pitch(),
        };
        super::start_teleport(&server, &player, dest, "el fondo".into(), TpKind::Bottom);
        Ok(1)
    }
}

struct TpPosCmd;
impl CommandHandler for TpPosCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 3 {
            return Err(fail("Uso: /tppos <x> <y> <z> [yaw] [pitch] [mundo]"));
        }
        let (px, py, pz) = player.get_position();
        let x = extras::parse_tp_coord(tokens[0], px).map_err(|error| fail(&error))?;
        let y = extras::parse_tp_coord(tokens[1], py).map_err(|error| fail(&error))?;
        let z = extras::parse_tp_coord(tokens[2], pz).map_err(|error| fail(&error))?;
        let mut idx = 3;
        let (yaw, pitch) = if tokens.len() >= idx + 2 {
            match (tokens[idx].parse::<f32>(), tokens[idx + 1].parse::<f32>()) {
                (Ok(a), Ok(b)) => {
                    idx += 2;
                    (a, b)
                }
                _ => (player.get_yaw(), player.get_pitch()),
            }
        } else {
            (player.get_yaw(), player.get_pitch())
        };
        let world_name = tokens[idx..].join(" ");
        let world = if world_name.is_empty() { player.get_world().get_name() } else { world_name };
        if server.get_world_by_name(&world).is_none() {
            return Err(fail(&format!("Mundo '{world}' no existe.")));
        }
        let dest = Location { world, x, y, z, yaw, pitch };
        super::complete_teleport(&server, &player, host::key_of(&player), &dest, "tppos", TpKind::Tppos);
        Ok(1)
    }
}

struct TprCmd;
impl CommandHandler for TprCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let world = if let Some(first) = tokens.first() {
            if server.has_world(first) {
                server.get_world_by_name(first).ok_or_else(|| fail("Mundo no disponible."))?
            } else {
                me.get_world()
            }
        } else {
            me.get_world()
        };
        let mut target_opt: Option<Player> = None;
        for name in &tokens {
            if let Some(other) = server.get_player_by_name(name) {
                target_opt = Some(other);
                break;
            }
        }
        if let Some(other) = &target_opt {
            if host::key_of(other) != host::key_of(&me) && !has_level(&sender, CmdLevel::Two) {
                return Err(CommandError::PermissionDenied);
            }
        }
        let target = target_opt.unwrap_or(me);
        let world_name = world.get_name();
        let (area, min_range, max_range, attempts) = with_state(|s| {
            let a = s.cfg.tpr.get(&world_name).cloned().unwrap_or_default();
            (a, s.cfg.tpr_min_range, s.cfg.tpr_max_range, s.cfg.tpr_attempts)
        })
        .ok_or_else(unavailable)?;
        let TprArea { center: custom, min_range: amin, max_range: amax } = area;
        let center = match custom {
            Some(loc) => loc,
            None => {
                let sp = world.get_spawn_location();
                Location {
                    world: world_name.clone(),
                    x: f64::from(sp.pos.x),
                    y: f64::from(sp.pos.y),
                    z: f64::from(sp.pos.z),
                    yaw: sp.yaw,
                    pitch: sp.pitch,
                }
            }
        };
        let lo = amin.unwrap_or(min_range);
        let hi = amax.unwrap_or(max_range);
        let seed_base = (host::now_ms() as u128) ^ u128::from(host::key_of(&target));
        let mut dest: Option<Location> = None;
        for attempt in 0..attempts {
            let seed = seed_base.wrapping_add(u128::from(attempt) * 7_919);
            let (x, z) = extras::random_point(center.x, center.z, lo, hi, seed);
            let top = world.get_top_block_y(x.floor() as i32, z.floor() as i32);
            if top >= world.get_min_y() && top < 320 {
                dest = Some(Location {
                    world: world_name.clone(),
                    x: x.floor() + 0.5,
                    y: extras::top_dest_y(top),
                    z: z.floor() + 0.5,
                    yaw: target.get_yaw(),
                    pitch: target.get_pitch(),
                });
                break;
            }
        }
        let dest = dest.ok_or_else(|| fail("No se encontró un punto aleatorio seguro; prueba otra zona."))?;
        info!("essentials: {} /tpr -> {world_name}", target.get_name());
        super::start_teleport(&server, &target, dest, "un punto aleatorio".into(), TpKind::Tpr);
        Ok(1)
    }
}

struct SetTprCmd;
impl CommandHandler for SetTprCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) {
            return Err(CommandError::PermissionDenied);
        }
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 {
            return Err(fail("Uso: /settpr <mundo> center|minrange|maxrange [valor]"));
        }
        let mode = tokens[1].to_ascii_lowercase();
        let mut saved = Ok(());
        match mode.as_str() {
            "center" => {
                with_state(|s| {
                    s.cfg.tpr.entry(tokens[0].to_string()).or_default().center = Some(host::location_of(&player));
                    saved = super::save_cfg(s);
                })
                .ok_or_else(unavailable)?;
            }
            "minrange" | "maxrange" => {
                let value = tokens
                    .get(2)
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or_else(|| fail("Uso: /settpr <mundo> minrange|maxrange <número>"))?;
                with_state(|s| {
                    let entry = s.cfg.tpr.entry(tokens[0].to_string()).or_default();
                    if mode == "minrange" {
                        entry.min_range = Some(value);
                    } else {
                        entry.max_range = Some(value);
                    }
                    saved = super::save_cfg(s);
                })
                .ok_or_else(unavailable)?;
            }
            _ => return Err(fail("Usa center, minrange o maxrange.")),
        }
        saved.map_err(|error| fail(&error))?;
        say(&sender, &format!("Zona /tpr de '{}' actualizada ({}).", tokens[0], mode));
        Ok(1)
    }
}

struct TpoCmd;
impl CommandHandler for TpoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        match tokens.len() {
            1 => {
                let dest = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
                let loc = host::location_of(&dest);
                super::complete_teleport(&server, &me, host::key_of(&me), &loc, tokens[0], TpKind::Tphere);
            }
            2 => {
                if !has_level(&sender, CmdLevel::Two) {
                    return Err(CommandError::PermissionDenied);
                }
                let who = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador de origen no conectado."))?;
                let dest = server.get_player_by_name(tokens[1]).ok_or_else(|| fail("Jugador de destino no conectado."))?;
                let loc = host::location_of(&dest);
                super::complete_teleport(&server, &who, host::key_of(&who), &loc, tokens[1], TpKind::Tphere);
            }
            _ => return Err(fail("Uso: /tpo <jugador> [a-jugador]")),
        }
        Ok(1)
    }
}

struct TpOfflineCmd;
impl CommandHandler for TpOfflineCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let name = plain(args.get_value("player"));
        if server.get_player_by_name(&name).is_some() {
            return Err(fail("Ese jugador está conectado; usa /tpo."));
        }
        let loc = with_state(|s| {
            let uuid = extras::find_uuid_by_name(&s.persisted, &name)?;
            s.persisted.last_location.get(&uuid).cloned()
        })
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail(&format!("No hay última ubicación guardada de '{name}'.")))?;
        super::start_teleport(&server, &player, loc, format!("la última ubicación de {name}"), TpKind::Tpoffline);
        Ok(1)
    }
}

struct TpOHereCmd;
impl CommandHandler for TpOHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let name = plain(args.get_value("player"));
        if let Some(target) = server.get_player_by_name(&name) {
            super::start_teleport(&server, &target, host::location_of(&me), me.get_name(), TpKind::Tphere);
            return Ok(1);
        }
        Err(fail("La API de Pumpkin no permite mover entidades desconectadas. Usa /tpoffline para ir a su última ubicación guardada."))
    }
}

struct WorldCmd;
impl CommandHandler for WorldCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        if raw.trim().is_empty() {
            let list = server
                .get_all_worlds()
                .iter()
                .map(|w| format!("{} ({})", w.get_name(), w.get_dimension()))
                .collect::<Vec<_>>()
                .join(", ");
            say(&sender, &format!("Mundos: {list}"));
            return Ok(1);
        }
        let world = server
            .get_world_by_name(raw.trim())
            .ok_or_else(|| fail(&format!("Mundo '{}' no existe.", raw.trim())))?;
        let sp = world.get_spawn_location();
        let dest = Location {
            world: world.get_name(),
            x: f64::from(sp.pos.x) + 0.5,
            y: f64::from(sp.pos.y) + 1.0,
            z: f64::from(sp.pos.z) + 0.5,
            yaw: sp.yaw,
            pitch: sp.pitch,
        };
        let label = format!("el mundo {}", dest.world);
        super::start_teleport(&server, &player, dest, label, TpKind::World);
        Ok(1)
    }
}

struct JumpCmd;
impl CommandHandler for JumpCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let (x, y, z) = forward(&player, 3.0);
        let dest = Location {
            world: player.get_world().get_name(),
            x: x.floor() + 0.5,
            y: y + 1.0,
            z: z.floor() + 0.5,
            yaw: player.get_yaw(),
            pitch: player.get_pitch(),
        };
        super::start_teleport(&server, &player, dest, "salto".into(), TpKind::Jump);
        Ok(1)
    }
}

struct FlyCmd {
    target: bool,
}
impl CommandHandler for FlyCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = if self.target {
            resolve_target(&sender, &server, &me, &plain(args.get_value("player")))?
        } else {
            me
        };
        let was_flying = target.is_flying();
        let creative = matches!(target.get_gamemode(), pumpkin_plugin_api::common::GameMode::Creative);
        let (allow, now_flying) = if was_flying { (creative, false) } else { (true, true) };
        target.set_allow_flight(allow);
        target.set_flying(now_flying);
        let speed = target.get_abilities().fly_speed;
        let key = host::key_of(&target);
        with_state(|s| {
            s.persisted.fly.insert(
                logic::uuid_string(key),
                FlyState { allow_flying: allow, flying: now_flying, fly_speed: speed },
            );
            s.save_persisted();
        });
        let name = target.get_name();
        let verb = if was_flying { "disabled" } else { "enabled" };
        host::ok(&target, if was_flying { "Vuelo desactivado." } else { "Vuelo activado." });
        info!("essentials: {} flight {}", name, verb);
        Ok(1)
    }
}

struct FlySpeedCmd;
impl CommandHandler for FlySpeedCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let first = tokens.next().ok_or_else(|| fail("Uso: /flyspeed [jugador] <0.0-10.0>"))?;
        let rest: Vec<&str> = tokens.collect();
        let (target_name, speed_raw) = if extras::parse_fly_speed(first).is_ok() {
            (None, first)
        } else if rest.len() == 1 && extras::parse_fly_speed(rest[0]).is_ok() {
            (Some(first), rest[0])
        } else {
            return Err(fail("Uso: /flyspeed [jugador] <0.0-10.0>"));
        };
        let value = extras::parse_fly_speed(speed_raw).map_err(|error| fail(&error))?;
        let target = match target_name {
            Some(name) => resolve_target(&sender, &server, &me, name)?,
            None => me,
        };
        let unit = with_state(|state| state.cfg.flyspeed_unit).ok_or_else(unavailable)?;
        let engine_speed = extras::engine_fly_speed(value, unit);
        target.set_fly_speed(engine_speed);
        let speed_now = target.get_abilities().fly_speed;
        with_state(|s| {
            let entry = s.persisted.fly.entry(logic::uuid_string(host::key_of(&target))).or_default();
            entry.fly_speed = speed_now;
            s.save_persisted();
        });
        let name = target.get_name();
        host::ok(&target, &format!("Velocidad de vuelo fijada en {value:.2}."));
        info!("essentials: {} flyspeed {value:.2}", name);
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
        ("back", "Usar /back"),
        ("spawn", "Ir al spawn"),
        ("top", "Ir a la cima de una columna"),
        ("tpr", "Teleport aleatorio"),
        ("world", "Listar mundos o ir al spawn de un mundo"),
        ("jump", "Saltar al punto mirado"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [
        ("fly", "Usar /fly"),
        ("flyspeed", "Usar /flyspeed"),
        ("tphere", "Teletransportar a otro jugador a ti"),
        ("bottom", "Ir al fondo de la columna"),
        ("tppos", "Teleport a coordenadas"),
        ("tpo", "Teleport instantáneo de/a jugadores"),
        ("tpoffline", "Ir a la última ubicación de un jugador desconectado"),
        ("tpohere", "Traer a un jugador conectado"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    for (n, d) in [
        ("setspawn", "Fijar el spawn del mundo actual"),
        ("tpall", "Teletransportar a todos los jugadores a ti"),
        ("settpr", "Configurar zonas de teleport aleatorio"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Three),
            children: vec![],
        })?;
    }

    reg(ctx, no_arg(&["back"], "Vuelve a tu ubicación anterior", BackCmd), "pumpkin_essentials:back")?;
    reg(ctx, no_arg(&["spawn"], "Teletransporta al spawn", SpawnCmd), "pumpkin_essentials:spawn")?;
    reg(ctx, no_arg(&["setspawn"], "Fija el spawn de este mundo (op)", SetSpawnCmd), "pumpkin_essentials:setspawn")?;
    reg(
        ctx,
        Command::new(&["top".to_string()], "Teletransporta al bloque más alto")
            .execute(TopCmd { target: false })
            .then(word("player").execute(TopCmd { target: true })),
        "pumpkin_essentials:top",
    )?;
    reg(
        ctx,
        Command::new(&["bottom".to_string()], "Ir al fondo")
            .execute(BottomCmd { target: false })
            .then(word("player").execute(BottomCmd { target: true })),
        "pumpkin_essentials:bottom",
    )?;
    reg(
        ctx,
        Command::new(&["tppos".to_string()], "Teleport a coordenadas").then(greedy_node("args").execute(TpPosCmd)),
        "pumpkin_essentials:tppos",
    )?;
    reg(
        ctx,
        Command::new(&["tpr".to_string()], "Teleport aleatorio").then(greedy_node("args").execute(TprCmd)),
        "pumpkin_essentials:tpr",
    )?;
    reg(
        ctx,
        Command::new(&["settpr".to_string()], "Configurar /tpr").then(greedy_node("args").execute(SetTprCmd)),
        "pumpkin_essentials:settpr",
    )?;
    reg(
        ctx,
        Command::new(&["tpo".to_string()], "Teleport instantáneo").then(greedy_node("args").execute(TpoCmd)),
        "pumpkin_essentials:tpo",
    )?;
    reg(ctx, one_arg(&["tpoffline"], "Ir a la última ubicación de un desconectado", "player", TpOfflineCmd), "pumpkin_essentials:tpoffline")?;
    reg(ctx, one_arg(&["tpohere"], "Traer a un jugador conectado", "player", TpOHereCmd), "pumpkin_essentials:tpohere")?;
    reg(ctx, one_arg(&["tphere"], "Teletransporta a un jugador a ti (op)", "player", TpHereCmd), "pumpkin_essentials:tphere")?;
    reg(ctx, no_arg(&["tpall"], "Teletransporta a todos a ti (op)", TpAllCmd), "pumpkin_essentials:tpall")?;
    reg(
        ctx,
        Command::new(&["world".to_string()], "Listar mundos / ir a un mundo").then(greedy_node("args").execute(WorldCmd)),
        "pumpkin_essentials:world",
    )?;
    reg(ctx, no_arg(&["jump"], "Saltar", JumpCmd), "pumpkin_essentials:jump")?;
    reg(
        ctx,
        Command::new(&["fly".to_string()], "Alterna el vuelo (opcionalmente de otro jugador)")
            .execute(FlyCmd { target: false })
            .then(word("player").execute(FlyCmd { target: true })),
        "pumpkin_essentials:fly",
    )?;
    reg(
        ctx,
        Command::new(&["flyspeed".to_string()], "Fija la velocidad de vuelo 0.0-10.0").then(greedy_node("args").execute(FlySpeedCmd)),
        "pumpkin_essentials:flyspeed",
    )?;
    Ok(())
}

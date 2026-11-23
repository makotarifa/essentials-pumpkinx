//! Iteración 4: información y teleport avanzado (semántica EssentialsX adaptada).

use crate::{
    commands,
    config::TprArea,
    extras::{self, NearEntry},
    host::{self, with_state},
    logic::{self, TpKind},
    store::Location,
};
use pumpkin_plugin_api::{
    command::{Arg, ArgumentType, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Player, Server,
};
use tracing::info;

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

fn target_of(sender: &CommandSender, server: &Server, _me: &Player, args: &ConsumedArgs, level: CmdLevel) -> Result<Player, CommandError> {
    if !has_level(sender, level) {
        return Err(CommandError::PermissionDenied);
    }
    let name = plain(args.get_value("player"));
    server.get_player_by_name(&name).ok_or_else(|| fail(&format!("Jugador '{name}' no conectado.")))
}

// ---- información ---------------------------------------------------------------

struct PingCmd {
    echo: bool,
}
impl CommandHandler for PingCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if self.echo {
            say(&sender, &greedy(&args, "msg"));
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
        let raw = greedy(&args, "args");
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

struct WarpInfoCmd;
impl CommandHandler for WarpInfoCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let name = logic::validate_name(&plain(args.get_value("name"))).map_err(|e| fail(e.message()))?;
        let info = with_state(|s| s.warps.warps.get(&name).map(|w| (w.loc.clone(), w.created_by.clone())))
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail(&format!("No existe el warp '{name}'.")))?;
        let (loc, by) = info;
        say(
            &sender,
            &format!("Warp '{name}' — creado por {by} — {} ({:.1}, {:.1}, {:.1}), yaw {:.1}, pitch {:.1}", loc.world, loc.x, loc.y, loc.z, loc.yaw, loc.pitch),
        );
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

// ---- teleports -----------------------------------------------------------------

struct BottomCmd {
    target: bool,
}
impl CommandHandler for BottomCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let p = if self.target { target_of(&sender, &server, &me, &args, CmdLevel::Two)? } else { me };
        let (x, _, z) = p.get_position();
        let world = p.get_world();
        let bottom = world.get_min_y() + 1;
        let dest = Location {
            world: world.get_name(),
            x: x.floor() + 0.5,
            y: bottom as f64,
            z: z.floor() + 0.5,
            yaw: p.get_yaw(),
            pitch: p.get_pitch(),
        };
        commands::start_teleport(&server, &p, dest, "el fondo".into(), TpKind::Bottom);
        Ok(1)
    }
}

struct TpPosCmd;
impl CommandHandler for TpPosCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 3 {
            return Err(fail("Uso: /tppos <x> <y> <z> [yaw] [pitch] [mundo]"));
        }
        let (px, py, pz) = p.get_position();
        let x = extras::parse_tp_coord(tokens[0], px).map_err(|e| fail(&e))?;
        let y = extras::parse_tp_coord(tokens[1], py).map_err(|e| fail(&e))?;
        let z = extras::parse_tp_coord(tokens[2], pz).map_err(|e| fail(&e))?;
        let mut idx = 3;
        let (yaw, pitch) = if tokens.len() >= idx + 2 {
            match (tokens[idx].parse::<f32>(), tokens[idx + 1].parse::<f32>()) {
                (Ok(a), Ok(b)) => {
                    idx += 2;
                    (a, b)
                }
                _ => (p.get_yaw(), p.get_pitch()),
            }
        } else {
            (p.get_yaw(), p.get_pitch())
        };
        let world_name = tokens[idx..].join(" ");
        let world = if world_name.is_empty() { p.get_world().get_name() } else { world_name };
        if server.get_world_by_name(&world).is_none() {
            return Err(fail(&format!("Mundo '{world}' no existe.")));
        }
        let dest = Location { world, x, y, z, yaw, pitch };
        commands::complete_teleport(&server, &p, host::key_of(&p), &dest, "tppos", TpKind::Tppos);
        Ok(1)
    }
}

struct TprCmd;
impl CommandHandler for TprCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
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
        commands::start_teleport(&server, &target, dest, "un punto aleatorio".into(), TpKind::Tpr);
        Ok(1)
    }
}

struct SetTprCmd;
impl CommandHandler for SetTprCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) {
            return Err(CommandError::PermissionDenied);
        }
        let p = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 {
            return Err(fail("Uso: /settpr <mundo> center|minrange|maxrange [valor]"));
        }
        let mode = tokens[1].to_ascii_lowercase();
        let mut saved = Ok(());
        match mode.as_str() {
            "center" => {
                with_state(|s| {
                    s.cfg.tpr.entry(tokens[0].to_string()).or_default().center = Some(host::location_of(&p));
                    saved = commands::save_cfg(s);
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
                    saved = commands::save_cfg(s);
                })
                .ok_or_else(unavailable)?;
            }
            _ => return Err(fail("Usa center, minrange o maxrange.")),
        }
        saved.map_err(|e| fail(&e))?;
        say(&sender, &format!("Zona /tpr de '{}' actualizada ({}).", tokens[0], mode));
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
        let raw = greedy(&args, "args");
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
        let raw = greedy(&args, "args");
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

struct TpoCmd;
impl CommandHandler for TpoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        match tokens.len() {
            1 => {
                let dest = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador no conectado."))?;
                let loc = host::location_of(&dest);
                commands::complete_teleport(&server, &me, host::key_of(&me), &loc, tokens[0], TpKind::Tphere);
            }
            2 => {
                if !has_level(&sender, CmdLevel::Two) {
                    return Err(CommandError::PermissionDenied);
                }
                let who = server.get_player_by_name(tokens[0]).ok_or_else(|| fail("Jugador de origen no conectado."))?;
                let dest = server.get_player_by_name(tokens[1]).ok_or_else(|| fail("Jugador de destino no conectado."))?;
                let loc = host::location_of(&dest);
                commands::complete_teleport(&server, &who, host::key_of(&who), &loc, tokens[1], TpKind::Tphere);
            }
            _ => return Err(fail("Uso: /tpo <jugador> [a-jugador]")),
        }
        Ok(1)
    }
}

struct TpOfflineCmd;
impl CommandHandler for TpOfflineCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
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
        commands::start_teleport(&server, &p, loc, format!("la última ubicación de {name}"), TpKind::Tpoffline);
        Ok(1)
    }
}

struct TpOHereCmd;
impl CommandHandler for TpOHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let name = plain(args.get_value("player"));
        if let Some(target) = server.get_player_by_name(&name) {
            commands::start_teleport(&server, &target, host::location_of(&me), me.get_name(), TpKind::Tphere);
            return Ok(1);
        }
        Err(fail("La API de Pumpkin no permite mover entidades desconectadas. Usa /tpoffline para ir a su última ubicación guardada."))
    }
}

struct RestCmd {
    target: bool,
}
impl CommandHandler for RestCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let p = if self.target { target_of(&sender, &server, &me, &args, CmdLevel::Two)? } else { me };
        p.set_exhaustion(0.0);
        p.set_saturation(10.0);
        host::info(&p, "Has descansado.");
        Ok(1)
    }
}

struct WorldCmd;
impl CommandHandler for WorldCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let raw = greedy(&args, "args");
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
        commands::start_teleport(&server, &p, dest, label, TpKind::World);
        Ok(1)
    }
}

// ---- registro ------------------------------------------------------------------

pub fn register_all(ctx: &Context) -> Result<(), String> {
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
        ("warpinfo", "Ver detalles de un warp"),
        ("info", "Ver información del servidor"),
        ("rest", "Reiniciar exhausión y saturación"),
        ("tpacancel", "Cancelar peticiones de teleport enviadas"),
        ("tpauto", "Aceptar peticiones automáticamente"),
        ("tpr", "Teleport aleatorio"),
        ("world", "Listar mundos o ir al spawn de un mundo"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [
        ("seen", "Ver última conexión de un jugador"),
        ("whois", "Ver información detallada de un jugador"),
        ("gc", "Ver uso de memoria y estado del servidor"),
        ("bottom", "Ir al fondo de la columna"),
        ("tppos", "Teleport a coordenadas"),
        ("tpo", "Teleport instantáneo de/a jugadores"),
        ("tpoffline", "Ir a la última ubicación de un jugador desconectado"),
        ("tpohere", "Traer a un jugador conectado"),
        ("tpaall", "Pedir a todos los jugadores que vengan"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:settpr".into(),
        description: "Configurar zonas de teleport aleatorio".into(),
        default: PermissionDefault::Op(PermissionLevel::Three),
        children: vec![],
    })?;

    let (ping, playtime, near, compass, depth, warpinfo, info, rest) = (
        "pumpkin_essentials:ping",
        "pumpkin_essentials:playtime",
        "pumpkin_essentials:near",
        "pumpkin_essentials:compass",
        "pumpkin_essentials:depth",
        "pumpkin_essentials:warpinfo",
        "pumpkin_essentials:info",
        "pumpkin_essentials:rest",
    );
    reg(ctx, Command::new(&["ping".to_string()], "Pong / repetir texto").execute(PingCmd { echo: false }).then(greedy_node("msg").execute(PingCmd { echo: true })), ping)?;
    reg(ctx, Command::new(&["playtime".to_string()], "Tiempo de juego").execute(PlaytimeCmd { target: false }).then(word("player").execute(PlaytimeCmd { target: true })), playtime)?;
    reg(ctx, Command::new(&["near".to_string()], "Jugadores cercanos").then(greedy_node("args").execute(NearCmd)), near)?;
    reg(ctx, no_arg(&["compass"], "Rumbo actual", CompassCmd), compass)?;
    reg(ctx, no_arg(&["depth"], "Profundidad", DepthCmd), depth)?;
    reg(ctx, one_arg(&["warpinfo"], "Información de un warp", "name", WarpInfoCmd), warpinfo)?;
    reg(ctx, no_arg(&["info"], "Información del servidor", InfoCmd), info)?;
    reg(ctx, Command::new(&["rest".to_string()], "Descansar").execute(RestCmd { target: false }).then(word("player").execute(RestCmd { target: true })), rest)?;

    reg(ctx, one_arg(&["seen"], "Última conexión", "player", SeenCmd), "pumpkin_essentials:seen")?;
    reg(ctx, one_arg(&["whois"], "Información de jugador", "player", WhoIsCmd), "pumpkin_essentials:whois")?;
    reg(ctx, no_arg(&["gc"], "Estado del servidor", GcCmd), "pumpkin_essentials:gc")?;
    reg(ctx, Command::new(&["bottom".to_string()], "Ir al fondo").execute(BottomCmd { target: false }).then(word("player").execute(BottomCmd { target: true })), "pumpkin_essentials:bottom")?;
    reg(ctx, Command::new(&["tppos".to_string()], "Teleport a coordenadas").then(greedy_node("args").execute(TpPosCmd)), "pumpkin_essentials:tppos")?;
    reg(ctx, Command::new(&["tpr".to_string()], "Teleport aleatorio").then(greedy_node("args").execute(TprCmd)), "pumpkin_essentials:tpr")?;
    reg(ctx, Command::new(&["settpr".to_string()], "Configurar /tpr").then(greedy_node("args").execute(SetTprCmd)), "pumpkin_essentials:settpr")?;
    reg(ctx, Command::new(&["tpaall".to_string()], "Pedir teleport a todos").then(greedy_node("player").execute(TpaAllCmd)), "pumpkin_essentials:tpaall")?;
    reg(ctx, Command::new(&["tpacancel".to_string()], "Cancelar peticiones").then(greedy_node("args").execute(TpaCancelCmd)), "pumpkin_essentials:tpacancel")?;
    reg(ctx, Command::new(&["tpauto".to_string()], "Auto-aceptar peticiones").then(greedy_node("args").execute(TpAutoCmd)), "pumpkin_essentials:tpauto")?;
    reg(ctx, Command::new(&["tpo".to_string()], "Teleport instantáneo").then(greedy_node("args").execute(TpoCmd)), "pumpkin_essentials:tpo")?;
    reg(ctx, one_arg(&["tpoffline"], "Ir a la última ubicación de un desconectado", "player", TpOfflineCmd), "pumpkin_essentials:tpoffline")?;
    reg(ctx, one_arg(&["tpohere"], "Traer a un jugador conectado", "player", TpOHereCmd), "pumpkin_essentials:tpohere")?;
    reg(ctx, Command::new(&["world".to_string()], "Listar mundos / ir a un mundo").then(greedy_node("args").execute(WorldCmd)), "pumpkin_essentials:world")?;
    Ok(())
}

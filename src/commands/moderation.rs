use crate::{
    i18n,
    cmdutil::{fail, greedy_arg, greedy_cmd, has_level, no_arg, one_arg, plain, player_of, reg, say, unavailable, uuid_by_name, CmdResult},
    events,
    host::{self, with_state},
    logic::{self, TpKind},
    state::{JailRecord, ModRecord},
    store::Location,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::CommandHandler,
    permission::{Permission, PermissionDefault, PermissionLevel},
    server::CommandSender as ServerCommandSender,
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Server,
};
use crate::extras;
use tracing::info;

struct MuteCmd;
impl CommandHandler for MuteCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() { return Err(fail(i18n::tr("Uso: /mute <jugador> [duración] [razón]", "Usage: /mute <player> [duration] [reason]"))); }
        let name = tokens[0];
        let uuid = uuid_by_name(&server, name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        let secs = extras::parse_duration(tokens.get(1).copied().unwrap_or("0")).map_err(|error| fail(&error))?;
        let reason = if tokens.len() > 2 { tokens[2..].join(" ") } else { String::new() };
        let now = host::now_ms();
        let by = sender.get_name();
        with_state(|state| {
            state.persisted.mutes.insert(uuid.clone(), ModRecord { until_ms: extras::expiry_from_now(secs, now), reason: reason.clone(), by: by.clone() });
            state.save_persisted();
        })
        .ok_or_else(unavailable)?;
        if let Some(player) = server.get_player_by_name(name) {
            player.send_system_message(TextComponent::text(i18n::tr("Has sido silenciado.", "You have been muted.")), false);
        }
        say(&sender, &i18n::trf("{} silenciado.", "{} muted.", &[name]));
        Ok(1)
    }
}

struct UnmuteCmd;
impl CommandHandler for UnmuteCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let name = greedy_arg(&args, "args").trim().to_string();
        let uuid = uuid_by_name(&server, &name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        with_state(|state| { state.persisted.mutes.remove(&uuid); state.save_persisted(); }).ok_or_else(unavailable)?;
        say(&sender, &i18n::trf("{} ya no está silenciado.", "{} is no longer muted.", &[&name]));
        Ok(1)
    }
}

struct TempBanCmd;
impl CommandHandler for TempBanCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 { return Err(fail(i18n::tr("Uso: /tempban <jugador> <duración> [razón]", "Usage: /tempban <player> <duration> [reason]"))); }
        let name = tokens[0];
        let uuid = uuid_by_name(&server, name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        let secs = extras::parse_duration(tokens[1]).map_err(|error| fail(&error))?;
        let reason = if tokens.len() > 2 { tokens[2..].join(" ") } else { String::new() };
        let by = sender.get_name();
        with_state(|state| {
            state.persisted.bans.insert(uuid.clone(), ModRecord { until_ms: extras::expiry_from_now(secs, host::now_ms()), reason: reason.clone(), by: by.clone() });
            state.save_persisted();
        })
        .ok_or_else(unavailable)?;
        if let Some(player) = server.get_player_by_name(name) {
            events::kick_player(&player, &i18n::trf("Baneado temporalmente: {}", "Temporarily banned: {}", &[&reason]));
        }
        say(&sender, &i18n::trf("{} baneado.", "{} banned.", &[name]));
        Ok(1)
    }
}

struct KickAllCmd;
impl CommandHandler for KickAllCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let reason = greedy_arg(&args, "args").trim().to_string();
        let reason = if reason.is_empty() { i18n::tr("Kick por administración.", "Administrative kick.").into() } else { reason };
        let me_key = player_of(&sender).ok().map(|player| host::key_of(&player));
        let mut count = 0;
        for player in server.get_all_players() {
            if me_key.is_some_and(|k| k == host::key_of(&player)) { continue; }
            events::kick_player(&player, &reason);
            count += 1;
        }
        let count_text = count.to_string();
        say(&sender, &i18n::trf("Expulsados {} jugador(es).", "Kicked {} player(s).", &[&count_text]));
        Ok(1)
    }
}

struct VanishCmd;
impl CommandHandler for VanishCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|state| {
            let current = state.persisted.vanished.get(&uuid).copied().unwrap_or(false);
            let next = crate::cmdutil::parse_mode_arg(mode_token, current);
            state.persisted.vanished.insert(uuid, next);
            state.save_persisted();
            next
        })
        .ok_or_else(unavailable)?;
        let target_key = server.get_player_by_name(&label).map(|player| host::key_of(&player));
        if let Some(target_key) = target_key {
            for other in server.get_all_players() {
                if host::key_of(&other) != target_key {
                    let Some(handle) = server.get_player_by_name(&label) else { continue };
                    if on { other.hide_player(handle); } else { other.show_player(handle); }
                }
            }
        }
        let state = if on { i18n::tr("activado", "enabled") } else { i18n::tr("desactivado", "disabled") };
        say(&sender, &i18n::trf("Vanish de {}: {}.", "Vanish for {}: {}.", &[&label, state]));
        Ok(1)
    }
}

struct GodCmd;
impl CommandHandler for GodCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let (uuid, label, mode_token) = if tokens.len() > 1 {
            let other = server.get_player_by_name(tokens[0]).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
            (logic::uuid_string(host::key_of(&other)), other.get_name(), tokens.get(1).copied())
        } else {
            (logic::uuid_string(host::key_of(&me)), me.get_name(), tokens.first().copied())
        };
        let on = with_state(|state| {
            let current = state.persisted.god.get(&uuid).copied().unwrap_or(false);
            let next = crate::cmdutil::parse_mode_arg(mode_token, current);
            state.persisted.god.insert(uuid, next);
            state.save_persisted();
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
        let state = if on { i18n::tr("activado", "enabled") } else { i18n::tr("desactivado", "disabled") };
        say(&sender, &i18n::trf("God de {}: {}.", "God mode for {}: {}.", &[&label, state]));
        Ok(1)
    }
}

struct SudoCmd;
impl CommandHandler for SudoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.splitn(2, char::is_whitespace);
        let name = tokens.next().unwrap_or("").trim();
        let command = tokens.next().unwrap_or("").trim().trim_start_matches('/').to_string();
        if name.is_empty() || command.is_empty() {
            return Err(fail(i18n::tr("Uso: /sudo <jugador> <comando>", "Usage: /sudo <player> <command>")));
        }
        if host::key_of(&me) == server.get_player_by_name(name).map(|player| host::key_of(&player)).unwrap_or(0) {
            return Err(fail(i18n::tr("No puedes hacerte sudo a ti mismo.", "You cannot sudo yourself.")));
        }
        let lower = command.to_ascii_lowercase();
        for blocked in ["sudo", "op ", "deop ", "perms", "lp ", "ban ", "tempban", "kickall", "backup"] {
            if lower.starts_with(blocked) {
                return Err(fail(i18n::tr("Ese comando no puede ejecutarse con /sudo.", "That command cannot be run with /sudo.")));
            }
        }
        let target = server.get_player_by_name(name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
        let full = format!("/{command}");
        server.execute_command(&full, ServerCommandSender::Player(target));
        info!("essentials: sudo {} -> {}", sender.get_name(), full);
        say(&sender, &i18n::trf("Ejecutado como {}: {}", "Executed as {}: {}", &[name, &full]));
        Ok(1)
    }
}

struct JailInfoCmd;
impl CommandHandler for JailInfoCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let list = with_state(|state| {
            let now = host::now_ms();
            state.persisted.jailed.iter()
                .filter(|(_, r)| r.until_ms == 0 || r.until_ms > now)
                .map(|(uuid, _)| state.persisted.names.get(uuid).cloned().unwrap_or_else(|| uuid.clone()).into())
                .collect::<Vec<String>>()
        })
        .ok_or_else(unavailable)?;
        let list_text = if list.is_empty() { i18n::tr("nadie", "nobody").to_string() } else { list.join(", ") };
        say(&sender, &i18n::trf("Encarcelados: {}", "Jailed players: {}", &[&list_text]));
        Ok(1)
    }
}

fn jail_player(server: &Server, sender: &CommandSender, args: &ConsumedArgs, release: bool) -> CmdResult {
    if !has_level(sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
    let raw = plain(args.get_value("player"));
    let uuid = uuid_by_name(server, &raw).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
    let binding = plain(args.get_value("player"));
    let tokens: Vec<&str> = binding.split_whitespace().collect();
    let by = sender.get_name();
    let (jail_name, until, message) = if release {
        (String::new(), 0u64, i18n::trf("{} liberado.", "{} released.", &[&raw]))
    } else {
        let jails = with_state(|state| state.cfg.jails.keys().cloned().collect::<Vec<_>>()).ok_or_else(unavailable)?;
        if jails.is_empty() { return Err(fail(i18n::tr("No hay cárceles definidas (/setjail).", "No jails defined (/setjail)."))); }
        let name = tokens.get(1).copied().filter(|n| jails.iter().any(|j| j == n)).unwrap_or(&jails[0]);
        let secs = tokens.get(2).map(|s| extras::parse_duration(s)).transpose().map_err(|error| fail(&error))?.unwrap_or(0);
        (name.to_string(), extras::expiry_from_now(secs, host::now_ms()), i18n::trf("{} encarcelado en '{}'.", "{} jailed in '{}'.", &[&raw, name]))
    };
    let jail_loc = if release { None } else { with_state(|state| state.cfg.jails.get(&jail_name).cloned()).ok_or_else(unavailable)? };
    with_state(|state| {
        if release {
            state.persisted.jailed.remove(&uuid);
        } else {
            state.persisted.jailed.insert(uuid.clone(), JailRecord { jail: jail_name.clone(), until_ms: until, by: by.clone() });
        }
        state.save_persisted();
    })
    .ok_or_else(unavailable)?;
    if let (Some(loc), Some(player)) = (jail_loc, server.get_player_by_name(&raw)) {
        super::start_teleport(server, &player, loc, i18n::tr("la cárcel", "jail").into(), TpKind::Tphere);
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
        let uuid = uuid_by_name(&server, &plain(args.get_value("player"))).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        let jailed = with_state(|state| state.persisted.jailed.contains_key(&uuid)).ok_or_else(unavailable)?;
        jail_player(&server, &sender, &args, jailed)
    }
}

struct SetJailCmd;
impl CommandHandler for SetJailCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let name = plain(args.get_value("name"));
        if name.trim().is_empty() { return Err(fail(i18n::tr("Uso: /setjail <nombre>", "Usage: /setjail <name>"))); }
        let loc: Location = host::location_of(&player);
        with_state(|state| { state.cfg.jails.insert(name.clone(), loc); super::save_cfg(state) })
            .ok_or_else(unavailable)?
            .map_err(|error| fail(&error))?;
        say(&sender, &i18n::trf("Cárcel '{}' guardada.", "Jail '{}' saved.", &[&name]));
        Ok(1)
    }
}

struct DelJailCmd;
impl CommandHandler for DelJailCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let name = plain(args.get_value("name"));
        let removed = with_state(|state| {
            let gone = state.cfg.jails.remove(&name).is_some();
            if gone { let _ = super::save_cfg(state); }
            gone
        })
        .ok_or_else(unavailable)?;
        if !removed { return Err(fail(i18n::tr("Esa cárcel no existe.", "That jail does not exist."))); }
        say(&sender, &i18n::trf("Cárcel '{}' eliminada.", "Jail '{}' deleted.", &[&name]));
        Ok(1)
    }
}

struct JailsCmd;
impl CommandHandler for JailsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let list = with_state(|state| {
            state.cfg.jails.iter().map(|(name, loc)| format!("{name}: {} ({:.0}, {:.0}, {:.0})", loc.world, loc.x, loc.y, loc.z)).collect::<Vec<String>>()
        })
        .ok_or_else(|| fail(i18n::tr("Configuración no disponible.", "Configuration unavailable.")))?;
        let list_text = if list.is_empty() { i18n::tr("ninguna", "none").to_string() } else { list.join(" | ") };
        say(&sender, &i18n::trf("Cárceles: {}", "Jails: {}", &[&list_text]));
        Ok(1)
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    let perm = |node: &str, desc: &str, level: PermissionLevel| Permission {
        node: format!("pumpkin_essentials:{node}"),
        description: desc.into(),
        default: PermissionDefault::Op(level),
        children: vec![],
    };
    let two = PermissionLevel::Two;
    let three = PermissionLevel::Three;
    for (n, d, l) in [
        ("mute", "Silenciar jugadores", two),
        ("unmute", "Quitar silencios", two),
        ("vanish", "Ocultarse", two),
        ("god", "Modo dios", two),
        ("jail", "Encarcelar", two),
        ("togglejail", "Alternar cárcel", two),
        ("jailedplayers", "Listar encarcelados", two),
        ("jails", "Listar cárceles configuradas", two),
        ("tempban", "Ban temporal", three),
        ("kickall", "Expulsar a todos", three),
        ("sudo", "Ejecutar como otro jugador", three),
        ("setjail", "Definir cárcel", three),
        ("deljail", "Eliminar cárcel", three),
    ] {
        ctx.register_permission(&perm(n, d, l))?;
    }

    reg(ctx, greedy_cmd("mute", i18n::tr("Silenciar", "Mute"), MuteCmd), "pumpkin_essentials:mute")?;
    reg(ctx, one_arg(&["unmute"], i18n::tr("Quitar silencio", "Unmute"), "player", UnmuteCmd), "pumpkin_essentials:unmute")?;
    reg(ctx, greedy_cmd("tempban", i18n::tr("Ban temporal", "Temporary ban"), TempBanCmd), "pumpkin_essentials:tempban")?;
    reg(ctx, greedy_cmd("kickall", i18n::tr("Expulsar a todos", "Kick all"), KickAllCmd), "pumpkin_essentials:kickall")?;
    reg(ctx, greedy_cmd("vanish", i18n::tr("Ocultarse", "Vanish"), VanishCmd), "pumpkin_essentials:vanish")?;
    reg(ctx, greedy_cmd("god", i18n::tr("Modo dios", "God mode"), GodCmd), "pumpkin_essentials:god")?;
    reg(ctx, greedy_cmd("sudo", i18n::tr("Ejecutar como otro", "Run as another player"), SudoCmd), "pumpkin_essentials:sudo")?;
    reg(ctx, no_arg(&["jailedplayers"], i18n::tr("Listar encarcelados", "List jailed players"), JailInfoCmd), "pumpkin_essentials:jailedplayers")?;
    reg(ctx, one_arg(&["jail"], i18n::tr("Encarcelar", "Jail"), "player", JailCmd), "pumpkin_essentials:jail")?;
    reg(ctx, one_arg(&["togglejail"], i18n::tr("Alternar cárcel", "Toggle jail"), "player", ToggleJailCmd), "pumpkin_essentials:togglejail")?;
    reg(ctx, one_arg(&["setjail"], i18n::tr("Definir cárcel", "Define jail"), "name", SetJailCmd), "pumpkin_essentials:setjail")?;
    reg(ctx, one_arg(&["deljail"], i18n::tr("Eliminar cárcel", "Delete jail"), "name", DelJailCmd), "pumpkin_essentials:deljail")?;
    reg(ctx, no_arg(&["jails"], i18n::tr("Listar cárceles", "List jails"), JailsCmd), "pumpkin_essentials:jails")?;
    Ok(())
}

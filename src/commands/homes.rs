use crate::{
    cmdutil::{button, fail, is_op, name_arg, name_arg_or, need_op, no_arg, plain, player_of, reg, unavailable, word, CmdResult},
    host::{self, State, with_state},
    i18n,
    logic::{self, CdKind, RenameHome, SetHome, TpKind},
    store::{self, Location, PlayerHomes},
};
use pumpkin_plugin_api::{
    command::{CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    common::NamedColor,
    permission::{Permission, PermissionDefault},
    text::TextComponent,
    Context, Player, Server,
};
use tracing::info;

/// Mutate homes.json atomically: edit a copy, save it, then commit it to memory.
fn mutate_homes<R>(state: &mut State, f: impl FnOnce(&mut store::HomesFile) -> R) -> Result<R, String> {
    let mut copy = state.homes.clone();
    let r = f(&mut copy);
    store::save_json(&state.homes_path(), &copy).map_err(|e| {
        tracing::error!("essentials: could not save homes.json: {e}");
        i18n::tr("No se pudieron guardar los datos; el cambio NO se aplicó.", "Could not save data; the change was NOT applied.").to_string()
    })?;
    state.homes = copy;
    Ok(r)
}

fn do_sethome(sender: &CommandSender, name: String) -> CmdResult {
    let p = player_of(sender)?;
    let (key, op) = (host::key_of(&p), is_op(sender));
    let loc = host::location_of(&p);
    let pname = p.get_name();
    let res = with_state(|s| {
        let uuid = logic::uuid_string(key);
        match logic::check_set_home(&s.cfg, s.homes.players.get(&uuid), &name, op) {
            SetHome::LimitReached(l) => Ok(Err(l)),
            kind => {
                let (n, loc2, pn) = (name.clone(), loc, pname);
                mutate_homes(s, move |h| {
                    let e = h.players.entry(uuid).or_default();
                    e.name_last_seen = pn;
                    e.homes.insert(n, loc2);
                })?;
                Ok(Ok(kind))
            }
        }
    })
    .ok_or_else(unavailable)?;
    match res.map_err(|e: String| fail(&e))? {
        Err(limit) => {
            let limit_s = limit.to_string();
            Err(fail(&i18n::trf("Has alcanzado tu límite de casas ({}). Borra una con /delhome primero.", "You have reached your home limit ({}). Delete one with /delhome first.", &[&limit_s])))
        }
        Ok(kind) => {
            info!("essentials: {} /sethome {name}", p.get_name());
            let verb = if kind == SetHome::Overwritten { i18n::tr("actualizada", "updated") } else { i18n::tr("creada", "created") };
            host::ok(&p, &i18n::trf("Casa '{}' {}.", "Home '{}' {}.", &[&name, verb]));
            Ok(1)
        }
    }
}

struct SetHomeCmd {
    default: bool,
}
impl CommandHandler for SetHomeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let name = if self.default { name_arg_or(&args, "home")? } else { name_arg(&args)? };
        do_sethome(&sender, name)
    }
}

fn do_home(server: &Server, p: &Player, name: &str) -> CmdResult {
    let key = host::key_of(p);
    let left = crate::cmdutil::cooldown_left(key, CdKind::Home);
    if left > 0 {
        return Err(crate::cmdutil::cooldown_err(left));
    }
    let dest = with_state(|s| s.homes.players.get(&logic::uuid_string(key)).and_then(|e| e.homes.get(name).cloned()))
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail(&i18n::trf("No tienes una casa llamada '{}'.", "You do not have a home named '{}'.", &[&name])))?;
    info!("essentials: {} /home {name}", p.get_name());
    super::start_teleport(server, p, dest, i18n::trf("la casa '{}'", "the home '{}'", &[&name]), TpKind::Home);
    Ok(1)
}

struct HomeCmd {
    default: bool,
}
impl CommandHandler for HomeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let name = if self.default {
            let key = host::key_of(&p);
            let chosen = with_state(|s| {
                s.homes.players.get(&logic::uuid_string(key)).and_then(|e| {
                    if e.homes.contains_key("home") {
                        Some("home".to_string())
                    } else if e.homes.len() == 1 {
                        e.homes.keys().next().cloned()
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail(i18n::tr("Uso: /home <nombre>. No tienes una casa llamada 'home' y tienes más de una.", "Usage: /home <name>. You do not have a home named 'home' and you have more than one.")))?;
            chosen
        } else {
            name_arg(&args)?
        };
        do_home(&server, &p, &name)
    }
}

fn do_delhome(p: &Player, name: String) -> CmdResult {
    let key = host::key_of(p);
    let removed = with_state(|s| {
        let uuid = logic::uuid_string(key);
        if !s.homes.players.get(&uuid).is_some_and(|e| e.homes.contains_key(&name)) {
            return Ok(false);
        }
        let n = name.clone();
        mutate_homes(s, move |h| {
            if let Some(e) = h.players.get_mut(&uuid) {
                e.homes.remove(&n);
            }
        })
        .map(|_| true)
    })
    .ok_or_else(unavailable)?
    .map_err(|e| fail(&e))?;
    if !removed {
        return Err(fail(&i18n::trf("No tienes una casa llamada '{}'.", "You do not have a home named '{}'.", &[&name])));
    }
    info!("essentials: {} /delhome {name}", p.get_name());
    host::ok(p, &i18n::trf("Casa '{}' eliminada.", "Home '{}' deleted.", &[&name]));
    Ok(1)
}

struct DelHomeCmd {
    default: bool,
}
impl CommandHandler for DelHomeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let name = if self.default { name_arg_or(&args, "home")? } else { name_arg(&args)? };
        do_delhome(&p, name)
    }
}

struct HomesCmd;
impl CommandHandler for HomesCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let (key, op) = (host::key_of(&p), is_op(&sender));
        let (homes, limit) = with_state(|s| {
            let e = s.homes.players.get(&logic::uuid_string(key));
            let homes: Vec<(String, Location)> =
                e.map(|e| e.homes.iter().map(|(n, l)| (n.clone(), l.clone())).collect()).unwrap_or_default();
            (homes, logic::effective_limit(&s.cfg, e.and_then(|e| e.limit), op))
        })
        .ok_or_else(unavailable)?;
        let lim = limit.map_or(i18n::tr("sin límite", "unlimited").to_string(), |l| l.to_string());
        let count = homes.len().to_string();
        let mut text = TextComponent::text(&i18n::trf("Casas ({}/{}):", "Homes ({}/{}):", &[&count, &lim])).color_named(NamedColor::Gold);
        if homes.is_empty() {
            text = text.add_child(TextComponent::text(i18n::tr(" (ninguna)", " (none)")).color_named(NamedColor::Gray));
        }
        for (i, (name, loc)) in homes.iter().enumerate() {
            let sep = if i == 0 { " " } else { ", " };
            text = text.add_child(TextComponent::text(sep).color_named(NamedColor::Gray));
            text = text.add_child(button(name, NamedColor::Aqua, &format!("/home {name}"), i18n::tr("Ir a", "Go to")));
            text = text.add_child(
                TextComponent::text(&format!(" ({}, {:.1}, {:.1}, {:.1})", loc.world, loc.x, loc.y, loc.z))
                    .color_named(NamedColor::Gray),
            );
        }
        p.send_system_message(text, false);
        Ok(1)
    }
}

struct RenameHomeCmd;
impl CommandHandler for RenameHomeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let old = logic::validate_name(&plain(args.get_value("old"))).map_err(|e| fail(e.message()))?;
        let new = logic::validate_name(&plain(args.get_value("new"))).map_err(|e| fail(e.message()))?;
        let key = host::key_of(&p);
        let result = with_state(|s| {
            let uuid = logic::uuid_string(key);
            mutate_homes(s, |h| {
                let entry = h.players.entry(uuid.clone()).or_default();
                entry.name_last_seen = p.get_name();
                logic::rename_home(entry, &old, &new)
            })
        })
        .ok_or_else(unavailable)?
        .map_err(|e| fail(&e))?;
        match result {
            Ok(()) => {
                host::ok(&p, &i18n::trf("Casa '{}' renombrada a '{}'.", "Home '{}' renamed to '{}'.", &[&old, &new]));
                Ok(1)
            }
            Err(RenameHome::Missing) => Err(fail(&i18n::trf("No tienes una casa llamada '{}'.", "You do not have a home named '{}'.", &[&old]))),
            Err(RenameHome::Exists) => Err(fail(&i18n::trf("Ya tienes una casa llamada '{}'.", "You already have a home named '{}'.", &[&new]))),
            Err(RenameHome::Same) => Err(fail(i18n::tr("El nuevo nombre debe ser diferente.", "The new name must be different."))),
        }
    }
}

struct SetHomeLimitCmd;
impl CommandHandler for SetHomeLimitCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        need_op(&sender)?;
        let target = plain(args.get_value("player"));
        let n: u32 = plain(args.get_value("n"))
            .parse()
            .map_err(|_| fail(i18n::tr("Uso: /sethomelimit <jugador> <n>  (n = 0 o más)", "Usage: /sethomelimit <player> <n>  (n = 0 or more)")))?;
        let online = server.get_player_by_name(&target);
        let res = with_state(|s| {
            let uuid = match &online {
                Some(p) => logic::uuid_string(host::key_of(p)),
                None => s.homes.find_by_name(&target).cloned().ok_or_else(|| i18n::trf("Jugador desconocido: '{}'.", "Unknown player: '{}'.", &[&target]))?,
            };
            let name = online.as_ref().map_or_else(|| target.clone(), |p| p.get_name());
            mutate_homes(s, move |h| {
                let e = h.players.entry(uuid).or_insert_with(PlayerHomes::default);
                e.name_last_seen = name;
                e.limit = Some(n);
            })
        })
        .ok_or_else(unavailable)?;
        res.map_err(|e| fail(&e))?;
        info!("essentials: {} /sethomelimit {target} {n}", sender.get_name());
        let n_s = n.to_string();
        sender.send_message(TextComponent::text(&i18n::trf("Límite de casas de {} fijado en {}.", "Home limit for {} set to {}.", &[&target, &n_s])));
        Ok(1)
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    for (n, d) in [
        ("home", "Usar /sethome, /home, /delhome, /homes"),
        ("renamehome", "Renombrar una de tus casas"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Allow,
            children: vec![],
        })?;
    }
    let home = "pumpkin_essentials:home";
    reg(
        ctx,
        Command::new(&["sethome".to_string()], i18n::tr("Guarda una casa aquí (por defecto: home)", "Save a home here (default: home)"))
            .execute(SetHomeCmd { default: true })
            .then(word("name").execute(SetHomeCmd { default: false })),
        home,
    )?;
    reg(
        ctx,
        Command::new(&["home".to_string()], i18n::tr("Teletransporta a una casa (por defecto: home)", "Teleport to a home (default: home)"))
            .execute(HomeCmd { default: true })
            .then(word("name").execute(HomeCmd { default: false })),
        home,
    )?;
    reg(
        ctx,
        Command::new(&["delhome".to_string()], i18n::tr("Elimina una casa (por defecto: home)", "Delete a home (default: home)"))
            .execute(DelHomeCmd { default: true })
            .then(word("name").execute(DelHomeCmd { default: false })),
        home,
    )?;
    reg(ctx, no_arg(&["homes", "listhomes"], i18n::tr("Lista tus casas", "List your homes"), HomesCmd), home)?;
    reg(
        ctx,
        Command::new(&["renamehome".to_string()], i18n::tr("Renombra una de tus casas", "Rename one of your homes"))
            .then(word("old").then(word("new").execute(RenameHomeCmd))),
        "pumpkin_essentials:renamehome",
    )?;
    reg(
        ctx,
        Command::new(&["sethomelimit".to_string()], i18n::tr("Fija el límite de casas de un jugador (op)", "Set a player's home limit (op)"))
            .then(word("player").then(word("n").execute(SetHomeLimitCmd))),
        "pumpkin_essentials:admin",
    )?;
    Ok(())
}

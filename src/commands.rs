//! Command handlers and the shared teleport glue (also used by the tick task).

use crate::{
    extras::{self, AfkInfo},
    host::{self, State, with_state},
    logic::{self, CdKind, RenameHome, SetHome, TpaKind, TpKind, Warmup},
    state::{AfkState, FlyState},
    store::{self, Location, PlayerHomes, Warp},
};
use pumpkin_plugin_api::{
    Context, Player, Screen, Server,
    command::{Arg, ArgumentType, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType},
    commands::{Command, CommandHandler},
    common::NamedColor,
    gui::Gui,
    text::TextComponent,
};
use pumpkin_plugin_api::wit::pumpkin::plugin::command::PermissionLevel as CmdLevel;
use pumpkin_plugin_api::permission::{Permission, PermissionDefault, PermissionLevel};
use tracing::info;

type CmdResult = Result<i32, CommandError>;

fn fail(msg: &str) -> CommandError {
    CommandError::CommandFailed(TextComponent::text(msg))
}

fn plain(arg: Arg) -> String {
    match arg {
        Arg::Simple(s) | Arg::Msg(s) => s,
        _ => String::new(),
    }
}

fn greedy_arg(args: &ConsumedArgs, name: &str) -> String {
    plain(args.get_value(name))
}

fn player_of(sender: &CommandSender) -> Result<Player, CommandError> {
    sender.as_player().ok_or_else(|| fail("Players only."))
}

fn is_op(sender: &CommandSender) -> bool {
    sender.has_permission_level(CmdLevel::Three)
}

fn need_op(sender: &CommandSender) -> Result<(), CommandError> {
    if is_op(sender) { Ok(()) } else { Err(CommandError::PermissionDenied) }
}

/// A command node granted to ops may target another player only at gamemaster level.
fn resolve_target(sender: &CommandSender, server: &Server, me: &Player, name: &str) -> Result<Player, CommandError> {
    let target = server.get_player_by_name(name).ok_or_else(|| fail(&format!("Player '{name}' is not online.")))?;
    if host::key_of(&target) != host::key_of(me) && !sender.has_permission_level(CmdLevel::Two) {
        return Err(CommandError::PermissionDenied);
    }
    Ok(target)
}

fn unavailable() -> CommandError {
    fail("EssentialsFusion is unavailable. Tell an admin.")
}

fn name_arg(args: &ConsumedArgs) -> Result<String, CommandError> {
    logic::validate_name(&plain(args.get_value("name"))).map_err(|e| fail(e.message()))
}

fn name_arg_or(args: &ConsumedArgs, default: &str) -> Result<String, CommandError> {
    let raw = plain(args.get_value("name"));
    let chosen = if raw.trim().is_empty() { default } else { &raw };
    logic::validate_name(chosen).map_err(|e| fail(e.message()))
}

// ---- teleport glue -----------------------------------------------------------

/// Do the teleport now: record /back, move, notify, start the /home cooldown.
pub fn complete_teleport(server: &Server, p: &Player, key: u128, dest: &Location, label: &str, kind: TpKind) {
    let before = host::location_of(p);
    match host::teleport_to(server, p, dest) {
        Ok(()) => {
            let now = host::now_ms();
            with_state(|s| {
                s.backs.record(key, before.clone());
                s.persisted.back.insert(logic::uuid_string(key), before);
                s.save_persisted();
                if kind == TpKind::Home {
                    let secs = s.cfg.cooldown_secs;
                    s.cooldowns.start(key, CdKind::Home, now, secs);
                }
            });
            p.show_actionbar(TextComponent::text(""));
            host::ok(p, &format!("Teleported to {label}."));
            info!("essentials: {} teleported ({kind:?}) to {label}", p.get_name());
        }
        Err(e) => host::err(p, &e),
    }
}

/// Begin a teleport: instant if warmup is 0, otherwise a cancellable countdown.
pub(crate) fn start_teleport(server: &Server, p: &Player, dest: Location, label: String, kind: TpKind) {
    let key = host::key_of(p);
    let now = host::now_ms();
    let Some(warmup) = with_state(|s| s.cfg.warmup_secs) else {
        host::err(p, "EssentialsFusion is unavailable.");
        return;
    };
    if warmup == 0 {
        with_state(|s| s.warmups.cancel(key));
        complete_teleport(server, p, key, &dest, &label, kind);
        return;
    }
    let w = Warmup {
        start: p.get_position(),
        entity_id: host::entity_id_of(p),
        due_ms: now + warmup * 1000,
        dest,
        label: label.clone(),
        kind,
        shown: None,
    };
    let replaced = with_state(|s| s.warmups.begin(key, w)).unwrap_or(false);
    if replaced {
        host::info(p, "Previous teleport cancelled.");
    }
    host::info(p, &format!("Teleporting to {label} in {warmup}s. Don't move."));
}

fn cooldown_left(key: u128, kind: CdKind) -> u64 {
    let now = host::now_ms();
    with_state(|s| s.cooldowns.remaining(key, kind, now)).unwrap_or(0)
}

fn cooldown_err(left: u64) -> CommandError {
    fail(&format!("You must wait {left}s before using that again."))
}

/// Mutate homes.json atomically: edit a copy, save it, then commit it to memory.
fn mutate_homes<R>(s: &mut State, f: impl FnOnce(&mut store::HomesFile) -> R) -> Result<R, String> {
    let mut copy = s.homes.clone();
    let r = f(&mut copy);
    store::save_json(&s.homes_path(), &copy).map_err(|e| {
        tracing::error!("essentials: could not save homes.json: {e}");
        "Could not save data; change NOT applied.".to_string()
    })?;
    s.homes = copy;
    Ok(r)
}

fn mutate_warps<R>(s: &mut State, f: impl FnOnce(&mut store::WarpsFile) -> R) -> Result<R, String> {
    let mut copy = s.warps.clone();
    let r = f(&mut copy);
    store::save_json(&s.warps_path(), &copy).map_err(|e| {
        tracing::error!("essentials: could not save warps.json: {e}");
        "Could not save data; change NOT applied.".to_string()
    })?;
    s.warps = copy;
    Ok(r)
}

/// Persist config.json atomically after a command changes a setting.
pub(crate) fn save_cfg(s: &State) -> Result<(), String> {
    store::save_json(&s.dir.join("config.json"), &s.cfg).map_err(|e| format!("Could not save config.json: {e}"))
}

fn button(label: &str, color: NamedColor, run: &str, hover: &str) -> TextComponent {
    TextComponent::text(label)
        .color_named(color)
        .click_run_command(run)
        .hover_show_text(TextComponent::text(hover))
}

fn clickable_list(head: &str, names: &[String], cmd: &str, hover_verb: &str) -> TextComponent {
    let mut t = TextComponent::text(head).color_named(NamedColor::Gold);
    if names.is_empty() {
        return t.add_child(TextComponent::text(" (none)").color_named(NamedColor::Gray));
    }
    for (i, n) in names.iter().enumerate() {
        let sep = if i == 0 { " " } else { ", " };
        t = t.add_child(TextComponent::text(sep).color_named(NamedColor::Gray));
        t = t.add_child(button(n, NamedColor::Aqua, &format!("{cmd} {n}"), &format!("{hover_verb} {n}")));
    }
    t
}

// ---- homes ---------------------------------------------------------------------

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
        Err(limit) => Err(fail(&format!("You have reached your home limit ({limit}). Delete one with /delhome first."))),
        Ok(kind) => {
            info!("essentials: {} /sethome {name}", p.get_name());
            let verb = if kind == SetHome::Overwritten { "updated" } else { "set" };
            host::ok(&p, &format!("Home '{name}' {verb}."));
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
    let left = cooldown_left(key, CdKind::Home);
    if left > 0 {
        return Err(cooldown_err(left));
    }
    let dest = with_state(|s| s.homes.players.get(&logic::uuid_string(key)).and_then(|e| e.homes.get(name).cloned()))
        .ok_or_else(unavailable)?
        .ok_or_else(|| fail(&format!("You have no home named '{name}'.")))?;
    info!("essentials: {} /home {name}", p.get_name());
    start_teleport(server, p, dest, format!("home '{name}'"), TpKind::Home);
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
            .ok_or_else(|| fail("Usage: /home <name>. You have no home named 'home' and more than one home."))?;
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
        return Err(fail(&format!("You have no home named '{name}'.")));
    }
    info!("essentials: {} /delhome {name}", p.get_name());
    host::ok(p, &format!("Home '{name}' deleted."));
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
        let lim = limit.map_or("unlimited".to_string(), |l| l.to_string());
        let mut text = TextComponent::text(&format!("Homes ({}/{}):", homes.len(), lim)).color_named(NamedColor::Gold);
        if homes.is_empty() {
            text = text.add_child(TextComponent::text(" (none)").color_named(NamedColor::Gray));
        }
        for (i, (name, loc)) in homes.iter().enumerate() {
            let sep = if i == 0 { " " } else { ", " };
            text = text.add_child(TextComponent::text(sep).color_named(NamedColor::Gray));
            text = text.add_child(button(name, NamedColor::Aqua, &format!("/home {name}"), "Teleport to"));
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
                host::ok(&p, &format!("Home '{old}' renamed to '{new}'."));
                Ok(1)
            }
            Err(RenameHome::Missing) => Err(fail(&format!("You have no home named '{old}'."))),
            Err(RenameHome::Exists) => Err(fail(&format!("You already have a home named '{new}'."))),
            Err(RenameHome::Same) => Err(fail("The new name must be different.")),
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
            .map_err(|_| fail("Usage: /sethomelimit <player> <n>  (n = 0 or more)"))?;
        let online = server.get_player_by_name(&target);
        let res = with_state(|s| {
            let uuid = match &online {
                Some(p) => logic::uuid_string(host::key_of(p)),
                None => s.homes.find_by_name(&target).cloned().ok_or_else(|| format!("Unknown player '{target}'."))?,
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
        sender.send_message(TextComponent::text(&format!("Home limit for {target} set to {n}.")));
        Ok(1)
    }
}

// ---- warps ---------------------------------------------------------------------

struct SetWarpCmd;
impl CommandHandler for SetWarpCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        need_op(&sender)?;
        let p = player_of(&sender)?;
        let name = name_arg(&args)?;
        let warp = Warp { loc: host::location_of(&p), created_by: p.get_name() };
        let n = name.clone();
        with_state(|s| mutate_warps(s, move |w| { w.warps.insert(n, warp); }))
            .ok_or_else(unavailable)?
            .map_err(|e| fail(&e))?;
        info!("essentials: {} /setwarp {name}", p.get_name());
        host::ok(&p, &format!("Warp '{name}' set."));
        Ok(1)
    }
}

struct DelWarpCmd;
impl CommandHandler for DelWarpCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        need_op(&sender)?;
        let name = name_arg(&args)?;
        let n = name.clone();
        let removed = with_state(|s| {
            if !s.warps.warps.contains_key(&n) {
                return Ok(false);
            }
            mutate_warps(s, move |w| { w.warps.remove(&n); }).map(|_| true)
        })
        .ok_or_else(unavailable)?
        .map_err(|e| fail(&e))?;
        if !removed {
            return Err(fail(&format!("No warp named '{name}'.")));
        }
        info!("essentials: {} /delwarp {name}", sender.get_name());
        sender.send_message(TextComponent::text(&format!("Warp '{name}' deleted.")));
        Ok(1)
    }
}

struct WarpCmd;
impl CommandHandler for WarpCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let name = name_arg(&args)?;
        let dest = with_state(|s| s.warps.warps.get(&name).map(|w| w.loc.clone()))
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail(&format!("No warp named '{name}'.")))?;
        info!("essentials: {} /warp {name}", p.get_name());
        start_teleport(&server, &p, dest, format!("warp '{name}'"), TpKind::Warp);
        Ok(1)
    }
}

struct WarpsCmd;
impl CommandHandler for WarpsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let names = with_state(|s| s.warps.warps.keys().cloned().collect::<Vec<_>>()).ok_or_else(unavailable)?;
        p.send_system_message(clickable_list("Warps:", &names, "/warp", "Teleport to warp"), false);
        Ok(1)
    }
}

// ---- tpa -----------------------------------------------------------------------

/// Execute an accepted TPA request. `accepter` is the player who said yes.
fn perform_accept(server: &Server, accepter: &Player, req: logic::TpaRequest) -> CmdResult {
    let requester = server.get_player_by_uuid(host::key_to_uuid(req.requester));
    let Some(r) = requester else {
        return Err(fail(&format!("{} is no longer online.", req.requester_name)));
    };
    host::ok(accepter, &format!("Accepted the request from {}.", req.requester_name));
    host::ok(&r, &format!("{} accepted your request.", accepter.get_name()));
    let (mover, destination, label) = match req.kind {
        TpaKind::ToTarget => (&r, accepter, accepter.get_name()),
        TpaKind::ToRequester => (accepter, &r, r.get_name()),
    };
    start_teleport(server, mover, host::location_of(destination), label, TpKind::Tpa);
    Ok(1)
}

fn send_tpa_request(sender: &CommandSender, server: &Server, args: &ConsumedArgs, here: bool) -> CmdResult {
    let p = player_of(sender)?;
    let target_name = plain(args.get_value("player"));
    let target = server
        .get_player_by_name(&target_name)
        .ok_or_else(|| fail(&format!("Player '{target_name}' is not online.")))?;
    let (key, tkey) = (host::key_of(&p), host::key_of(&target));
    if key == tkey {
        return Err(fail("You cannot send a teleport request to yourself."));
    }
    let (blocked, auto) = with_state(|s| {
        let uuid = logic::uuid_string(tkey);
        (
            *s.persisted.tpa_off.get(&uuid).unwrap_or(&false),
            *s.persisted.tpa_auto.get(&uuid).unwrap_or(&false),
        )
    })
    .ok_or_else(unavailable)?;
    if blocked {
        return Err(fail("That player has teleport requests disabled (/tptoggle)."));
    }
    let left = cooldown_left(key, CdKind::Tpa);
    if left > 0 {
        return Err(cooldown_err(left));
    }
    let now = host::now_ms();
    let (me, them) = (p.get_name(), target.get_name());
    let expire_secs = with_state(|s| {
        let exp = now + s.cfg.tpa_expire_secs * 1000;
        if here {
            s.tpas.send_here(key, &me, tkey, &them, exp);
        } else {
            s.tpas.send(key, &me, tkey, &them, exp);
        }
        let secs = s.cfg.cooldown_secs;
        s.cooldowns.start(key, CdKind::Tpa, now, secs);
        s.cfg.tpa_expire_secs
    })
    .ok_or_else(unavailable)?;
    info!("essentials: {} /{} {them}", me, if here { "tpahere" } else { "tpa" });
    if auto {
        let accepted = with_state(|s| s.tpas.take(tkey, Some(&me), host::now_ms())).ok_or_else(unavailable)?;
        if let Some(req) = accepted {
            host::ok(&target, &format!("Auto-accepted request from {me}."));
            return perform_accept(server, &target, req);
        }
    }
    host::ok(&p, &format!("Teleport request sent to {them}. It expires in {expire_secs}s."));
    let verb = if here { "wants you to teleport to them" } else { "wants to teleport to you" };
    let msg = TextComponent::text(&format!("{me} {verb}. "))
        .color_named(NamedColor::Gold)
        .add_child(button("[Accept]", NamedColor::Green, &format!("/tpaccept {me}"), "Accept the request"))
        .add_child(TextComponent::text(" "))
        .add_child(button("[Deny]", NamedColor::Red, &format!("/tpdeny {me}"), "Deny the request"));
    target.send_system_message(msg, false);
    Ok(1)
}

struct TpaCmd;
impl CommandHandler for TpaCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        send_tpa_request(&sender, &server, &args, false)
    }
}

struct TpaHereCmd;
impl CommandHandler for TpaHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        send_tpa_request(&sender, &server, &args, true)
    }
}

struct TpReply {
    accept: bool,
    has_arg: bool,
}
impl CommandHandler for TpReply {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let from = if self.has_arg { Some(plain(args.get_value("player"))) } else { None };
        let (tkey, now) = (host::key_of(&p), host::now_ms());
        let req = with_state(|s| s.tpas.take(tkey, from.as_deref(), now))
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail("You have no pending teleport request."))?;
        let verb = if self.accept { "accept" } else { "deny" };
        info!("essentials: {} /tp{verb} {}", p.get_name(), req.requester_name);
        if !self.accept {
            let requester = server.get_player_by_uuid(host::key_to_uuid(req.requester));
            host::ok(&p, &format!("Denied the request from {}.", req.requester_name));
            if let Some(r) = requester {
                host::err(&r, &format!("{} denied your teleport request.", p.get_name()));
            }
            return Ok(1);
        }
        perform_accept(&server, &p, req)
    }
}

// ---- back ----------------------------------------------------------------------

struct BackCmd;
impl CommandHandler for BackCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let key = host::key_of(&p);
        let dest = with_state(|s| s.backs.get(key).cloned())
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail("No previous location to go back to."))?;
        info!("essentials: {} /back", p.get_name());
        start_teleport(&server, &p, dest, "your previous location".into(), TpKind::Back);
        Ok(1)
    }
}

// ---- gamemode -------------------------------------------------------------------

struct FixedGmCmd {
    mode: &'static str,
    target: bool,
}
impl CommandHandler for FixedGmCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = if self.target {
            resolve_target(&sender, &server, &me, &plain(args.get_value("player")))?
        } else {
            me
        };
        let mode = extras::parse_mode(self.mode).ok_or_else(|| fail("Unknown game mode."))?;
        return_gamemode(&sender, &target, mode)
    }
}

struct GmCmd {
    target: bool,
}
impl CommandHandler for GmCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = plain(args.get_value("mode"));
        let mode = extras::parse_mode(&raw)
            .ok_or_else(|| fail("Usage: /gm <survival|creative|adventure|spectator> [player] (or 0/1/2/3, s/c/a/sp)."))?;
        let target = if self.target {
            resolve_target(&sender, &server, &me, &plain(args.get_value("player")))?
        } else {
            me
        };
        return_gamemode(&sender, &target, mode)
    }
}

fn return_gamemode(sender: &CommandSender, target: &Player, mode: pumpkin_plugin_api::common::GameMode) -> CmdResult {
    if target.set_gamemode(mode) {
        let name = target.get_name();
        host::ok(target, &format!("Game mode set to {}.", extras::mode_name(&mode)));
        if host::key_of(target) != sender.as_player().map_or(host::key_of(target), |p| host::key_of(&p)) {
            sender.send_message(TextComponent::text(&format!("{name}'s game mode set to {}.", extras::mode_name(&mode))));
        }
        info!("essentials: {} gamemode -> {}", name, extras::mode_name(&mode));
        Ok(1)
    } else {
        Err(fail("Could not change the game mode."))
    }
}

// ---- fly / flyspeed -------------------------------------------------------------

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
        host::ok(&target, &format!("Flight {verb}."));
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
        let first = tokens.next().ok_or_else(|| fail("Usage: /flyspeed [player] <0.0-10.0>"))?;
        let rest: Vec<&str> = tokens.collect();
        let (target_name, speed_raw) = if extras::parse_fly_speed(first).is_ok() {
            (None, first)
        } else if rest.len() == 1 && extras::parse_fly_speed(rest[0]).is_ok() {
            (Some(first), rest[0])
        } else {
            return Err(fail("Usage: /flyspeed [player] <0.0-10.0>"));
        };
        let value = extras::parse_fly_speed(speed_raw).map_err(|e| fail(&e))?;
        let target = match target_name {
            Some(name) => resolve_target(&sender, &server, &me, name)?,
            None => me,
        };
        let unit = with_state(|s| s.cfg.flyspeed_unit).ok_or_else(unavailable)?;
        let engine_speed = extras::engine_fly_speed(value, unit);
        target.set_fly_speed(engine_speed);
        let speed_now = target.get_abilities().fly_speed;
        with_state(|s| {
            let entry = s.persisted.fly.entry(logic::uuid_string(host::key_of(&target))).or_default();
            entry.fly_speed = speed_now;
            s.save_persisted();
        });
        let name = target.get_name();
        host::ok(&target, &format!("Fly speed set to {value:.2}."));
        info!("essentials: {} flyspeed {value:.2}", name);
        Ok(1)
    }
}

// ---- afk -----------------------------------------------------------------------

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
        let (now_afk, broadcast, show_tab, nick) = with_state(|s| {
            let was = s.afks.remove(key).is_some();
            if was {
                s.persisted.afk.remove(&uuid);
            } else {
                s.afks.set(key, AfkInfo { reason: reason.clone(), since_ms: now, pos });
                s.persisted.afk.insert(
                    uuid.clone(),
                    AfkState { active: true, reason: reason.clone(), since_ms: now },
                );
            }
            s.save_persisted();
            let nick = s.persisted.nicks.get(&uuid).cloned();
            (!was, s.cfg.afk_broadcast, s.cfg.afk_tab_name, nick)
        })
        .ok_or_else(unavailable)?;
        if show_tab {
            crate::events::apply_display_name(&target, nick.as_deref(), now_afk);
        }
        if now_afk {
            let suffix = if reason.is_empty() { String::new() } else { format!(": {reason}") };
            if broadcast {
                server.broadcast(&format!("* {name} is now AFK{suffix}."));
            }
        } else if broadcast {
            server.broadcast(&format!("* {name} is no longer AFK."));
        }
        let word = if now_afk { "AFK" } else { "no longer AFK" };
        host::ok(&target, &format!("You are now {word}."));
        info!("essentials: {} is {word}", name);
        Ok(1)
    }
}

// ---- EssentialsX-style utility commands ----------------------------------------

fn optional_target(
    sender: &CommandSender,
    server: &Server,
    me: &Player,
    args: &ConsumedArgs,
    has_target: bool,
) -> Result<Player, CommandError> {
    if has_target { resolve_target(sender, server, me, &plain(args.get_value("player"))) } else {
        sender.as_player().ok_or_else(|| fail("Players only."))
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
            sender.send_message(TextComponent::text(&format!("Healed {}.", target.get_name())));
        } else {
            host::ok(&target, "You have been healed.");
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
            sender.send_message(TextComponent::text(&format!("Fed {}.", target.get_name())));
        } else {
            host::ok(&target, "You have been fed.");
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
        let first = tokens.next().ok_or_else(|| fail("Usage: /speed [player] <0.1-10.0>"))?;
        let rest: Vec<&str> = tokens.collect();
        let (target, speed_raw) = if extras::parse_walk_speed(first).is_ok() {
            (me, first)
        } else if rest.len() == 1 && extras::parse_walk_speed(rest[0]).is_ok() {
            (resolve_target(&sender, &server, &me, first)?, rest[0])
        } else {
            return Err(fail("Usage: /speed [player] <0.1-10.0>"));
        };
        let value = extras::parse_walk_speed(speed_raw).map_err(|e| fail(&e))?;
        target.set_walk_speed(extras::engine_walk_speed(value));
        host::ok(&target, &format!("Walking speed set to {value:.2}."));
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
                let level = extras::parse_level(value).map_err(|e| fail(&e))?;
                (resolve_target(&sender, &server, &me, name)?, Some(level))
            }
            _ => return Err(fail("Usage: /exp [player] [level]")),
        };
        let name = target.get_name();
        match level {
            Some(n) => {
                target.set_experience_level(n);
                let msg = format!("Experience level for {name} set to {n}.");
                if host::key_of(&target) == me_key {
                    host::ok(&target, &msg);
                } else {
                    sender.send_message(TextComponent::text(&msg));
                }
            }
            None => {
                let current = target.get_experience_level();
                let msg = if host::key_of(&target) == me_key {
                    format!("Your experience level is {current}.")
                } else {
                    format!("{name}'s experience level is {current}.")
                };
                sender.send_message(TextComponent::text(&msg));
            }
        }
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
            "{} — world: {}, x: {x:.2}, y: {y:.2}, z: {z:.2}, yaw: {yaw:.1}, pitch: {pitch:.1}",
            target.get_name(),
            world
        );
        if world == me.get_world().get_name() {
            let (mx, my, mz) = me.get_position();
            let distance = ((x - mx).powi(2) + (y - my).powi(2) + (z - mz).powi(2)).sqrt();
            msg.push_str(&format!(" — distance: {distance:.2}"));
        }
        sender.send_message(TextComponent::text(&msg));
        Ok(1)
    }
}

struct SpawnCmd;
impl CommandHandler for SpawnCmd {
    fn handle(&self, sender: CommandSender, server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let world = p.get_world();
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
        let dest = with_state(|s| s.cfg.spawns.get(&world_name).cloned())
            .ok_or_else(unavailable)?
            .unwrap_or(fallback);
        info!("essentials: {} /spawn", p.get_name());
        start_teleport(&server, &p, dest, "spawn".into(), TpKind::Spawn);
        Ok(1)
    }
}

struct SetSpawnCmd;
impl CommandHandler for SetSpawnCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        need_op(&sender)?;
        let p = player_of(&sender)?;
        let loc = host::location_of(&p);
        with_state(|s| {
            s.cfg.spawns.insert(loc.world.clone(), loc.clone());
            save_cfg(s)
        })
        .ok_or_else(unavailable)?
        .map_err(|e| fail(&e))?;
        host::ok(&p, &format!("Spawn for '{}' set to your position.", loc.world));
        info!("essentials: {} /setspawn in {}", p.get_name(), loc.world);
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
            return Err(fail("Could not find a safe top block in this column."));
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
        start_teleport(&server, &target, dest, "the top".into(), TpKind::Top);
        Ok(1)
    }
}

// ---- staff teleports -------------------------------------------------------------

struct TpHereCmd;
impl CommandHandler for TpHereCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Two) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let target = server
            .get_player_by_name(&plain(args.get_value("player")))
            .ok_or_else(|| fail("That player is not online."))?;
        start_teleport(&server, &target, host::location_of(&me), me.get_name(), TpKind::Tphere);
        host::ok(&me, &format!("Teleported {} to you.", target.get_name()));
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
            start_teleport(&server, &other, dest.clone(), label.clone(), TpKind::Tpall);
            count += 1;
        }
        host::ok(&me, &format!("Teleport request sent to {count} player(s)."));
        Ok(1)
    }
}

struct TpToggleCmd;
impl CommandHandler for TpToggleCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let key = host::key_of(&p);
        let now_blocked = with_state(|s| {
            let uuid = logic::uuid_string(key);
            let blocked = !s.persisted.tpa_off.get(&uuid).copied().unwrap_or(false);
            s.persisted.tpa_off.insert(uuid, blocked);
            s.save_persisted();
            blocked
        })
        .ok_or_else(unavailable)?;
        host::ok(&p, if now_blocked { "Teleport requests are now blocked." } else { "Teleport requests are now allowed." });
        Ok(1)
    }
}

// ---- inventory views ------------------------------------------------------------

fn view_inventory(
    viewer: &Player,
    title: String,
    source: &pumpkin_plugin_api::inventory::Inventory,
    slots: u32,
) -> Result<(), String> {
    let gui = Gui::new(Screen::Generic9x4, TextComponent::text(&title));
    let target = gui.get_inventory();
    let count = slots.min(target.get_size()).min(source.get_size());
    for slot in 0..count {
        let item = source.get_item(slot);
        target.set_item(slot, item);
    }
    gui.set_allow_grab_items(false);
    gui.set_allow_put_items(false);
    viewer.open_gui(gui);
    Ok(())
}

struct InvSeeCmd;
impl CommandHandler for InvSeeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Two) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let target = server
            .get_player_by_name(&plain(args.get_value("player")))
            .ok_or_else(|| fail("That player is not online."))?;
        let inv = target.get_inventory().as_inventory();
        view_inventory(&me, format!("{}'s inventory", target.get_name()), &inv, 36).map_err(|e| fail(&e))?;
        Ok(1)
    }
}

struct EnderChestCmd {
    target: bool,
}
impl CommandHandler for EnderChestCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = optional_target(&sender, &server, &me, &args, self.target)?;
        if host::key_of(&target) == host::key_of(&me) {
            me.open_ender_chest();
            return Ok(1);
        }
        let inv = target.get_ender_chest();
        view_inventory(&me, format!("{}'s ender chest", target.get_name()), &inv, 27).map_err(|e| fail(&e))?;
        Ok(1)
    }
}
// ---- registration ----------------------------------------------------------------

fn word(name: &str) -> CommandNode {
    CommandNode::argument(name, &ArgumentType::String(StringType::SingleWord))
}

fn greedy(name: &str) -> CommandNode {
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

pub fn register_all(ctx: &Context) -> Result<(), String> {
    let allow = |node: &str, desc: &str| Permission {
        node: node.into(),
        description: desc.into(),
        default: PermissionDefault::Allow,
        children: vec![],
    };
    for (n, d) in [
        ("home", "Use /sethome, /home, /delhome, /homes"),
        ("warp", "Use /warp, /warps"),
        ("tpa", "Use /tpa, /tpaccept, /tpdeny"),
        ("back", "Use /back"),
    ] {
        ctx.register_permission(&allow(&format!("pumpkin_essentials:{n}"), d))?;
    }
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:admin".into(),
        description: "Manage warps and home limits".into(),
        default: PermissionDefault::Op(PermissionLevel::Three),
        children: vec![],
    })?;
    for (n, d) in [
        ("afk", "Use /afk"),
        ("spawn", "Teleport to spawn"),
        ("top", "Teleport to the top of a column"),
        ("tptoggle", "Block or allow incoming teleport requests"),
        ("tpahere", "Ask another player to teleport to you"),
        ("renamehome", "Rename one of your homes"),
    ] {
        ctx.register_permission(&allow(&format!("pumpkin_essentials:{n}"), d))?;
    }
    for (n, d) in [
        ("gamemode", "Use /gm and /gma, /gmc, /gms, /gmsp"),
        ("fly", "Use /fly"),
        ("flyspeed", "Use /flyspeed"),
        ("heal", "Use /heal"),
        ("feed", "Use /feed"),
        ("speed", "Use /speed"),
        ("exp", "Use /exp"),
        ("getpos", "Use /getpos"),
        ("tphere", "Teleport another player to you"),
        ("invsee", "View another player's inventory"),
        ("enderchest", "Open an ender chest"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    for (n, d) in [
        ("setspawn", "Set the spawn of the current world"),
        ("tpall", "Teleport every online player to you"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Three),
            children: vec![],
        })?;
    }

    let (home, warp, tpa, back, admin) = (
        "pumpkin_essentials:home",
        "pumpkin_essentials:warp",
        "pumpkin_essentials:tpa",
        "pumpkin_essentials:back",
        "pumpkin_essentials:admin",
    );
    let (gm, fly, flyspeed, afk) = (
        "pumpkin_essentials:gamemode",
        "pumpkin_essentials:fly",
        "pumpkin_essentials:flyspeed",
        "pumpkin_essentials:afk",
    );

    // Homes. No argument = "home" (or the only home when reading).
    reg(
        ctx,
        Command::new(&["sethome".to_string()], "Save a home here (default: home)")
            .execute(SetHomeCmd { default: true })
            .then(word("name").execute(SetHomeCmd { default: false })),
        home,
    )?;
    reg(
        ctx,
        Command::new(&["home".to_string()], "Teleport to a home (default: home)")
            .execute(HomeCmd { default: true })
            .then(word("name").execute(HomeCmd { default: false })),
        home,
    )?;
    reg(
        ctx,
        Command::new(&["delhome".to_string()], "Delete a home (default: home)")
            .execute(DelHomeCmd { default: true })
            .then(word("name").execute(DelHomeCmd { default: false })),
        home,
    )?;
    reg(ctx, no_arg(&["homes", "listhomes"], "List your homes", HomesCmd), home)?;
    reg(
        ctx,
        Command::new(&["sethomelimit".to_string()], "Set a player's home limit (op)")
            .then(word("player").then(word("n").execute(SetHomeLimitCmd))),
        admin,
    )?;

    // Warps.
    reg(ctx, one_arg(&["setwarp"], "Create or overwrite a warp (op)", "name", SetWarpCmd), admin)?;
    reg(ctx, one_arg(&["delwarp"], "Delete a warp (op)", "name", DelWarpCmd), admin)?;
    reg(
        ctx,
        Command::new(&["warp".to_string()], "Teleport to a warp, or list warps")
            .execute(WarpsCmd)
            .then(word("name").execute(WarpCmd)),
        warp,
    )?;
    reg(ctx, no_arg(&["warps"], "List warps", WarpsCmd), warp)?;

    // Teleports.
    reg(ctx, one_arg(&["tpa"], "Request to teleport to a player", "player", TpaCmd), tpa)?;
    for (label, accept) in [("tpaccept", true), ("tpdeny", false)] {
        let cmd = no_arg(&[label], "Answer a teleport request", TpReply { accept, has_arg: false })
            .then(word("player").execute(TpReply { accept, has_arg: true }));
        reg(ctx, cmd, tpa)?;
    }
    reg(ctx, no_arg(&["back"], "Return to your previous location", BackCmd), back)?;

    // Game modes.
    for (name, mode) in [("gma", "a"), ("gmc", "c"), ("gms", "s"), ("gmsp", "sp")] {
        reg(
            ctx,
            Command::new(&[name.to_string()], "Change game mode (optionally for another player)")
                .execute(FixedGmCmd { mode, target: false })
                .then(word("player").execute(FixedGmCmd { mode, target: true })),
            gm,
        )?;
    }
    reg(
        ctx,
        Command::new(&["gm".to_string()], "Change game mode").then(
            word("mode")
                .execute(GmCmd { target: false })
                .then(word("player").execute(GmCmd { target: true })),
        ),
        gm,
    )?;

    // Flight and AFK.
    reg(
        ctx,
        Command::new(&["fly".to_string()], "Toggle flight (optionally for another player)")
            .execute(FlyCmd { target: false })
            .then(word("player").execute(FlyCmd { target: true })),
        fly,
    )?;
    reg(
        ctx,
        Command::new(&["flyspeed".to_string()], "Set fly speed 0.0-10.0").then(greedy("args").execute(FlySpeedCmd)),
        flyspeed,
    )?;
    reg(
        ctx,
        Command::new(&["afk".to_string()], "Toggle AFK status, optionally with a reason").then(greedy("args").execute(AfkCmd)),
        afk,
    )?;
    // Iteration 2: EssentialsX-style utilities.
    reg(
        ctx,
        Command::new(&["heal".to_string()], "Heal yourself or another player")
            .execute(HealCmd { target: false })
            .then(word("player").execute(HealCmd { target: true })),
        "pumpkin_essentials:heal",
    )?;
    reg(
        ctx,
        Command::new(&["feed".to_string()], "Feed yourself or another player")
            .execute(FeedCmd { target: false })
            .then(word("player").execute(FeedCmd { target: true })),
        "pumpkin_essentials:feed",
    )?;
    reg(
        ctx,
        Command::new(&["speed".to_string()], "Set walk speed 0.1-10.0").then(greedy("args").execute(SpeedCmd)),
        "pumpkin_essentials:speed",
    )?;
    reg(
        ctx,
        Command::new(&["exp".to_string()], "Show or set an experience level").then(greedy("args").execute(ExpCmd)),
        "pumpkin_essentials:exp",
    )?;
    reg(
        ctx,
        Command::new(&["getpos".to_string()], "Show coordinates, world and rotation")
            .execute(GetPosCmd { target: false })
            .then(word("player").execute(GetPosCmd { target: true })),
        "pumpkin_essentials:getpos",
    )?;
    reg(ctx, no_arg(&["spawn"], "Teleport to spawn", SpawnCmd), "pumpkin_essentials:spawn")?;
    reg(ctx, no_arg(&["setspawn"], "Set this world's spawn (op)", SetSpawnCmd), "pumpkin_essentials:setspawn")?;
    reg(
        ctx,
        Command::new(&["top".to_string()], "Teleport to the highest block")
            .execute(TopCmd { target: false })
            .then(word("player").execute(TopCmd { target: true })),
        "pumpkin_essentials:top",
    )?;

    // Iteration 3: staff teleports, inventory views and home rename.
    reg(
        ctx,
        Command::new(&["renamehome".to_string()], "Rename one of your homes")
            .then(word("old").then(word("new").execute(RenameHomeCmd))),
        "pumpkin_essentials:renamehome",
    )?;
    reg(ctx, one_arg(&["tpahere"], "Ask a player to teleport to you", "player", TpaHereCmd), "pumpkin_essentials:tpahere")?;
    reg(ctx, one_arg(&["tphere"], "Teleport a player to you (op)", "player", TpHereCmd), "pumpkin_essentials:tphere")?;
    reg(ctx, no_arg(&["tptoggle"], "Block or allow teleport requests", TpToggleCmd), "pumpkin_essentials:tptoggle")?;
    reg(ctx, no_arg(&["tpall"], "Teleport everyone to you (op)", TpAllCmd), "pumpkin_essentials:tpall")?;
    reg(ctx, one_arg(&["invsee"], "View a player's inventory (op)", "player", InvSeeCmd), "pumpkin_essentials:invsee")?;
    reg(
        ctx,
        Command::new(&["enderchest".to_string()], "Open your ender chest, or view another player's")
            .execute(EnderChestCmd { target: false })
            .then(word("player").execute(EnderChestCmd { target: true })),
        "pumpkin_essentials:enderchest",
    )?;
    crate::iteration4::register_all(ctx)?;
    crate::iteration5::register_all(ctx)?;
    crate::iteration6::register_all(ctx)?;
    crate::iteration7::register_all(ctx)?;
    crate::iteration8::register_all(ctx)?;
    crate::iteration9::register_all(ctx)?;
    Ok(())
}

/// `Context::register_command` returns `()` in this API revision.
fn reg(ctx: &Context, command: Command, permission: &str) -> Result<(), String> {
    ctx.register_command(command, permission);
    Ok(())
}

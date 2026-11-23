//! Shared helpers for the command layer.
//!
//! Single copy of every utility the command modules need: error building,
//! argument decoding, node/command builders and small cross-domain helpers.
//! The domain modules under `commands/` import from here instead of
//! redefining these.

use crate::{
    extras,
    host::{self, with_state},
    i18n,
    logic,
};
use pumpkin_plugin_api::{
    command::{Arg, ArgumentType, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType},
    commands::{Command, CommandHandler},
    common::NamedColor,
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, ItemStack, Player, Server,
};

pub type CmdResult = Result<i32, CommandError>;

pub fn fail(msg: &str) -> CommandError {
    CommandError::CommandFailed(TextComponent::text(msg))
}

/// The command layer cannot run without the installed host state.
pub fn unavailable() -> CommandError {
    fail(i18n::msg(i18n::UNAVAILABLE))
}

pub fn plain(arg: Arg) -> String {
    match arg {
        Arg::Simple(s) | Arg::Msg(s) => s,
        _ => String::new(),
    }
}

pub fn greedy_arg(args: &ConsumedArgs, name: &str) -> String {
    plain(args.get_value(name))
}

pub fn say(sender: &CommandSender, msg: &str) {
    sender.send_message(TextComponent::text(msg));
}

pub fn player_of(sender: &CommandSender) -> Result<Player, CommandError> {
    sender.as_player().ok_or_else(|| fail(i18n::msg(i18n::ONLY_PLAYERS)))
}

pub fn has_level(sender: &CommandSender, level: CmdLevel) -> bool {
    sender.has_permission_level(level)
}

pub fn is_op(sender: &CommandSender) -> bool {
    has_level(sender, CmdLevel::Three)
}

pub fn need_op(sender: &CommandSender) -> Result<(), CommandError> {
    if is_op(sender) { Ok(()) } else { Err(CommandError::PermissionDenied) }
}

pub fn word(name: &str) -> CommandNode {
    CommandNode::argument(name, &ArgumentType::String(StringType::SingleWord))
}

pub fn greedy_node(name: &str) -> CommandNode {
    CommandNode::argument(name, &ArgumentType::String(StringType::Greedy))
}

pub fn one_arg<H: CommandHandler + 'static>(names: &[&str], desc: &str, arg: &str, h: H) -> Command {
    let names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
    Command::new(&names, desc).then(word(arg).execute(h))
}

pub fn no_arg<H: CommandHandler + 'static>(names: &[&str], desc: &str, h: H) -> Command {
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    Command::new(&names, desc).execute(h)
}

pub fn greedy_cmd<H: CommandHandler + 'static>(name: &str, desc: &str, handler: H) -> Command {
    Command::new(&[name.to_string()], desc).then(greedy_node("args").execute(handler))
}

/// `Context::register_command` returns `()` in this API revision.
pub fn reg(ctx: &Context, command: Command, permission: &str) -> Result<(), String> {
    ctx.register_command(command, permission);
    Ok(())
}

pub fn name_arg(args: &ConsumedArgs) -> Result<String, CommandError> {
    logic::validate_name(&plain(args.get_value("name"))).map_err(|error| fail(error.message()))
}

pub fn name_arg_or(args: &ConsumedArgs, default: &str) -> Result<String, CommandError> {
    let raw = plain(args.get_value("name"));
    let chosen = if raw.trim().is_empty() { default } else { &raw };
    logic::validate_name(chosen).map_err(|error| fail(error.message()))
}

/// A command node granted to ops may target another player only at gamemaster level.
pub fn resolve_target(sender: &CommandSender, server: &Server, me: &Player, name: &str) -> Result<Player, CommandError> {
    let target = server.get_player_by_name(name).ok_or_else(|| fail(&i18n::trf("Jugador '{}' no conectado.", "Player '{}' is not online.", &[name])))?;
    if host::key_of(&target) != host::key_of(me) && !sender.has_permission_level(CmdLevel::Two) {
        return Err(CommandError::PermissionDenied);
    }
    Ok(target)
}

pub fn optional_target(
    sender: &CommandSender,
    server: &Server,
    me: &Player,
    args: &ConsumedArgs,
    has_target: bool,
) -> Result<Player, CommandError> {
    if has_target { resolve_target(sender, server, me, &plain(args.get_value("player"))) } else { player_of(sender) }
}

/// Resolve the "player" argument for an op-only node.
pub fn target_of(sender: &CommandSender, server: &Server, _me: &Player, args: &ConsumedArgs, level: CmdLevel) -> Result<Player, CommandError> {
    if !has_level(sender, level) {
        return Err(CommandError::PermissionDenied);
    }
    let name = plain(args.get_value("player"));
    server.get_player_by_name(&name).ok_or_else(|| fail(&i18n::trf("Jugador '{}' no conectado.", "Player '{}' is not online.", &[&name])))
}

/// Like [`resolve_target`], but refuses to target yourself.
pub fn target_from_name(sender: &CommandSender, server: &Server, me: &Player, name: &str, level: CmdLevel) -> Result<Player, CommandError> {
    if !has_level(sender, level) {
        return Err(CommandError::PermissionDenied);
    }
    let target = server.get_player_by_name(name).ok_or_else(|| fail(&i18n::trf("Jugador '{}' no conectado.", "Player '{}' is not online.", &[name])))?;
    if host::key_of(&target) == host::key_of(me) {
        return Err(fail(i18n::tr("No puedes aplicarte esto a ti mismo.", "You cannot apply this to yourself.")));
    }
    Ok(target)
}

/// Interpret an optional on/off token; toggling when absent or unrecognized.
pub fn parse_mode_arg(raw: Option<&str>, current: bool) -> bool {
    match raw.map(|token| token.to_ascii_lowercase()) {
        Some(token) if token == "on" || token == "true" => true,
        Some(token) if token == "off" || token == "false" => false,
        _ => !current,
    }
}

pub fn uuid_of(player: &Player) -> String {
    logic::uuid_string(host::key_of(player))
}

/// UUID string of an online or known offline player, if we have any record.
pub fn uuid_by_name(server: &Server, name: &str) -> Option<String> {
    if let Some(player) = server.get_player_by_name(name) {
        return Some(logic::uuid_string(host::key_of(&player)));
    }
    with_state(|state| extras::find_uuid_by_name(&state.persisted, name)).flatten()
}

pub fn cooldown_left(key: u128, kind: logic::CdKind) -> u64 {
    let now = host::now_ms();
    with_state(|state| state.cooldowns.remaining(key, kind, now)).unwrap_or(0)
}

pub fn cooldown_err(left: u64) -> CommandError {
    fail(&i18n::trf("Debes esperar {}s antes de volver a usarlo.", "You must wait {}s before using it again.", &[&left.to_string()]))
}

pub fn button(label: &str, color: NamedColor, run: &str, hover: &str) -> TextComponent {
    TextComponent::text(label)
        .color_named(color)
        .click_run_command(run)
        .hover_show_text(TextComponent::text(hover))
}

pub fn clickable_list(head: &str, names: &[String], cmd: &str, hover_verb: &str) -> TextComponent {
    let mut t = TextComponent::text(head).color_named(NamedColor::Gold);
    if names.is_empty() {
        return t.add_child(TextComponent::text(i18n::tr(" (ninguna)", " (none)")).color_named(NamedColor::Gray));
    }
    for (i, n) in names.iter().enumerate() {
        let sep = if i == 0 { " " } else { ", " };
        t = t.add_child(TextComponent::text(sep).color_named(NamedColor::Gray));
        t = t.add_child(button(n, NamedColor::Aqua, &format!("{cmd} {n}"), &format!("{hover_verb} {n}")));
    }
    t
}

/// Point `distance` blocks ahead of where the player is looking.
pub fn forward(player: &Player, distance: f64) -> (f64, f64, f64) {
    let (x, y, z) = player.get_position();
    let yaw = f64::from(player.get_yaw()).to_radians();
    let pitch = f64::from(player.get_pitch()).to_radians();
    (
        x - yaw.sin() * distance * pitch.cos(),
        y - pitch.sin() * distance,
        z + yaw.cos() * distance * pitch.cos(),
    )
}

/// Put a stack in the first empty inventory slot.
pub fn give_stack(player: &Player, key: &str, count: u8) -> CmdResult {
    let pinv = player.get_inventory();
    let inv = pinv.as_inventory();
    let mut target = None;
    for slot in 0..inv.get_size() {
        if inv.get_item(slot).is_none() {
            target = Some(slot);
            break;
        }
    }
    let slot = target.ok_or_else(|| fail(i18n::tr("Inventario lleno.", "Inventory full.")))?;
    inv.set_item(slot, Some(ItemStack::new(key, count)));
    Ok(1)
}

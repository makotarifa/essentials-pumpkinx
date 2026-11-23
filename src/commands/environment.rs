use crate::{
    cmdutil::{fail, forward, greedy_arg, greedy_node, has_level, no_arg, player_of, reg, say, unavailable, CmdResult},
    environment,
    host::{self, with_state},
    i18n,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    common::BlockPos,
    gui::Gui,
    permission::{Permission, PermissionDefault, PermissionLevel},
    player::PlayerWeather,
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    world::{BlockFlags, ExplosionInteraction},
    Context, Player, Screen, Server,
};

fn block_pos(pos: (f64, f64, f64)) -> BlockPos {
    BlockPos { x: pos.0.floor() as i32, y: pos.1.floor() as i32, z: pos.2.floor() as i32 }
}

fn update_flags() -> BlockFlags {
    BlockFlags::NOTIFY_NEIGHBORS | BlockFlags::NOTIFY_LISTENERS
}

#[derive(Clone, Copy)]
enum GuiKind {
    Anvil,
    Cartography,
    Grindstone,
    Loom,
    Smithing,
    Workbench,
    Disposal,
}

struct GuiCmd {
    kind: GuiKind,
}
impl CommandHandler for GuiCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let (screen, title) = match self.kind {
            GuiKind::Anvil => (Screen::Anvil, i18n::tr("Yunque", "Anvil")),
            GuiKind::Cartography => (Screen::CartographyTable, i18n::tr("Mesa de cartografía", "Cartography table")),
            GuiKind::Grindstone => (Screen::Grindstone, i18n::tr("Piedra de afilar", "Grindstone")),
            GuiKind::Loom => (Screen::Loom, i18n::tr("Telar", "Loom")),
            GuiKind::Smithing => (Screen::Smithing, i18n::tr("Mesa de herrería", "Smithing table")),
            GuiKind::Workbench => (Screen::Crafting, i18n::tr("Mesa de trabajo", "Crafting table")),
            GuiKind::Disposal => (Screen::Generic9x6, i18n::tr("Basura", "Trash")),
        };
        player.open_gui(Gui::new(screen, TextComponent::text(title)));
        Ok(1)
    }
}

struct EditSignCmd;
impl CommandHandler for EditSignCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let pos = block_pos(forward(&player, 5.0));
        player.open_sign_editor(pos, true);
        host::info(&player, i18n::tr("Editor de cartel abierto (bloque aproximado a 5 bloques).", "Sign editor opened (approximate block 5 blocks away)."));
        Ok(1)
    }
}

struct StoneCutterCmd;
impl CommandHandler for StoneCutterCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        player.open_gui(Gui::new(Screen::Stonecutter, TextComponent::text(i18n::tr("Cortador de piedra", "Stonecutter"))));
        Ok(1)
    }
}

struct IceCmd;
impl CommandHandler for IceCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let name = greedy_arg(&args, "args").trim().to_string();
        let target = if name.is_empty() {
            me
        } else {
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            _server.get_player_by_name(&name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?
        };
        target.set_freeze_ticks(140);
        target.as_entity().set_velocity((0.0, 0.0, 0.0));
        host::info(&target, i18n::tr("Has sido congelado.", "You have been frozen."));
        Ok(1)
    }
}

fn grow_tree(player: &Player, big: bool) -> CmdResult {
    let base = block_pos(forward(&player, 5.0));
    let world = player.get_world();
    let height = if big { 7u32 } else { 4u32 };
    for i in 0..height {
        world.set_block_by_name(BlockPos { y: base.y + i as i32, ..base }, "minecraft:oak_log", update_flags());
    }
    let radius = if big { 2i32 } else { 1i32 };
    for dy in -2..=2 {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs() == radius && dz.abs() == radius && dy > 0 {
                    continue;
                }
                world.set_block_by_name(
                    BlockPos { x: base.x + dx, y: base.y + height as i32 - 2 + dy, z: base.z + dz },
                    "minecraft:oak_leaves",
                    update_flags(),
                );
            }
        }
    }
    host::ok(player, if big { i18n::tr("Árbol grande generado.", "Large tree generated.") } else { i18n::tr("Árbol generado.", "Tree generated.") });
    Ok(1)
}

struct TreeCmd {
    big: bool,
}
impl CommandHandler for TreeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        grow_tree(&player_of(&sender)?, self.big)
    }
}

struct BreakCmd;
impl CommandHandler for BreakCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let pos = block_pos(forward(&player, 5.0));
        let block = player.get_world().get_block(pos);
        let name = block.name.to_ascii_lowercase();
        if name.contains("bedrock") && !has_level(&sender, CmdLevel::Three) {
            return Err(fail(i18n::tr("No puedes romper bedrock.", "You cannot break bedrock.")));
        }
        player.get_world().set_block_by_name(pos, "minecraft:air", update_flags());
        host::ok(&player, i18n::tr("Bloque eliminado.", "Block removed."));
        Ok(1)
    }
}

struct BurnCmd;
impl CommandHandler for BurnCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /burn <jugador> <segundos>", "Usage: /burn <player> <seconds>")))?;
        let secs = tokens.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(5).clamp(0, 300);
        let target = server.get_player_by_name(name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
        target.as_entity().set_fire_ticks(secs * 20);
        let secs_s = secs.to_string();
        host::ok(&target, &i18n::trf("Ardes durante {}s.", "You are burning for {}s.", &[&secs_s]));
        Ok(1)
    }
}

struct LightningCmd;
impl CommandHandler for LightningCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let name = greedy_arg(&args, "args").trim().to_string();
        let (pos, label) = if name.is_empty() {
            (forward(&me, 5.0), i18n::tr("el objetivo", "the target").to_string())
        } else {
            let target = server.get_player_by_name(&name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
            (target.get_position(), target.get_name())
        };
        me.get_world().strike_lightning(pos, false);
        host::ok(&me, &i18n::trf("Rayo caído en {}.", "Lightning struck {}.", &[&label]));
        Ok(1)
    }
}

struct ThunderCmd;
impl CommandHandler for ThunderCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        let on = match raw.as_str() {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => return Err(fail(i18n::tr("Uso: /thunder on|off", "Usage: /thunder on|off"))),
        };
        let world = player.get_world();
        world.set_thundering(on);
        if on { world.set_raining(true); }
        let state = if on { i18n::tr("activada", "enabled") } else { i18n::tr("desactivada", "disabled") };
        host::ok(&player, &i18n::trf("Tormenta {}.", "Storm {}.", &[state]));
        Ok(1)
    }
}

struct NukeCmd;
impl CommandHandler for NukeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let label = format!("nuke:{}", greedy_arg(&args, "args").trim().to_ascii_lowercase());
        let confirmed = with_state(|state| environment::take_or_set(&mut state.pending_confirm, host::key_of(&player), &label))
            .ok_or_else(unavailable)?;
        if !confirmed {
            return Err(fail(i18n::tr("Confirmación requerida: vuelve a ejecutar /nuke para confirmar.", "Confirmation required: run /nuke again to confirm.")));
        }
        let targets: Vec<Player> = if greedy_arg(&args, "args").trim().is_empty() {
            server.get_all_players()
        } else {
            greedy_arg(&args, "args").split_whitespace().filter_map(|n| server.get_player_by_name(n)).collect()
        };
        for target in targets {
            let (x, y, z) = target.get_position();
            let world = target.get_world();
            for dx in [-10, -5, 0, 5, 10] {
                for dz in [-10, -5, 0, 5, 10] {
                    world.create_explosion((x + f64::from(dx), y + 30.0, z + f64::from(dz)), 4.0, true, ExplosionInteraction::Tnt);
                }
            }
        }
        host::ok(&player, i18n::tr("Bombardeo ejecutado.", "Bombardment executed."));
        Ok(1)
    }
}

struct PlayerTimeCmd;
impl CommandHandler for PlayerTimeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        let world = player.get_world();
        if raw.is_empty() || raw.eq_ignore_ascii_case("get") {
            let now_ticks = world.get_time_of_day().to_string();
            say(&sender, &i18n::trf("Hora del mundo: {} ticks.", "World time: {} ticks.", &[&now_ticks]));
            return Ok(1);
        }
        let ticks = environment::parse_time(&raw).ok_or_else(|| fail(i18n::tr("Hora no reconocida (day/noon/night/midnight o ticks).", "Unrecognized time (day/noon/night/midnight or ticks).")))?;
        world.set_time_of_day(ticks);
        let ticks_s = ticks.to_string();
        host::ok(&player, &i18n::trf("Hora del mundo ajustada a {} ticks (la API no tiene hora por jugador).", "World time set to {} ticks (the API has no per-player time).", &[&ticks_s]));
        Ok(1)
    }
}

struct PlayerWeatherCmd;
impl CommandHandler for PlayerWeatherCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        if raw.is_empty() || raw.eq_ignore_ascii_case("get") {
            let current = player.get_player_weather().map_or(i18n::tr("por defecto", "default").into(), |w| format!("{w:?}"));
            say(&sender, &i18n::trf("Clima del jugador: {}.", "Player weather: {}.", &[&current]));
            return Ok(1);
        }
        match environment::parse_weather(&raw).ok_or_else(|| fail(i18n::tr("Clima no reconocido (clear/rain/reset).", "Unrecognized weather (clear/rain/reset).")))? {
            environment::WeatherKind::Clear => player.set_player_weather(PlayerWeather::Clear),
            environment::WeatherKind::Downfall => player.set_player_weather(PlayerWeather::Downfall),
            environment::WeatherKind::Reset => player.reset_player_weather(),
        }
        host::ok(&player, i18n::tr("Clima personalizado actualizado.", "Custom weather updated."));
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
    let zero = PermissionLevel::Zero;
    let two = PermissionLevel::Two;
    let three = PermissionLevel::Three;
    for (n, d, l) in [
        ("anvil", "Abrir yunque", zero),
        ("cartographytable", "Abrir mesa de cartografía", zero),
        ("grindstone", "Abrir piedra de afilar", zero),
        ("loom", "Abrir telar", zero),
        ("smithingtable", "Abrir mesa de herrería", zero),
        ("workbench", "Abrir mesa de trabajo", zero),
        ("disposal", "Abrir basura", zero),
        ("editsign", "Editar carteles", zero),
        ("ptime", "Ver/ajustar hora", zero),
        ("pweather", "Ver/ajustar clima", zero),
        ("stonecutter", "Abrir cortador de piedra", zero),
        ("ice", "Congelar jugadores", two),
        ("tree", "Generar árbol", two),
        ("bigtree", "Generar árbol grande", two),
        ("break", "Romper bloques", two),
        ("burn", "Prender fuego", two),
        ("lightning", "Invocar rayos", two),
        ("thunder", "Activar tormenta", two),
        ("nuke", "Bombardeo", three),
    ] {
        // PermissionLevel::Zero maps to Allow to keep the defaults readable.
        if l == zero {
            ctx.register_permission(&Permission {
                node: format!("pumpkin_essentials:{n}"),
                description: d.into(),
                default: PermissionDefault::Allow,
                children: vec![],
            })?;
        } else {
            ctx.register_permission(&perm(n, d, l))?;
        }
    }

    reg(ctx, no_arg(&["anvil"], i18n::tr("Abrir yunque", "Open anvil"), GuiCmd { kind: GuiKind::Anvil }), "pumpkin_essentials:anvil")?;
    reg(ctx, no_arg(&["cartographytable"], i18n::tr("Abrir cartografía", "Open cartography table"), GuiCmd { kind: GuiKind::Cartography }), "pumpkin_essentials:cartographytable")?;
    reg(ctx, no_arg(&["grindstone"], i18n::tr("Abrir piedra de afilar", "Open grindstone"), GuiCmd { kind: GuiKind::Grindstone }), "pumpkin_essentials:grindstone")?;
    reg(ctx, no_arg(&["loom"], i18n::tr("Abrir telar", "Open loom"), GuiCmd { kind: GuiKind::Loom }), "pumpkin_essentials:loom")?;
    reg(ctx, no_arg(&["smithingtable"], i18n::tr("Abrir herrería", "Open smithing table"), GuiCmd { kind: GuiKind::Smithing }), "pumpkin_essentials:smithingtable")?;
    reg(ctx, no_arg(&["workbench"], i18n::tr("Abrir mesa de trabajo", "Open crafting table"), GuiCmd { kind: GuiKind::Workbench }), "pumpkin_essentials:workbench")?;
    reg(ctx, no_arg(&["disposal"], i18n::tr("Abrir basura", "Open trash"), GuiCmd { kind: GuiKind::Disposal }), "pumpkin_essentials:disposal")?;
    reg(ctx, no_arg(&["editsign"], i18n::tr("Editar cartel", "Edit sign"), EditSignCmd), "pumpkin_essentials:editsign")?;
    reg(ctx, no_arg(&["stonecutter"], i18n::tr("Abrir cortador de piedra", "Open stonecutter"), StoneCutterCmd), "pumpkin_essentials:stonecutter")?;
    reg(ctx, Command::new(&["ice".to_string()], i18n::tr("Congelar", "Freeze")).then(greedy_node("args").execute(IceCmd)), "pumpkin_essentials:ice")?;
    reg(ctx, no_arg(&["tree"], i18n::tr("Generar árbol", "Generate tree"), TreeCmd { big: false }), "pumpkin_essentials:tree")?;
    reg(ctx, no_arg(&["bigtree"], i18n::tr("Generar árbol grande", "Generate large tree"), TreeCmd { big: true }), "pumpkin_essentials:bigtree")?;
    reg(ctx, no_arg(&["break"], i18n::tr("Romper bloque", "Break block"), BreakCmd), "pumpkin_essentials:break")?;
    reg(ctx, Command::new(&["burn".to_string()], i18n::tr("Prender fuego", "Ignite")).then(greedy_node("args").execute(BurnCmd)), "pumpkin_essentials:burn")?;
    reg(ctx, Command::new(&["lightning".to_string()], i18n::tr("Rayo", "Lightning")).then(greedy_node("args").execute(LightningCmd)), "pumpkin_essentials:lightning")?;
    reg(ctx, Command::new(&["thunder".to_string()], i18n::tr("Tormenta", "Storm")).then(greedy_node("args").execute(ThunderCmd)), "pumpkin_essentials:thunder")?;
    reg(ctx, Command::new(&["nuke".to_string()], i18n::tr("Bombardeo", "Bombardment")).then(greedy_node("args").execute(NukeCmd)), "pumpkin_essentials:nuke")?;
    reg(ctx, Command::new(&["ptime".to_string()], i18n::tr("Hora", "Time")).then(greedy_node("args").execute(PlayerTimeCmd)), "pumpkin_essentials:ptime")?;
    reg(ctx, Command::new(&["pweather".to_string()], i18n::tr("Clima", "Weather")).then(greedy_node("args").execute(PlayerWeatherCmd)), "pumpkin_essentials:pweather")?;
    Ok(())
}

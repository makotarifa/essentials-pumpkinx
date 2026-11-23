//! Entorno: mesas de trabajo (/anvil, /cartographytable, /grindstone,
//! /loom, /smithingtable, /workbench, /disposal, /editsign, /stonecutter),
//! /ptime, /pweather, /thunder, /ice, /tree, /bigtree, /break, /burn,
//! /lightning y /nuke.

use crate::{
    cmdutil::{fail, forward, greedy_arg, greedy_node, has_level, no_arg, player_of, reg, say, unavailable, CmdResult},
    environment,
    host::{self, with_state},
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

// ---- mesas y GUI ----------------------------------------------------------------

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
        let p = player_of(&sender)?;
        let (screen, title) = match self.kind {
            GuiKind::Anvil => (Screen::Anvil, "Yunque"),
            GuiKind::Cartography => (Screen::CartographyTable, "Mesa de cartografía"),
            GuiKind::Grindstone => (Screen::Grindstone, "Piedra de afilar"),
            GuiKind::Loom => (Screen::Loom, "Telar"),
            GuiKind::Smithing => (Screen::Smithing, "Mesa de herrería"),
            GuiKind::Workbench => (Screen::Crafting, "Mesa de trabajo"),
            GuiKind::Disposal => (Screen::Generic9x6, "Basura"),
        };
        p.open_gui(Gui::new(screen, TextComponent::text(title)));
        Ok(1)
    }
}

struct EditSignCmd;
impl CommandHandler for EditSignCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let pos = block_pos(forward(&p, 5.0));
        p.open_sign_editor(pos, true);
        host::info(&p, "Editor de cartel abierto (bloque aproximado a 5 bloques).");
        Ok(1)
    }
}

struct StoneCutterCmd;
impl CommandHandler for StoneCutterCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        p.open_gui(Gui::new(Screen::Stonecutter, TextComponent::text("Cortador de piedra")));
        Ok(1)
    }
}

// ---- entornos -------------------------------------------------------------------

struct IceCmd;
impl CommandHandler for IceCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let name = greedy_arg(&args, "args").trim().to_string();
        let target = if name.is_empty() {
            me
        } else {
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            _server.get_player_by_name(&name).ok_or_else(|| fail("Jugador no conectado."))?
        };
        target.set_freeze_ticks(140);
        target.as_entity().set_velocity((0.0, 0.0, 0.0));
        host::info(&target, "Has sido congelado.");
        Ok(1)
    }
}

fn grow_tree(p: &Player, big: bool) -> CmdResult {
    let base = block_pos(forward(&p, 5.0));
    let world = p.get_world();
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
    host::ok(p, if big { "Árbol grande generado." } else { "Árbol generado." });
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
        let p = player_of(&sender)?;
        let pos = block_pos(forward(&p, 5.0));
        let block = p.get_world().get_block(pos);
        let name = block.name.to_ascii_lowercase();
        if name.contains("bedrock") && !has_level(&sender, CmdLevel::Three) {
            return Err(fail("No puedes romper bedrock."));
        }
        p.get_world().set_block_by_name(pos, "minecraft:air", update_flags());
        host::ok(&p, "Bloque eliminado.");
        Ok(1)
    }
}

struct BurnCmd;
impl CommandHandler for BurnCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail("Uso: /burn <jugador> <segundos>"))?;
        let secs = tokens.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(5).clamp(0, 300);
        let target = server.get_player_by_name(name).ok_or_else(|| fail("Jugador no conectado."))?;
        target.as_entity().set_fire_ticks(secs * 20);
        host::ok(&target, &format!("Ardes durante {secs}s."));
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
            (forward(&me, 5.0), "el objetivo".to_string())
        } else {
            let target = server.get_player_by_name(&name).ok_or_else(|| fail("Jugador no conectado."))?;
            (target.get_position(), target.get_name())
        };
        me.get_world().strike_lightning(pos, false);
        host::ok(&me, &format!("Rayo caído en {label}."));
        Ok(1)
    }
}

struct ThunderCmd;
impl CommandHandler for ThunderCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let p = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        let on = match raw.as_str() {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => return Err(fail("Uso: /thunder on|off")),
        };
        let world = p.get_world();
        world.set_thundering(on);
        if on { world.set_raining(true); }
        host::ok(&p, &format!("Tormenta {}.", if on { "activada" } else { "desactivada" }));
        Ok(1)
    }
}

struct NukeCmd;
impl CommandHandler for NukeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let p = player_of(&sender)?;
        let label = format!("nuke:{}", greedy_arg(&args, "args").trim().to_ascii_lowercase());
        let confirmed = with_state(|s| environment::take_or_set(&mut s.pending_confirm, host::key_of(&p), &label))
            .ok_or_else(unavailable)?;
        if !confirmed {
            return Err(fail("Confirmación requerida: vuelve a ejecutar /nuke para confirmar."));
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
        host::ok(&p, "Bombardeo ejecutado.");
        Ok(1)
    }
}

struct PlayerTimeCmd;
impl CommandHandler for PlayerTimeCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        let world = p.get_world();
        if raw.is_empty() || raw.eq_ignore_ascii_case("get") {
            say(&sender, &format!("Hora del mundo: {} ticks.", world.get_time_of_day()));
            return Ok(1);
        }
        let ticks = environment::parse_time(&raw).ok_or_else(|| fail("Hora no reconocida (day/noon/night/midnight o ticks)."))?;
        world.set_time_of_day(ticks);
        host::ok(&p, &format!("Hora del mundo ajustada a {ticks} ticks (la API no tiene hora por jugador)."));
        Ok(1)
    }
}

struct PlayerWeatherCmd;
impl CommandHandler for PlayerWeatherCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        if raw.is_empty() || raw.eq_ignore_ascii_case("get") {
            let current = p.get_player_weather().map_or("por defecto".into(), |w| format!("{w:?}"));
            say(&sender, &format!("Clima del jugador: {current}."));
            return Ok(1);
        }
        match environment::parse_weather(&raw).ok_or_else(|| fail("Clima no reconocido (clear/rain/reset)."))? {
            environment::WeatherKind::Clear => p.set_player_weather(PlayerWeather::Clear),
            environment::WeatherKind::Downfall => p.set_player_weather(PlayerWeather::Downfall),
            environment::WeatherKind::Reset => p.reset_player_weather(),
        }
        host::ok(&p, "Clima personalizado actualizado.");
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
        // PermissionLevel::Zero se traduce a Allow para mantener defaults legibles.
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

    reg(ctx, no_arg(&["anvil"], "Abrir yunque", GuiCmd { kind: GuiKind::Anvil }), "pumpkin_essentials:anvil")?;
    reg(ctx, no_arg(&["cartographytable"], "Abrir cartografía", GuiCmd { kind: GuiKind::Cartography }), "pumpkin_essentials:cartographytable")?;
    reg(ctx, no_arg(&["grindstone"], "Abrir piedra de afilar", GuiCmd { kind: GuiKind::Grindstone }), "pumpkin_essentials:grindstone")?;
    reg(ctx, no_arg(&["loom"], "Abrir telar", GuiCmd { kind: GuiKind::Loom }), "pumpkin_essentials:loom")?;
    reg(ctx, no_arg(&["smithingtable"], "Abrir herrería", GuiCmd { kind: GuiKind::Smithing }), "pumpkin_essentials:smithingtable")?;
    reg(ctx, no_arg(&["workbench"], "Abrir mesa de trabajo", GuiCmd { kind: GuiKind::Workbench }), "pumpkin_essentials:workbench")?;
    reg(ctx, no_arg(&["disposal"], "Abrir basura", GuiCmd { kind: GuiKind::Disposal }), "pumpkin_essentials:disposal")?;
    reg(ctx, no_arg(&["editsign"], "Editar cartel", EditSignCmd), "pumpkin_essentials:editsign")?;
    reg(ctx, no_arg(&["stonecutter"], "Abrir cortador de piedra", StoneCutterCmd), "pumpkin_essentials:stonecutter")?;
    reg(ctx, Command::new(&["ice".to_string()], "Congelar").then(greedy_node("args").execute(IceCmd)), "pumpkin_essentials:ice")?;
    reg(ctx, no_arg(&["tree"], "Generar árbol", TreeCmd { big: false }), "pumpkin_essentials:tree")?;
    reg(ctx, no_arg(&["bigtree"], "Generar árbol grande", TreeCmd { big: true }), "pumpkin_essentials:bigtree")?;
    reg(ctx, no_arg(&["break"], "Romper bloque", BreakCmd), "pumpkin_essentials:break")?;
    reg(ctx, Command::new(&["burn".to_string()], "Prender fuego").then(greedy_node("args").execute(BurnCmd)), "pumpkin_essentials:burn")?;
    reg(ctx, Command::new(&["lightning".to_string()], "Rayo").then(greedy_node("args").execute(LightningCmd)), "pumpkin_essentials:lightning")?;
    reg(ctx, Command::new(&["thunder".to_string()], "Tormenta").then(greedy_node("args").execute(ThunderCmd)), "pumpkin_essentials:thunder")?;
    reg(ctx, Command::new(&["nuke".to_string()], "Bombardeo").then(greedy_node("args").execute(NukeCmd)), "pumpkin_essentials:nuke")?;
    reg(ctx, Command::new(&["ptime".to_string()], "Hora").then(greedy_node("args").execute(PlayerTimeCmd)), "pumpkin_essentials:ptime")?;
    reg(ctx, Command::new(&["pweather".to_string()], "Clima").then(greedy_node("args").execute(PlayerWeatherCmd)), "pumpkin_essentials:pweather")?;
    Ok(())
}

//! Iteración 9: huecos reales detectados en la auditoría de EssentialsX.

use crate::host::{self, with_state};
use pumpkin_plugin_api::{
    command::{CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    gui::Gui,
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    Context, Player, Screen, Server,
};

type CmdResult = Result<i32, CommandError>;
use pumpkin_plugin_api::command::CommandError;

fn fail(msg: &str) -> CommandError {
    CommandError::CommandFailed(TextComponent::text(msg))
}

fn say(sender: &CommandSender, msg: &str) {
    sender.send_message(TextComponent::text(msg));
}

fn player_of(sender: &CommandSender) -> Result<Player, CommandError> {
    sender.as_player().ok_or_else(|| fail("Solo jugadores."))
}

fn reg(ctx: &Context, command: Command, permission: &str) -> Result<(), String> {
    ctx.register_command(command, permission);
    Ok(())
}

struct StoneCutterCmd;
impl CommandHandler for StoneCutterCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        p.open_gui(Gui::new(Screen::Stonecutter, TextComponent::text("Cortador de piedra")));
        Ok(1)
    }
}

struct FireworkCmd;
impl CommandHandler for FireworkCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let pinv = p.get_inventory();
        let inv = pinv.as_inventory();
        let slot = (0..inv.get_size()).find(|s| inv.get_item(*s).is_none()).ok_or_else(|| fail("Inventario lleno."))?;
        inv.set_item(slot, Some(pumpkin_plugin_api::ItemStack::new("minecraft:firework_rocket", 1)));
        host::ok(&p, "Cohete entregado.");
        Ok(1)
    }
}

struct JailsCmd;
impl CommandHandler for JailsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let list = with_state(|s| {
            s.cfg.jails.iter().map(|(name, loc)| format!("{name}: {} ({:.0}, {:.0}, {:.0})", loc.world, loc.x, loc.y, loc.z)).collect::<Vec<_>>()
        })
        .ok_or_else(|| fail("Configuración no disponible."))?;
        say(&sender, &format!("Cárceles: {}", if list.is_empty() { "ninguna".into() } else { list.join(" | ") }));
        Ok(1)
    }
}

pub fn register_all(ctx: &Context) -> Result<(), String> {
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:stonecutter".into(),
        description: "Abrir cortador de piedra".into(),
        default: PermissionDefault::Allow,
        children: vec![],
    })?;
    for (node, desc) in [
        ("firework", "Entregar cohete pirotécnico"),
        ("jails", "Listar cárceles configuradas"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{node}"),
            description: desc.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    reg(ctx, Command::new(&["stonecutter".to_string()], "Abrir cortador de piedra").execute(StoneCutterCmd), "pumpkin_essentials:stonecutter")?;
    reg(ctx, Command::new(&["firework".to_string()], "Entregar cohete").execute(FireworkCmd), "pumpkin_essentials:firework")?;
    reg(ctx, Command::new(&["jails".to_string()], "Listar cárceles").execute(JailsCmd), "pumpkin_essentials:jails")?;
    Ok(())
}

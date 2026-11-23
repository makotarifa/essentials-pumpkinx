use crate::{
    cmdutil::{fail, player_of, plain, reg, resolve_target, word, CmdResult},
    extras,
    host,
};
use pumpkin_plugin_api::{
    command::{CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    common::GameMode,
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    Context, Player, Server,
};
use tracing::info;

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
        let mode = extras::parse_mode(self.mode).ok_or_else(|| fail("Modo de juego desconocido."))?;
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
            .ok_or_else(|| fail("Uso: /gm <survival|creative|adventure|spectator> [jugador] (o 0/1/2/3, s/c/a/sp)."))?;
        let target = if self.target {
            resolve_target(&sender, &server, &me, &plain(args.get_value("player")))?
        } else {
            me
        };
        return_gamemode(&sender, &target, mode)
    }
}

fn return_gamemode(sender: &CommandSender, target: &Player, mode: GameMode) -> CmdResult {
    if target.set_gamemode(mode) {
        let name = target.get_name();
        host::ok(target, &format!("Modo de juego cambiado a {}.", extras::mode_name(&mode)));
        if host::key_of(target) != sender.as_player().map_or(host::key_of(target), |p| host::key_of(&p)) {
            sender.send_message(TextComponent::text(&format!("Modo de juego de {name} cambiado a {}.", extras::mode_name(&mode))));
        }
        info!("essentials: {} gamemode -> {}", name, extras::mode_name(&mode));
        Ok(1)
    } else {
        Err(fail("No se pudo cambiar el modo de juego."))
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    ctx.register_permission(&Permission {
        node: "pumpkin_essentials:gamemode".into(),
        description: "Usar /gm y /gma, /gmc, /gms, /gmsp".into(),
        default: PermissionDefault::Op(PermissionLevel::Two),
        children: vec![],
    })?;
    let gm = "pumpkin_essentials:gamemode";
    for (name, mode) in [("gma", "a"), ("gmc", "c"), ("gms", "s"), ("gmsp", "sp")] {
        reg(
            ctx,
            Command::new(&[name.to_string()], "Cambia el modo de juego (opcionalmente de otro jugador)")
                .execute(FixedGmCmd { mode, target: false })
                .then(word("player").execute(FixedGmCmd { mode, target: true })),
            gm,
        )?;
    }
    reg(
        ctx,
        Command::new(&["gm".to_string()], "Cambia el modo de juego").then(
            word("mode")
                .execute(GmCmd { target: false })
                .then(word("player").execute(GmCmd { target: true })),
        ),
        gm,
    )?;
    Ok(())
}

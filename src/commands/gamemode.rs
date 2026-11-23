use crate::{
    cmdutil::{fail, player_of, plain, reg, resolve_target, word, CmdResult},
    extras,
    host,
    i18n,
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
        let mode = extras::parse_mode(self.mode).ok_or_else(|| fail(i18n::tr("Modo de juego desconocido.", "Unknown game mode.")))?;
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
        let mode = extras::parse_mode(&raw).ok_or_else(|| {
            fail(i18n::tr("Uso: /gm <survival|creative|adventure|spectator> [jugador] (o 0/1/2/3, s/c/a/sp).", "Usage: /gm <survival|creative|adventure|spectator> [player] (or 0/1/2/3, s/c/a/sp)."))
        })?;
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
        let mode_label = extras::mode_name(&mode);
        host::ok(target, &i18n::trf("Modo de juego cambiado a {}.", "Game mode changed to {}.", &[mode_label]));
        if host::key_of(target) != sender.as_player().map_or(host::key_of(target), |p| host::key_of(&p)) {
            sender.send_message(TextComponent::text(&i18n::trf("Modo de juego de {} cambiado a {}.", "{}'s game mode changed to {}.", &[&name, mode_label])));
        }
        info!("essentials: {} gamemode -> {}", name, mode_label);
        Ok(1)
    } else {
        Err(fail(i18n::tr("No se pudo cambiar el modo de juego.", "Could not change game mode.")))
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
            Command::new(&[name.to_string()], i18n::tr("Cambia el modo de juego (opcionalmente de otro jugador)", "Change the game mode (optionally of another player)"))
                .execute(FixedGmCmd { mode, target: false })
                .then(word("player").execute(FixedGmCmd { mode, target: true })),
            gm,
        )?;
    }
    reg(
        ctx,
        Command::new(&["gm".to_string()], i18n::tr("Cambia el modo de juego", "Change the game mode")).then(
            word("mode")
                .execute(GmCmd { target: false })
                .then(word("player").execute(GmCmd { target: true })),
        ),
        gm,
    )?;
    Ok(())
}

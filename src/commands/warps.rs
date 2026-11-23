use crate::{
    cmdutil::{clickable_list, fail, name_arg, need_op, no_arg, one_arg, player_of, reg, say, unavailable, word, CmdResult},
    host::{self, with_state, State},
    i18n,
    logic::TpKind,
    store::{self, Warp},
};
use pumpkin_plugin_api::{
    command::{CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault},
    text::TextComponent,
    Context, Server,
};
use tracing::info;

/// Mutate warps.json atomically: edit a copy, save it, then commit it to memory.
fn mutate_warps<R>(state: &mut State, f: impl FnOnce(&mut store::WarpsFile) -> R) -> Result<R, String> {
    let mut copy = state.warps.clone();
    let r = f(&mut copy);
    store::save_json(&state.warps_path(), &copy).map_err(|e| {
        tracing::error!("essentials: could not save warps.json: {e}");
        i18n::tr("No se pudieron guardar los datos; el cambio NO se aplicó.", "Could not save data; the change was NOT applied.").to_string()
    })?;
    state.warps = copy;
    Ok(r)
}

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
        host::ok(&p, &i18n::trf("Warp '{}' creado.", "Warp '{}' created.", &[&name]));
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
            return Err(fail(&i18n::trf("No existe el warp '{}'.", "Warp '{}' does not exist.", &[&name])));
        }
        info!("essentials: {} /delwarp {name}", sender.get_name());
        sender.send_message(TextComponent::text(&i18n::trf("Warp '{}' eliminado.", "Warp '{}' deleted.", &[&name])));
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
            .ok_or_else(|| fail(&i18n::trf("No existe el warp '{}'.", "Warp '{}' does not exist.", &[&name])))?;
        info!("essentials: {} /warp {name}", p.get_name());
        super::start_teleport(&server, &p, dest, i18n::trf("el warp '{}'", "warp '{}'", &[&name]), TpKind::Warp);
        Ok(1)
    }
}

struct WarpsCmd;
impl CommandHandler for WarpsCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let p = player_of(&sender)?;
        let names = with_state(|s| s.warps.warps.keys().cloned().collect::<Vec<_>>()).ok_or_else(unavailable)?;
        p.send_system_message(clickable_list("Warps:", &names, "/warp", i18n::tr("Ir al warp", "Go to warp")), false);
        Ok(1)
    }
}

struct WarpInfoCmd;
impl CommandHandler for WarpInfoCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let name = name_arg(&args)?;
        let info = with_state(|s| s.warps.warps.get(&name).map(|w| (w.loc.clone(), w.created_by.clone())))
            .ok_or_else(unavailable)?
            .ok_or_else(|| fail(&i18n::trf("No existe el warp '{}'.", "Warp '{}' does not exist.", &[&name])))?;
        let (loc, by) = info;
        let (x, y, z, yaw, pitch) = (
            format!("{:.1}", loc.x),
            format!("{:.1}", loc.y),
            format!("{:.1}", loc.z),
            format!("{:.1}", loc.yaw),
            format!("{:.1}", loc.pitch),
        );
        say(
            &sender,
            &i18n::trf(
                "Warp '{}' — creado por {} — {} ({}, {}, {}), yaw {}, pitch {}",
                "Warp '{}' — created by {} — {} ({}, {}, {}), yaw {}, pitch {}",
                &[&name, &by, &loc.world, &x, &y, &z, &yaw, &pitch],
            ),
        );
        Ok(1)
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    for (n, d) in [
        ("warp", "Usar /warp, /warps"),
        ("warpinfo", "Ver detalles de un warp"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Allow,
            children: vec![],
        })?;
    }
    let warp = "pumpkin_essentials:warp";
    reg(ctx, one_arg(&["setwarp"], i18n::tr("Crea o sobrescribe un warp (op)", "Create or overwrite a warp (op)"), "name", SetWarpCmd), "pumpkin_essentials:admin")?;
    reg(ctx, one_arg(&["delwarp"], i18n::tr("Elimina un warp (op)", "Delete a warp (op)"), "name", DelWarpCmd), "pumpkin_essentials:admin")?;
    reg(
        ctx,
        Command::new(&["warp".to_string()], i18n::tr("Teletransporta a un warp o los lista", "Teleport to a warp or list warps"))
            .execute(WarpsCmd)
            .then(word("name").execute(WarpCmd)),
        warp,
    )?;
    reg(ctx, no_arg(&["warps"], i18n::tr("Lista los warps", "List warps"), WarpsCmd), warp)?;
    reg(ctx, one_arg(&["warpinfo"], i18n::tr("Información de un warp", "Warp information"), "name", WarpInfoCmd), "pumpkin_essentials:warpinfo")?;
    Ok(())
}

//! Kits: /kit, /createkit, /delkit, /kitreset y /showkit.

use crate::{
    cmdutil::{fail, give_stack, greedy_arg, greedy_node, has_level, player_of, reg, say, unavailable, uuid_of, word, CmdResult},
    host::{self, with_state},
    kits::{self, Kit, KitItem},
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Server,
};

struct KitCmd {
    has_target: bool,
}
impl CommandHandler for KitCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() {
            let names = with_state(|s| s.kits.kits.keys().cloned().collect::<Vec<_>>()).ok_or_else(unavailable)?;
            say(&sender, &format!("Kits: {}", if names.is_empty() { "ninguno".into() } else { names.join(", ") }));
            return Ok(1);
        }
        let name = tokens[0].to_ascii_lowercase();
        let receiver = if self.has_target {
            let other = server.get_player_by_name(tokens.get(1).copied().unwrap_or("")).ok_or_else(|| fail("Jugador no conectado."))?;
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            other
        } else {
            me
        };
        let kit = with_state(|s| s.kits.kits.get(&name).cloned()).ok_or_else(unavailable)?
            .ok_or_else(|| fail(&format!("Kit '{name}' no existe.")))?;
        let uuid = uuid_of(&receiver);
        let now = host::now_ms();
        if !with_state(|s| kits::is_ready(&s.kits, &uuid, &name, now)).ok_or_else(unavailable)? {
            return Err(fail(&format!("Ya recibiste el kit '{name}' recientemente.")));
        }
        for item in &kit.items {
            give_stack(&receiver, &item.id, item.count)?;
        }
        with_state(|s| {
            kits::mark_used(&mut s.kits, &uuid, &name, kit.delay_secs, now);
            s.save_kits();
        })
        .ok_or_else(unavailable)?;
        host::ok(&receiver, &format!("Kit '{name}' recibido."));
        Ok(1)
    }
}

struct CreateKitCmd;
impl CommandHandler for CreateKitCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let p = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail("Uso: /createkit <nombre> <delay_segundos>"))?.to_ascii_lowercase();
        let delay = tokens.next().and_then(|s| s.parse::<u64>().ok()).ok_or_else(|| fail("Uso: /createkit <nombre> <delay_segundos>"))?;
        let pinv = p.get_inventory();
        let inv = pinv.as_inventory();
        let mut items = Vec::new();
        for slot in 0..inv.get_size() {
            if let Some(item) = inv.get_item(slot) {
                items.push(KitItem { id: item.get_registry_key(), count: item.get_count() });
            }
        }
        if items.is_empty() { return Err(fail("Tu inventario está vacío.")); }
        with_state(|s| { s.kits.kits.insert(name.clone(), Kit { items, delay_secs: delay }); s.save_kits(); })
            .ok_or_else(unavailable)?;
        say(&sender, &format!("Kit '{name}' creado con {delay}s de cooldown."));
        Ok(1)
    }
}

struct DelKitCmd;
impl CommandHandler for DelKitCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let name = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        let gone = with_state(|s| { let gone = s.kits.kits.remove(&name).is_some(); if gone { s.save_kits(); } gone })
            .ok_or_else(unavailable)?;
        if !gone { return Err(fail("Ese kit no existe.")); }
        say(&sender, &format!("Kit '{name}' eliminado."));
        Ok(1)
    }
}

struct KitResetCmd;
impl CommandHandler for KitResetCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail("Uso: /kitreset <kit> [jugador]"))?.to_ascii_lowercase();
        let target = match tokens.next() {
            Some(n) => server.get_player_by_name(n).ok_or_else(|| fail("Jugador no conectado."))?,
            None => me,
        };
        with_state(|s| { s.kits.cooldowns.remove(&kits::cooldown_key(&uuid_of(&target), &name)); s.save_kits(); })
            .ok_or_else(unavailable)?;
        say(&sender, &format!("Cooldown de '{name}' reiniciado para {}.", target.get_name()));
        Ok(1)
    }
}

struct ShowKitCmd;
impl CommandHandler for ShowKitCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let name = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        let kit = with_state(|s| s.kits.kits.get(&name).cloned()).ok_or_else(unavailable)?
            .ok_or_else(|| fail("Ese kit no existe."))?;
        let list = kit.items.iter().map(|i| format!("{}x{}", i.count, i.id)).collect::<Vec<_>>().join(", ");
        say(&sender, &format!("Kit '{name}' (cooldown {}s): {}", kit.delay_secs, list));
        Ok(1)
    }
}

pub fn register(ctx: &Context) -> Result<(), String> {
    let allow = |node: &str, desc: &str| Permission {
        node: format!("pumpkin_essentials:{node}"),
        description: desc.into(),
        default: PermissionDefault::Allow,
        children: vec![],
    };
    for (n, d) in [("kit", "Usar kits"), ("showkit", "Ver kits")] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d, l) in [
        ("createkit", "Crear kits", PermissionLevel::Three),
        ("delkit", "Borrar kits", PermissionLevel::Three),
        ("kitreset", "Reiniciar cooldowns de kits", PermissionLevel::Two),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(l),
            children: vec![],
        })?;
    }

    reg(
        ctx,
        Command::new(&["kit".to_string()], "Usar o listar kits")
            .then(greedy_node("args").execute(KitCmd { has_target: false }).then(word("player").execute(KitCmd { has_target: true }))),
        "pumpkin_essentials:kit",
    )?;
    reg(
        ctx,
        Command::new(&["createkit".to_string()], "Crear kit desde el inventario").then(greedy_node("args").execute(CreateKitCmd)),
        "pumpkin_essentials:createkit",
    )?;
    reg(
        ctx,
        Command::new(&["delkit".to_string()], "Borrar kit").then(greedy_node("args").execute(DelKitCmd)),
        "pumpkin_essentials:delkit",
    )?;
    reg(
        ctx,
        Command::new(&["kitreset".to_string()], "Reiniciar cooldown").then(greedy_node("args").execute(KitResetCmd)),
        "pumpkin_essentials:kitreset",
    )?;
    reg(
        ctx,
        Command::new(&["showkit".to_string()], "Ver contenido de un kit").then(greedy_node("args").execute(ShowKitCmd)),
        "pumpkin_essentials:showkit",
    )?;
    Ok(())
}

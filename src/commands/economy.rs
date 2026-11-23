use crate::{
    cmdutil::{fail, greedy_arg, greedy_node, player_of, reg, say, unavailable, uuid_of, word, CmdResult},
    economy::{self, PendingPay},
    host::{self, with_state},
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    permission::{Permission, PermissionDefault, PermissionLevel},
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    Context, Server,
};
use tracing::info;

fn money(cents: i64) -> String {
    with_state(|state| economy::format_money(cents, &state.cfg.currency_symbol, state.cfg.currency_decimals))
        .unwrap_or_else(|| format!("{cents}"))
}

fn parse_amount(raw: &str) -> Result<i64, CommandError> {
    with_state(|state| economy::parse_money(raw, state.cfg.currency_decimals))
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))
}

fn uuid_by_name(server: &Server, name: &str) -> Option<String> {
    if let Some(player) = server.get_player_by_name(name) {
        return Some(uuid_of(&player));
    }
    with_state(|state| {
        let uuid = state.persisted.names.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(u, _)| u.clone())?;
        Some(uuid)
    })
    .flatten()
}

fn display_name(server: &Server, uuid: &str) -> String {
    if let Some(player) = server.get_player_by_uuid(host::key_to_uuid(u128::from_str_radix(&uuid.replace('-', ""), 16).unwrap_or(0))) {
        return player.get_name();
    }
    with_state(|state| state.persisted.names.get(uuid).cloned()).flatten().unwrap_or_else(|| uuid.to_string())
}

fn economy_balance(uuid: &str) -> i64 {
    with_state(|state| economy::balance_of(&state.economy, uuid)).unwrap_or(0)
}

struct BalanceCmd {
    target: bool,
}
impl CommandHandler for BalanceCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let (uuid, label) = if self.target {
            let name = crate::cmdutil::plain(args.get_value("player"));
            let u = uuid_by_name(&server, &name).ok_or_else(|| fail("Jugador desconocido."))?;
            (u.clone(), display_name(&server, &u))
        } else {
            (uuid_of(&me), me.get_name())
        };
        let amount = with_state(|state| economy::balance_of(&state.economy, &uuid)).ok_or_else(unavailable)?;
        say(&sender, &format!("Saldo de {label}: {}.", money(amount)));
        Ok(1)
    }
}

struct BalanceTopCmd;
impl CommandHandler for BalanceTopCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let page: usize = greedy_arg(&args, "args").trim().parse().unwrap_or(1).max(1);
        let mut rows = with_state(|state| state.economy.balances.iter().map(|(u, v)| (u.clone(), *v)).collect::<Vec<_>>())
            .ok_or_else(unavailable)?;
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let per_page = 10usize;
        let start = (page - 1) * per_page;
        if rows.is_empty() || start >= rows.len() {
            say(&sender, "No hay datos para esa página.");
            return Ok(1);
        }
        say(&sender, &format!("--- Balances (página {page}) ---"));
        for (rank, (uuid, cents)) in rows.iter().skip(start).take(per_page).enumerate() {
            say(&sender, &format!("{}. {}: {}", start + rank + 1, display_name(&server, uuid), money(*cents)));
        }
        Ok(1)
    }
}

struct EcoCmd;
impl CommandHandler for EcoCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Three) {
            return Err(CommandError::PermissionDenied);
        }
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 3 {
            return Err(fail("Uso: /eco <set|add|take|reset> <jugador> <cantidad>"));
        }
        let mode = tokens[0].to_ascii_lowercase();
        let uuid = uuid_by_name(&server, tokens[1]).ok_or_else(|| fail("Jugador desconocido."))?;
        let amount = if mode == "reset" { 0 } else { parse_amount(tokens[2])? };
        let result = with_state(|state| {
            let r = match mode.as_str() {
                "set" => economy::set_balance(&mut state.economy, &uuid, amount),
                "add" => economy::adjust(&mut state.economy, &uuid, amount),
                "take" => economy::adjust(&mut state.economy, &uuid, -amount),
                "reset" => economy::set_balance(&mut state.economy, &uuid, 0),
                _ => Err("Modo desconocido. Usa set, add, take o reset.".into()),
            };
            if r.is_ok() {
                state.save_economy();
            }
            r
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        let _ = result;
        info!("essentials: /eco {mode} {uuid} {amount}");
        say(&sender, &format!("Economía actualizada: {} -> {}", display_name(&server, &uuid), money(economy_balance(&uuid))));
        Ok(1)
    }
}

struct PayCmd;
impl CommandHandler for PayCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail("Uso: /pay <jugador> <cantidad>"))?;
        let amount_raw = tokens.next().ok_or_else(|| fail("Uso: /pay <jugador> <cantidad>"))?;
        let amount = parse_amount(amount_raw)?;
        if amount <= 0 {
            return Err(fail("La cantidad debe ser positiva."));
        }
        let from = uuid_of(&player);
        let to = uuid_by_name(&server, name).ok_or_else(|| fail("Jugador desconocido."))?;
        if to == from {
            return Err(fail("No puedes pagarte a ti mismo."));
        }
        let (blocked, confirm_required, sufficient) = with_state(|state| {
            (
                state.economy.pay_off.get(&to).copied().unwrap_or(false),
                !state.economy.payconfirm_off.get(&from).copied().unwrap_or(false),
                economy::balance_of(&state.economy, &from) >= amount,
            )
        })
        .ok_or_else(unavailable)?;
        if blocked {
            return Err(fail("Ese jugador no acepta pagos."));
        }
        if !sufficient {
            return Err(fail("Saldo insuficiente."));
        }
        if confirm_required {
            with_state(|state| {
                state.economy.pending_pay.insert(
                    from.clone(),
                    PendingPay { to_uuid: to.clone(), to_name: display_name(&server, &to), amount, at_ms: host::now_ms() },
                );
                state.save_economy();
            })
            .ok_or_else(unavailable)?;
            say(&sender, &format!("Pago de {} a {name} preparado. Confirma con /payconfirm.", money(amount)));
            return Ok(1);
        }
        with_state(|state| {
            let r = economy::transfer(&mut state.economy, &from, &to, amount);
            if r.is_ok() { state.save_economy(); }
            r
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        say(&sender, &format!("Pagaste {} a {name}.", money(amount)));
        Ok(1)
    }
}

struct PayConfirmCmd;
impl CommandHandler for PayConfirmCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let from = uuid_of(&player);
        let pending = with_state(|state| state.economy.pending_pay.remove(&from)).ok_or_else(unavailable)?
            .ok_or_else(|| fail("No tienes ningún pago pendiente."))?;
        let result = with_state(|state| {
            let r = economy::transfer(&mut state.economy, &from, &pending.to_uuid, pending.amount);
            if r.is_err() {
                state.economy.pending_pay.insert(from.clone(), pending.clone());
            } else {
                state.save_economy();
            }
            r
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        let _ = result;
        say(&sender, &format!("Pago confirmado: {} a {}.", money(pending.amount), pending.to_name));
        Ok(1)
    }
}

struct PayToggleCmd;
impl CommandHandler for PayToggleCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        let (uuid, label) = if raw.is_empty() {
            (uuid_of(&me), me.get_name())
        } else {
            if !sender.has_permission_level(CmdLevel::Two) {
                return Err(CommandError::PermissionDenied);
            }
            let other = server.get_player_by_name(&raw).ok_or_else(|| fail("Jugador no conectado."))?;
            (uuid_of(&other), other.get_name())
        };
        let accepting = with_state(|state| {
            let off = !state.economy.pay_off.get(&uuid).copied().unwrap_or(false);
            state.economy.pay_off.insert(uuid, off);
            state.save_economy();
            !off
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("{label} {} aceptar pagos.", if accepting { "volverá a" } else { "dejará de" }));
        Ok(1)
    }
}

struct PayConfirmToggleCmd;
impl CommandHandler for PayConfirmToggleCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let uuid = uuid_of(&player);
        let no_confirm = with_state(|state| {
            let next = !state.economy.payconfirm_off.get(&uuid).copied().unwrap_or(false);
            state.economy.payconfirm_off.insert(uuid.clone(), next);
            state.economy.pending_pay.remove(&uuid);
            state.save_economy();
            next
        })
        .ok_or_else(unavailable)?;
        say(&sender, if no_confirm { "Confirmación desactivada: /pay transfiere al instante." } else { "Confirmación activada: usa /payconfirm después de /pay." });
        Ok(1)
    }
}

struct SellCmd;
impl CommandHandler for SellCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let filter = greedy_arg(&args, "args").trim().to_ascii_lowercase();
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let size = inv.get_size();
        let only_key = if filter.is_empty() || filter == "hand" || filter == "all" { None } else { Some(economy::normalize_item_key(&filter)) };
        let all = filter == "all";
        let slots: Vec<u32> = if filter == "hand" || filter.is_empty() {
            vec![u32::from(pinv.get_selected_slot())]
        } else {
            (0..size).collect()
        };
        let mut total = 0i64;
        let mut sold = 0u32;
        for slot in slots {
            let Some(item) = inv.remove_item(slot) else { continue };
            let key = item.get_registry_key();
            let unit = with_state(|state| economy::worth_of(&state.economy, &key)).ok_or_else(unavailable)?;
            let keep = unit <= 0 || only_key.as_ref().is_some_and(|wanted| wanted != &key);
            if keep {
                inv.set_item(slot, Some(item));
                continue;
            }
            total = total.saturating_add(unit.saturating_mul(i64::from(item.get_count())));
            sold += 1;
        }
        if sold == 0 {
            return Err(fail("No hay objetos vendibles en el inventario."));
        }
        let uuid = uuid_of(&player);
        with_state(|state| {
            let r = economy::adjust(&mut state.economy, &uuid, total);
            if r.is_ok() { state.save_economy(); }
            r
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        let _ = all;
        host::ok(&player, &format!("Vendiste {sold} montón(es) por {}.", money(total)));
        Ok(1)
    }
}

struct WorthCmd;
impl CommandHandler for WorthCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 {
            return Err(fail("Uso: /worth <cantidad> <ítem> (p. ej. /worth 32 minecraft:stone)"));
        }
        let count: u32 = tokens[0].parse().map_err(|_| fail("La cantidad debe ser un entero positivo."))?;
        let key = economy::normalize_item_key(&tokens[1..].join(" "));
        let unit = with_state(|state| economy::worth_of(&state.economy, &key)).ok_or_else(unavailable)?;
        say(&sender, &format!("{key}: {} por unidad, {} por {count}.", money(unit), money(unit.saturating_mul(i64::from(count)))));
        Ok(1)
    }
}

struct SetWorthCmd;
impl CommandHandler for SetWorthCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Three) {
            return Err(CommandError::PermissionDenied);
        }
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let key = economy::normalize_item_key(tokens.next().ok_or_else(|| fail("Uso: /setworth <ítem> <precio>"))?);
        let price = parse_amount(tokens.next().ok_or_else(|| fail("Uso: /setworth <ítem> <precio>"))?)?;
        with_state(|state| {
            state.economy.worth.insert(key.clone(), price);
            state.save_economy();
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Precio de {key} fijado en {}.", money(price)));
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
    for (n, d) in [
        ("balance", "Ver saldo"),
        ("balancetop", "Ver rankings de saldo"),
        ("pay", "Pagar a otros jugadores"),
        ("paytoggle", "Aceptar o rechazar pagos"),
        ("payconfirmtoggle", "Activar o desactivar confirmación de pagos"),
        ("payconfirm", "Confirmar un pago pendiente"),
        ("sell", "Vender objetos"),
        ("worth", "Consultar precios"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [("eco", "Administrar economía"), ("setworth", "Fijar precios")] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Three),
            children: vec![],
        })?;
    }

    reg(
        ctx,
        Command::new(&["balance".to_string()], "Ver saldo")
            .execute(BalanceCmd { target: false })
            .then(word("player").execute(BalanceCmd { target: true })),
        "pumpkin_essentials:balance",
    )?;
    reg(
        ctx,
        Command::new(&["bal".to_string()], "Ver saldo")
            .execute(BalanceCmd { target: false })
            .then(word("player").execute(BalanceCmd { target: true })),
        "pumpkin_essentials:balance",
    )?;
    reg(
        ctx,
        Command::new(&["balancetop".to_string()], "Ranking de saldos").then(greedy_node("args").execute(BalanceTopCmd)),
        "pumpkin_essentials:balancetop",
    )?;
    reg(
        ctx,
        Command::new(&["eco".to_string()], "Administrar economía").then(greedy_node("args").execute(EcoCmd)),
        "pumpkin_essentials:eco",
    )?;
    reg(
        ctx,
        Command::new(&["pay".to_string()], "Pagar").then(greedy_node("args").execute(PayCmd)),
        "pumpkin_essentials:pay",
    )?;
    reg(
        ctx,
        Command::new(&["paytoggle".to_string()], "Aceptar pagos").then(greedy_node("args").execute(PayToggleCmd)),
        "pumpkin_essentials:paytoggle",
    )?;
    reg(
        ctx,
        Command::new(&["payconfirmtoggle".to_string()], "Confirmación de pagos").execute(PayConfirmToggleCmd),
        "pumpkin_essentials:payconfirmtoggle",
    )?;
    reg(
        ctx,
        Command::new(&["payconfirm".to_string()], "Confirmar pago").execute(PayConfirmCmd),
        "pumpkin_essentials:payconfirm",
    )?;
    reg(
        ctx,
        Command::new(&["sell".to_string()], "Vender objetos").then(greedy_node("args").execute(SellCmd)),
        "pumpkin_essentials:sell",
    )?;
    reg(
        ctx,
        Command::new(&["worth".to_string()], "Valor de un objeto").then(greedy_node("args").execute(WorthCmd)),
        "pumpkin_essentials:worth",
    )?;
    reg(
        ctx,
        Command::new(&["setworth".to_string()], "Fijar precio").then(greedy_node("args").execute(SetWorthCmd)),
        "pumpkin_essentials:setworth",
    )?;
    Ok(())
}

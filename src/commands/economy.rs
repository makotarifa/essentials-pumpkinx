use crate::{
    cmdutil::{fail, greedy_arg, greedy_node, player_of, reg, say, unavailable, uuid_of, word, CmdResult},
    economy::{self, PendingPay},
    host::{self, with_state},
    i18n,
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
            let u = uuid_by_name(&server, &name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
            (u.clone(), display_name(&server, &u))
        } else {
            (uuid_of(&me), me.get_name())
        };
        let amount = with_state(|state| economy::balance_of(&state.economy, &uuid)).ok_or_else(unavailable)?;
        let balance = money(amount);
        say(&sender, &i18n::trf("Saldo de {}: {}.", "Balance for {}: {}.", &[&label, &balance]));
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
            say(&sender, i18n::tr("No hay datos para esa página.", "No data for that page."));
            return Ok(1);
        }
        let page_label = page.to_string();
        say(&sender, &i18n::trf("--- Balances (página {}) ---", "--- Balances (page {}) ---", &[&page_label]));
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
            return Err(fail(i18n::tr("Uso: /eco <set|add|take|reset> <jugador> <cantidad>", "Usage: /eco <set|add|take|reset> <player> <amount>")));
        }
        let mode = tokens[0].to_ascii_lowercase();
        let uuid = uuid_by_name(&server, tokens[1]).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        let amount = if mode == "reset" { 0 } else { parse_amount(tokens[2])? };
        let result = with_state(|state| {
            let r = match mode.as_str() {
                "set" => economy::set_balance(&mut state.economy, &uuid, amount),
                "add" => economy::adjust(&mut state.economy, &uuid, amount),
                "take" => economy::adjust(&mut state.economy, &uuid, -amount),
                "reset" => economy::set_balance(&mut state.economy, &uuid, 0),
                _ => Err(i18n::tr("Modo desconocido. Usa set, add, take o reset.", "Unknown mode. Use set, add, take or reset.").into()),
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
        let name = display_name(&server, &uuid);
        let balance = money(economy_balance(&uuid));
        say(&sender, &i18n::trf("Economía actualizada: {} -> {}", "Economy updated: {} -> {}", &[&name, &balance]));
        Ok(1)
    }
}

struct PayCmd;
impl CommandHandler for PayCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.split_whitespace();
        let name = tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /pay <jugador> <cantidad>", "Usage: /pay <player> <amount>")))?;
        let amount_raw = tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /pay <jugador> <cantidad>", "Usage: /pay <player> <amount>")))?;
        let amount = parse_amount(amount_raw)?;
        if amount <= 0 {
            return Err(fail(i18n::tr("La cantidad debe ser positiva.", "The amount must be positive.")));
        }
        let from = uuid_of(&player);
        let to = uuid_by_name(&server, name).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_UNKNOWN)))?;
        if to == from {
            return Err(fail(i18n::tr("No puedes pagarte a ti mismo.", "You cannot pay yourself.")));
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
            return Err(fail(i18n::tr("Ese jugador no acepta pagos.", "That player is not accepting payments.")));
        }
        if !sufficient {
            return Err(fail(i18n::tr("Saldo insuficiente.", "Insufficient balance.")));
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
            let amount_label = money(amount);
            say(&sender, &i18n::trf("Pago de {} a {} preparado. Confirma con /payconfirm.", "Payment of {} to {} prepared. Confirm with /payconfirm.", &[&amount_label, &name]));
            return Ok(1);
        }
        with_state(|state| {
            let r = economy::transfer(&mut state.economy, &from, &to, amount);
            if r.is_ok() { state.save_economy(); }
            r
        })
        .ok_or_else(unavailable)?
        .map_err(|error| fail(&error))?;
        let amount_label = money(amount);
        say(&sender, &i18n::trf("Pagaste {} a {}.", "You paid {} to {}.", &[&amount_label, &name]));
        Ok(1)
    }
}

struct PayConfirmCmd;
impl CommandHandler for PayConfirmCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let from = uuid_of(&player);
        let pending = with_state(|state| state.economy.pending_pay.remove(&from)).ok_or_else(unavailable)?
            .ok_or_else(|| fail(i18n::tr("No tienes ningún pago pendiente.", "You have no pending payments.")))?;
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
        let amount_label = money(pending.amount);
        say(&sender, &i18n::trf("Pago confirmado: {} a {}.", "Payment confirmed: {} to {}.", &[&amount_label, &pending.to_name]));
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
            let other = server.get_player_by_name(&raw).ok_or_else(|| fail(i18n::msg(i18n::PLAYER_NOT_FOUND)))?;
            (uuid_of(&other), other.get_name())
        };
        let accepting = with_state(|state| {
            let off = !state.economy.pay_off.get(&uuid).copied().unwrap_or(false);
            state.economy.pay_off.insert(uuid, off);
            state.save_economy();
            !off
        })
        .ok_or_else(unavailable)?;
        let action = if accepting { i18n::tr("volverá a aceptar pagos", "will accept payments again") } else { i18n::tr("dejará de aceptar pagos", "will stop accepting payments") };
        say(&sender, &i18n::trf("{} {}.", "{} {}.", &[&label, action]));
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
        say(&sender, if no_confirm {
            i18n::tr("Confirmación desactivada: /pay transfiere al instante.", "Confirmation disabled: /pay transfers instantly.")
        } else {
            i18n::tr("Confirmación activada: usa /payconfirm después de /pay.", "Confirmation enabled: use /payconfirm after /pay.")
        });
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
            return Err(fail(i18n::tr("No hay objetos vendibles en el inventario.", "There are no sellable items in the inventory.")));
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
        let sold_label = sold.to_string();
        let total_label = money(total);
        host::ok(&player, &i18n::trf("Vendiste {} montón(es) por {}.", "You sold {} stack(s) for {}.", &[&sold_label, &total_label]));
        Ok(1)
    }
}

struct WorthCmd;
impl CommandHandler for WorthCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() < 2 {
            return Err(fail(i18n::tr("Uso: /worth <cantidad> <ítem> (p. ej. /worth 32 minecraft:stone)", "Usage: /worth <amount> <item> (e.g. /worth 32 minecraft:stone)")));
        }
        let count: u32 = tokens[0].parse().map_err(|_| fail(i18n::tr("La cantidad debe ser un entero positivo.", "The amount must be a positive integer.")))?;
        let key = economy::normalize_item_key(&tokens[1..].join(" "));
        let unit = with_state(|state| economy::worth_of(&state.economy, &key)).ok_or_else(unavailable)?;
        let unit_label = money(unit);
        let total_label = money(unit.saturating_mul(i64::from(count)));
        let count_label = count.to_string();
        say(&sender, &i18n::trf("{}: {} por unidad, {} por {}.", "{}: {} per unit, {} for {}.", &[&key, &unit_label, &total_label, &count_label]));
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
        let key = economy::normalize_item_key(tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /setworth <ítem> <precio>", "Usage: /setworth <item> <price>")))?);
        let price = parse_amount(tokens.next().ok_or_else(|| fail(i18n::tr("Uso: /setworth <ítem> <precio>", "Usage: /setworth <item> <price>")))?)?;
        with_state(|state| {
            state.economy.worth.insert(key.clone(), price);
            state.save_economy();
        })
        .ok_or_else(unavailable)?;
        let price_label = money(price);
        say(&sender, &i18n::trf("Precio de {} fijado en {}.", "Price of {} set to {}.", &[&key, &price_label]));
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
        Command::new(&["balance".to_string()], i18n::tr("Ver saldo", "View balance"))
            .execute(BalanceCmd { target: false })
            .then(word("player").execute(BalanceCmd { target: true })),
        "pumpkin_essentials:balance",
    )?;
    reg(
        ctx,
        Command::new(&["bal".to_string()], i18n::tr("Ver saldo", "View balance"))
            .execute(BalanceCmd { target: false })
            .then(word("player").execute(BalanceCmd { target: true })),
        "pumpkin_essentials:balance",
    )?;
    reg(
        ctx,
        Command::new(&["balancetop".to_string()], i18n::tr("Ranking de saldos", "Balance ranking")).then(greedy_node("args").execute(BalanceTopCmd)),
        "pumpkin_essentials:balancetop",
    )?;
    reg(
        ctx,
        Command::new(&["eco".to_string()], i18n::tr("Administrar economía", "Manage economy")).then(greedy_node("args").execute(EcoCmd)),
        "pumpkin_essentials:eco",
    )?;
    reg(
        ctx,
        Command::new(&["pay".to_string()], i18n::tr("Pagar", "Pay")).then(greedy_node("args").execute(PayCmd)),
        "pumpkin_essentials:pay",
    )?;
    reg(
        ctx,
        Command::new(&["paytoggle".to_string()], i18n::tr("Aceptar pagos", "Accept payments")).then(greedy_node("args").execute(PayToggleCmd)),
        "pumpkin_essentials:paytoggle",
    )?;
    reg(
        ctx,
        Command::new(&["payconfirmtoggle".to_string()], i18n::tr("Confirmación de pagos", "Payment confirmation")).execute(PayConfirmToggleCmd),
        "pumpkin_essentials:payconfirmtoggle",
    )?;
    reg(
        ctx,
        Command::new(&["payconfirm".to_string()], i18n::tr("Confirmar pago", "Confirm payment")).execute(PayConfirmCmd),
        "pumpkin_essentials:payconfirm",
    )?;
    reg(
        ctx,
        Command::new(&["sell".to_string()], i18n::tr("Vender objetos", "Sell items")).then(greedy_node("args").execute(SellCmd)),
        "pumpkin_essentials:sell",
    )?;
    reg(
        ctx,
        Command::new(&["worth".to_string()], i18n::tr("Valor de un objeto", "Item value")).then(greedy_node("args").execute(WorthCmd)),
        "pumpkin_essentials:worth",
    )?;
    reg(
        ctx,
        Command::new(&["setworth".to_string()], i18n::tr("Fijar precio", "Set price")).then(greedy_node("args").execute(SetWorthCmd)),
        "pumpkin_essentials:setworth",
    )?;
    Ok(())
}

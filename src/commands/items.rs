use crate::{
    cmdutil::{fail, forward, give_stack, greedy_arg, greedy_node, has_level, no_arg, player_of, plain, reg, say, unavailable, uuid_of, word, CmdResult},
    environment,
    host::{self, with_state},
    kits,
};
use pumpkin_plugin_api::{
    command::{CommandError, CommandSender, ConsumedArgs},
    commands::{Command, CommandHandler},
    common::{BlockPos, NamedColor},
    gui::Gui,
    permission::{Permission, PermissionDefault, PermissionLevel},
    text::TextComponent,
    wit::pumpkin::plugin::command::PermissionLevel as CmdLevel,
    world::BlockFlags,
    Context, EntityType, ItemStack, Player, Screen, Server,
};
use std::collections::BTreeMap;

fn held_slot(player: &Player) -> u32 {
    u32::from(player.get_inventory().get_selected_slot())
}

fn parse_entity(raw: &str) -> Option<EntityType> {
    match kits::normalize_item_key(raw).as_str() {
        "minecraft:pig" => Some(EntityType::Pig),
        "minecraft:cow" => Some(EntityType::Cow),
        "minecraft:sheep" => Some(EntityType::Sheep),
        "minecraft:chicken" => Some(EntityType::Chicken),
        "minecraft:zombie" => Some(EntityType::Zombie),
        "minecraft:creeper" => Some(EntityType::Creeper),
        "minecraft:skeleton" => Some(EntityType::Skeleton),
        "minecraft:spider" => Some(EntityType::Spider),
        "minecraft:enderman" => Some(EntityType::Enderman),
        "minecraft:wolf" => Some(EntityType::Wolf),
        "minecraft:cat" => Some(EntityType::Cat),
        "minecraft:bee" => Some(EntityType::Bee),
        "minecraft:horse" => Some(EntityType::Horse),
        "minecraft:villager" => Some(EntityType::Villager),
        "minecraft:bat" => Some(EntityType::Bat),
        "minecraft:blaze" => Some(EntityType::Blaze),
        "minecraft:slime" => Some(EntityType::Slime),
        "minecraft:rabbit" => Some(EntityType::Rabbit),
        "minecraft:fox" => Some(EntityType::Fox),
        "minecraft:ocelot" => Some(EntityType::Ocelot),
        "minecraft:snow_golem" => Some(EntityType::SnowGolem),
        "minecraft:iron_golem" => Some(EntityType::IronGolem),
        "minecraft:witch" => Some(EntityType::Witch),
        "minecraft:drowned" => Some(EntityType::Drowned),
        "minecraft:husk" => Some(EntityType::Husk),
        "minecraft:stray" => Some(EntityType::Stray),
        "minecraft:wither_skeleton" => Some(EntityType::WitherSkeleton),
        "minecraft:guardian" => Some(EntityType::Guardian),
        "minecraft:dolphin" => Some(EntityType::Dolphin),
        "minecraft:turtle" => Some(EntityType::Turtle),
        "minecraft:fireball" => Some(EntityType::Fireball),
        "minecraft:small_fireball" => Some(EntityType::SmallFireball),
        "minecraft:arrow" => Some(EntityType::Arrow),
        "minecraft:egg" => Some(EntityType::Egg),
        "minecraft:snowball" => Some(EntityType::Snowball),
        "minecraft:ender_pearl" => Some(EntityType::EnderPearl),
        "minecraft:wither_skull" => Some(EntityType::WitherSkull),
        _ => None,
    }
}

struct ItemCmd;
impl CommandHandler for ItemCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.is_empty() { return Err(fail("Uso: /item <id> [cantidad] [jugador]")); }
        let key = kits::normalize_item_key(tokens[0]);
        let count = tokens.get(1).and_then(|s| s.parse::<u8>().ok()).filter(|n| *n > 0).unwrap_or(1).min(64);
        let target = if let Some(name) = tokens.get(2) {
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            server.get_player_by_name(name).ok_or_else(|| fail("Jugador no conectado."))?
        } else { me };
        give_stack(&target, &key, count)?;
        host::ok(&target, &format!("Recibiste {count}x{key}."));
        Ok(1)
    }
}

struct ItemDbCmd;
impl CommandHandler for ItemDbCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let raw = greedy_arg(&args, "args").trim().to_string();
        if raw.is_empty() {
            let player = player_of(&sender)?;
            let slot = held_slot(&player);
            let key = player.get_inventory().as_inventory().get_item(slot).map(|i| i.get_registry_key()).unwrap_or_else(|| "minecraft:air".into());
            say(&sender, &format!("Ítem en mano: {key}"));
        } else {
            say(&sender, &format!("Clave: {}", kits::normalize_item_key(&raw)));
        }
        Ok(1)
    }
}

struct ItemLoreCmd;
impl CommandHandler for ItemLoreCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let mut tokens = raw.splitn(2, char::is_whitespace);
        let action = tokens.next().unwrap_or("").to_ascii_lowercase();
        let text = tokens.next().unwrap_or("").trim().to_string();
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = held_slot(&player);
        let Some(item) = inv.get_item(slot) else { return Err(fail("No tienes un ítem en la mano.")); };
        let mut lore = item.get_lore();
        match action.as_str() {
            "add" | "set" => {
                if text.is_empty() { return Err(fail("Falta el texto del lore.")); }
                if action == "set" { lore.clear(); }
                lore.push(TextComponent::text(&text));
            }
            "clear" => lore.clear(),
            _ => return Err(fail("Uso: /itemlore add|set|clear <texto>")),
        }
        item.set_lore(lore);
        inv.set_item(slot, Some(item));
        host::ok(&player, "Lore actualizado.");
        Ok(1)
    }
}

struct ItemNameCmd;
impl CommandHandler for ItemNameCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = held_slot(&player);
        let Some(item) = inv.get_item(slot) else { return Err(fail("No tienes un ítem en la mano.")); };
        if raw.is_empty() || raw.eq_ignore_ascii_case("off") {
            item.set_custom_name(None);
        } else {
            item.set_custom_name(Some(TextComponent::text(&raw).color_named(NamedColor::Gold)));
        }
        inv.set_item(slot, Some(item));
        host::ok(&player, "Nombre del ítem actualizado.");
        Ok(1)
    }
}

struct RepairCmd;
impl CommandHandler for RepairCmd {
    fn handle(&self, _sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        Err(fail("La API WIT 0.1 de Pumpkin no expone durabilidad de objetos (get_damage/set_damage), por lo que /repair no puede restaurarlos sin destruir sus datos. Limitación documentada."))
    }
}

struct UnlimitedCmd;
impl CommandHandler for UnlimitedCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let action = tokens.first().copied().unwrap_or("").to_ascii_lowercase();
        let target = if tokens.len() > 1 {
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            server.get_player_by_name(tokens[1]).ok_or_else(|| fail("Jugador no conectado."))?
        } else { me };
        let uuid = uuid_of(&target);
        let (message, current) = with_state(|s| {
            let list = s.kits.unlimited.entry(uuid.clone()).or_default();
            match action.as_str() {
                "list" => (format!("Objetos ilimitados: {}", if list.is_empty() { "ninguno".into() } else { list.join(", ") }), list.clone()),
                "clear" => { list.clear(); ("Objetos ilimitados borrados.".into(), Vec::new()) }
                "" => (format!("Objetos ilimitados: {}", if list.is_empty() { "ninguno".into() } else { list.join(", ") }), list.clone()),
                _ => {
                    let key = kits::normalize_item_key(action.as_str());
                    let now_unlimited = if let Some(pos) = list.iter().position(|k| k == &key) { list.remove(pos); false } else { list.push(key.clone()); true };
                    (format!("{key}: {}", if now_unlimited { "ilimitado" } else { "limitado" }), list.clone())
                }
            }
        })
        .ok_or_else(unavailable)?;
        with_state(|state| state.save_kits());
        let _ = current;
        say(&sender, &message);
        Ok(1)
    }
}

struct HatCmd;
impl CommandHandler for HatCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = held_slot(&player);
        let Some(held) = inv.get_item(slot) else { return Err(fail("No tienes un ítem en la mano.")); };
        let old = pinv.get_helmet();
        pinv.set_helmet(Some(held));
        inv.set_item(slot, old);
        host::ok(&player, "Sombrero colocado.");
        Ok(1)
    }
}

struct MoreCmd;
impl CommandHandler for MoreCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = held_slot(&player);
        let Some(item) = inv.get_item(slot) else { return Err(fail("No tienes un ítem en la mano.")); };
        let max = item.get_max_count();
        let extra = greedy_arg(&args, "args").trim().parse::<u8>().unwrap_or(max).max(1);
        let next = item.get_count().saturating_add(extra).min(max);
        if next == item.get_count() { return Err(fail("El montón ya está completo.")); }
        item.set_count(next);
        inv.set_item(slot, Some(item));
        host::ok(&player, &format!("Montón ampliado a {next}."));
        Ok(1)
    }
}

struct BookCmd;
impl CommandHandler for BookCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        give_stack(&player, "minecraft:writable_book", 1)?;
        host::ok(&player, "Libro entregado. La API no permite editar páginas ni título desde el plugin.");
        Ok(1)
    }
}

struct FireworkCmd;
impl CommandHandler for FireworkCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = (0..inv.get_size()).find(|s| inv.get_item(*s).is_none()).ok_or_else(|| fail("Inventario lleno."))?;
        inv.set_item(slot, Some(ItemStack::new("minecraft:firework_rocket", 1)));
        host::ok(&player, "Cohete entregado.");
        Ok(1)
    }
}

struct PowerToolCmd;
impl CommandHandler for PowerToolCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let raw = greedy_arg(&args, "args").trim().to_string();
        if raw.is_empty() { return Err(fail("Uso: /powertool <comando|d:> [ítem]")); }
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = held_slot(&player);
        let key = inv.get_item(slot).map(|i| i.get_registry_key()).ok_or_else(|| fail("No tienes un ítem en la mano."))?;
        let uuid = uuid_of(&player);
        with_state(|s| {
            let entry = s.kits.powertools.entry(uuid).or_default();
            let list = entry.entry(key.clone()).or_default();
            if raw.eq_ignore_ascii_case("d:") {
                list.clear();
            } else if raw.starts_with("d:") {
                let removable = raw[2..].trim().to_ascii_lowercase();
                list.retain(|c| c.to_ascii_lowercase() != removable);
            } else {
                list.push(raw.trim_start_matches('/').to_string());
            }
            s.save_kits();
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Powertool de {key} actualizado."));
        Ok(1)
    }
}

struct PowerToolListCmd;
impl CommandHandler for PowerToolListCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let list = with_state(|state| kits::list_powertools(&state.kits, &uuid_of(&player))).ok_or_else(unavailable)?;
        if list.is_empty() { return Err(fail("No tienes powertools.")); }
        for (key, commands) in list {
            say(&sender, &format!("{key}: {}", commands.join(", ")));
        }
        Ok(1)
    }
}

struct PowerToolToggleCmd;
impl CommandHandler for PowerToolToggleCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let uuid = uuid_of(&player);
        let on = with_state(|s| {
            let next = !s.kits.powertool_toggle.get(&uuid).copied().unwrap_or(true);
            s.kits.powertool_toggle.insert(uuid, next);
            s.save_kits();
            next
        })
        .ok_or_else(unavailable)?;
        say(&sender, &format!("Powertools {}.", if on { "activados" } else { "desactivados" }));
        Ok(1)
    }
}

struct CondenseCmd;
impl CommandHandler for CondenseCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let filter = greedy_arg(&args, "args").trim().to_string();
        let wanted = if filter.is_empty() { None } else { Some(kits::normalize_item_key(&filter)) };
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for slot in 0..inv.get_size() {
            if let Some(item) = inv.remove_item(slot) {
                let key = item.get_registry_key();
                if wanted.as_ref().is_none_or(|w| w == &key) {
                    *counts.entry(key).or_default() += u32::from(item.get_count());
                } else {
                    inv.set_item(slot, Some(item));
                }
            }
        }
        let mut changed = false;
        for (key, total) in counts {
            let mut left = total;
            while left > 0 {
                let empty = (0..inv.get_size()).find(|s| inv.get_item(*s).is_none()).ok_or_else(|| fail("Inventario lleno al condensar."))?;
                let count = left.min(64) as u8;
                inv.set_item(empty, Some(ItemStack::new(&key, count)));
                left -= u32::from(count);
                changed = true;
            }
        }
        if !changed { return Err(fail("No había objetos para condensar.")); }
        host::ok(&player, "Inventario condensado.");
        Ok(1)
    }
}

fn view_inventory(
    viewer: &Player,
    title: String,
    source: &pumpkin_plugin_api::inventory::Inventory,
    slots: u32,
) -> Result<(), String> {
    let gui = Gui::new(Screen::Generic9x4, TextComponent::text(&title));
    let target = gui.get_inventory();
    let count = slots.min(target.get_size()).min(source.get_size());
    for slot in 0..count {
        let item = source.get_item(slot);
        target.set_item(slot, item);
    }
    gui.set_allow_grab_items(false);
    gui.set_allow_put_items(false);
    viewer.open_gui(gui);
    Ok(())
}

struct InvSeeCmd;
impl CommandHandler for InvSeeCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !sender.has_permission_level(CmdLevel::Two) {
            return Err(CommandError::PermissionDenied);
        }
        let me = player_of(&sender)?;
        let name = plain(args.get_value("player"));
        let target = server
            .get_player_by_name(&name)
            .ok_or_else(|| fail(&format!("Jugador '{name}' no conectado.")))?;
        let inv = target.get_inventory().as_inventory();
        view_inventory(&me, format!("Inventario de {}", target.get_name()), &inv, 36).map_err(|error| fail(&error))?;
        Ok(1)
    }
}

struct EnderChestCmd {
    target: bool,
}
impl CommandHandler for EnderChestCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let target = crate::cmdutil::optional_target(&sender, &server, &me, &args, self.target)?;
        if host::key_of(&target) == host::key_of(&me) {
            me.open_ender_chest();
            return Ok(1);
        }
        let inv = target.get_ender_chest();
        view_inventory(&me, format!("Ender chest de {}", target.get_name()), &inv, 27).map_err(|error| fail(&error))?;
        Ok(1)
    }
}

struct ClearInventoryCmd;
impl CommandHandler for ClearInventoryCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        let me = player_of(&sender)?;
        let me_uuid = uuid_of(&me);
        let me_key = host::key_of(&me);
        let name = greedy_arg(&args, "args").trim().to_string();
        let target = if name.is_empty() {
            me
        } else {
            if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
            server.get_player_by_name(&name).ok_or_else(|| fail("Jugador no conectado."))?
        };
        let label = format!("clear:{}", uuid_of(&target));
        let (needed, confirmed) = with_state(|s| {
            let needed = environment::needs_confirmation(&s.persisted, &me_uuid);
            let confirmed = environment::take_or_set(&mut s.pending_confirm, me_key, &label);
            (needed, confirmed)
        })
        .ok_or_else(unavailable)?;
        if needed && !confirmed {
            return Err(fail("Confirma con /clearinventory otra vez (o usa /clearinventoryconfirmtoggle)."));
        }
        target.get_inventory().clear_all();
        host::ok(&target, "Inventario vaciado.");
        Ok(1)
    }
}

struct ClearConfirmToggleCmd;
impl CommandHandler for ClearConfirmToggleCmd {
    fn handle(&self, sender: CommandSender, _server: Server, _args: ConsumedArgs) -> CmdResult {
        let player = player_of(&sender)?;
        let off = with_state(|s| {
            let off = environment::toggle_confirmation(&mut s.persisted, &uuid_of(&player));
            s.save_persisted();
            off
        })
        .ok_or_else(unavailable)?;
        host::ok(&player, if off { "Confirmación desactivada." } else { "Confirmación activada." });
        Ok(1)
    }
}

struct RemoveCmd;
impl CommandHandler for RemoveCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let filter = greedy_arg(&args, "args").trim().to_string();
        let wanted = if filter.is_empty() { None } else { parse_entity(&filter) };
        if !filter.is_empty() && wanted.is_none() { return Err(fail("Tipo de entidad no reconocido.")); }
        let world = player.get_world();
        let entities = world.get_entities();
        let player_ids: Vec<u32> = server.get_players_in_world(world).iter().map(|pl| pl.as_entity().get_id()).collect();
        let mut removed = 0;
        for entity in entities {
            if player_ids.contains(&entity.get_id()) { continue; }
            if wanted.is_some() { continue; }
            entity.remove();
            removed += 1;
        }
        say(&sender, &format!("Eliminadas {removed} entidades."));
        Ok(1)
    }
}

struct SpawnMobCmd;
impl CommandHandler for SpawnMobCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let me = player_of(&sender)?;
        let raw = greedy_arg(&args, "args");
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        let kind = parse_entity(tokens.first().copied().unwrap_or("")).ok_or_else(|| fail("Mob no reconocido."))?;
        let count = tokens.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1).min(50);
        let target = if let Some(name) = tokens.get(2) {
            server.get_player_by_name(name).ok_or_else(|| fail("Jugador no conectado."))?
        } else { me };
        let (x, y, z) = target.get_position();
        for i in 0..count {
            target.get_world().spawn_entity(kind, (x + f64::from(i % 5), y + 1.0, z + f64::from(i / 5)));
        }
        say(&sender, &format!("Generadas {count} entidades."));
        Ok(1)
    }
}

struct SpawnerCmd;
impl CommandHandler for SpawnerCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Three) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let kind = greedy_arg(&args, "args").trim().to_string();
        if parse_entity(&kind).is_none() { return Err(fail("Mob no reconocido.")); }
        let (x, y, z) = forward(&player, 3.0);
        let pos = BlockPos { x: x.floor() as i32, y: y.floor() as i32, z: z.floor() as i32 };
        let ok = player.get_world().set_block_by_name(pos, "minecraft:spawner", BlockFlags::NOTIFY_NEIGHBORS | BlockFlags::NOTIFY_LISTENERS);
        if !ok { return Err(fail("No se pudo colocar el spawner.")); }
        host::ok(&player, "Spawner colocado (el tipo de mob no se puede guardar con la API WIT 0.1).");
        Ok(1)
    }
}

struct SkullCmd;
impl CommandHandler for SkullCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let name = greedy_arg(&args, "args").trim().to_string();
        if name.is_empty() { return Err(fail("Uso: /skull <nombre>")); }
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = inv.get_size().saturating_sub(1);
        let item = ItemStack::new("minecraft:player_head", 1);
        item.set_custom_name(Some(TextComponent::text(&name)));
        inv.set_item(slot, Some(item));
        host::ok(&player, &format!("Cabeza de {name} entregada (sin textura de perfil)."));
        Ok(1)
    }
}

struct FunSpawnCmd {
    kind: FunKind,
}
#[derive(Clone, Copy)]
enum FunKind {
    Kitty,
    Bee,
    Fireball,
}
impl CommandHandler for FunSpawnCmd {
    fn handle(&self, sender: CommandSender, server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let (entity_kind, count) = match self.kind {
            FunKind::Kitty => (EntityType::Cat, 5u32),
            FunKind::Bee => (EntityType::Bee, 1),
            FunKind::Fireball => {
                let raw = greedy_arg(&args, "args").trim().to_string();
                (parse_entity(if raw.is_empty() { "fireball" } else { &raw }).unwrap_or(EntityType::Fireball), 1)
            }
        };
        let (dx, dy, dz) = forward(&player, 2.0);
        let (px, py, pz) = player.get_position();
        let vx = dx - px;
        let vy = dy - py;
        let vz = dz - pz;
        for i in 0..count {
            let entity = player.get_world().spawn_entity(entity_kind, (dx + f64::from(i % 3), dy + 1.0, dz + f64::from(i / 3)));
            entity.set_velocity((vx * 2.0, vy * 2.0, vz * 2.0));
            if matches!(self.kind, FunKind::Bee) {
                player.get_world().create_explosion((dx, dy, dz), 0.5, false, pumpkin_plugin_api::world::ExplosionInteraction::None);
            }
        }
        let _ = server;
        say(&sender, "Entidades lanzadas.");
        Ok(1)
    }
}

struct PotionCmd;
impl CommandHandler for PotionCmd {
    fn handle(&self, sender: CommandSender, _server: Server, args: ConsumedArgs) -> CmdResult {
        if !has_level(&sender, CmdLevel::Two) { return Err(CommandError::PermissionDenied); }
        let player = player_of(&sender)?;
        let effect = greedy_arg(&args, "args").trim().to_string();
        if effect.is_empty() { return Err(fail("Uso: /potion <efecto>")); }
        let pinv = player.get_inventory();
        let inv = pinv.as_inventory();
        let slot = inv.get_size().saturating_sub(1);
        let item = ItemStack::new("minecraft:potion", 1);
        item.set_custom_name(Some(TextComponent::text(&format!("Poción: {effect}")).color_named(NamedColor::DarkPurple)));
        inv.set_item(slot, Some(item));
        host::ok(&player, "Poción entregada como objeto con nombre; la API no permite fijar efectos NBT.");
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
        ("itemdb", "Consultar claves de objetos"),
        ("powertoollist", "Listar powertools"),
        ("powertooltoggle", "Activar powertools"),
    ] {
        ctx.register_permission(&allow(n, d))?;
    }
    for (n, d) in [
        ("item", "Crear objetos"),
        ("itemlore", "Editar lore"),
        ("itemname", "Renombrar objetos"),
        ("repair", "Reparar objetos"),
        ("unlimited", "Objetos ilimitados"),
        ("hat", "Ponerse objetos en la cabeza"),
        ("more", "Ampliar montones"),
        ("book", "Entregar libros"),
        ("powertool", "Asignar powertools"),
        ("condense", "Condensar inventario"),
        ("skull", "Cabezas de jugador"),
        ("kittycannon", "Lanzar gatos"),
        ("beezooka", "Lanzar abejas"),
        ("fireball", "Lanzar proyectiles"),
        ("potion", "Entregar pociones"),
        ("invsee", "Ver el inventario de otro jugador"),
        ("enderchest", "Abrir un ender chest"),
        ("clearinventory", "Vaciar inventarios"),
        ("clearinventoryconfirmtoggle", "Confirmación de inventario"),
        ("firework", "Entregar cohete pirotécnico"),
    ] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Two),
            children: vec![],
        })?;
    }
    for (n, d) in [("remove", "Eliminar entidades"), ("spawnmob", "Generar entidades"), ("spawner", "Colocar spawners")] {
        ctx.register_permission(&Permission {
            node: format!("pumpkin_essentials:{n}"),
            description: d.into(),
            default: PermissionDefault::Op(PermissionLevel::Three),
            children: vec![],
        })?;
    }

    reg(ctx, Command::new(&["item".to_string()], "Crear objeto").then(greedy_node("args").execute(ItemCmd)), "pumpkin_essentials:item")?;
    reg(ctx, Command::new(&["itemdb".to_string()], "Clave de objeto").then(greedy_node("args").execute(ItemDbCmd)), "pumpkin_essentials:itemdb")?;
    reg(ctx, Command::new(&["itemlore".to_string()], "Editar lore").then(greedy_node("args").execute(ItemLoreCmd)), "pumpkin_essentials:itemlore")?;
    reg(ctx, Command::new(&["itemname".to_string()], "Renombrar objeto").then(greedy_node("args").execute(ItemNameCmd)), "pumpkin_essentials:itemname")?;
    reg(ctx, Command::new(&["repair".to_string()], "Reparar").execute(RepairCmd), "pumpkin_essentials:repair")?;
    reg(ctx, Command::new(&["unlimited".to_string()], "Objetos ilimitados").then(greedy_node("args").execute(UnlimitedCmd)), "pumpkin_essentials:unlimited")?;
    reg(ctx, no_arg(&["hat"], "Poner ítem en la cabeza", HatCmd), "pumpkin_essentials:hat")?;
    reg(ctx, Command::new(&["more".to_string()], "Ampliar montón").then(greedy_node("args").execute(MoreCmd)), "pumpkin_essentials:more")?;
    reg(ctx, no_arg(&["book"], "Entregar libro", BookCmd), "pumpkin_essentials:book")?;
    reg(ctx, no_arg(&["firework"], "Entregar cohete", FireworkCmd), "pumpkin_essentials:firework")?;
    reg(ctx, Command::new(&["powertool".to_string()], "Asignar comando a ítem").then(greedy_node("args").execute(PowerToolCmd)), "pumpkin_essentials:powertool")?;
    reg(ctx, no_arg(&["powertoollist"], "Listar powertools", PowerToolListCmd), "pumpkin_essentials:powertoollist")?;
    reg(ctx, no_arg(&["powertooltoggle"], "Activar powertools", PowerToolToggleCmd), "pumpkin_essentials:powertooltoggle")?;
    reg(ctx, Command::new(&["condense".to_string()], "Condensar inventario").then(greedy_node("args").execute(CondenseCmd)), "pumpkin_essentials:condense")?;
    reg(ctx, Command::new(&["remove".to_string()], "Eliminar entidades").then(greedy_node("args").execute(RemoveCmd)), "pumpkin_essentials:remove")?;
    reg(ctx, Command::new(&["spawnmob".to_string()], "Generar mob").then(greedy_node("args").execute(SpawnMobCmd)), "pumpkin_essentials:spawnmob")?;
    reg(ctx, Command::new(&["spawner".to_string()], "Colocar spawner").then(greedy_node("args").execute(SpawnerCmd)), "pumpkin_essentials:spawner")?;
    reg(ctx, Command::new(&["skull".to_string()], "Cabeza de jugador").then(greedy_node("args").execute(SkullCmd)), "pumpkin_essentials:skull")?;
    reg(ctx, Command::new(&["kittycannon".to_string()], "Lanzar gatos").execute(FunSpawnCmd { kind: FunKind::Kitty }), "pumpkin_essentials:kittycannon")?;
    reg(ctx, Command::new(&["beezooka".to_string()], "Lanzar abeja").execute(FunSpawnCmd { kind: FunKind::Bee }), "pumpkin_essentials:beezooka")?;
    reg(ctx, Command::new(&["fireball".to_string()], "Lanzar proyectil").then(greedy_node("args").execute(FunSpawnCmd { kind: FunKind::Fireball })), "pumpkin_essentials:fireball")?;
    reg(ctx, Command::new(&["potion".to_string()], "Entregar poción").then(greedy_node("args").execute(PotionCmd)), "pumpkin_essentials:potion")?;
    reg(ctx, Command::new(&["invsee".to_string()], "Ver el inventario de un jugador (op)").then(word("player").execute(InvSeeCmd)), "pumpkin_essentials:invsee")?;
    reg(
        ctx,
        Command::new(&["enderchest".to_string()], "Abre tu ender chest o ve el de otro jugador")
            .execute(EnderChestCmd { target: false })
            .then(word("player").execute(EnderChestCmd { target: true })),
        "pumpkin_essentials:enderchest",
    )?;
    reg(
        ctx,
        Command::new(&["clearinventory".to_string()], "Vaciar inventario").then(greedy_node("args").execute(ClearInventoryCmd)),
        "pumpkin_essentials:clearinventory",
    )?;
    reg(
        ctx,
        no_arg(&["clearinventoryconfirmtoggle"], "Confirmación de inventario", ClearConfirmToggleCmd),
        "pumpkin_essentials:clearinventoryconfirmtoggle",
    )?;
    Ok(())
}

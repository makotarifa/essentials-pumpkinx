# EssentialsPumpkinX


[![CI](https://github.com/makotarifa/essentials-pumpkinx/actions/workflows/ci.yml/badge.svg)](https://github.com/makotarifa/essentials-pumpkinx/actions/workflows/ci.yml)

A single WASM plugin for Pumpkin that merges **PumpkinEssentials 0.1.0** (homes, warps, `/tpa`, `/back`, warmups/cooldowns), **EssentialsX**-style utilities, and the **VEssentials 1.2.7** functions (`/gm[a|c|s|sp]`, `/fly`, `/flyspeed`, `/afk`).

> **Development status:** EssentialsPumpkinX is under active development and is not feature-complete. Some EssentialsX behaviours cannot be reproduced with the current Pumpkin WIT API; the exact gaps are documented in [`COMPAT.md`](COMPAT.md). Bug reports and pull requests are welcome at https://github.com/makotarifa/essentials-pumpkinx/pulls.

## Compatibility

- Server: Pumpkin `0.2.2+26.3-26.60` (Java protocol 777 / Bedrock 1.26.60).
- API: `pumpkin-plugin-api` from the server's exact tag (`rev eda69440e6369c5f8a05c3f69fc809c40dab93a6`), WIT bindings `pumpkin:plugin@0.1.0`, WASI `p2` (0.2.x).
- WASI permissions: `fs.read.data` and `fs.write.data`.
- `PluginMetadata.name` is **`pumpkin_essentials`** by design: all permission nodes use that namespace and the data folder is the usual one.

## Code structure

```text
src/
  cmdutil.rs        // shared helpers for the command layer
  commands/         // central registry + one file per domain
    homes.rs warps.rs tpa.rs movement.rs gamemode.rs info.rs
    chat.rs moderation.rs economy.rs kits.rs items.rs
    environment.rs misc.rs mod.rs
  store/persist.rs  // FileStore: load, migration, and atomic save
  host/host_traits.rs // minimal adapter over the WIT API
```

Coding conventions, the command checklist, and design guidelines are in [`SKILLS.md`](SKILLS.md).

## Building

```sh
rustup toolchain install stable
rustup target add wasm32-wasip2
cd dev/essentials-pumpkinx
cargo test --lib
cargo check --target wasm32-wasip2
cargo build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/essentials_pumpkinx.wasm ./essentials_pumpkinx.wasm
```

`cargo check --target wasm32-wasip2` is part of the verification because ~70% of the code (everything that depends on Pumpkin's WIT API) only compiles for that target and is therefore only checked when building the `.wasm`.

`Cargo.toml` pins the Pumpkin upstream repository to the server tag; there are no private dependencies.

## CI

- `ci.yml` runs on pushes to `main`, pull requests, and manual dispatch: host unit tests, WASM check, release build, checksum generation, and artifact upload.
- `release.yml` runs on `v*` tags: the same checks, then publishes the `.wasm` and `.sha256` as a GitHub Release.

## Installation and migration

1. Shut down the server (or use the plugin reload).
2. Remove or rename `plugins/PumpkinEssentials.wasm` so that two plugins with the same name do not coexist.
3. Copy `essentials_pumpkinx.wasm` to `plugins/` and start the server.
4. File permissions are approved automatically; if your configuration blocks them:

   ```toml
   [plugins.overrides.pumpkin_essentials]
   enabled = true
   allowed_permissions = ["fs.read.data", "fs.write.data"]
   ```

Data is read from and written to `plugins/data/pumpkin_essentials/` **directly**; there is no copy or conversion. The canonical formats are preserved:

```jsonc
// homes.json
{ "<uuid>": { "name_last_seen": "Name", "limit": 5, "homes": { "base": { "world": "world", "x": 1.0, "y": 64.0, "z": 2.0, "yaw": 0.0, "pitch": 0.0 } } } }
// warps.json
{ "spawn": { "world": "world", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0, "created_by": "Owner" } }
```

Automatic migration on load: `players`/`warps` wrappers, `position`/`rotation`, `dimension`/`world_name`, numbers as strings, missing `created_by`, and the `{"state": ...}` wrapper in `state.json`. Everything is rewritten atomically only if it changed. Invalid JSON is **never** overwritten: the plugin fails to load in order to protect the data.

### `config.json`

Keeps the five original keys and adds new options with default values:

| Key | Default | Use |
|---|---|---|
| `default_max_homes` | `3` | Per-player home limit |
| `ops_bypass_home_limit` | `false` | Ops ignore the limit |
| `warmup_secs` | `3` | Wait before teleporting (0 = instant) |
| `language` | `"en"` | User-facing language: `"en"` or `"es"` |
| `cooldown_secs` | `30` | Cooldown for `/home` and `/tpa` |
| `tpa_expire_secs` | `60` | Request expiry |
| `flyspeed_unit` | `0.05` | Engine speed per 1 unit of `/flyspeed` |
| `afk_broadcast` | `true` | Announce AFK changes |
| `afk_auto_unafk_on_move` | `true` | Clear AFK when moving >0.5 blocks |
| `afk_tab_name` | `true` | Show `[AFK]` in the tab list |
| `restore_fly_on_join` | `true` | Restore saved flight on join |
| `restore_afk_on_join` | `true` | Restore saved AFK on join |
| `spawns` | `{}` | Per-world spawns saved by `/setspawn` |

An older `config.json` remains valid.

Change it at runtime with `/essentials language <en|es>`; the command updates `config.json`. Static texts live in `src/i18n.rs`; internal logs stay in English.

## Commands

### Homes and warps

| Command | Permission | Description |
|---|---|---|
| `/sethome [name]`, `/home [name]`, `/delhome [name]` | `pumpkin_essentials:home` | Manages homes; default name `home` |
| `/homes`, `/listhomes` | `pumpkin_essentials:home` | Clickable list **with world and coordinates** and `used/limit` |
| `/renamehome <old> <new>` | `pumpkin_essentials:renamehome` | Renames a home while keeping its location |
| `/sethomelimit <player> <n>` | `pumpkin_essentials:admin` | Persistent per-player limit |
| `/setwarp`, `/delwarp` | `pumpkin_essentials:admin` | Manages public warps |
| `/warp [name]`, `/warps` | `pumpkin_essentials:warp` | Teleports to or lists warps |

### Teleportation

| Command | Permission | Description |
|---|---|---|
| `/tpa <player>` | `pumpkin_essentials:tpa` | Requests to teleport to another player |
| `/tpahere <player>` | `pumpkin_essentials:tpahere` | Requests that the other player teleport to you |
| `/tpaccept [player]`, `/tpdeny [player]` | `pumpkin_essentials:tpa` | Answers the request (the most recent one if no argument) |
| `/tphere <player>` | `pumpkin_essentials:tphere` | Brings a player (op level 2) |
| `/tpall` | `pumpkin_essentials:tpall` | Brings all online players (op level 3) |
| `/tptoggle` | `pumpkin_essentials:tptoggle` | Blocks/allows incoming requests |
| `/back` | `pumpkin_essentials:back` | Returns to the last location (now persistent) |
| `/spawn`, `/setspawn` | `pumpkin_essentials:spawn` / `:setspawn` | Go to spawn / save the world spawn |
| `/top [player]` | `pumpkin_essentials:top` | Teleports to the highest block in the column |

Warmups are cancelled by movement or damage. `/tpahere` uses the same request system; when accepted, the accepting player is teleported. `/tptoggle` applies to both `/tpa` and `/tpahere`.

### EssentialsX-style utilities

| Command | Permission | Description |
|---|---|---|
| `/heal [player]` | `pumpkin_essentials:heal` | Full health, food 20, and saturation 10 |
| `/feed [player]` | `pumpkin_essentials:feed` | Food 20, saturation 10, and exhaustion 0 |
| `/speed [player] <0.1-10.0>` | `pumpkin_essentials:speed` | Walking speed (1.0 = 0.1 vanilla) |
| `/exp [player] [level]` | `pumpkin_essentials:exp` | Shows or sets the experience level |
| `/getpos [player]` | `pumpkin_essentials:getpos` | World, position, yaw/pitch, and distance |

### Modes, flight, AFK, and inventories

| Command | Permission | Description |
|---|---|---|
| `/gma`, `/gmc`, `/gms`, `/gmsp` [player] | `pumpkin_essentials:gamemode` | Adventure, creative, survival, spectator |
| `/gm <mode> [player]` | `pumpkin_essentials:gamemode` | `s/c/a/sp`, `0/1/2/3`, or the full name |
| `/fly [player]` | `pumpkin_essentials:fly` | Toggles flight |
| `/flyspeed [player] <0.0-10.0>` | `pumpkin_essentials:flyspeed` | Validated flight speed |
| `/afk [player] [reason]` | `pumpkin_essentials:afk` | AFK state, optional reason, `[AFK]` in the tab list |
| `/invsee <player>` | `pumpkin_essentials:invsee` | Read-only view of the player's inventory |
| `/enderchest [player]` | `pumpkin_essentials:enderchest` | Opens your ender chest; read-only view when another player is targeted |

### Iteration 4 — information and advanced teleport (EssentialsX adapted)

| Command | Permission | Description |
|---|---|---|
| `/ping [text]` | `pumpkin_essentials:ping` | Shows your latency or repeats the text |
| `/playtime [player]` | `pumpkin_essentials:playtime` | Accumulated play time |
| `/near [player] [radius]` | `pumpkin_essentials:near` | Nearby players sorted by distance |
| `/seen <player>` | `pumpkin_essentials:seen` | Last connection and last saved location |
| `/whois <player>` | `pumpkin_essentials:whois` | UUID, mode, health, food, exp, ping, world, and AFK |
| `/info` | `pumpkin_essentials:info` | Pumpkin version, TPS/MSPT, players, and worlds |
| `/compass` | `pumpkin_essentials:compass` | Cardinal heading from yaw |
| `/depth` | `pumpkin_essentials:depth` | Blocks above/below sea level |
| `/warpinfo <name>` | `pumpkin_essentials:warpinfo` | Creator, world, and coordinates of a warp |
| `/gc` | `pumpkin_essentials:gc` | CPU, memory, TPS, players, and worlds |
| `/bottom [player]` | `pumpkin_essentials:bottom` | Teleport to `min_y + 1` of the column |
| `/tppos <x> <y> <z> [yaw] [pitch] [world]` | `pumpkin_essentials:tppos` | Teleport to coordinates (supports relative `~`) |
| `/tpr [world] [player]` | `pumpkin_essentials:tpr` | Random teleport to a safe point in the world |
| `/settpr <world> center\|minrange\|maxrange [value]` | `pumpkin_essentials:settpr` | Configures the `/tpr` area (op level 3) |
| `/tpaall [player]` | `pumpkin_essentials:tpaall` | Sends `/tpahere` to all online players |
| `/tpacancel [player]` | `pumpkin_essentials:tpacancel` | Cancels your pending `/tpa` / `/tpahere` requests |
| `/tpauto [on\|off] [player]` | `pumpkin_essentials:tpauto` | Automatically accepts incoming requests |
| `/tpo <player> [target-player]` | `pumpkin_essentials:tpo` | Instant teleport (your own or another player's) |
| `/tpoffline <player>` | `pumpkin_essentials:tpoffline` | Goes to the last saved location of an offline player |
| `/tpohere <player>` | `pumpkin_essentials:tpohere` | Brings an online player; see the limitation below |
| `/rest [player]` | `pumpkin_essentials:rest` | Resets exhaustion and saturation |
| `/world [name]` | `pumpkin_essentials:world` | Lists worlds or goes to the given world's spawn |

New `config.json` keys (with default values): `near_max_radius=200`, `tpr_min_range=50.0`, `tpr_max_range=1000.0`, `tpr_attempts=24`, and `tpr` (a per-world map with `center`, `min_range`, `max_range`). `/settpr center` saves your current position as the center; `/tpr` picks a random angle and distance within that ring, uses `get_top_block_y`, and applies the same warmup as other teleports.

### Iteration 5 — chat, moderation, and staff (EssentialsX adapted)

| Command | Permission | Description |
|---|---|---|
| `/broadcastworld [world] <message>` | `pumpkin_essentials:broadcastworld` | Announcement only for players in a world |
| `/msgtoggle [on\|off] [player]` | `pumpkin_essentials:msgtoggle` | Blocks incoming `/r` replies |
| `/socialspy [on\|off] [player]` | `pumpkin_essentials:socialspy` | Copies `/r` replies to staff |
| `/ignore [player]` | `pumpkin_essentials:ignore` | Lists/toggles ignored players (affects `/r` and `/mail`) |
| `/nick [player] <nick\|off>` | `pumpkin_essentials:nick` | Tab-list nickname; `/nick off` removes it |
| `/realname <nickname>` | `pumpkin_essentials:realname` | Real name associated with a nickname |
| `/mail [read\|clear\|<player> <message>]` | `pumpkin_essentials:mail` | Persistent internal mail |
| `/r <message>` | `pumpkin_essentials:r` | Replies to the last known sender |
| `/rtoggle [on\|off] [player]` | `pumpkin_essentials:rtoggle` | Allows or blocks replies |
| `/antioch` | `pumpkin_essentials:antioch` | TNT explosion in front of the player |
| `/customtext [key]` | `pumpkin_essentials:customtext` | Shows configured texts (`config.json`) |
| `/rules` | `pumpkin_essentials:rules` | Shows the rules from `config.json` |
| `/helpop <message>` | `pumpkin_essentials:helpop` | Sends the message to players with `:helpop.receive` |
| `/backup` | `pumpkin_essentials:backup` | Copies the plugin JSON files to `backups/<timestamp>/` |
| `/essentials` | `pumpkin_essentials:essentials` | Prints the plugin name and version |
| `/essentials help` | `pumpkin_essentials:essentials` | Lists all plugin commands grouped by domain |
| `/ext [player]` | `pumpkin_essentials:ext` | Extinguishes fire (`set_fire_ticks(0)`) |
| `/mute <player> [duration] [reason]` | `pumpkin_essentials:mute` | Mutes chat (alias `/unmute`) |
| `/tempban <player> <duration> [reason]` | `pumpkin_essentials:tempban` | Temporarily bans and kicks the player |
| `/kickall [reason]` | `pumpkin_essentials:kickall` | Kicks everyone except the sender |
| `/vanish [on\|off] [player]` | `pumpkin_essentials:vanish` | Hides players using `hide_player` |
| `/god [on\|off] [player]` | `pumpkin_essentials:god` | Persistent invulnerability |
| `/sudo <player> <command>` | `pumpkin_essentials:sudo` | Runs a command as another player (with a blocklist) |
| `/jailedplayers` | `pumpkin_essentials:jailedplayers` | Lists jailed players |
| `/jail <player> [jail] [duration]` | `pumpkin_essentials:jail` | Jails and teleports to the jail |
| `/setjail <name>` | `pumpkin_essentials:setjail` | Saves your position as a jail |
| `/deljail <name>` | `pumpkin_essentials:deljail` | Deletes a jail |
| `/togglejail <player> [jail] [duration]` | `pumpkin_essentials:togglejail` | Jails or releases depending on the current state |

Durations: `30` (seconds), `5m`, `2h`, `1d`, or `perm`/`0` (permanent). New `config.json` keys: `custom_texts` (a key -> lines map), `rules` (a list), and `jails` (a name -> location map).

### Iteration 6 — integrated economy

| Command | Permission | Description |
|---|---|---|
| `/balance [player]` (alias `/bal`) | `pumpkin_essentials:balance` | Own or another player's balance |
| `/balancetop [page]` | `pumpkin_essentials:balancetop` | Balance ranking, 10 per page |
| `/eco <set\|add\|take\|reset> <player> <amount>` | `pumpkin_essentials:eco` | Balance administration (op level 3) |
| `/pay <player> <amount>` | `pumpkin_essentials:pay` | Payment; requires `/payconfirm` by default |
| `/payconfirm` | `pumpkin_essentials:payconfirm` | Confirms the last prepared payment |
| `/paytoggle [player]` | `pumpkin_essentials:paytoggle` | Stops/resumes accepting payments |
| `/payconfirmtoggle` | `pumpkin_essentials:payconfirmtoggle` | Toggles `/pay` confirmation |
| `/sell [all\|hand\|<item>]` | `pumpkin_essentials:sell` | Sells stacks according to the `worth` table |
| `/worth <amount> <item>` | `pumpkin_essentials:worth` | Shows the unit and total price |
| `/setworth <item> <price>` | `pumpkin_essentials:setworth` | Sets the unit price (op level 3) |

Money is stored in `plugins/data/pumpkin_essentials/economy.json` (a private ledger, no Vault/PVault). Internal amounts are integer cents; `config.json` adds `currency_symbol` (`$`) and `currency_decimals` (`2`). The file format is:

```jsonc
{
  "schema": 1,
  "balances": { "<uuid>": 1500 },
  "worth": { "minecraft:stone": 1, "minecraft:diamond": 500 },
  "pay_off": { "<uuid>": false },
  "payconfirm_off": { "<uuid>": false },
  "pending_pay": { "<uuid>": { "to_uuid": "<uuid>", "to_name": "Name", "amount": 250, "at_ms": 0 } }
}
```

`load_economy` only reads: legacy forms (`{"economy": ...}` or an older `schema`) are normalised in memory and the first tick writes the canonical form. Corrupt JSON = load error with no overwrite. Item keys accept `minecraft:stone` or `stone` (normalised to lowercase with a namespace).

### Iteration 7 — kits and items

| Command | Permission | Description |
|---|---|---|
| `/kit [name] [player]` | `pumpkin_essentials:kit` | Lists kits or gives one (with a cooldown) |
| `/createkit <name> <delay_seconds>` | `pumpkin_essentials:createkit` | Saves the current inventory as a kit (op level 3) |
| `/delkit <name>` | `pumpkin_essentials:delkit` | Deletes a kit (op level 3) |
| `/kitreset <kit> [player]` | `pumpkin_essentials:kitreset` | Resets a kit's cooldown (op level 2) |
| `/showkit <name>` | `pumpkin_essentials:showkit` | Shows the contents of a kit |
| `/item <id> [amount] [player]` | `pumpkin_essentials:item` | Creates and gives an item |
| `/itemdb [id]` | `pumpkin_essentials:itemdb` | Shows the registry key of the item in hand |
| `/itemlore add\|set\|clear <text>` | `pumpkin_essentials:itemlore` | Edits the lore of the item in hand |
| `/itemname <name\|off>` | `pumpkin_essentials:itemname` | Renames the item in hand |
| `/repair [all]` | `pumpkin_essentials:repair` | API limitation: see the note below |
| `/unlimited <item\|list\|clear> [player]` | `pumpkin_essentials:unlimited` | Manages items marked as unlimited |
| `/hat` | `pumpkin_essentials:hat` | Puts the item in hand on your head |
| `/more [amount]` | `pumpkin_essentials:more` | Expands the stack in hand up to its maximum |
| `/book` | `pumpkin_essentials:book` | Gives a blank book |
| `/powertool <command\|d:> [item]` | `pumpkin_essentials:powertool` | Assigns/removes commands on the item in hand |
| `/powertoollist` | `pumpkin_essentials:powertoollist` | Lists powertools |
| `/powertooltoggle` | `pumpkin_essentials:powertooltoggle` | Toggles powertools |
| `/condense [item]` | `pumpkin_essentials:condense` | Packs inventory stacks |
| `/remove [type]` | `pumpkin_essentials:remove` | Removes entities in the world (op level 3) |
| `/spawnmob <mob> [amount] [player]` | `pumpkin_essentials:spawnmob` | Spawns entities (op level 3) |
| `/spawner <mob> [delay]` | `pumpkin_essentials:spawner` | Places a spawner block (op level 3) |
| `/skull <name>` | `pumpkin_essentials:skull` | Gives a named head |
| `/kittycannon` | `pumpkin_essentials:kittycannon` | Launches several cats |
| `/beezooka` | `pumpkin_essentials:beezooka` | Launches a bee that explodes on impact |
| `/fireball [type]` | `pumpkin_essentials:fireball` | Launches a projectile |
| `/potion <effect>` | `pumpkin_essentials:potion` | Gives a named potion |

Kits, cooldowns, powertools, and unlimited items are stored in `plugins/data/pumpkin_essentials/kits.json`:

```jsonc
{
  "schema": 1,
  "kits": { "starter": { "items": [{ "id": "minecraft:stone", "count": 16 }], "delay_secs": 3600 } },
  "cooldowns": { "<uuid>|<kit>": 1700000000000 },
  "powertools": { "<uuid>": { "minecraft:stone": ["home"] } },
  "powertool_toggle": { "<uuid>": true },
  "unlimited": { "<uuid>": ["minecraft:stone"] }
}
```

As with `state.json`/`economy.json`: `load_kits` only reads, applies legacy migrations in memory, and the first tick writes the canonical file. Corrupt JSON = load error with no overwrite. Item keys accept `minecraft:stone` or `stone`.

### Iteration 8 — blocks, environments, and effects

| Command | Permission | Description |
|---|---|---|
| `/anvil`, `/cartographytable`, `/grindstone`, `/loom`, `/smithingtable`, `/workbench` | nodes with the same name | Opens the corresponding screen via `Gui::new(Screen::…)` |
| `/disposal` | `pumpkin_essentials:disposal` | Opens an empty 54-slot inventory |
| `/editsign` | `pumpkin_essentials:editsign` | Opens the sign editor of the block being looked at (~5 blocks) |
| `/jump` | `pumpkin_essentials:jump` | Teleports to the targeted point, one block above |
| `/ice [player]` | `pumpkin_essentials:ice` | Freezes (frost ticks + zero velocity) |
| `/tree`, `/bigtree` | `pumpkin_essentials:tree` / `:bigtree` | Generates a simple or large oak tree |
| `/break` | `pumpkin_essentials:break` | Removes the targeted block (protects bedrock except op3) |
| `/burn <player> <seconds>` | `pumpkin_essentials:burn` | Sets the player on fire |
| `/lightning [player]` | `pumpkin_essentials:lightning` | Summons lightning at the target or targeted point |
| `/thunder on\|off` | `pumpkin_essentials:thunder` | Toggles the world's storm and rain |
| `/nuke [players]` | `pumpkin_essentials:nuke` | Grid bombardment with confirmation (op level 3) |
| `/suicide` | `pumpkin_essentials:suicide` | Ends your own life and announces it |
| `/clearinventory [player]` | `pumpkin_essentials:clearinventory` | Clears inventory, armour, and hand (confirmable) |
| `/clearinventoryconfirmtoggle` | `pumpkin_essentials:clearinventoryconfirmtoggle` | Toggles that confirmation |
| `/ptime [get\|day\|noon\|night\|midnight\|ticks]` | `pumpkin_essentials:ptime` | Gets/sets the world time |
| `/pweather [get\|clear\|rain\|reset]` | `pumpkin_essentials:pweather` | Per-player weather (API available) |

`/nuke` and `/clearinventory` use an in-memory confirmation: the first run prepares the action and you must repeat the command to execute it. `/clearinventoryconfirmtoggle` saves `clear_confirm_off` in `state.json` (the prefix has already been migrated safely). `/ptime` is world-wide because WIT 0.1 does not expose per-player time; `/pweather` does use `set_player_weather`/`reset_player_weather`.

### Iteration 9 — final audit and gap closure

The **153 official commands** of EssentialsX 2.x (`plugin.yml`) were compared with the 146 names the plugin registered at the time and with the commands of the Pumpkin core and PumpChat/dMOTD. The full inventory is in [`COMPAT.md`](COMPAT.md): 88 IMPLEMENTED, 19 COVERED, 43 LIMITED, and 3 ADDED NOW.

| Command | Permission | Description |
|---|---|---|
| `/stonecutter` | `pumpkin_essentials:stonecutter` | Opens the stonecutter screen (real gap) |
| `/firework` | `pumpkin_essentials:firework` | Gives a firework rocket (real gap) |
| `/jails` | `pumpkin_essentials:jails` | Lists jails defined with `/setjail` (real gap) |

After this iteration the plugin registers **149 command names** (including its own aliases such as `/bal`, `/gm`, `/listhomes`, and `/payconfirm`). No core commands or commands from other plugins are duplicated.

## Migration status vs EssentialsX 2.x

EssentialsPumpkinX registers **149 command names**. The 153 official EssentialsX 2.x commands split into four groups: 88 implemented here, 19 already provided by the Pumpkin core or other server plugins, 43 implemented with an API limitation, and 3 added after the audit. The complete row-by-row inventory is in [`COMPAT.md`](COMPAT.md).

### Commands added in this project (149)

| Domain | Commands |
|---|---|
| Homes (7) | `sethome`, `home`, `delhome`, `homes`, `listhomes`, `renamehome`, `sethomelimit` |
| Warps (5) | `setwarp`, `delwarp`, `warp`, `warps`, `warpinfo` |
| TPA (8) | `tpa`, `tpaccept`, `tpdeny`, `tpahere`, `tptoggle`, `tpaall`, `tpacancel`, `tpauto` |
| Movement (17) | `back`, `spawn`, `setspawn`, `top`, `tphere`, `tpall`, `bottom`, `tppos`, `tpr`, `settpr`, `tpo`, `tpoffline`, `tpohere`, `world`, `jump`, `fly`, `flyspeed` |
| Gamemode (5) | `gm`, `gma`, `gmc`, `gms`, `gmsp` |
| Info (10) | `ping`, `playtime`, `near`, `seen`, `whois`, `info`, `gc`, `compass`, `depth`, `getpos` |
| Chat (12) | `broadcastworld`, `msgtoggle`, `socialspy`, `ignore`, `nick`, `realname`, `mail`, `r`, `rtoggle`, `helpop`, `customtext`, `rules` |
| Moderation (13) | `mute`, `unmute`, `tempban`, `kickall`, `vanish`, `god`, `sudo`, `jail`, `togglejail`, `jailedplayers`, `setjail`, `deljail`, `jails` |
| Economy (11) | `balance`, `bal`, `balancetop`, `eco`, `pay`, `payconfirm`, `paytoggle`, `payconfirmtoggle`, `sell`, `worth`, `setworth` |
| Kits (5) | `kit`, `createkit`, `delkit`, `kitreset`, `showkit` |
| Items (26) | `item`, `itemdb`, `itemlore`, `itemname`, `repair`, `unlimited`, `hat`, `more`, `book`, `firework`, `powertool`, `powertoollist`, `powertooltoggle`, `condense`, `invsee`, `enderchest`, `clearinventory`, `clearinventoryconfirmtoggle`, `remove`, `spawnmob`, `spawner`, `skull`, `kittycannon`, `beezooka`, `fireball`, `potion` |
| Environment (19) | `anvil`, `cartographytable`, `grindstone`, `loom`, `smithingtable`, `workbench`, `disposal`, `editsign`, `stonecutter`, `ice`, `tree`, `bigtree`, `break`, `burn`, `lightning`, `thunder`, `nuke`, `ptime`, `pweather` |
| Misc (11) | `afk`, `heal`, `feed`, `speed`, `exp`, `rest`, `backup`, `essentials`, `ext`, `antioch`, `suicide` |

### Covered by the core or other plugins (19 — not duplicated here)

| Command | Provided by |
|---|---|
| `ban` | Pumpkin core (`/ban`) |
| `banip` | Pumpkin core (`/banip`) |
| `broadcast` | PumpChat (`/broadcast`) |
| `enchant` | Pumpkin core (`/enchant`) |
| `gamemode` | Pumpkin core (`/gamemode`); `/gmc`, `/gms`, `/gma`, `/gmsp` aliases live in EssentialsPumpkinX |
| `give` | Pumpkin core (`/give`) |
| `help` | Pumpkin core (`/help`) |
| `kick` | Pumpkin core (`/kick`) |
| `kill` | Pumpkin core (`/kill`) |
| `list` | Pumpkin core (`/list`) |
| `me` | Pumpkin core (`/me`) |
| `motd` | dMOTD (`/motd`) |
| `msg` | PumpChat (`/msg`) |
| `recipe` | Pumpkin core (`/recipe`) |
| `time` | Pumpkin core (`/time`) |
| `tp` | Pumpkin core (`/tp`) |
| `unban` | Pumpkin core (`/pardon`) |
| `unbanip` | Pumpkin core (`/pardonip`) |
| `weather` | Pumpkin core (`/weather`) |

### Limited / not fully migrated (43)

These commands exist but cannot be exact ports with WIT 0.1. The root causes are: no arbitrary NBT on items or block entities, no per-player time, no IP-ban management from plugins, no way to intercept PumpChat's `/msg` without duplicating it, no queryable entity type from `get_entities()`, and no reliable item-consumption event.

| Command | Why it is not a full port |
|---|---|
| `antioch` | `create_explosion` at an approximate point, no real TNT entity |
| `backup` | plugin JSON only, no world or `save-all` |
| `beezooka` | immediate explosion, no 1-second delay |
| `book` | no author/title/pages API; gives a `writable_book` |
| `bottom` | uses `get_min_y + 1` without safe-location search |
| `break` | no block-break event or drops |
| `bigtree` | simplified large oak generation |
| `clearinventory` | in-memory confirmation, lost on reload |
| `condense` | recreates `ItemStack`s and loses NBT/enchantments |
| `enderchest` | read-only view when the target is another player |
| `fireball` | `spawn_entity` + `set_velocity`, no projectile metadata |
| `gc` | no per-world chunk/entity count |
| `ice` | `set_freeze_ticks` + zero velocity, partial freeze |
| `ignore` | internal messaging only, not PumpChat `/msg` |
| `info` | only data exposed by the API (version/TPS/memory/worlds) |
| `invsee` | read-only view of another player's inventory |
| `kit` | stores only id and count per item |
| `kittycannon` | simple spawns, no variants or taming |
| `msgtoggle` | does not intercept PumpChat `/msg`; affects internal `/r` and `/mail` |
| `nick` | uses tab-list and may be overwritten by other plugins |
| `nuke` | grid explosions, no timed TNT |
| `tpoffline` | uses this plugin's last saved location, not Pumpkin usercache |
| `potion` | no effect NBT; gives a named potion |
| `ptime` | no per-player time in the API; adjusts world time |
| `r` | uses the reply target stored by this plugin |
| `rtoggle` | internal messaging only |
| `remove` | `get_entities()` does not expose type; filter validated but not applied |
| `repair` | WIT 0.1 has no `get_damage`/`set_damage` on `item-stack` |
| `seen` | only players seen since the plugin was installed |
| `sell` | no enchantments/NBT; items without a price are not sold |
| `setworth` | price per item key, no NBT |
| `editsign` | target block approximated to 5 blocks |
| `skull` | no player profile/texture |
| `socialspy` | only copies this plugin's `/r` and `/mail` |
| `spawner` | `set_block_by_name` without mob-type NBT |
| `sudo` | blocklist of dangerous commands; runs with the target's permissions |
| `tempbanip` | WIT API has no IP-ban management; only `/tempban` by UUID |
| `top` | uses `get_top_block_y` without safe-location search |
| `tpohere` | API does not allow moving offline entities |
| `tree` | simplified oak generation |
| `unlimited` | no reliable item-consumption event to restock |
| `world` | teleports to world spawn, no Nether scaling |
| `worth` | price per item key, no NBT |

## Session persistence (`state.json`)

New file in `plugins/data/pumpkin_essentials/`:

```jsonc
{
  "schema": 2,
  "back": { "<uuid>": { "world": "...", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0 } },
  "fly": { "<uuid>": { "allow_flying": true, "flying": true, "fly_speed": 0.15 } },
  "afk": { "<uuid>": { "active": true, "reason": "eating", "since_ms": 0 } },
  "tpa_off": { "<uuid>": true },
  "tpa_auto": { "<uuid>": false },
  "playtime_ms": { "<uuid>": 3600000 },
  "last_seen_ms": { "<uuid>": 1700000000000 },
  "last_location": { "<uuid>": { "world": "world", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0 } },
  "names": { "<uuid>": "Name" },
  "msg_off": { "<uuid>": false },
  "social_spy": { "<uuid>": true },
  "ignored": { "<uuid>": ["<other-uuid>"] },
  "nicks": { "<uuid>": "Nick" },
  "reply_target": { "<uuid>": "<other-uuid>" },
  "rtoggle": { "<uuid>": true },
  "mail": { "<uuid>": [{ "from": "Name", "from_uuid": "<uuid>", "body": "hello", "time_ms": 0, "read": false }] },
  "mutes": { "<uuid>": { "until_ms": 0, "reason": "spam", "by": "Mod" } },
  "bans": { "<uuid>": { "until_ms": 0, "reason": "abuse", "by": "Admin" } },
  "vanished": { "<uuid>": false },
  "god": { "<uuid>": false },
  "jailed": { "<uuid>": { "jail": "main", "until_ms": 0, "by": "Mod" } }
}
```

- `load_state` **only reads and parses**: legacy forms (`{"state": ...}`, `schema=1`) are normalised in memory and `needs_migration` is flagged. The first tick writes the canonical form, so startup performs no blocking writes.
- State is saved after every teleport, flight/AFK change, moderation action, mail, chat preference change, and disconnect. Playtime accumulates in the tick every ~250 ms and is written every ~2 s.
- On **join**, `/back`, flight, AFK, the `[AFK]` tag, vanish, god, and jail are restored; if an active ban exists, the player is kicked with the reason.
- Chat is cancelled for muted or jailed players; `/tempban` kicks the player and the ban is checked on every connection.
- Missing file = no state. Corrupt JSON = load error, no overwrite. Older schemas are extended without data loss.

## Permission nodes

All belong to the `pumpkin_essentials:` namespace (required: the loader rejects other namespaces).

**Allow:** `home`, `warp`, `tpa`, `back`, `afk`, `spawn`, `top`, `tptoggle`, `tpahere`, `renamehome`, `ping`, `playtime`, `near`, `compass`, `depth`, `warpinfo`, `info`, `rest`, `tpacancel`, `tpauto`, `tpr`, `world`, `msgtoggle`, `ignore`, `nick`, `realname`, `mail`, `r`, `rtoggle`, `helpop`, `customtext`, `rules`, `balance`, `balancetop`, `pay`, `paytoggle`, `payconfirmtoggle`, `payconfirm`, `sell`, `worth`, `kit`, `showkit`, `itemdb`, `powertoollist`, `powertooltoggle`, `anvil`, `cartographytable`, `grindstone`, `loom`, `smithingtable`, `workbench`, `disposal`, `editsign`, `jump`, `ptime`, `pweather`, `stonecutter`.

**Op level 2:** `gamemode`, `fly`, `flyspeed`, `heal`, `feed`, `speed`, `exp`, `getpos`, `tphere`, `invsee`, `enderchest`, `seen`, `whois`, `gc`, `bottom`, `tppos`, `tpo`, `tpoffline`, `tpohere`, `tpaall`, `socialspy`, `mute`, `unmute`, `vanish`, `god`, `jail`, `togglejail`, `jailedplayers`, `broadcastworld`, `ext`, `antioch`, `essentials`, `helpop.receive`, `item`, `itemlore`, `itemname`, `repair`, `unlimited`, `hat`, `more`, `book`, `powertool`, `condense`, `skull`, `kittycannon`, `beezooka`, `fireball`, `potion`, `kitreset`, `ice`, `tree`, `bigtree`, `break`, `burn`, `lightning`, `thunder`, `suicide`, `clearinventory`, `clearinventoryconfirmtoggle`, `firework`, `jails`.

**Op level 3:** `admin`, `setspawn`, `tpall`, `settpr`, `tempban`, `kickall`, `sudo`, `backup`, `setjail`, `deljail`, `eco`, `setworth`, `createkit`, `delkit`, `remove`, `spawnmob`, `spawner`, `nuke`.

The current groups (`default`, `vip`, `mod`, `owner`, `admin`) already have `pumpkin_essentials:home|warp|tpa|back|admin`, so they keep working. To grant new capabilities, add, for example, `pumpkin_essentials:heal` and `:feed` to `mod` and `pumpkin_essentials:fly` to `vip`, then run `/lp sync`. Targeting another player with `/gm`, `/fly`, `/flyspeed`, `/heal`, `/feed`, or `/enderchest` also requires op level 2; `/tphere` and `/tpall` check op levels 2 and 3 respectively.

## Known limitations

- VEssentials is not public: its command list was taken from the marketplace; the semantics of `/flyspeed` are documented above.
- `/invsee` and `/enderchest <player>` show a **read-only copy** because the API allows creating a `Gui` and copying `ItemStack`s, but not linking the GUI directly to another player's inventory. `/enderchest` without a player does open the player's real screen.
- `/top` uses `World::get_top_block_y(x, z)`; the API does not expose safe-location search (-fluids, ceiling, etc.), so the destination is `y + 1` above the column's highest block.
- The `[AFK]` tag is applied with `set_tab_list_name`: it may overwrite the tab-list name set by another plugin. The chat announcement remains active.
- `/back` now persists, but the history is a single location per player (as in PumpkinEssentials).
- `/tpohere` can only bring **online** players: the WIT API does not expose offline entity manipulation. If the player is offline, the command explains the case and suggests `/tpoffline`; it does not fake success.
- `/seen`, `/tpoffline`, and `/tpauto` use the data saved by EssentialsPumpkinX in `state.json`, not `usercache.json` or `perms/names.json` (the WASI sandbox only pre-opens the plugin's data folder). Only players who have joined since the plugin was installed appear.
- `/info` and `/gc` show only what the API exposes: Pumpkin version, TPS/MSPT, CPU, memory, players, and worlds. There is no configurable text page or per-world chunk/entity count as in Bukkit.
- `/world <name>` goes to the **world spawn** using `get_spawn_location` + `teleport_world`; it does not replicate EssentialsX's Nether/Overworld coordinate scaling.
- `/msgtoggle`, `/ignore`, `/socialspy`, `/rtoggle`, and `/r` affect only the messaging implemented by this plugin (`/r`, `/mail`). They cannot intercept PumpChat/core `/msg` or `/me` without duplicating those commands; the no-duplicates rule is respected.
- `/nick` changes `set_tab_list_name`; if another plugin (or `[AFK]`) rewrites that name, it may be overwritten. `/realname` only knows nicknames seen since this plugin was installed.
- `/tempban` and `/kickall` use the Java/Bedrock kick API (or an error message if the handle is unavailable). The core ban list is not modified: the control is private to `state.json`.
- `/sudo` runs the command with the target player's `CommandSender`, so their permissions apply; it also blocks dangerous prefixes (`sudo`, `op`, `perms`, `ban`, `backup`, etc.) to prevent escalation.
- `/backup` copies only this plugin's JSON files to its private `backups/<timestamp>/` folder; it does not back up the world or call `save-all`.
- `/antioch` computes an approximate point 6 blocks in front of the player and uses `World::create_explosion` with TNT interaction; it does not spawn a real TNT entity.
- The economy is **independent** of Vault/PVault and other plugins: `/balance`, `/pay`, etc. only read `economy.json`. There is no external bank integration.
- `/sell` uses `get_registry_key()` and `get_count()`; items without a price in `worth` are not sold and are returned to the inventory. Enchantments and NBT are not distinguished: the price is based only on item key and amount.
- `/sell` without arguments or with `hand` sells only the item in the selected slot; `all` iterates the 36 inventory slots. The API does not expose direct selling from chests.
- `/pay` requires confirmation by default (`/payconfirm`); `/payconfirmtoggle` disables it. `/paytoggle` blocks incoming payments. Pending payments are saved in `economy.json` and validated again on confirmation (balance and preferences).
- There are no interest, taxes, or maximum money limit; amounts are validated as positive and stored as `i64` cents.
- `/repair` cannot restore durability: WIT 0.1 does not expose `get_damage`/`set_damage` on `item-stack`. The command reports that concrete limitation instead of destroying the item.
- `/book` gives a `minecraft:writable_book`, but the API does not expose author/title/pages, so they cannot be filled in. `/potion` gives a named potion; NBT effects cannot be set.
- `/skull` gives a named `minecraft:player_head`: the player profile/texture cannot be assigned with this API.
- `/spawner` places the `minecraft:spawner` block on the block being looked at (approximated 3 blocks ahead), but the spawner's mob type cannot be stored: WIT offers no block entity NBT for spawners.
- `/remove` removes all non-player entities in the current world; the type filter is validated but not applied because `get_entities()` does not expose the entity type in a queryable way from the API.
- `/unlimited` stores the item list and it can be queried/cleared, but the API offers no reliable item-consumption event to restock automatically.
- `/condense` repacks stacks by item key; recreating `ItemStack`s loses NBT/enchantments. `/kit` stores only id and amount for the same reason.
- `/fireball`, `/beezooka`, and `/kittycannon` use `spawn_entity` + `set_velocity`; `/beezooka`'s explosion is immediate (there is no 1-second delay as in Bukkit).
- `/ptime` changes **world** time, not player time: WIT 0.1 does not expose `set_player_time`. `/pweather` is per-player (`set_player_weather`).
- `/ice` applies `set_freeze_ticks(140)` and zeroes velocity; it is the available/visual equivalent, not a complete movement freeze as in Bukkit.
- `/tree` and `/bigtree` generate a simplified oak with `set_block_by_name` (trunk + leaves); the API does not expose per-species tree generation.
- `/editsign` opens the editor on the block approximately 5 blocks ahead; it cannot verify that the block is really a sign, nor edit lines by command.
- `/nuke` and `/clearinventory` use in-memory confirmation (lost on reload) and persistent `clear_confirm_off`; `/nuke` creates grid explosions, not timed TNT entities.
- `/break` removes the block without simulating break events or drops; the API does not expose a plugin-invocable block-break event.

## License and credits

**GPL-3.0** (see `LICENSE-GPL`). This plugin is a derivative work of:

- [EssentialsX/Essentials](https://github.com/EssentialsX/Essentials) — GPL-3.0. Its commands (heal/feed/speed/exp/getpos/spawn/top and, in iteration 4, ping/playtime/near/seen/whois/info/compass/depth/warpinfo/gc/bottom/tppos/tpr/settpr/tpaall/tpacancel/tpauto/tpo/tpoffline/tpohere/rest/world) were adapted to Pumpkin's API.
- [DarkTama/pumpkin-plugins](https://github.com/DarkTama/pumpkin-plugins) (`pumpkin-essentials`) — MIT OR Apache-2.0, with `LICENSE-MIT` and `LICENSE-APACHE` kept and credited. The permissive code can be integrated into a GPL-3.0 project.
- The API and WIT of [Pumpkin-MC/Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) — MIT OR Apache-2.0.

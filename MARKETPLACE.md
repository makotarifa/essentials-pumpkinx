# EssentialsPumpkinX

EssentialsPumpkinX is a single WASM plugin for PumpkinMC that brings the everyday commands players expect from an Essentials-style server: homes, warps, teleport requests, economy, kits, moderation, chat tools and world utilities. It supports English and Spanish out of the box.

- **Server:** Pumpkin 0.2.2+26.3-26.60 (Java 26.3 / Bedrock 1.26.60)
- **Commands:** 149 registered names
- **Languages:** English (`en`, default) and Spanish (`es`)
- **License:** GPL-3.0

> **Development status:** this plugin is still in active development and is not feature-complete. Some EssentialsX behaviours are limited by the Pumpkin WIT API. Bug reports and pull requests are welcome: https://github.com/makotarifa/essentials-pumpkinx/pulls

## Features

- Homes with limits, renaming, warmups and cooldowns
- Public warps and teleport requests (`/tpa`, `/tpahere`, `/tphere`, `/tpall`)
- Back, spawn, top/bottom and random teleport
- Gamemode aliases, flight, fly speed, walk speed, AFK and inventories
- Built-in economy (`balance`, `pay`, `sell`, `worth`) with JSON persistence
- Kits with cooldowns and powertools bound to items
- Chat and moderation tools: mail, nick, ignore, mute, jail, tempban, vanish, god, sudo
- GUI screens for anvil, cartography, grindstone, loom, smithing, workbench, disposal and stonecutter
- Per-player weather, world time, explosions, trees, fire and cleanup commands
- Atomic JSON persistence with safe migration; corrupt files are never overwritten
- `/essentials help` lists every command; `/essentials language <en|es>` switches language

## Commands

**Homes (7):** `/sethome`, `/home`, `/delhome`, `/homes`, `/listhomes`, `/renamehome`, `/sethomelimit`

**Warps (5):** `/setwarp`, `/delwarp`, `/warp`, `/warps`, `/warpinfo`

**Teleport (8):** `/tpa`, `/tpaccept`, `/tpdeny`, `/tpahere`, `/tptoggle`, `/tpaall`, `/tpacancel`, `/tpauto`

**Movement (17):** `/back`, `/spawn`, `/setspawn`, `/top`, `/tphere`, `/tpall`, `/bottom`, `/tppos`, `/tpr`, `/settpr`, `/tpo`, `/tpoffline`, `/tpohere`, `/world`, `/jump`, `/fly`, `/flyspeed`

**Gamemode (5):** `/gm`, `/gma`, `/gmc`, `/gms`, `/gmsp`

**Info (10):** `/ping`, `/playtime`, `/near`, `/seen`, `/whois`, `/info`, `/gc`, `/compass`, `/depth`, `/getpos`

**Chat (12):** `/broadcastworld`, `/msgtoggle`, `/socialspy`, `/ignore`, `/nick`, `/realname`, `/mail`, `/r`, `/rtoggle`, `/helpop`, `/customtext`, `/rules`

**Moderation (13):** `/mute`, `/unmute`, `/tempban`, `/kickall`, `/vanish`, `/god`, `/sudo`, `/jail`, `/togglejail`, `/jailedplayers`, `/setjail`, `/deljail`, `/jails`

**Economy (11):** `/balance`, `/bal`, `/balancetop`, `/eco`, `/pay`, `/payconfirm`, `/paytoggle`, `/payconfirmtoggle`, `/sell`, `/worth`, `/setworth`

**Kits (5):** `/kit`, `/createkit`, `/delkit`, `/kitreset`, `/showkit`

**Items (26):** `/item`, `/itemdb`, `/itemlore`, `/itemname`, `/repair`, `/unlimited`, `/hat`, `/more`, `/book`, `/firework`, `/powertool`, `/powertoollist`, `/powertooltoggle`, `/condense`, `/invsee`, `/enderchest`, `/clearinventory`, `/clearinventoryconfirmtoggle`, `/remove`, `/spawnmob`, `/spawner`, `/skull`, `/kittycannon`, `/beezooka`, `/fireball`, `/potion`

**Environment (19):** `/anvil`, `/cartographytable`, `/grindstone`, `/loom`, `/smithingtable`, `/workbench`, `/disposal`, `/editsign`, `/stonecutter`, `/ice`, `/tree`, `/bigtree`, `/break`, `/burn`, `/lightning`, `/thunder`, `/nuke`, `/ptime`, `/pweather`

**Misc (11):** `/afk`, `/heal`, `/feed`, `/speed`, `/exp`, `/rest`, `/backup`, `/essentials`, `/ext`, `/antioch`, `/suicide`

## Permissions

All permission nodes use the `pumpkin_essentials:` namespace. Common commands are available to players; staff and destructive commands require higher permission levels or operator status. Commands already provided by the Pumpkin core or other plugins are not duplicated.

## Install

1. Enable plugins in `pumpkin.toml`.
2. Copy `EssentialsPumpkinX.wasm` into `plugins/`.
3. Restart the server.
4. Optional: set `"language": "es"` in `plugins/data/pumpkin_essentials/config.json`, or run `/essentials language es`.

Source: https://github.com/makotarifa/essentials-pumpkinx

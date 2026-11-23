# Compatibility with EssentialsX 2.x (final audit)

Official source: `Essentials/src/main/resources/plugin.yml` from EssentialsX 2.x (GPL-3.0), 153 commands.

Compared with the commands registered in the `src/` tree of EssentialsPumpkinX, the Pumpkin core 0.2.2+26.3-26.60, and the server plugins (PumpChat, dMOTD).

## Statistics

- **IMPLEMENTED:** 88
- **COVERED:** 19
- **LIMITED:** 43
- **ADDED NOW:** 3
- **Total:** 153

## Inventory

| Command | Status | Detail |
|---|---|---|
| `afk` | IMPLEMENTED | — |
| `antioch` | LIMITED | create_explosion at an approximate point, no real TNT |
| `anvil` | IMPLEMENTED | — |
| `back` | IMPLEMENTED | — |
| `backup` | LIMITED | plugin JSON only, no world or save-all |
| `balance` | IMPLEMENTED | — |
| `balancetop` | IMPLEMENTED | — |
| `ban` | COVERED | Pumpkin core (/ban) |
| `banip` | COVERED | Pumpkin core (/banip) |
| `beezooka` | LIMITED | immediate explosion, no 1s delay |
| `book` | LIMITED | no author/title/pages API; gives writable_book |
| `bottom` | LIMITED | uses get_min_y + 1 without safe-location search |
| `break` | LIMITED | no break event or drops |
| `broadcast` | COVERED | PumpChat (/broadcast) |
| `broadcastworld` | IMPLEMENTED | — |
| `bigtree` | LIMITED | simplified large oak generation |
| `burn` | IMPLEMENTED | — |
| `cartographytable` | IMPLEMENTED | — |
| `clearinventory` | LIMITED | in-memory confirmation, lost on reload |
| `clearinventoryconfirmtoggle` | IMPLEMENTED | — |
| `condense` | LIMITED | recreates ItemStacks and loses NBT/enchantments |
| `compass` | IMPLEMENTED | — |
| `createkit` | IMPLEMENTED | — |
| `customtext` | IMPLEMENTED | — |
| `delhome` | IMPLEMENTED | — |
| `deljail` | IMPLEMENTED | — |
| `delkit` | IMPLEMENTED | — |
| `delwarp` | IMPLEMENTED | — |
| `depth` | IMPLEMENTED | — |
| `disposal` | IMPLEMENTED | — |
| `eco` | IMPLEMENTED | — |
| `enchant` | COVERED | Pumpkin core (/enchant) |
| `enderchest` | LIMITED | read-only view when the target is another player |
| `essentials` | IMPLEMENTED | — |
| `exp` | IMPLEMENTED | — |
| `ext` | IMPLEMENTED | — |
| `feed` | IMPLEMENTED | — |
| `fly` | IMPLEMENTED | — |
| `fireball` | LIMITED | spawn_entity + set_velocity, no projectile metadata |
| `firework` | ADDED NOW | gives a standard rocket; no custom firework NBT |
| `gamemode` | COVERED | Pumpkin core (/gamemode); /gmc,/gms,/gma,/gmsp aliases in EssentialsPumpkinX |
| `gc` | LIMITED | no per-world chunk/entity count |
| `getpos` | IMPLEMENTED | — |
| `give` | COVERED | Pumpkin core (/give) |
| `god` | IMPLEMENTED | — |
| `grindstone` | IMPLEMENTED | — |
| `hat` | IMPLEMENTED | — |
| `heal` | IMPLEMENTED | — |
| `help` | COVERED | Pumpkin core (/help) |
| `helpop` | IMPLEMENTED | — |
| `home` | IMPLEMENTED | — |
| `ice` | LIMITED | set_freeze_ticks + zero velocity, partial freeze |
| `ignore` | LIMITED | internal messaging only, not PumpChat /msg |
| `info` | LIMITED | only data exposed by the API (version/TPS/memory/worlds) |
| `invsee` | LIMITED | read-only view of another player's inventory |
| `item` | IMPLEMENTED | — |
| `itemdb` | IMPLEMENTED | — |
| `itemlore` | IMPLEMENTED | — |
| `itemname` | IMPLEMENTED | — |
| `jailedplayers` | IMPLEMENTED | — |
| `jails` | ADDED NOW | real audit gap, implemented in this iteration |
| `jump` | IMPLEMENTED | — |
| `kick` | COVERED | Pumpkin core (/kick) |
| `kickall` | IMPLEMENTED | — |
| `kill` | COVERED | Pumpkin core (/kill) |
| `kit` | LIMITED | stores only id and count per item |
| `kitreset` | IMPLEMENTED | — |
| `kittycannon` | LIMITED | simple spawns, no variants or taming |
| `lightning` | IMPLEMENTED | — |
| `list` | COVERED | Pumpkin core (/list) |
| `loom` | IMPLEMENTED | — |
| `mail` | IMPLEMENTED | — |
| `me` | COVERED | Pumpkin core (/me) |
| `more` | IMPLEMENTED | — |
| `motd` | COVERED | dMOTD (/motd) |
| `msg` | COVERED | PumpChat (/msg) |
| `msgtoggle` | LIMITED | does not intercept PumpChat /msg; affects internal /r and /mail |
| `mute` | IMPLEMENTED | — |
| `near` | IMPLEMENTED | — |
| `nick` | LIMITED | uses tab-list and may be overwritten by other plugins |
| `nuke` | LIMITED | grid explosions, no timed TNT |
| `tpoffline` | LIMITED | uses own last saved location, not Pumpkin usercache |
| `pay` | IMPLEMENTED | — |
| `paytoggle` | IMPLEMENTED | — |
| `payconfirmtoggle` | IMPLEMENTED | — |
| `ping` | IMPLEMENTED | — |
| `playtime` | IMPLEMENTED | — |
| `potion` | LIMITED | no effect NBT; gives a named potion |
| `powertool` | IMPLEMENTED | — |
| `powertoollist` | IMPLEMENTED | — |
| `powertooltoggle` | IMPLEMENTED | — |
| `ptime` | LIMITED | no per-player time in the API; adjusts world time |
| `pweather` | IMPLEMENTED | — |
| `r` | LIMITED | uses the reply target stored by this plugin |
| `rtoggle` | LIMITED | internal messaging only |
| `realname` | IMPLEMENTED | — |
| `recipe` | COVERED | Pumpkin core (/recipe) |
| `remove` | LIMITED | get_entities does not expose type; filter validated but not applied |
| `renamehome` | IMPLEMENTED | — |
| `repair` | LIMITED | WIT 0.1 does not expose get_damage/set_damage on item-stack |
| `rest` | IMPLEMENTED | — |
| `rules` | IMPLEMENTED | — |
| `seen` | LIMITED | only players seen since the plugin was installed |
| `sell` | LIMITED | no enchantments/NBT; items without a price are not sold |
| `sethome` | IMPLEMENTED | — |
| `setjail` | IMPLEMENTED | — |
| `settpr` | IMPLEMENTED | — |
| `setwarp` | IMPLEMENTED | — |
| `setworth` | LIMITED | price per item key, no NBT |
| `showkit` | IMPLEMENTED | — |
| `editsign` | LIMITED | target block approximated to 5 blocks |
| `skull` | LIMITED | no player profile/texture |
| `smithingtable` | IMPLEMENTED | — |
| `socialspy` | LIMITED | only copies this plugin's /r and /mail |
| `spawner` | LIMITED | set_block_by_name without mob type NBT |
| `spawnmob` | IMPLEMENTED | — |
| `speed` | IMPLEMENTED | — |
| `stonecutter` | ADDED NOW | real audit gap, implemented in this iteration |
| `sudo` | LIMITED | blocklist of dangerous commands; runs with the target's permissions |
| `suicide` | IMPLEMENTED | — |
| `tempban` | IMPLEMENTED | — |
| `tempbanip` | LIMITED | WIT API does not expose IP ban management; only /tempban by UUID |
| `thunder` | IMPLEMENTED | — |
| `time` | COVERED | Pumpkin core (/time) |
| `togglejail` | IMPLEMENTED | — |
| `top` | LIMITED | uses get_top_block_y without safe-location search |
| `tp` | COVERED | Pumpkin core (/tp) |
| `tpa` | IMPLEMENTED | — |
| `tpaall` | IMPLEMENTED | — |
| `tpaccept` | IMPLEMENTED | — |
| `tpahere` | IMPLEMENTED | — |
| `tpall` | IMPLEMENTED | — |
| `tpauto` | IMPLEMENTED | — |
| `tpacancel` | IMPLEMENTED | — |
| `tpdeny` | IMPLEMENTED | — |
| `tphere` | IMPLEMENTED | — |
| `tpo` | IMPLEMENTED | — |
| `tpohere` | LIMITED | API does not allow moving offline entities |
| `tppos` | IMPLEMENTED | — |
| `tpr` | IMPLEMENTED | — |
| `tptoggle` | IMPLEMENTED | — |
| `tree` | LIMITED | simplified oak generation |
| `unban` | COVERED | Pumpkin core (/pardon) |
| `unbanip` | COVERED | Pumpkin core (/pardonip) |
| `unlimited` | LIMITED | no reliable item-consumption event to restock |
| `vanish` | IMPLEMENTED | — |
| `warp` | IMPLEMENTED | — |
| `warpinfo` | IMPLEMENTED | — |
| `weather` | COVERED | Pumpkin core (/weather) |
| `whois` | IMPLEMENTED | — |
| `workbench` | IMPLEMENTED | — |
| `world` | LIMITED | teleports to world spawn, no Nether scaling |
| `worth` | LIMITED | price per item key, no NBT |

## General limitations

- Pumpkin's WIT 0.1 API does not expose arbitrary item or block entity NBT: enchantments, durability, potion effects, head textures, and spawner mob types cannot be set.
- There is no per-player time: `/ptime` changes the world. Weather is per-player.
- No ban group/IP management from plugins: `/tempbanip` remains LIMITED; own-name bans live in `state.json` and do not replace the core ban list.
- PumpChat private messaging cannot be intercepted without duplicating `/msg`: `/msgtoggle`, `/socialspy`, `/ignore`, `/r`, and `/rtoggle` cover only EssentialsPumpkinX's internal messaging.
- Destructive confirmations are stored in memory (lost on reload); the `/clearinventory` preference does persist in `state.json`.
- Data files (`config.json`, `homes.json`, `warps.json`, `state.json` schema 2, `economy.json` schema 1, `kits.json` schema 1) are preserved, and their migrations are read-only with a deferred write on the first tick.

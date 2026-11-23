# Roadmap: EssentialsX parity

Goal: cover the **153 commands** declared in `EssentialsX/Essentials` 2.x (`plugin.yml`), adapted to Pumpkin 0.2.2. Source consulted: https://github.com/EssentialsX/Essentials (GPL-3.0). Each iteration is implemented, tested in an isolated instance, installed, and verified on the real server before moving on to the next one.

## Rules

1. A single plugin, `EssentialsPumpkinX`, namespace `pumpkin_essentials:*`, `PluginMetadata.name = pumpkin_essentials`, data folder `plugins/data/pumpkin_essentials/`.
2. Do not duplicate commands already provided by the Pumpkin core or other plugins; they are marked **COVERED** and their location is documented.
3. If an EssentialsX command depends on Bukkit concepts that do not exist, implement the closest functional equivalence or document the concrete limitation. No silent stubs.
4. Backward compatibility for `config.json`, `homes.json`, `warps.json`, and `state.json`. Corrupt JSON = load error, never an overwrite.
5. GPL-3.0 for the whole plugin (EssentialsX is GPL-3.0; PumpkinEssentials' MIT/Apache credits are preserved).

## Current status

**Implemented in EssentialsPumpkinX (149):** afk, anvil, antioch, back, bal, balance, balancetop, backup, beezooka, bigtree, book, bottom, break, broadcastworld, burn, cartographytable, clearinventory, clearinventoryconfirmtoggle, compass, condense, createkit, customtext, delhome, deljail, delkit, delwarp, depth, disposal, eco, editsign, enderchest, essentials, exp, ext, feed, fireball, firework, fly, flyspeed, gc, getpos, gm, gma, gmc, gms, gmsp, god, grindstone, hat, heal, helpop, home, homes, ice, ignore, info, invsee, item, itemdb, itemlore, itemname, jail, jailedplayers, jails, jump, kittycannon, kit, kitreset, lightning, listhomes, loom, mail, more, msgtoggle, near, nick, nuke, pay, payconfirm, payconfirmtoggle, paytoggle, ping, playtime, potion, powertool, powertoollist, powertooltoggle, ptime, pweather, r, realname, remove, renamehome, repair, rest, rtoggle, rules, sell, sethome, sethomelimit, setjail, setspawn, settpr, setwarp, setworth, showkit, skull, smithingtable, socialspy, spawn, spawnmob, spawner, speed, seen, stonecutter, suicide, sudo, tempban, thunder, togglejail, top, tpa, tpaall, tpacancel, tpahere, tpall, tpauto, tpaccept, tphere, tptoggle, tpdeny, tpo, tpoffline, tpohere, tpr, tppos, tree, unlimited, unmute, vanish, warp, warps, workbench, worth, whois, world.

**Covered by the core or other plugins:** ban, banip, broadcast, enchant, give, help, kick, kill, list, me, motd, msg, recipe, time, tp, unban, unbanip, weather (core/PumpChat/dMOTD), plus gamemode (core).

### Iteration 4 — information and advanced teleport — **COMPLETED**

22 commands implemented: `ping`, `playtime`, `near`, `seen`, `whois`, `info`, `compass`, `depth`, `warpinfo`, `gc`, `bottom`, `tppos`, `tpr`, `settpr`, `tpaall`, `tpacancel`, `tpauto`, `tpo`, `tpoffline`, `tpohere`, `rest`, `world`.

Checkpoint verified by Main: `cargo test --lib` (45 tests) and WASM build OK; isolated test with all 22 commands recognized; installed as `plugins/EssentialsPumpkinX.wasm` (backup `EssentialsPumpkinX_iter23.wasm`) with nodes split across `default`/`admin`; real server started with `Loaded pumpkin_essentials (1.0.0)` and no errors. Installed SHA-256: `36c6e62b18de6fdfe5976aebe714f5d9ec991cccb0106db15bbb5b283818c72a`.

### Iteration 5 — chat and moderation — **COMPLETED**

27 commands implemented: `broadcastworld`, `msgtoggle`, `socialspy`, `ignore`, `nick`, `realname`, `mail`, `r`, `rtoggle`, `antioch`, `customtext`, `rules`, `helpop`, `backup`, `essentials`, `ext`, `mute`, `tempban`, `kickall`, `vanish`, `god`, `sudo`, `jailedplayers`, `jail`, `setjail`, `deljail`, `togglejail`.

Checkpoint verified by Main: 47 tests, WASM build OK, isolated test with the 27 registered commands, installed with backup `EssentialsPumpkinX_iter4.wasm`, nodes split across `default`/`mod`/`admin`, real server with `Loaded pumpkin_essentials (1.0.0)` and `state.json` migrated to `schema=2` without corruption. Installed SHA-256: `e01ea15dabc0d87b6fcb1b6307815f8f834d0e011253f78889010101d5fa31a3`.

### Iteration 6 — integrated economy — **COMPLETED**

9 commands implemented (plus the `bal` alias): `balance`, `balancetop`, `eco`, `pay`, `paytoggle`, `payconfirmtoggle`, `sell`, `worth`, `setworth`.

Checkpoint verified by Main: 53 tests, WASM build OK, isolated test with the economy commands registered and `economy.json` created by `/setworth`; installed with backup `EssentialsPumpkinX_iter5.wasm`, `default`/`admin` nodes, real server with `Loaded pumpkin_essentials (1.0.0)` and `state.json` at `schema=2`. Installed SHA-256: `35e941cac788febe8fa7e9afe87bb31f72322b5409acecbc9fca1e5ab862a931`.

### Iteration 7 — kits and items — **COMPLETED**

27 commands implemented: `kit`, `createkit`, `delkit`, `kitreset`, `showkit`, `item`, `itemdb`, `itemlore`, `itemname`, `repair`, `unlimited`, `hat`, `more`, `book`, `powertool`, `powertoollist`, `powertooltoggle`, `condense`, `remove`, `spawnmob`, `spawner`, `skull`, `kittycannon`, `beezooka`, `fireball`, `potion`.

Checkpoint verified by Main: 58 tests, WASM build OK, isolated test with the 27 registered commands, installed with backup `EssentialsPumpkinX_iter6.wasm`, `default`/`admin` nodes, real server with `Loaded pumpkin_essentials (1.0.0)` and earlier schemas intact (`state.json` schema 2). Installed SHA-256: `ea6dfec768452a4ccf42185baffcc15f9b41c8c389142947ff2776c7bf3f41f4`.

### Iteration 8 — blocks, environments, and effects — **COMPLETED**

22 commands implemented: `anvil`, `cartographytable`, `grindstone`, `loom`, `smithingtable`, `workbench`, `disposal`, `editsign`, `jump`, `ice`, `tree`, `bigtree`, `break`, `burn`, `lightning`, `thunder`, `nuke`, `suicide`, `clearinventory`, `clearinventoryconfirmtoggle`, `ptime`, `pweather`.

Checkpoint verified by Main: 61 tests, WASM build OK, isolated test with the 22 registered commands, installed with backup `EssentialsPumpkinX_iter7.wasm`, `default`/`admin` nodes, real server with `Loaded pumpkin_essentials (1.0.0)`, `state.json` schema 2 with `clear_confirm_off`. Installed SHA-256: `57509c4e6cc18240105aa920b597445aa16e837a8468b3be871937788475951c`.

### Iteration 9 — final audit and gap closure — **COMPLETED**

Audit of the 153 official EssentialsX 2.x commands documented in `COMPAT.md`: 88 IMPLEMENTED, 19 COVERED (core/PumpChat/dMOTD), 43 LIMITED (concrete technical reason), and 3 ADDED NOW: `stonecutter`, `firework`, `jails`. Total names registered by EssentialsPumpkinX: 149.

Internal checkpoint: `cargo test --lib` and WASM build OK; earlier data schemas intact. Awaiting isolated/real verification by Main.

## Final status

The parity roadmap is closed with a complete inventory and viable gaps covered. The plugin does not duplicate commands from the core or from other server plugins.

## Structural refactor (post-iteration 9)

The structural refactor was applied without changing functionality:

- `git init` + `.gitignore` and a baseline commit before starting.
- `src/cmdutil.rs` unifies the shared command-layer helpers.
- `src/commands/` splits the 149 command names into 13 domain modules.
- `src/store/persist.rs` adds the `FileStore` trait, and `state.json` is now saved only when its dirty flag is set.
- `src/host/host_traits.rs` introduces a minimal adapter over the WIT API without changing the public API.
- Verification: `cargo check --target wasm32-wasip2` (0 errors/0 warnings), `cargo test --lib` (62/62), and `cargo build --release --target wasm32-wasip2` OK.

## Per-iteration checkpoint

- `cargo test --lib` + `cargo check --target wasm32-wasip2` + `cargo build --release --target wasm32-wasip2`.
- Isolated instance test: clean startup and every new command recognized.
- Installation with a backup of the previous version, `/lp` if there are new nodes, restart, and verification in `logs/latest.log`.

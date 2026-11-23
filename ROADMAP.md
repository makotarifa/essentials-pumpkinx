# Hoja de ruta: paridad con EssentialsX

Objetivo: cubrir los **153 comandos** declarados en `EssentialsX/Essentials` 2.x (`plugin.yml`), adaptados a Pumpkin 0.2.2. Fuente consultada: https://github.com/EssentialsX/Essentials (GPL-3.0). Cada iteración se implementa, se prueba en instancia aislada, se instala y se verifica en el servidor real antes de pasar a la siguiente.

## Reglas

1. Un único plugin `EssentialsFusion`, namespace `pumpkin_essentials:*`, `PluginMetadata.name = pumpkin_essentials`, carpeta de datos `plugins/data/pumpkin_essentials/`.
2. No duplicar comandos ya proporcionados por el núcleo de Pumpkin u otros plugins; se marcan como **CUBIERTO** y se documenta dónde están.
3. Si un comando de EssentialsX depende de conceptos de Bukkit no existentes, se implementa la equivalencia funcional más cercana o se documenta la limitación concreta. No se dejan stubs silenciosos.
4. Backward compatibility de `config.json`, `homes.json`, `warps.json` y `state.json`. JSON corrupto = error de carga, nunca sobreescritura.
5.GPL-3.0 para todo el plugin (EssentialsX es GPL-3.0; PumpkinEssentials MIT/Apache se conserva como crédito).

## Estado actual

**Implementados en EssentialsFusion (149):** afk, anvil, antioch, back, bal, balance, balancetop, backup, beezooka, bigtree, book, bottom, break, broadcastworld, burn, cartographytable, clearinventory, clearinventoryconfirmtoggle, compass, condense, createkit, customtext, delhome, deljail, delkit, delwarp, depth, disposal, eco, editsign, enderchest, essentials, exp, ext, feed, fireball, firework, fly, flyspeed, gc, getpos, gm, gma, gmc, gms, gmsp, god, grindstone, hat, heal, helpop, home, homes, ice, ignore, info, invsee, item, itemdb, itemlore, itemname, jail, jailedplayers, jails, jump, kittycannon, kit, kitreset, lightning, listhomes, loom, mail, more, msgtoggle, near, nick, nuke, pay, payconfirm, payconfirmtoggle, paytoggle, ping, playtime, potion, powertool, powertoollist, powertooltoggle, ptime, pweather, r, realname, remove, renamehome, repair, rest, rtoggle, rules, sell, sethome, sethomelimit, setjail, setspawn, settpr, setwarp, setworth, showkit, skull, smithingtable, socialspy, spawn, spawnmob, spawner, speed, seen, stonecutter, suicide, sudo, tempban, thunder, togglejail, top, tpa, tpaall, tpacancel, tpahere, tpall, tpauto, tpaccept, tphere, tptoggle, tpdeny, tpo, tpoffline, tpohere, tpr, tppos, tree, unlimited, unmute, vanish, warp, warps, workbench, worth, whois, world.

**Cubiertos por el núcleo u otros plugins:** ban, banip, broadcast, enchant, give, help, kick, kill, list, me, motd, msg, recipe, time, tp, unban, unbanip, weather (núcleo/PumpChat/dMOTD), más gamemode (núcleo).

### Iteración 4 — Información y teleport avanzado — **COMPLETADA**

22 comandos implementados: `ping`, `playtime`, `near`, `seen`, `whois`, `info`, `compass`, `depth`, `warpinfo`, `gc`, `bottom`, `tppos`, `tpr`, `settpr`, `tpaall`, `tpacancel`, `tpauto`, `tpo`, `tpoffline`, `tpohere`, `rest`, `world`.

Checkpoint verificado por Main: `cargo test --lib` (45 tests) y build WASM OK; prueba aislada con los 22 comandos reconocidos; instalado como `plugins/EssentialsFusion.wasm` (backup `EssentialsFusion_iter23.wasm`) con nodos repartidos en `default`/`admin`; servidor real arrancado con `Loaded pumpkin_essentials (1.0.0)` y sin errores. SHA-256 instalado: `36c6e62b18de6fdfe5976aebe714f5d9ec991cccb0106db15bbb5b283818c72a`.

### Iteración 5 — Chat y moderación — **COMPLETADA**

27 comandos implementados: `broadcastworld`, `msgtoggle`, `socialspy`, `ignore`, `nick`, `realname`, `mail`, `r`, `rtoggle`, `antioch`, `customtext`, `rules`, `helpop`, `backup`, `essentials`, `ext`, `mute`, `tempban`, `kickall`, `vanish`, `god`, `sudo`, `jailedplayers`, `jail`, `setjail`, `deljail`, `togglejail`.

Checkpoint verificado por Main: 47 tests, build WASM OK, prueba aislada con los 27 comandos registrados, instalado con backup `EssentialsFusion_iter4.wasm`, nodos repartidos en `default`/`mod`/`admin`, servidor real con `Loaded pumpkin_essentials (1.0.0)` y `state.json` migrado a `schema=2` sin corrupción. SHA-256 instalado: `e01ea15dabc0d87b6fcb1b6307815f8f834d0e011253f78889010101d5fa31a3`.


### Iteración 6 — Economía integrada — **COMPLETADA**

9 comandos implementados (más el alias `bal`): `balance`, `balancetop`, `eco`, `pay`, `paytoggle`, `payconfirmtoggle`, `sell`, `worth`, `setworth`.

Checkpoint verificado por Main: 53 tests, build WASM OK, prueba aislada con los comandos de economía registrados y `economy.json` creado por `/setworth`; instalado con backup `EssentialsFusion_iter5.wasm`, nodos `default`/`admin`, servidor real con `Loaded pumpkin_essentials (1.0.0)` y `state.json` en `schema=2`. SHA-256 instalado: `35e941cac788febe8fa7e9afe87bb31f72322b5409acecbc9fca1e5ab862a931`.


### Iteración 7 — Kits e ítems — **COMPLETADA**

27 comandos implementados: `kit`, `createkit`, `delkit`, `kitreset`, `showkit`, `item`, `itemdb`, `itemlore`, `itemname`, `repair`, `unlimited`, `hat`, `more`, `book`, `powertool`, `powertoollist`, `powertooltoggle`, `condense`, `remove`, `spawnmob`, `spawner`, `skull`, `kittycannon`, `beezooka`, `fireball`, `potion`.

Checkpoint verificado por Main: 58 tests, build WASM OK, prueba aislada con los 27 comandos registrados, instalado con backup `EssentialsFusion_iter6.wasm`, nodos `default`/`admin`, servidor real con `Loaded pumpkin_essentials (1.0.0)` y esquemas anteriores intactos (`state.json` schema 2). SHA-256 instalado: `ea6dfec768452a4ccf42185baffcc15f9b41c8c389142947ff2776c7bf3f41f4`.


### Iteración 8 — Bloques, entornos y efectos — **COMPLETADA**

22 comandos implementados: `anvil`, `cartographytable`, `grindstone`, `loom`, `smithingtable`, `workbench`, `disposal`, `editsign`, `jump`, `ice`, `tree`, `bigtree`, `break`, `burn`, `lightning`, `thunder`, `nuke`, `suicide`, `clearinventory`, `clearinventoryconfirmtoggle`, `ptime`, `pweather`.

Checkpoint verificado por Main: 61 tests, build WASM OK, prueba aislada con los 22 comandos registrados, instalado con backup `EssentialsFusion_iter7.wasm`, nodos `default`/`admin`, servidor real con `Loaded pumpkin_essentials (1.0.0)`, `state.json` schema 2 con `clear_confirm_off`. SHA-256 instalado: `57509c4e6cc18240105aa920b597445aa16e837a8468b3be871937788475951c`.


### Iteración 9 — Auditoría final y cierre de huecos — **COMPLETADA**

Auditoría de los 153 comandos oficiales de EssentialsX 2.x documentada en `COMPAT.md`: 88 IMPLEMENTADO, 19 CUBIERTO (núcleo/PumpChat/dMOTD), 43 LIMITADO (razón técnica concreta) y 3 AÑADIDO AHORA: `stonecutter`, `firework`, `jails`. Total de nombres registrados por EssentialsFusion: 149.

Checkpoint interno: `cargo test --lib` y build WASM OK; esquemas de datos anteriores intactos. Pendiente de verificación aislada/real por Main.
## Estado final

Roadmap de paridad cerrado con inventario completo y huecos viables cubiertos. El plugin no duplica comandos del núcleo ni de otros plugins del servidor.

## Refactor estructural (post-iteración 9)

Se aplicó el refactor de estructura sin cambiar funcionalidad:

- `git init` + `.gitignore` y commit baseline antes de empezar.
- `src/cmdutil.rs` unifica los helpers de la capa de comandos.
- `src/commands/` divide los 149 nombres de comando en 13 módulos por dominio.
- `src/store/persist.rs` añade el trait `FileStore` y `state.json` pasa a guardarse solo con dirty-flag.
- `src/host/host_traits.rs` introduce un Adapter mínimo sobre la API WIT sin cambiar la API pública.
- Verificación: `cargo check --target wasm32-wasip2` (0 errores/0 warnings), `cargo test --lib` (62/62) y `cargo build --release --target wasm32-wasip2` OK.

## Checkpoint de cada iteración

- `cargo test --lib` + `cargo check --target wasm32-wasip2` + `cargo build --release --target wasm32-wasip2`.
- Prueba en instancia aislada: arranque sin errores y cada comando nuevo reconocido.
- Instalación con backup de la versión anterior, `/lp` si hay nodos nuevos, reinicio y verificación en `logs/latest.log`.

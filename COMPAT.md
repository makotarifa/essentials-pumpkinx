# Compatibilidad con EssentialsX 2.x (auditoría final)

Fuente oficial: `Essentials/src/main/resources/plugin.yml` de EssentialsX 2.x (GPL-3.0), 153 comandos.

Comparado con los comandos registrados en `src/` de EssentialsFusion, el núcleo Pumpkin 0.2.2+26.3-26.60 y los plugins del servidor (PumpChat, dMOTD).

## Estadísticas

- **IMPLEMENTADO:** 88
- **CUBIERTO:** 19
- **LIMITADO:** 43
- **AÑADIDO AHORA:** 3
- **Total:** 153

## Inventario

| Comando | Estado | Detalle |
|---|---|---|
| `afk` | IMPLEMENTADO | — |
| `antioch` | LIMITADO | create_explosion en punto aproximado, no TNT real |
| `anvil` | IMPLEMENTADO | — |
| `back` | IMPLEMENTADO | — |
| `backup` | LIMITADO | solo JSON del plugin, no mundo ni save-all |
| `balance` | IMPLEMENTADO | — |
| `balancetop` | IMPLEMENTADO | — |
| `ban` | CUBIERTO | núcleo Pumpkin (/ban) |
| `banip` | CUBIERTO | núcleo Pumpkin (/banip) |
| `beezooka` | LIMITADO | explosión inmediata, sin retardo de 1s |
| `book` | LIMITADO | sin API de autor/título/páginas; entrega writable_book |
| `bottom` | LIMITADO | usa get_min_y + 1 sin búsqueda segura |
| `break` | LIMITADO | sin evento de rotura ni drops |
| `broadcast` | CUBIERTO | PumpChat (/broadcast) |
| `broadcastworld` | IMPLEMENTADO | — |
| `bigtree` | LIMITADO | generación simplificada de roble grande |
| `burn` | IMPLEMENTADO | — |
| `cartographytable` | IMPLEMENTADO | — |
| `clearinventory` | LIMITADO | confirmación en memoria, se pierde al recargar |
| `clearinventoryconfirmtoggle` | IMPLEMENTADO | — |
| `condense` | LIMITADO | recrea ItemStacks y pierde NBT/encantamientos |
| `compass` | IMPLEMENTADO | — |
| `createkit` | IMPLEMENTADO | — |
| `customtext` | IMPLEMENTADO | — |
| `delhome` | IMPLEMENTADO | — |
| `deljail` | IMPLEMENTADO | — |
| `delkit` | IMPLEMENTADO | — |
| `delwarp` | IMPLEMENTADO | — |
| `depth` | IMPLEMENTADO | — |
| `disposal` | IMPLEMENTADO | — |
| `eco` | IMPLEMENTADO | — |
| `enchant` | CUBIERTO | núcleo Pumpkin (/enchant) |
| `enderchest` | LIMITADO | vista de solo lectura cuando el objetivo es otro jugador |
| `essentials` | IMPLEMENTADO | — |
| `exp` | IMPLEMENTADO | — |
| `ext` | IMPLEMENTADO | — |
| `feed` | IMPLEMENTADO | — |
| `fly` | IMPLEMENTADO | — |
| `fireball` | LIMITADO | spawn_entity + set_velocity, sin metadatos de proyectil |
| `firework` | AÑADIDO AHORA | entrega cohete estándar; sin NBT pirotécnico personalizado |
| `gamemode` | CUBIERTO | núcleo Pumpkin (/gamemode); alias /gmc,/gms,/gma,/gmsp en EssentialsFusion |
| `gc` | LIMITADO | sin conteo de chunks/entidades por mundo |
| `getpos` | IMPLEMENTADO | — |
| `give` | CUBIERTO | núcleo Pumpkin (/give) |
| `god` | IMPLEMENTADO | — |
| `grindstone` | IMPLEMENTADO | — |
| `hat` | IMPLEMENTADO | — |
| `heal` | IMPLEMENTADO | — |
| `help` | CUBIERTO | núcleo Pumpkin (/help) |
| `helpop` | IMPLEMENTADO | — |
| `home` | IMPLEMENTADO | — |
| `ice` | LIMITADO | set_freeze_ticks + velocidad cero, congelamiento parcial |
| `ignore` | LIMITADO | solo mensajería interna, no /msg de PumpChat |
| `info` | LIMITADO | solo datos expuestos por la API (version/TPS/memoria/mundos) |
| `invsee` | LIMITADO | vista de solo lectura del inventario ajeno |
| `item` | IMPLEMENTADO | — |
| `itemdb` | IMPLEMENTADO | — |
| `itemlore` | IMPLEMENTADO | — |
| `itemname` | IMPLEMENTADO | — |
| `jailedplayers` | IMPLEMENTADO | — |
| `jails` | AÑADIDO AHORA | hueco real de la auditoría, implementado en esta iteración |
| `jump` | IMPLEMENTADO | — |
| `kick` | CUBIERTO | núcleo Pumpkin (/kick) |
| `kickall` | IMPLEMENTADO | — |
| `kill` | CUBIERTO | núcleo Pumpkin (/kill) |
| `kit` | LIMITADO | guarda solo id y count por objeto |
| `kitreset` | IMPLEMENTADO | — |
| `kittycannon` | LIMITADO | spawns simples, sin variantes ni domesticación |
| `lightning` | IMPLEMENTADO | — |
| `list` | CUBIERTO | núcleo Pumpkin (/list) |
| `loom` | IMPLEMENTADO | — |
| `mail` | IMPLEMENTADO | — |
| `me` | CUBIERTO | núcleo Pumpkin (/me) |
| `more` | IMPLEMENTADO | — |
| `motd` | CUBIERTO | dMOTD (/motd) |
| `msg` | CUBIERTO | PumpChat (/msg) |
| `msgtoggle` | LIMITADO | no intercepta /msg de PumpChat; afecta a /r y /mail internos |
| `mute` | IMPLEMENTADO | — |
| `near` | IMPLEMENTADO | — |
| `nick` | LIMITADO | usa tab-list y puede ser sobrescrito por otros plugins |
| `nuke` | LIMITADO | explosiones en rejilla, sin TNT temporizada |
| `tpoffline` | LIMITADO | usa última ubicación propia, no usercache de Pumpkin |
| `pay` | IMPLEMENTADO | — |
| `paytoggle` | IMPLEMENTADO | — |
| `payconfirmtoggle` | IMPLEMENTADO | — |
| `ping` | IMPLEMENTADO | — |
| `playtime` | IMPLEMENTADO | — |
| `potion` | LIMITADO | sin NBT de efectos; entrega poción con nombre |
| `powertool` | IMPLEMENTADO | — |
| `powertoollist` | IMPLEMENTADO | — |
| `powertooltoggle` | IMPLEMENTADO | — |
| `ptime` | LIMITADO | API sin hora por jugador; ajusta la hora del mundo |
| `pweather` | IMPLEMENTADO | — |
| `r` | LIMITADO | usa el reply target guardado por este plugin |
| `rtoggle` | LIMITADO | solo mensajería interna |
| `realname` | IMPLEMENTADO | — |
| `recipe` | CUBIERTO | núcleo Pumpkin (/recipe) |
| `remove` | LIMITADO | get_entities no expone tipo; filtro validado pero no aplicado |
| `renamehome` | IMPLEMENTADO | — |
| `repair` | LIMITADO | WIT 0.1 no expone get_damage/set_damage en item-stack |
| `rest` | IMPLEMENTADO | — |
| `rules` | IMPLEMENTADO | — |
| `seen` | LIMITADO | solo jugadores vistos desde la instalación del plugin |
| `sell` | LIMITADO | sin encantamientos/NBT; artículos sin precio no se venden |
| `sethome` | IMPLEMENTADO | — |
| `setjail` | IMPLEMENTADO | — |
| `settpr` | IMPLEMENTADO | — |
| `setwarp` | IMPLEMENTADO | — |
| `setworth` | LIMITADO | precio por clave de objeto, sin NBT |
| `showkit` | IMPLEMENTADO | — |
| `editsign` | LIMITADO | bloque objetivo aproximado a 5 bloques |
| `skull` | LIMITADO | sin perfil/textura de jugador |
| `smithingtable` | IMPLEMENTADO | — |
| `socialspy` | LIMITADO | solo copia /r y /mail de este plugin |
| `spawner` | LIMITADO | set_block_by_name sin NBT de tipo de mob |
| `spawnmob` | IMPLEMENTADO | — |
| `speed` | IMPLEMENTADO | — |
| `stonecutter` | AÑADIDO AHORA | hueco real de la auditoría, implementado en esta iteración |
| `sudo` | LIMITADO | lista de bloqueo de comandos peligrosos; se ejecuta con permisos del objetivo |
| `suicide` | IMPLEMENTADO | — |
| `tempban` | IMPLEMENTADO | — |
| `tempbanip` | LIMITADO | API WIT no expone gestión de bans por IP; solo /tempban por UUID |
| `thunder` | IMPLEMENTADO | — |
| `time` | CUBIERTO | núcleo Pumpkin (/time) |
| `togglejail` | IMPLEMENTADO | — |
| `top` | LIMITADO | usa get_top_block_y sin búsqueda de ubicación segura |
| `tp` | CUBIERTO | núcleo Pumpkin (/tp) |
| `tpa` | IMPLEMENTADO | — |
| `tpaall` | IMPLEMENTADO | — |
| `tpaccept` | IMPLEMENTADO | — |
| `tpahere` | IMPLEMENTADO | — |
| `tpall` | IMPLEMENTADO | — |
| `tpauto` | IMPLEMENTADO | — |
| `tpacancel` | IMPLEMENTADO | — |
| `tpdeny` | IMPLEMENTADO | — |
| `tphere` | IMPLEMENTADO | — |
| `tpo` | IMPLEMENTADO | — |
| `tpohere` | LIMITADO | API no permite mover entidades offline |
| `tppos` | IMPLEMENTADO | — |
| `tpr` | IMPLEMENTADO | — |
| `tptoggle` | IMPLEMENTADO | — |
| `tree` | LIMITADO | generación simplificada de roble |
| `unban` | CUBIERTO | núcleo Pumpkin (/pardon) |
| `unbanip` | CUBIERTO | núcleo Pumpkin (/pardonip) |
| `unlimited` | LIMITADO | sin evento fiable de consumo para reponer objetos |
| `vanish` | IMPLEMENTADO | — |
| `warp` | IMPLEMENTADO | — |
| `warpinfo` | IMPLEMENTADO | — |
| `weather` | CUBIERTO | núcleo Pumpkin (/weather) |
| `whois` | IMPLEMENTADO | — |
| `workbench` | IMPLEMENTADO | — |
| `world` | LIMITADO | teletransporta al spawn del mundo, sin escalado Nether |
| `worth` | LIMITADO | precio por clave de objeto, sin NBT |

## Limitaciones generales

- La API WIT 0.1 de Pumpkin no expone NBT arbitrario de objetos ni de block entities: no se pueden fijar encantamientos, durabilidad, efectos de poción, textura de cabezas ni tipo de mob en spawners.
- No existe hora por jugador: `/ptime` modifica el mundo. El clima sí es por jugador.
- No hay bangroup/IP management desde plugins: `/tempbanip` queda LIMITADO; los baneos de nombre propios viven en `state.json` y no reemplazan al banco del núcleo.
- La mensajería privada de PumpChat no se puede interceptar sin duplicar `/msg`: `/msgtoggle`, `/socialspy`, `/ignore`, `/r` y `/rtoggle` cubren solo la mensajería interna de EssentialsFusion.
- Las confirmaciones destructivas se guardan en memoria (se pierden al recargar); la preferencia de `/clearinventory` sí persiste en `state.json`.
- Los archivos de datos (`config.json`, `homes.json`, `warps.json`, `state.json` schema 2, `economy.json` schema 1, `kits.json` schema 1) se mantienen y sus migraciones son read-only con escritura diferida al primer tick.

# EssentialsFusion

Un único plugin WASM para Pumpkin que fusiona **PumpkinEssentials 0.1.0** (homes, warps, `/tpa`, `/back`, warmups/cooldowns), las utilidades estilo **EssentialsX** y las funciones de **VEssentials 1.2.7** (`/gm[a|c|s|sp]`, `/fly`, `/flyspeed`, `/afk`).

## Compatibilidad

- Servidor: Pumpkin `0.2.2+26.3-26.60` (Java protocolo 777 / Bedrock 1.26.60).
- API: `pumpkin-plugin-api` del tag exacto del servidor (`rev eda69440e6369c5f8a05c3f69fc809c40dab93a6`), bindings WIT `pumpkin:plugin@0.1.0`, WASI `p2` (0.2.x).
- Permisos WASI: `fs.read.data` y `fs.write.data`.
- `PluginMetadata.name` es **`pumpkin_essentials`** por diseño: todos los nodos de permiso usan ese namespace y la carpeta de datos es la misma de siempre.

## Compilación

```sh
rustup toolchain install stable
rustup target add wasm32-wasip2
cd dev/essentials-fusion
cargo test --lib
cargo build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/essentials_fusion.wasm ./essentials_fusion.wasm
```

`Cargo.toml` fija el repositorio upstream de Pumpkin al tag del servidor; no hay dependencias privadas.

## Instalación y migración

1. Apaga el servidor (o usa la recarga de plugins).
2. Quita/renombra `plugins/PumpkinEssentials.wasm` para no tener dos plugins con el mismo nombre.
3. Copia `essentials_fusion.wasm` a `plugins/` y arranca.
4. Los permisos de archivo se aprueban automáticamente; si tu configuración los bloquea:

   ```toml
   [plugins.overrides.pumpkin_essentials]
   enabled = true
   allowed_permissions = ["fs.read.data", "fs.write.data"]
   ```

Los datos se leen y escriben **directamente** en `plugins/data/pumpkin_essentials/`; no hay copia ni conversión. Se mantienen los formatos canónicos:

```jsonc
// homes.json
{ "<uuid>": { "name_last_seen": "Nombre", "limit": 5, "homes": { "base": { "world": "world", "x": 1.0, "y": 64.0, "z": 2.0, "yaw": 0.0, "pitch": 0.0 } } } }
// warps.json
{ "spawn": { "world": "world", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0, "created_by": "Owner" } }
```

Migración automática en carga: envoltorios `players`/`warps`, `position`/`rotation`, `dimension`/`world_name`, números como cadena, `created_by` ausente y wrapper `{"state": ...}` en `state.json`. Todo se reescribe de forma atómica solo si cambió. Un JSON inválido **nunca** se sobreescribe: el plugin falla al cargar para proteger los datos.

### `config.json`

Conserva las cinco claves originales y añade opciones nuevas con valores por defecto:

| Clave | Defecto | Uso |
|---|---|---|
| `default_max_homes` | `3` | Límite de homes por jugador |
| `ops_bypass_home_limit` | `false` | Los ops ignoran el límite |
| `warmup_secs` | `3` | Espera antes del teleport (0 = instantáneo) |
| `cooldown_secs` | `30` | Cooldown de `/home` y `/tpa` |
| `tpa_expire_secs` | `60` | Caducidad de peticiones |
| `flyspeed_unit` | `0.05` | Velocidad del motor por 1 unidad de `/flyspeed` |
| `afk_broadcast` | `true` | Anunciar cambios de AFK |
| `afk_auto_unafk_on_move` | `true` | Quitarse AFK al moverse >0.5 bloques |
| `afk_tab_name` | `true` | Mostrar `[AFK]` en la lista de jugadores |
| `restore_fly_on_join` | `true` | Restaurar vuelo guardado al entrar |
| `restore_afk_on_join` | `true` | Restaurar AFK guardado al entrar |
| `spawns` | `{}` | Spawns por mundo guardados por `/setspawn` |

Un `config.json` antiguo sigue siendo válido.

## Comandos

### Homes y warps

| Comando | Permiso | Descripción |
|---|---|---|
| `/sethome [nombre]`, `/home [nombre]`, `/delhome [nombre]` | `pumpkin_essentials:home` | Gestiona homes; nombre por defecto `home` |
| `/homes`, `/listhomes` | `pumpkin_essentials:home` | Lista clicable **con mundo y coordenadas** y `usados/límite` |
| `/renamehome <viejo> <nuevo>` | `pumpkin_essentials:renamehome` | Renombra un home conservando su ubicación |
| `/sethomelimit <jugador> <n>` | `pumpkin_essentials:admin` | Límite individual persistente |
| `/setwarp`, `/delwarp` | `pumpkin_essentials:admin` | Gestiona warps públicos |
| `/warp [nombre]`, `/warps` | `pumpkin_essentials:warp` | Teletransporta o lista warps |

### Teletransporte

| Comando | Permiso | Descripción |
|---|---|---|
| `/tpa <jugador>` | `pumpkin_essentials:tpa` | Pide ir a otro jugador |
| `/tpahere <jugador>` | `pumpkin_essentials:tpahere` | Pide que el otro jugador venga a ti |
| `/tpaccept [jugador]`, `/tpdeny [jugador]` | `pumpkin_essentials:tpa` | Responde la petición (la más reciente si no hay argumento) |
| `/tphere <jugador>` | `pumpkin_essentials:tphere` | Trae a un jugador (op nivel 2) |
| `/tpall` | `pumpkin_essentials:tpall` | Trae a todos los jugadores online (op nivel 3) |
| `/tptoggle` | `pumpkin_essentials:tptoggle` | Bloquea/permite peticiones entrantes |
| `/back` | `pumpkin_essentials:back` | Vuelve a la última ubicación (ahora persistente) |
| `/spawn`, `/setspawn` | `pumpkin_essentials:spawn` / `:setspawn` | Ir al spawn / guardar el spawn del mundo |
| `/top [jugador]` | `pumpkin_essentials:top` | Teletransporta al bloque más alto de la columna |

Los warmups se cancelan al moverte o recibir daño. `/tpahere` usa el mismo sistema de peticiones; al aceptar se teletransporta quien aceptó. `/tptoggle` se respeta tanto en `/tpa` como en `/tpahere`.

### Utilidades estilo EssentialsX

| Comando | Permiso | Descripción |
|---|---|---|
| `/heal [jugador]` | `pumpkin_essentials:heal` | Vida al máximo, comida 20 y saturación 10 |
| `/feed [jugador]` | `pumpkin_essentials:feed` | Comida 20, saturación 10 y exhausión 0 |
| `/speed [jugador] <0.1-10.0>` | `pumpkin_essentials:speed` | Velocidad de caminata (1.0 = 0.1 vanilla) |
| `/exp [jugador] [nivel]` | `pumpkin_essentials:exp` | Muestra o fija el nivel de experiencia |
| `/getpos [jugador]` | `pumpkin_essentials:getpos` | Mundo, posición, yaw/pitch y distancia |

### Modos, vuelo, AFK e inventarios

| Comando | Permiso | Descripción |
|---|---|---|
| `/gma`, `/gmc`, `/gms`, `/gmsp` [jugador] | `pumpkin_essentials:gamemode` | Aventura, creativo, supervivencia, espectador |
| `/gm <modo> [jugador]` | `pumpkin_essentials:gamemode` | `s/c/a/sp`, `0/1/2/3` o nombre completo |
| `/fly [jugador]` | `pumpkin_essentials:fly` | Activa/desactiva vuelo |
| `/flyspeed [jugador] <0.0-10.0>` | `pumpkin_essentials:flyspeed` | Velocidad de vuelo validada |
| `/afk [jugador] [razón]` | `pumpkin_essentials:afk` | Estado AFK, razón opcional, `[AFK]` en tab-list |
| `/invsee <jugador>` | `pumpkin_essentials:invsee` | Vista de solo lectura del inventario del jugador |
| `/enderchest [jugador]` | `pumpkin_essentials:enderchest` | Abre tu ender chest; vista de solo lectura si es otro |

### Iteración 4 — información y teleport avanzado (EssentialsX adaptado)

| Comando | Permiso | Descripción |
|---|---|---|
| `/ping [texto]` | `pumpkin_essentials:ping` | Muestra tu latencia o repite el texto |
| `/playtime [jugador]` | `pumpkin_essentials:playtime` | Tiempo acumulado de juego |
| `/near [jugador] [radio]` | `pumpkin_essentials:near` | Jugadores cercanos ordenados por distancia |
| `/seen <jugador>` | `pumpkin_essentials:seen` | Última conexión y último lugar guardado |
| `/whois <jugador>` | `pumpkin_essentials:whois` | UUID, modo, vida, comida, exp, ping, mundo y AFK |
| `/info` | `pumpkin_essentials:info` | Versión de Pumpkin, TPS/MSPT, jugadores y mundos |
| `/compass` | `pumpkin_essentials:compass` | Rumbo cardinal según yaw |
| `/depth` | `pumpkin_essentials:depth` | Bloques sobre/bajo el nivel del mar |
| `/warpinfo <nombre>` | `pumpkin_essentials:warpinfo` | Creador, mundo y coordenadas de un warp |
| `/gc` | `pumpkin_essentials:gc` | CPU, memoria, TPS, jugadores y mundos |
| `/bottom [jugador]` | `pumpkin_essentials:bottom` | Teleport a `min_y + 1` de la columna |
| `/tppos <x> <y> <z> [yaw] [pitch] [mundo]` | `pumpkin_essentials:tppos` | Teleport a coordenadas (admite `~` relativas) |
| `/tpr [mundo] [jugador]` | `pumpkin_essentials:tpr` | Teleport aleatorio a un punto seguro del mundo |
| `/settpr <mundo> center\|minrange\|maxrange [valor]` | `pumpkin_essentials:settpr` | Configura la zona de `/tpr` (op nivel 3) |
| `/tpaall [jugador]` | `pumpkin_essentials:tpaall` | Envía `/tpahere` a todos los jugadores online |
| `/tpacancel [jugador]` | `pumpkin_essentials:tpacancel` | Cancela tus peticiones `/tpa` / `/tpahere` pendientes |
| `/tpauto [on\|off] [jugador]` | `pumpkin_essentials:tpauto` | Acepta automáticamente peticiones entrantes |
| `/tpo <jugador> [a-jugador]` | `pumpkin_essentials:tpo` | Teleport instantáneo (propio o de otro jugador) |
| `/tpoffline <jugador>` | `pumpkin_essentials:tpoffline` | Va a la última ubicación guardada de un desconectado |
| `/tpohere <jugador>` | `pumpkin_essentials:tpohere` | Trae a un jugador conectado; ver limitación abajo |
| `/rest [jugador]` | `pumpkin_essentials:rest` | Reinicia exhausión y saturación |
| `/world [nombre]` | `pumpkin_essentials:world` | Lista mundos o va al spawn del mundo indicado |

Nuevas claves de `config.json` (valores por defecto): `near_max_radius=200`, `tpr_min_range=50.0`, `tpr_max_range=1000.0`, `tpr_attempts=24` y `tpr` (mapa por mundo con `center`, `min_range`, `max_range`). `/settpr center` guarda tu posición actual como centro; `/tpr` elige un ángulo y distancia aleatorios en ese anillo, usa `get_top_block_y` y aplica el mismo warmup que los demás teleports.

### Iteración 5 — chat, moderación y staff (EssentialsX adaptado)

| Comando | Permiso | Descripción |
|---|---|---|
| `/broadcastworld [mundo] <mensaje>` | `pumpkin_essentials:broadcastworld` | Anuncio solo para los jugadores de un mundo |
| `/msgtoggle [on\|off] [jugador]` | `pumpkin_essentials:msgtoggle` | Bloquea respuestas `/r` entrantes |
| `/socialspy [on\|off] [jugador]` | `pumpkin_essentials:socialspy` | Copia de las respuestas `/r` al staff |
| `/ignore [jugador]` | `pumpkin_essentials:ignore` | Lista/alternar ignorados (afecta a `/r` y `/mail`) |
| `/nick [jugador] <nick\|off>` | `pumpkin_essentials:nick` | Apodo en tab-list; `/nick off` lo quita |
| `/realname <apodo>` | `pumpkin_essentials:realname` | Nombre real asociado a un apodo |
| `/mail [read\|clear\|<jugador> <mensaje>]` | `pumpkin_essentials:mail` | Correo interno persistente |
| `/r <mensaje>` | `pumpkin_essentials:r` | Responde al último remitente conocido |
| `/rtoggle [on\|off] [jugador]` | `pumpkin_essentials:rtoggle` | Permite o bloquea respuestas |
| `/antioch` | `pumpkin_essentials:antioch` | Explosión de TNT frente al jugador |
| `/customtext [clave]` | `pumpkin_essentials:customtext` | Muestra textos configurados (`config.json`) |
| `/rules` | `pumpkin_essentials:rules` | Muestra las reglas de `config.json` |
| `/helpop <mensaje>` | `pumpkin_essentials:helpop` | Envía el mensaje a quienes tienen `:helpop.receive` |
| `/backup` | `pumpkin_essentials:backup` | Copia los JSON del plugin a `backups/<timestamp>/` |
| `/essentials [version]` | `pumpkin_essentials:essentials` | Información del plugin |
| `/ext [jugador]` | `pumpkin_essentials:ext` | Apaga el fuego (`set_fire_ticks(0)`) |
| `/mute <jugador> [duración] [razón]` | `pumpkin_essentials:mute` | Silencia el chat (alias `/unmute`) |
| `/tempban <jugador> <duración> [razón]` | `pumpkin_essentials:tempban` | Banea temporalmente y expulsa al jugador |
| `/kickall [razón]` | `pumpkin_essentials:kickall` | Expulsa a todos salvo al emisor |
| `/vanish [on\|off] [jugador]` | `pumpkin_essentials:vanish` | Oculta jugadores mediante `hide_player` |
| `/god [on\|off] [jugador]` | `pumpkin_essentials:god` | Invulnerabilidad persistente |
| `/sudo <jugador> <comando>` | `pumpkin_essentials:sudo` | Ejecuta un comando como otro jugador (con lista de bloqueo) |
| `/jailedplayers` | `pumpkin_essentials:jailedplayers` | Lista jugadores encarcelados |
| `/jail <jugador> [cárcel] [duración]` | `pumpkin_essentials:jail` | Encarcela y teletransporta a la cárcel |
| `/setjail <nombre>` | `pumpkin_essentials:setjail` | Guarda tu posición como cárcel |
| `/deljail <nombre>` | `pumpkin_essentials:deljail` | Elimina una cárcel |
| `/togglejail <jugador> [cárcel] [duración]` | `pumpkin_essentials:togglejail` | Encarcela o libera según su estado |

Duraciones: `30` (segundos), `5m`, `2h`, `1d` o `perm`/`0` (permanente). Nuevas claves de `config.json`: `custom_texts` (mapa clave -> líneas), `rules` (lista) y `jails` (mapa nombre -> ubicación).

### Iteración 6 — Economía integrada

| Comando | Permiso | Descripción |
|---|---|---|
| `/balance [jugador]` (alias `/bal`) | `pumpkin_essentials:balance` | Saldo propio o de otro jugador |
| `/balancetop [página]` | `pumpkin_essentials:balancetop` | Ranking de saldos, 10 por página |
| `/eco <set\|add\|take\|reset> <jugador> <cantidad>` | `pumpkin_essentials:eco` | Administración de saldo (op nivel 3) |
| `/pay <jugador> <cantidad>` | `pumpkin_essentials:pay` | Pagar; por defecto requiere `/payconfirm` |
| `/payconfirm` | `pumpkin_essentials:payconfirm` | Confirma el último pago preparado |
| `/paytoggle [jugador]` | `pumpkin_essentials:paytoggle` | Dejar de / volver a aceptar pagos |
| `/payconfirmtoggle` | `pumpkin_essentials:payconfirmtoggle` | Activa/desactiva la confirmación de `/pay` |
| `/sell [all\|hand\|<ítem>]` | `pumpkin_essentials:sell` | Vende montones según la tabla `worth` |
| `/worth <cantidad> <ítem>` | `pumpkin_essentials:worth` | Consulta precio unitario y total |
| `/setworth <ítem> <precio>` | `pumpkin_essentials:setworth` | Fija precio por unidad (op nivel 3) |

El dinero se guarda en `plugins/data/pumpkin_essentials/economy.json` (ledger propio, sin Vault/PVault). Los importes internos son enteros en céntavos; `config.json` añade `currency_symbol` (`$`) y `currency_decimals` (`2`). El formato del archivo es:

```jsonc
{
  "schema": 1,
  "balances": { "<uuid>": 1500 },
  "worth": { "minecraft:stone": 1, "minecraft:diamond": 500 },
  "pay_off": { "<uuid>": false },
  "payconfirm_off": { "<uuid>": false },
  "pending_pay": { "<uuid>": { "to_uuid": "<uuid>", "to_name": "Nombre", "amount": 250, "at_ms": 0 } }
}
```

`load_economy` solo lee: las formas legacy (`{"economy": ...}` o `schema` antiguo) se normalizan en memoria y la primera ejecución del tick escribe la forma canónica. JSON corrupto = error de carga sin sobreescritura. Las claves de ítem aceptan `minecraft:stone` o `stone` (se normalizan a minúsculas con namespace).

### Iteración 7 — Kits e ítems

| Comando | Permiso | Descripción |
|---|---|---|
| `/kit [nombre] [jugador]` | `pumpkin_essentials:kit` | Lista kits o entrega uno (con cooldown) |
| `/createkit <nombre> <delay_segundos>` | `pumpkin_essentials:createkit` | Guarda el inventario actual como kit (op nivel 3) |
| `/delkit <nombre>` | `pumpkin_essentials:delkit` | Elimina un kit (op nivel 3) |
| `/kitreset <kit> [jugador]` | `pumpkin_essentials:kitreset` | Reinicia el cooldown de un kit (op nivel 2) |
| `/showkit <nombre>` | `pumpkin_essentials:showkit` | Muestra el contenido de un kit |
| `/item <id> [cantidad] [jugador]` | `pumpkin_essentials:item` | Crea y entrega un objeto |
| `/itemdb [id]` | `pumpkin_essentials:itemdb` | Muestra la clave de registry del objeto en mano |
| `/itemlore add\|set\|clear <texto>` | `pumpkin_essentials:itemlore` | Edita el lore del objeto en mano |
| `/itemname <nombre\|off>` | `pumpkin_essentials:itemname` | Renombra el objeto en mano |
| `/repair [all]` | `pumpkin_essentials:repair` | Limitación de API: ver nota abajo |
| `/unlimited <ítem\|list\|clear> [jugador]` | `pumpkin_essentials:unlimited` | Gestiona objetos marcados como ilimitados |
| `/hat` | `pumpkin_essentials:hat` | Coloca el objeto en mano como casco |
| `/more [cantidad]` | `pumpkin_essentials:more` | Amplía el montón en mano hasta su máximo |
| `/book` | `pumpkin_essentials:book` | Entrega un libro en blanco |
| `/powertool <comando\|d:> [ítem]` | `pumpkin_essentials:powertool` | Asigna/borra comandos al objeto en mano |
| `/powertoollist` | `pumpkin_essentials:powertoollist` | Lista powertools |
| `/powertooltoggle` | `pumpkin_essentials:powertooltoggle` | Activa/desactiva powertools |
| `/condense [ítem]` | `pumpkin_essentials:condense` | Empaqueta montones del inventario |
| `/remove [tipo]` | `pumpkin_essentials:remove` | Elimina entidades del mundo (op nivel 3) |
| `/spawnmob <mob> [cantidad] [jugador]` | `pumpkin_essentials:spawnmob` | Genera entidades (op nivel 3) |
| `/spawner <mob> [delay]` | `pumpkin_essentials:spawner` | Coloca un bloque spawner (op nivel 3) |
| `/skull <nombre>` | `pumpkin_essentials:skull` | Entrega una cabeza con nombre |
| `/kittycannon` | `pumpkin_essentials:kittycannon` | Lanza varios gatos |
| `/beezooka` | `pumpkin_essentials:beezooka` | Lanza una abeja que explota al impactar |
| `/fireball [tipo]` | `pumpkin_essentials:fireball` | Lanza un proyectil |
| `/potion <efecto>` | `pumpkin_essentials:potion` | Entrega una poción con nombre |

Kits, cooldowns, powertools y objetos ilimitados se guardan en `plugins/data/pumpkin_essentials/kits.json`:

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

Igual que `state.json`/`economy.json`: `load_kits` solo lee, aplica migraciones legacy en memoria y la primera ejecución del tick escribe el archivo canónico. JSON corrupto = error de carga sin sobreescritura. Las claves de objeto aceptan `minecraft:stone` o `stone`.

### Iteración 8 — Bloques, entornos y efectos

| Comando | Permiso | Descripción |
|---|---|---|
| `/anvil`, `/cartographytable`, `/grindstone`, `/loom`, `/smithingtable`, `/workbench` | nodos con el mismo nombre | Abre la pantalla correspondiente mediante `Gui::new(Screen::…)` |
| `/disposal` | `pumpkin_essentials:disposal` | Abre un inventario de 54 ranuras vacío |
| `/editsign` | `pumpkin_essentials:editsign` | Abre el editor de cartel del bloque mirado (~5 bloques) |
| `/jump` | `pumpkin_essentials:jump` | Teletransporta al punto mirado, un bloque arriba |
| `/ice [jugador]` | `pumpkin_essentials:ice` | Congela (ticks de escarcha + velocidad cero) |
| `/tree`, `/bigtree` | `pumpkin_essentials:tree` / `:bigtree` | Genera un árbol de roble simple o grande |
| `/break` | `pumpkin_essentials:break` | Elimina el bloque mirado (protege bedrock salvo op3) |
| `/burn <jugador> <segundos>` | `pumpkin_essentials:burn` | Prende fuego al jugador |
| `/lightning [jugador]` | `pumpkin_essentials:lightning` | Invoca un rayo en el objetivo o punto mirado |
| `/thunder on\|off` | `pumpkin_essentials:thunder` | Activa/desactiva tormenta y lluvia del mundo |
| `/nuke [jugadores]` | `pumpkin_essentials:nuke` | Bombardeo con confirmación (op nivel 3) |
| `/suicide` | `pumpkin_essentials:suicide` | Termina con tu propia vida y lo anuncia |
| `/clearinventory [jugador]` | `pumpkin_essentials:clearinventory` | Vacía inventario, armour y mano (confirmable) |
| `/clearinventoryconfirmtoggle` | `pumpkin_essentials:clearinventoryconfirmtoggle` | Activa/desactiva esa confirmación |
| `/ptime [get\|day\|noon\|night\|midnight\|ticks]` | `pumpkin_essentials:ptime` | Consulta/ajusta la hora del mundo |
| `/pweather [get\|clear\|rain\|reset]` | `pumpkin_essentials:pweather` | Clima personal por jugador (API disponible) |

`/nuke` y `/clearinventory` usan una confirmación en memoria: la primera ejecución prepara la acción y hay que repetir el comando para ejecutarla. `/clearinventoryconfirmtoggle` guarda `clear_confirm_off` en `state.json` (prefijo ya migrado de forma segura). `/ptime` es mundial porque la API WIT 0.1 no expone hora por jugador; `/pweather` sí usa `set_player_weather`/`reset_player_weather`.

### Iteración 9 — Auditoría final y cierre de huecos

Se compararon los **153 comandos oficiales** de EssentialsX 2.x (`plugin.yml`) con los 141 nombres que el plugin registraba y con los comandos del núcleo Pumpkin y de PumpChat/dMOTD. El inventario completo está en [`COMPAT.md`](COMPAT.md): 88 IMPLEMENTADO, 19 CUBIERTO, 43 LIMITADO y 3 AÑADIDO AHORA.

| Comando | Permiso | Descripción |
|---|---|---|
| `/stonecutter` | `pumpkin_essentials:stonecutter` | Abre la pantalla del cortador de piedra (hueco real) |
| `/firework` | `pumpkin_essentials:firework` | Entrega un cohete pirotécnico (hueco real) |
| `/jails` | `pumpkin_essentials:jails` | Lista las cárceles definidas con `/setjail` (hueco real) |

Tras esta iteración el plugin registra **144 nombres de comando** (incluye alias propios como `/bal`, `/gm`, `/listhomes` y `/payconfirm`). No se duplican comandos del núcleo ni de otros plugins.

## Persistencia de sesión (`state.json`)

Nuevo archivo en `plugins/data/pumpkin_essentials/`:

```jsonc
{
  "schema": 2,
  "back": { "<uuid>": { "world": "...", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0 } },
  "fly": { "<uuid>": { "allow_flying": true, "flying": true, "fly_speed": 0.15 } },
  "afk": { "<uuid>": { "active": true, "reason": "comiendo", "since_ms": 0 } },
  "tpa_off": { "<uuid>": true },
  "tpa_auto": { "<uuid>": false },
  "playtime_ms": { "<uuid>": 3600000 },
  "last_seen_ms": { "<uuid>": 1700000000000 },
  "last_location": { "<uuid>": { "world": "world", "x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0 } },
  "names": { "<uuid>": "Nombre" },
  "msg_off": { "<uuid>": false },
  "social_spy": { "<uuid>": true },
  "ignored": { "<uuid>": ["<otro-uuid>"] },
  "nicks": { "<uuid>": "Nick" },
  "reply_target": { "<uuid>": "<otro-uuid>" },
  "rtoggle": { "<uuid>": true },
  "mail": { "<uuid>": [{ "from": "Nombre", "from_uuid": "<uuid>", "body": "hola", "time_ms": 0, "read": false }] },
  "mutes": { "<uuid>": { "until_ms": 0, "reason": "spam", "by": "Mod" } },
  "bans": { "<uuid>": { "until_ms": 0, "reason": "abuso", "by": "Admin" } },
  "vanished": { "<uuid>": false },
  "god": { "<uuid>": false },
  "jailed": { "<uuid>": { "jail": "main", "until_ms": 0, "by": "Mod" } }
}
```

- `load_state` **solo lee y parsea**: las formas legacy (`{"state": ...}`, `schema=1`) se normalizan en memoria y se marca `needs_migration`. La primera ejecución del tick escribe la forma canónica, así que el arranque no hace escrituras bloqueantes.
- Se guarda tras cada teleport, cambio de vuelo/AFK, moderación, correo, preferencias dechat y desconexión. El tiempo de juego se acumula en el tick cada ~250 ms y se escribe cada ~2 s.
- Al **entrar** se restauran `/back`, vuelo, AFK, apodo `[AFK]`, vanish, god y cárcel; si hay un ban activo se expulsa al jugador con el motivo.
- El chat se cancela para jugadores silenciados o encarcelados; `/tempban` expulsa al jugador y el ban se comprueba en cada conexión.
- Archivo ausente = sin estado. JSON corrupto = error de carga, sin sobreescritura. Los esquemas anteriores se amplían sin perder datos.

## Nodos de permiso

Todos pertenecen al namespace `pumpkin_essentials:` (obligatorio: el loader rechaza otros namespaces).

**Allow:** `home`, `warp`, `tpa`, `back`, `afk`, `spawn`, `top`, `tptoggle`, `tpahere`, `renamehome`, `ping`, `playtime`, `near`, `compass`, `depth`, `warpinfo`, `info`, `rest`, `tpacancel`, `tpauto`, `tpr`, `world`, `msgtoggle`, `ignore`, `nick`, `realname`, `mail`, `r`, `rtoggle`, `helpop`, `customtext`, `rules`, `balance`, `balancetop`, `pay`, `paytoggle`, `payconfirmtoggle`, `payconfirm`, `sell`, `worth`, `kit`, `showkit`, `itemdb`, `powertoollist`, `powertooltoggle`, `anvil`, `cartographytable`, `grindstone`, `loom`, `smithingtable`, `workbench`, `disposal`, `editsign`, `jump`, `ptime`, `pweather`, `stonecutter`.

**Op nivel 2:** `gamemode`, `fly`, `flyspeed`, `heal`, `feed`, `speed`, `exp`, `getpos`, `tphere`, `invsee`, `enderchest`, `seen`, `whois`, `gc`, `bottom`, `tppos`, `tpo`, `tpoffline`, `tpohere`, `tpaall`, `socialspy`, `mute`, `unmute`, `vanish`, `god`, `jail`, `togglejail`, `jailedplayers`, `broadcastworld`, `ext`, `antioch`, `essentials`, `helpop.receive`, `item`, `itemlore`, `itemname`, `repair`, `unlimited`, `hat`, `more`, `book`, `powertool`, `condense`, `skull`, `kittycannon`, `beezooka`, `fireball`, `potion`, `kitreset`, `ice`, `tree`, `bigtree`, `break`, `burn`, `lightning`, `thunder`, `suicide`, `clearinventory`, `clearinventoryconfirmtoggle`, `firework`, `jails`.

**Op nivel 3:** `admin`, `setspawn`, `tpall`, `settpr`, `tempban`, `kickall`, `sudo`, `backup`, `setjail`, `deljail`, `eco`, `setworth`, `createkit`, `delkit`, `remove`, `spawnmob`, `spawner`, `nuke`.



Los grupos actuales (`default`, `vip`, `mod`, `owner`, `admin`) ya tienen `pumpkin_essentials:home|warp|tpa|back|admin`, así que siguen funcionando. Para dar nuevas facultades, añade p. ej. `pumpkin_essentials:heal` y `:feed` a `mod`, `pumpkin_essentials:fly` a `vip`, y ejecuta `/lp sync`. Apuntar a otro jugador con `/gm`, `/fly`, `/flyspeed`, `/heal`, `/feed` o `/enderchest` requiere además nivel de op 2; `/tphere` y `/tpall` comprueban op 2 y 3 respectivamente.

## Limitaciones conocidas

- VEssentials no es público: su lista de comandos se tomó del marketplace; la semántica de `/flyspeed` se documenta arriba.
- `/invsee` y `/enderchest <jugador>` muestran una **copia de solo lectura** porque la API permite crear un `Gui` y copiar `ItemStack`s, pero no enlazar la GUI directamente al inventario de otro jugador. `/enderchest` sin jugador sí abre la pantalla real del jugador.
- `/top` usa `World::get_top_block_y(x, z)`; la API no expone búsqueda de ubicación segura (-fluids, techo, etc.), por lo que el destino es `y + 1` sobre el bloque más alto de la columna.
- El `[AFK]` se aplica con `set_tab_list_name`: puede sobrescribir el nombre de tab-list puesto por otro plugin. El anuncio por chat sigue activo.
- `/back` ahora persiste, pero el historial es una sola ubicación por jugador (igual que en PumpkinEssentials).
- `/tpohere` solo puede traer a jugadores **conectados**: la API WIT no expone manipulación de entidades offline. Si el jugador está desconectado, el comando explica el caso y sugiere `/tpoffline`; no finge éxito.
- `/seen`, `/tpoffline` y `/tpauto` usan los datos guardados por EssentialsFusion en `state.json`, no `usercache.json` ni `perms/names.json` (el sandbox WASI solo preabre la carpeta de datos del plugin). Solo aparecen jugadores que hayan entrado desde que el plugin está instalado.
- `/info` y `/gc` muestran únicamente lo que expone la API: versión de Pumpkin, TPS/MSPT, CPU, memoria, jugadores y mundos. No hay página de texto configurable ni conteo de chunks/entidades por mundo como en Bukkit.
- `/world <nombre>` lleva al **spawn del mundo** usando `get_spawn_location` + `teleport_world`; no replica el escalado de coordenadas Nether/Overworld de EssentialsX.
- `/msgtoggle`, `/ignore`, `/socialspy`, `/rtoggle` y `/r` solo afectan a la mensajería implementada por este plugin (`/r`, `/mail`). No pueden interceptar `/msg` ni `/me` de PumpChat/core sin duplicar esos comandos; se respeta la regla de no duplicados.
- `/nick` cambia `set_tab_list_name`; si otro plugin (o `[AFK]`) reescribe ese nombre, puede sobrescribirse. `/realname` solo conoce apodos vistos desde que este plugin está instalado.
- `/tempban` y `/kickall` usan la API de expulsión de Java/Bedrock (o un mensaje de error si el handle no está disponible). No se modifica la lista de baneos del núcleo: el control es propio de `state.json`.
- `/sudo` ejecuta el comando con el `CommandSender` del jugador objetivo, por lo que se aplican sus permisos; además bloquea prefijos peligrosos (`sudo`, `op`, `perms`, `ban`, `backup`, etc.) para evitar escalado.
- `/backup` copia únicamente los JSON de este plugin a su carpeta privada `backups/<timestamp>/`; no hace copias del mundo ni llama a `save-all`.
- `/antioch` calcula un punto aproximado 6 bloques delante del jugador y usa `World::create_explosion` con interacción TNT; no spawna una entidad TNT real.
- La economía es **independiente** de Vault/PVault y de otros plugins: `/balance`, `/pay`, etc. solo consultan `economy.json`. No hay integración con un banco externo.
- `/sell` usa `get_registry_key()` y `get_count()`; los objetos sin precio en `worth` no se venden y se devuelven al inventario. No se distinguen encantamientos ni NBT: el precio es únicamente por clave de objeto y cantidad.
- `/sell` sin argumentos o `hand` vende solo el objeto de la ranura seleccionada; `all` recorre las 36 ranuras del inventario. La API no expone venta directa desde cofres.
- `/pay` requiere confirmación por defecto (`/payconfirm`); `/payconfirmtoggle` la desactiva. `/paytoggle` bloquea pagos entrantes. Los pagos pendientes se guardan en `economy.json` y se validan de nuevo al confirmar (saldo y preferencias).
- No hay intereses, impuestos ni límite máximo de dinero; los importes se validan como positivos y se almacenan en céntavos `i64`.
- `/repair` no puede restaurar durabilidad: la API WIT 0.1 no expone `get_damage`/`set_damage` en `item-stack`. El comando devuelve esa limitación concreta en lugar de destruir el objeto.
- `/book` entrega un `minecraft:writable_book`, pero la API no expone autor/título/páginas, por lo que no se pueden rellenar. `/potion` entrega una poción con nombre; los efectos NBT no se pueden fijar.
- `/skull` entrega `minecraft:player_head` con nombre: no se puede asignar el perfil/textura del jugador con esta API.
- `/spawner` coloca el bloque `minecraft:spawner` en el bloque mirado (aproximado 3 bloques delante), pero el tipo de mob del spawner no se puede guardar: el WIT no ofrece NBT de block entity para spawners.
- `/remove` elimina todas las entidades no jugador del mundo actual; el filtro por tipo se valida pero no se aplica porque `get_entities()` no expone el tipo de entidad de forma consultable desde la API.
- `/unlimited` guarda la lista de objetos y se puede consultar/borrar, pero la API no ofrece un evento de consumo de ítem fiable para reponerlos automáticamente.
- `/condense` reempaqueta montones por clave de objeto; al recrear los `ItemStack` se pierden NBT/encantamientos. `/kit` guarda solo id y cantidad por el mismo motivo.
- `/fireball`, `/beezooka` y `/kittycannon` usan `spawn_entity` + `set_velocity`; la explosión de `/beezooka` es inmediata (no hay retardo de 1 segundo como en Bukkit).
- `/ptime` modifica la hora del **mundo**, no del jugador: WIT 0.1 no expone `set_player_time`. `/pweather` sí es por jugador (`set_player_weather`).
- `/ice` aplica `set_freeze_ticks(140)` y anula la velocidad; es el equivalente visual/disponible, no un congelamiento completo de movimiento como Bukkit.
- `/tree` y `/bigtree` generan un roble simplificado con `set_block_by_name` (tronco + hojas); la API no expone generación de árboles por especie.
- `/editsign` abre el editor en el bloque aproximado 5 bloques delante; no se puede comprobar que sea realmente un cartel ni editar líneas por comando.
- `/nuke` y `/clearinventory` usan confirmación en memoria (se pierde al recargar) y `clear_confirm_off` persistente; `/nuke` crea explosiones en una rejilla, no entidades TNT con temporizador.
- `/break` elimina el bloque sin simular eventos de rotura ni drops; la API no expone un evento de block-break invocable desde plugin.



## Licencia y créditos

**GPL-3.0** (ver `LICENSE-GPL`). Este plugin es un trabajo derivado de:

- [EssentialsX/Essentials](https://github.com/EssentialsX/Essentials) — GPL-3.0. Se adaptaron sus comandos (heal/feed/speed/exp/getpos/spawn/top y, en la iteración 4, ping/playtime/near/seen/whois/info/compass/depth/warpinfo/gc/bottom/tppos/tpr/settpr/tpaall/tpacancel/tpauto/tpo/tpoffline/tpohere/rest/world) a la API de Pumpkin.
- [DarkTama/pumpkin-plugins](https://github.com/DarkTama/pumpkin-plugins) (`pumpkin-essentials`) — MIT OR Apache-2.0, se conservan `LICENSE-MIT` y `LICENSE-APACHE` con su crédito. El código permissivo puede integrarse en un proyecto GPL-3.0.
- La API y WIT de [Pumpkin-MC/Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) — MIT OR Apache-2.0.

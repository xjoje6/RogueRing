# Cooperativo con salas al estilo StS2

Investigación del 15-10-2026. Sustituye la parte de cooperativo de `MODO_MESA_REDONDA_Y_COOP.md`
(borrado en b0111a0). Marcas:
- **SABIDO**: comprobado en código, en el SDK o en documentación.
- **SUPUESTO**: muy probable, sin probar.
- **PROBAR**: no se sabe; hay que medirlo antes de prometerlo.

## En corto

1. **La sala es la parte fácil.** StS2 la resuelve con un anfitrión que manda, mensajes pequeños y una
   semilla común: cada juego genera la misma run por su cuenta y solo viajan las decisiones.
   RogueRing ya funciona así por dentro (`rng_comun` separado de `rng_rewards`), y en septiembre
   escribimos la mitad del protocolo (commit abb36da).
2. **Lo difícil es el combate.** Elden Ring solo sincroniza enemigos dentro de una sesión del propio
   juego. Sin servidores de FromSoft, esa sesión hoy solo la monta Seamless Co-op, que es de código
   cerrado. Hay un intento libre en Rust (unseamless-coop), pero todavía no mete a dos jugadores en el
   mismo mundo.
3. **Recomendación.** Hacer ya la sala propia y la run compartida, con el combate «en paralelo» (cada
   uno su copia del jefe, a la vez) como primer modo jugable. El combate en el mismo mundo iría encima,
   con Seamless debajo y un coliseo por combate (punto 5). Falta la prueba con dos PCs, así que no
   conviene prometerlo todavía.
4. **Fase 0 hecha (15-10, tarde):** el «jefe espejo» queda descartado. El juego no tiene búfer de red
   para los personajes de `spawn_chr`. Además, tu Seamless actual se cuelga al arrancar con me3 antes
   de cargar nuestra DLL. Detalle en «Resultados de la fase 0».
5. **Vía buena encontrada (15-10, noche): un coliseo por combate.** Antes de cada combate se cambia el
   MSB del coliseo por una variante que ya trae ese jefe colocado.
   - **Probado en local:** el jefe queda como enemigo del mapa, en un grupo con búfer de red.
   - **Probado en local:** el juego recoge el archivo nuevo en cada visita.
   - Como todos los juegos conocen el combate por la semilla, todos cargan la misma variante. Es lo
     mismo que hace el Randomizer, que funciona con Seamless.
   - Detalle en «Un coliseo por combate».

## Cómo lo hace StS2 (decomp en `sts2-mod/build/decomp`)

**SABIDO**, leído en el código:

- **Sala** (`StartRunLobby`):
  - Manda el anfitrión. Lleva la lista de jugadores y contesta a cada petición de entrada: acepta, o
    rechaza por sala llena (`LobbyFull`) o versión distinta.
  - Cada jugador elige personaje y marca «listo».
  - La Ascensión la pone el anfitrión, con un tope: la menor de las desbloqueadas por todos.
  - Con todos listos, el anfitrión envía `LobbyBeginRun` (semilla y jugadores) y cierra la sala.
- **Transporte:** sala de Steam solo para amigos (`CreateLobby(FriendsOnly)`) y sockets P2P en
  estrella hacia el anfitrión. Los amigos ven la sala por el juego de su amigo
  (`GetFriendGamePlayed().m_steamIDLobby`). Tiempos de espera de 10 a 20 s.
- **Mapa** (`MapSelectionSynchronizer`):
  - Vota cada jugador y no hay cuenta atrás: se espera a todos.
  - Con todos los votos, el anfitrión saca uno al azar con un generador propio de la semilla
    (`map_point_selection`). Dos votos al mismo nodo cuentan doble.
- **Eventos:** unos son compartidos y se votan; el empate lo rompe un generador de la semilla. En los
  demás, cada uno elige el suyo.
- **Personal:** recompensas, hoguera y tienda. Las recompensas salen de la semilla, así que no
  viajan; la tienda sí se sincroniza (`RewardSynchronizer`).
- **Seguridad:**
  - `ChecksumTracker` compara el estado de todos para detectar si alguien se ha desviado.
  - `LoadRunLobby` y `ClientRejoinRequest` permiten volver a una run a medias.

## Qué se lleva RogueRing

| Momento | En grupo | Cómo |
|---|---|---|
| Sala | Común | Personaje de cada uno, Ascensión del anfitrión (tope: la menor desbloqueada), semilla, «listo» |
| Bendición inicial | Personal | Cada uno la suya |
| Mapa | Votación | Como StS2: se espera a todos y el anfitrión sortea entre los votos con `rng_comun`. Una cuenta atrás larga (30 s) evita que alguien bloquee la partida |
| Combate, élite, jefe | Común | Mismo encuentro (sale de la semilla) |
| Recompensas | Personal | Generador por jugador: `rewards:<hueco>`. Así cada uno ve cartas distintas y la reconexión las puede recalcular |
| Tienda, gracia, forja | Personal | Cada uno con sus runas |
| Evento «?» | Votación | El contenido sale de `rng_comun`, como ya hace hoy. Si una opción es «solo tú», se resuelve para cada uno |
| Caer en combate | Grupo | Fuera hasta acabar el nodo. Si gana alguien, los caídos vuelven con poca vida (StS2 hace algo parecido). Si caen todos, se acaba la run |
| Pasar de nodo | Grupo | El mapa espera a que todos acaben sus recompensas («Esperando a tu compañero») |

## La sala

### Transporte: Steam, sin sala de Steam

- **SABIDO:**
  - La DLL ya habló con Steam en septiembre por la API plana de `steam_api64.dll`, que el juego carga.
    Se usó `ISteamNetworkingMessages` con mensajes fiables al SteamID y el canal 0x5252.
  - Sin puertos ni servidores, y funciona con el juego sin conexión a FromSoft.
  - El código está en abb36da (`dll/harness/src/coop.rs`) y se puede recuperar.
- **SABIDO** (documentación de Steam): enviar un mensaje a alguien acepta su sesión. No hace falta
  registrar callbacks si los dos se escriben.
- **Descubrimiento por presencia enriquecida (SUPUESTO):** el anfitrión publica
  `SetRichPresence("rr_sala", "<versión>|<plazas>")`.
  - La pantalla «Unirse» lista los amigos que juegan a Elden Ring y tienen esa clave.
  - Para entrar, el invitado pone `rr_unirse=<SteamID del anfitrión>` y escribe al anfitrión.
  - El anfitrión lo ve al mirar la presencia de sus amigos y contesta. Con eso la sesión queda
    aceptada en los dos sentidos.
  - Así no hace falta registrar callbacks de Steam en Rust.
- **Por qué no una sala de Steam como StS2 (SUPUESTO):**
  - Steam enseña a los amigos una sola sala por juego (`m_steamIDLobby` es un campo). Si el juego o
    Seamless tienen la suya, la nuestra competiría con ella.
  - Las invitaciones desde la interfaz de Steam abren Elden Ring sin me3, y no sirven.
  - **PROBAR** que Elden Ring no borre nuestra clave de presencia. Si la borra, se repone cada 10 s.
- **Con Seamless activo**, la sala se llena sola con los jugadores de la sesión del juego.
  `CSSessionManager.players` trae SteamID, anfitrión y jugador local; ya lo leímos en septiembre.
  El canal de mensajes es el mismo.

### Mensajes

JSON pequeño con número de versión, como en septiembre. Lo envía el anfitrión salvo que se diga otra
cosa.

- **Sala:**
  - El invitado envía `Hola`: versión, huella de los datos, nombre, personajes desbloqueados y
    Ascensión máxima.
  - El anfitrión contesta `Bienvenida` (hueco) o `Rechazo` (llena, versión distinta o run ya empezada).
  - El anfitrión reenvía `Sala` en cada cambio: jugadores, personaje, listo, Ascensión y semilla.
  - El invitado envía `Elegir` y `Listo`.
  - El anfitrión cierra con `Empezar`: semilla, Ascensión y jugadores con su hueco.
- **Run:**
  - Votos y decisiones del anfitrión: `Voto`/`Decision` en el mapa y `VotoEvento`/`DecisionEvento` en
    los eventos.
  - Cada uno avisa en qué pantalla está (`Fase`) para las esperas.
  - Al acabar cada nodo, cada uno envía `Suma`, una huella del estado común. Si no cuadra, el anfitrión
    reenvía la `Foto` de su estado común.
- **Combate:**
  - Los invitados envían `Danio` al anfitrión (ver bendiciones).
  - `Caido` cuando cae un jugador; el anfitrión cierra con `Fin`.

La **huella de los datos** es un hash de `data/*.json` y de la versión del `regulation.bin`. Si no
coincide, no se entra: cada juego genera la run por su cuenta, y cualquier diferencia la desvía.

### Reconexión

- El estado común es pequeño: semilla, acto, nodo, decisiones y `rng_comun`.
- El personal (bendiciones, arma, runas, frascos) lo guarda cada uno en su `runtime/coop.json`.
- Si alguien se cae, el resto sigue. Al volver, el anfitrión le manda la `Foto` y entra en el
  siguiente nodo. Es el `ClientRejoinRequest` de StS2 en pequeño.

## El combate: los caminos

### Lo que sabemos de la red del juego

- **SABIDO** (SDK `eldenring` 0.14, `net_chr_sync.rs` y `world_chr_man.rs`):
  - El juego tiene un búfer de sincronización por cada grupo de personajes (`NetChrSync`, 196
    huecos).
  - Llegan posición, rotación y vida (`ChrSyncHealthUpdate`: vida actual y daño recibido).
  - Cada personaje se identifica por su grupo y su número (`FieldInsHandle`).
  - Cada entrada dice si su IA corre aquí o la manda otro (`ChrUpdateType::Local` / `Remote`).
- **SABIDO:** los creados con `spawn_chr` van al grupo de depuración (`debug_chr_set`), no a un mapa.
- **SABIDO:**
  - El enemigo se simula en el anfitrión (lo dice er-crit-coop, hecho sobre el mismo SDK).
  - El anfitrión envía los eventos EMEVD a los invitados (paquete 104, `disable_emk_sync`).
  - Los enemigos colocados en el MSB y encendidos por EMEVD son enemigos normales del juego, y se
    sincronizan.
- **SABIDO (fase 0):** el grupo de depuración no tiene búfer de sincronización. Los mapas, las
  invocaciones y el mundo abierto sí.

### Camino A: Seamless debajo (sin probar con una versión que arranque)

- **SABIDO:** nuestro me3 sabe arrancar Seamless.
  - `tools/vendor/me3/crates/mod-host/src/host.rs:107` llama a `modengine_ext_init`, que es como
    arranca `ersc.dll`.
  - El cambio oficial de me3 0.5.0 dice «Support Seamless Coop».
  - Basta un `[[natives]]` más en el perfil.
- **SABIDO:** con el Seamless que tienes instalado (de abril), el juego se queda parado al arrancar.
  Ver «Resultados de la fase 0». **PROBAR** con la versión actual (2.0.1).
- **Cómo se juega:**
  - Cada uno instala Seamless, que no es nuestro y no se reparte con el mod.
  - Entran en la sesión con los objetos de Seamless y abren la sala de RogueRing, que ya trae a los
    de la sesión.
  - El anfitrión empieza; todos viajan al coliseo del anfitrión.
- **El jefe espejo: DESCARTADO** en la fase 0.
  - La idea era que todos crearan el mismo jefe en el mismo hueco y que en los invitados lo moviera la
    red del juego.
  - Pero el grupo de depuración no tiene búfer de red, y cambiar `Local`/`Remote` no para su IA.
- **La vía que queda:** jefes colocados en el MSB del coliseo, apagados y encendidos por EMEVD.
  - Los grupos de los mapas sí tienen búfer de red (fase 0), y son los enemigos normales del juego.
  - No hace falta meterlos todos: basta una variante del coliseo por combate, cambiada antes de cada
    viaje. Probado en local; ver «Un coliseo por combate».
- **Idea sin estudiar:** las invocaciones (cenizas y Torrent) también tienen búfer de red.
  - Un jefe hostil creado como «invocación» se sincronizaría sin tocar el MSB.
  - Necesita `BuddyParam` y el mecanismo de invocación del juego. **PROBAR** si un `teamType` enemigo
    funciona ahí.
- **Riesgos:**
  - Seamless tiene sus propios modos de horda y de jefes en los coliseos, que pueden chocar con el
    nuestro. **PROBAR.**
  - Sus reglas de muerte y de reinicio del mundo al descansar en una gracia.
  - Su escalado, que hay que sumar al nuestro sin doblarlo.
  - Todos necesitan la misma versión de Seamless y la misma contraseña en su INI. Eso lo ponen ellos;
    nosotros no leemos ni copiamos ese archivo.
  - Si Seamless cambia, nos puede romper.

### Camino B: en paralelo (sin mundo compartido)

- Cada uno lucha contra su copia del mismo jefe, en su juego y a la vez.
- La pantalla enseña la barra del jefe del compañero, su vida y si ha caído. El nodo se gana cuando
  todos acaban.
- **SABIDO:** funciona con lo que hay; era lo que hacía el cooperativo de septiembre. No necesita
  Seamless.
- No es «cooperativo real» (en septiembre lo descartaste como objetivo final), pero sirve para tres
  cosas:
  - es el primer modo jugable;
  - con él se prueban la sala, la run compartida y la reconexión con tu amigo;
  - es lo que queda si el camino A no sale.

### Camino C: red propia, sin Seamless (no ahora)

- Sería pintar al compañero como un personaje creado por nosotros y mover el jefe del invitado con lo
  que envía el anfitrión: posición, animación y vida.
- Cada golpe se resuelve en el juego del que lo recibe, como hace FromSoft.
- Es lo que hizo el autor de Seamless con Sekiro Online, y es un proyecto enorme.
- Problemas: agarres, contraataques, proyectiles que persiguen y cambios de fase. Son meses de trabajo
  y el resultado no está garantizado.
- Solo tendría sentido si A falla y quieres no depender de Seamless.

### Descartado: montar la sesión del juego nosotros

- Es lo que intenta unseamless-coop (Rust, sobre el mismo SDK, licencia MIT).
- Ya empareja jugadores y les abre un canal, pero según su propio README la capa que mete a los
  jugadores en el mundo del otro sigue «en ingeniería inversa».
- Conviene vigilarlo: si lo consigue, sería un camino A sin código cerrado.

## Las bendiciones en grupo (con el camino A)

- **Vida del jefe:** solo la toca el anfitrión. Hoy el motor la escribe con `set_chr_hp` (Festín,
  Espinas, quemaduras, marca, remates por debajo del 5 %).
  - En el invitado ese daño se envía como `Danio` y lo aplica el anfitrión.
  - La vida nueva llega a todos por la red del juego.
- **Lo que sale de golpear:** proyectiles visuales, estados, runas y frascos ya funcionan sobre el
  jugador local, así que cada uno tiene los suyos.
  - **PROBAR** que los proyectiles visuales de un invitado no se dupliquen en el anfitrión.
- **Lo que depende de que el jefe te pegue** (Golpetón, parrys): local, de cada uno. Hay que mirar
  qué pasa cuando el jefe persigue a otro.
- **Barra del jefe con estados y marcas:** cada uno ve la acumulación que lee en su juego.
  **PROBAR** si la del jefe remoto llega.
- **Escalado:** el juego y Seamless ya suben la vida de los enemigos con más jugadores. Nuestro
  escalado por acto se aplica encima una sola vez, en el anfitrión.

## Pruebas, por orden

0. **En este PC, sin segundo jugador** (1 sesión):
   - Perfil de me3 de prueba con `ersc.dll` (de tu carpeta de Seamless, sin tocar su INI) y nuestra
     DLL. ¿Arrancan juntos?
   - ¿Existe `NetChrSync.net_chr_set_sync[<grupo de depuración>]`? Si no existe, el espejo queda
     descartado sin gastar la tarde con tu amigo.
   - Memoria del coliseo con 5, 10 y 20 jefes en el MSB, para saber si el plan de reserva cabe.
1. **Sala y run compartida con la «sombra»** (compañero simulado en el mismo juego, como en
   septiembre). Todo el protocolo, sin red.
2. **Dos PCs, una tarde con tu amigo:** canal, sala, run en paralelo (camino B), votos y reconexión.
3. **Misma tarde, con Seamless actualizado en los dos PCs:** un jefe del MSB encendido por EMEVD.
   ¿Lo ven todos? ¿Baja su vida con los golpes de cualquiera?

## Resultados de la fase 0 (15-10-2026, tarde)

Orden nueva del banco: `red [entidad]` y `red control <entidad> <local|remoto>` (`banco.rs`). Datos en
el juego, un PC, sin conexión:

- **SABIDO, búferes de red:** el juego reserva búfer de sincronización (`NetChrSync`) solo para cuatro
  grupos:
  - 0 y 1, los mapas cargados (el coliseo);
  - 113, invocaciones (capacidad 80);
  - 115, mundo abierto (capacidad 1500).

  El grupo de depuración (114, capacidad 76), donde caen todos los jefes de `spawn_chr`, **no tiene**.
  Tampoco lo tienen los fantasmas (112).
- **SABIDO, identificador:** el jefe de un combate real es `FieldIns(m255_255_255_255, 114, 0)`, es
  decir, grupo 114 y entrada 0.
- **SABIDO, control:** el jefe de un combate real ya sale como `Remote` y `ReadyForActivation`, y aun
  así pelea normal: anda unos 3 m cada 3 s detrás del jugador.
  - Pasarlo a `Local` y volver a `Remote` no cambia nada.
  - En este grupo esos campos no mandan, o el SDK los nombra mal. No sirven para convertir un jefe en
    marioneta.
- **SABIDO, varios jefes a la vez:**
  - 15 jefes de modelos distintos más el del combate, a la vez en el coliseo: todos cargados, sin
    caídas y con el banco respondiendo.
  - RAM del proceso: 5654 MB → 5664 MB. VRAM: 3464 MB → 3516 MB.
  - El juego reserva su memoria al arrancar (17 GB de memoria privada), así que esto no dice cuántos
    modelos cabrían en un MSB.
- **SABIDO, Seamless con me3:** con tu `ersc.dll` (carpeta de abril, sin leer ni tocar su INI) en un
  perfil de prueba (`runtime/fase0-seamless.me3`):
  - me3 lo inyecta y `ersc.dll` aparece cargada en el proceso.
  - Después el juego se queda parado: 0 s de CPU, sin ventana y sin llegar a cargar nuestra DLL.
  - Con `load_early = true` se para aún antes.
  - Lo más probable es que esa versión no vaya con el juego actual (Codex ya vio en septiembre que
    pedía actualizarse). **PROBAR** con Seamless 2.0.1; actualizarlo es cosa tuya.
- **Partidas:** se copiaron antes en `runtime/fase0_saves_*`. Después se comprobaron sus huellas:
  ER0000.sl2, ER0000.co2, ER0000.co2.bak, ER0000.cnv.co2 y RogueRing.sl2 siguen iguales. La prueba
  usó `RogueRingPrueba.sl2` y una copia nueva, `RogueRingPrueba.co2`.

**Qué cambia:**
- El combate en el mismo mundo ya no puede salir barato. Quedan dos opciones:
  - jefes en el MSB, con Seamless actualizado y una prueba de cuánto cabe;
  - la idea de las invocaciones, sin estudiar.
- La sala y el combate en paralelo (fases 1 y 2) no dependen de nada de esto.
- Lo de «cuánto cabe» quedó resuelto después: no hace falta meter todos los jefes. Ver la sección
  siguiente.

## Otros mods que hacen algo parecido (15-10, noche)

- **Enemy Randomizer** de thefifthmatt (código abierto, `SoulsRandomizers`):
  - Mueve jefes de cualquier mapa a cualquier otro editando los MSB, con sus modelos.
  - Según su README es «en parte compatible» con Seamless: todos usan los mismos archivos y los
    enemigos se ven.
  - Su modo de jefes multiplicados («Enemy Onslaught») da problemas: si alguien muere, todos tienen
    que salir del mapa antes de reintentar. A nosotros nos da igual, porque al acabar cada combate ya
    salimos del coliseo.
  - **SABIDO:** los jefes colocados en el MSB, incluso de otros mapas, funcionan en Seamless.
- **Seamless Co-op**, tu versión es la 1.9.8 (lo dice dentro de `ersc.dll`):
  - Trae sus propios modos de horda y de jefes (`OPTIONSELECT_BOSSRUSH`, `BOSSRUSHDLC`,
    `*ENEMYRUSH`).
  - Crea enemigos sobre la marcha con un generador propio (`ERSCRuntimeEnemyGenerator`,
    `EnemyGeneratorCtrl`).
  - También tiene un sistema propio de invocaciones (`seamless buddy system`).
  - Es decir, el autor de Seamless consiguió enemigos creados en marcha y sincronizados. Lo hizo con el
    sistema de generadores de enemigos del juego, no con el de depuración.
  - El código va protegido (WinLicense), así que copiar su método exige ingeniería inversa seria. Con
    lo de abajo no hace falta.
- **Mods de invocar jefes como cenizas** (DLC Summon Bosses, Nexus 5264; NPCs and Bosses Spirit
  Summons):
  - Demuestran que cualquier jefe sirve como invocación (`BuddyParam`).
  - La idea de un jefe hostil como invocación sigue sin probar. Ya no hace falta.
- **unseamless-coop / er-crit-coop** (Rust, MIT): siguen sin meter a dos jugadores en el mismo mundo.

## Un coliseo por combate (probado en local, 15-10, noche)

**La idea:**
- ModBuilder genera una variante del MSB del coliseo por cada encuentro, con el jefe ya colocado.
- Antes de viajar al coliseo, la DLL copia la variante del encuentro sobre
  `mod/map/mapstudio/m45_00_00_00.msb.dcx`.
- Todos los juegos de la sala conocen el encuentro por la semilla compartida, así que todos cargan la
  misma variante, con el mismo jefe en el mismo hueco.
- El jefe es un enemigo normal del mapa: lo mueve el anfitrión y la vida la sincroniza el juego, como
  con el Randomizer.

**Probado** con `ModBuilder <raíz> coliseo_con <chr> <npc> <think> <entidad> <salida>` y la orden `red`:
- **SABIDO:** un Caballero del Crisol colocado en `(0, 0, 5)` con la entidad 45000990 sale como
  `FieldIns(m45_00_00_00, 107, 1)`, en el grupo 107. Ese grupo tiene búfer de red: con el coliseo
  cargado, los grupos con red son 107, 113 y 115.
- **SABIDO:** el jefe está vivo y activo: tiene la vida llena (1680) y va a por el jugador.
- **SABIDO:** se cambió el archivo por la variante del Oso rúnico, se salió del coliseo y se volvió a
  entrar. Salió el Oso rúnico (NPC 46300912, vida 2585) en el mismo hueco. El juego, con me3, lee el
  MSB del disco en cada visita.
- **SABIDO:** cada variante pesa 15 KB, así que 150 encuentros son unos 2 MB.
- El jefe de `spawn_chr` del director también salió a la vez. En la versión de verdad sobraría.

**Hecho después (15-10, noche; commits a8560b6 y 7ba56b6):**
- Todos los encuentros del coliseo tienen un solo enemigo: basta un hueco por variante. Son 135
  variantes y unos 2,2 MB.
- El jefe espera apagado: hay un evento en el MSB del coliseo y la DLL enciende el flag 1049300110 al
  empezar el combate.
  - **SABIDO:** antes del flag no se mueve; después va a por el jugador.
  - La línea que lo arranca va al principio del evento 0 del mapa: el evento salta según el tipo de
    partida de arena (`GOTO IF Arena Match Type`) y al final no se llegaba nunca.
- El director usa ya el jefe del MSB, también jugando solo. `spawn_chr` queda de reserva.
  - **SABIDO:** la prueba de humo pasa con los tres personajes.
  - `tools/validar_msb.py` comprueba los 132 jefes uno a uno.

## Seamless 2.0.1 con el mod (15-10, noche)

- **SABIDO, el cuelgue era otra cosa.** Seamless arranca bien con me3, sin parches ni opciones. Sin
  contraseña en su INI, enseña un diálogo de error fatal («Please enter a session password») y se queda
  esperando. No lo veíamos porque la lista de ventanas desde PowerShell no devolvía nada.
- **SABIDO, y peligroso: Seamless no usa la partida de me3.** Siempre carga `ER0000.<save_file_extension>`.
  - Con `co2`, la de siempre, el mod cargó la partida real de Seamless del jugador y la modificó.
  - Se restauró desde la copia de las 16:05, idéntica a la de `_codex_work/Respaldo Coop`.
  - La nube de Steam solo sincroniza `ER0000.sl2`, que no se tocó.
- **Arreglo:**
  - La DLL lee solo la clave `save_file_extension` del INI que va junto a `ersc.dll`.
  - Con `co2` o `sl2`, el mod no se activa.
  - Con otra (`rogue`), crea `ER0000.rogue` desde `RogueRing.sl2` o desde la plantilla, y juega en ella.
  - El lanzador `Play RogueRing (Seamless).bat` no arranca sin contraseña ni con `co2`.
  - **SABIDO:** se probaron los dos casos. Con `co2`, nada se toca. Con `rogue`, el combate va y las
    partidas reales siguen iguales.
- **Menú Cooperativo:**
  - Con Seamless cargado, deja las manos libres. START abre el menú del juego para usar la Tiny Great
    Pot (abrir el mundo) o la Effigy of Malenia (unirse). SELECT vuelve al menú del mod.
  - En una sesión con compañeros, la run nueva usa una semilla que sale del SteamID del anfitrión, igual
    para todos.
  - Con Seamless, el registro apunta el jefe (`FieldIns`), la sesión y la vida del jefe cada 5 s, para
    comparar los registros de los dos juegos.
- **SABIDO, en local con Seamless:**
  - Partida aparte, jefe colocado en el grupo 107, combate, barra, victoria.
  - La Tiny Great Pot está en las herramientas del personaje del mod.
- **Paquete de prueba:** `dist/RogueRing-0.4.0-coop.zip`. Las instrucciones van en LEEME.txt, sección
  COOPERATIVO.
- **PROBAR, con dos PCs:**
  - ¿Ve el invitado el jefe y baja su vida con los golpes de cualquiera?
  - ¿Qué pasa si cae uno, o si uno llega antes?
  - ¿Expulsa Seamless del coliseo? Tiene un aviso «The power of grace expelled you from the arena».

## Plan

| Fase | Qué | Sesiones |
|---|---|---|
| 0 | Prueba local: HECHA, el espejo queda descartado | 1 |
| 1 | Sala: pantallas Crear, Unirse y Sala; protocolo; presencia de Steam; sombra | 2 |
| 2 | Run compartida: votos, esperas, recompensas por jugador, huellas, reconexión; combate en paralelo | 2 |
| 3 | Tarde con dos PCs (B completo y prueba de A) | 1 tarde |
| 4 | Mismo mundo: un coliseo por combate + Seamless: HECHO en local, falta la prueba con dos PCs | 3 a 5 |
| 5 | Bendiciones en grupo, escalado, muerte de grupo | 2 |

Las fases 1 y 2 valen para cualquier camino. La fase 4 depende de la 3.

## Fuentes

- Decomp de StS2: `StartRunLobby.cs`, `SteamHost.cs`, `SteamClient.cs`,
  `MapSelectionSynchronizer.cs`, `EventSynchronizer.cs`, `RewardSynchronizer`, `ChecksumTracker`.
- Cooperativo de septiembre: commits 4970690 (estudio), abb36da (código) y b0111a0 (retirada);
  `_codex_work/Prueba cooperativa en un PC.md` y `Sonda cooperativa local.ps1`.
- SDK `eldenring` 0.14: `net_chr_sync.rs`, `world_chr_man.rs`, `session_manager.rs`,
  `network_session.rs`.
- me3: `crates/mod-host/src/host.rs` y PR #165 (0.5.0, «Support Seamless Coop»):
  https://github.com/garyttierney/me3/pull/165
- unseamless-coop: https://github.com/micthiesen/unseamless-coop
- er-crit-coop: https://github.com/micthiesen/er-crit-coop
- Seamless Co-op, cambios y modos de horda y jefes: https://www.nexusmods.com/eldenring/mods/510?tab=logs
- Steam, `ISteamNetworkingMessages`: https://partner.steamgames.com/doc/api/ISteamNetworkingMessages
- Steam, salas: https://partner.steamgames.com/doc/features/multiplayer/matchmaking

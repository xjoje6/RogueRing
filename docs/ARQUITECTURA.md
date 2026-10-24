# Arquitectura

El combate es Elden Ring tal cual. Todo lo demás (menú, personajes, mapa, recompensas, tienda, eventos,
gracias y pausa) son pantallas propias que dibuja una DLL encima del juego.

## Piezas

- **`dll/core`**: la lógica sin el juego. Mapa (port del `StandardActMap` de Slay the Spire 2: siete
  columnas, caminos sin cruces, colas barajadas de encuentros por tipo), RNG con nombre por propósito,
  estado y guardado de la run, recompensas, bendiciones, eventos, personajes y perfil. Todo con pruebas
  (`cargo test`).
- **`dll/ui`**: las pantallas en imgui. Se pueden ver sin el juego con los ejemplos `preview_*`, que
  dibujan con un rasterizador por CPU (`soft.rs`).
- **`dll/harness`**: la DLL que carga me3.
  - `director/`: la run dentro del juego, por fases (`Phase`): qué pantalla se ve, viajes, combates,
    recompensas. `tick.rs` es lo que pasa en cada fotograma.
  - `motor.rs`: interpreta el lenguaje de efectos de las bendiciones (`core/src/efectos.rs`) en cada
    fotograma del combate.
  - `juego.rs` y `equip.rs`: lo que se lee y escribe en la memoria del juego (personajes, efectos,
    proyectiles, postura, inventario y equipo).
  - `globales.rs`: reglas del modo en los params en memoria (peso 0, escalado neutro, viales que curan
    entero). Nunca se guardan, así que el juego sin el mod no las nota.
  - `overlay.rs`: hudhook (DX12) e imgui; carga las imágenes y dibuja `dll/ui`.
  - `arranque.rs`: tapa el título y entra solo en la partida del mod. `plantilla.rs` instala la
    partida plantilla a quien no la tenga. `sonido.rs` silencia el juego mientras la pantalla está
    tapada.
  - `banco.rs` y `pad.rs`: banco de pruebas (solo con la característica `banco`).
- **`tools/ModBuilder`**: genera `mod/` desde los ficheros del juego: eventos de viaje (la DLL enciende
  un flag y un evento hace el warp), animaciones del Monk y del Samurai, `c0000.hks` (el de la
  comunidad más `tools/hks/rogue.hks`), un SpEffect por nivel de cada bendición y el punto de aparición
  del coliseo.
- **`data/`**: todo el contenido en JSON, editado a mano. Las pruebas de `dll/core/tests/datos.rs`
  comprueban que unos ficheros cuadran con otros.

## Idioma

El mod habla el idioma del juego: la DLL lo lee del manifiesto de Steam del juego
(`steamapps/appmanifest_1245620.acf`, «language»: `spanish` y `latam` son español) y
`runtime/idioma.txt` (`es` o `en`) manda sobre él. El código y los datos están en inglés; las
traducciones viven en `data/i18n/<idioma>.json`:

- `ui`: textos de pantalla por su texto en inglés. En el código pasan por `rogue_core::idioma::t`
  (fijos) o `tf` (con huecos `{}` en orden); las ayudas de botones y los tipos de arma, por `td`
  al enseñarlos (el tipo de arma es también un dato: los estilos de cada personaje).
- `bendiciones`, `eventos`, `personajes`, `mutadores`: por id; se aplican al cargar los datos
  (`director::build_director`), así el resto del código no cambia.
- `objetos` y `enemigos`: los nombres los pone el juego (`extract/fmg`); `tools/i18n.py` los
  rellena, recoge del código los textos de pantalla nuevos y avisa de lo que falta.

`dll/core/tests/datos.rs` (traduccion_completa) falla si falta algo y `dll/ui/tests/textos_carta.rs`
mide que las bendiciones en español también caben en su carta.

## Combates

Casi todos se pelean en el Coliseo de Limgrave, con un coliseo por combate:
- ModBuilder genera una variante del MSB del coliseo por encuentro (`mod/map/rogue_coliseo/<id>.msb.dcx`)
  con el jefe ya colocado, con la entidad 45000990.
- Antes de cada viaje, el director copia la variante sobre `mod/map/mapstudio/m45_00_00_00.msb.dcx`. El
  juego relee el MSB en cada visita.
- El evento 79609000 del mapa (al principio del evento 0 de m45, que salta según el tipo de partida de
  arena) tiene al jefe apagado hasta que la DLL enciende el flag 1049300110.
- Así el jefe es un enemigo del mapa (`FieldIns(m45_00_00_00, 107, 1)`), que el juego sincroniza en
  cooperativo. Los de `spawn_chr` van a un grupo sin red (ver docs/COOP_SALAS.md).
- Sin variante, o con `runtime/jefe_spawn`, vuelve el coliseo normal (`base.msb.dcx`) y el jefe se crea
  con `spawn_chr` como antes. `tools/validar_msb.py` comprueba todos los jefes.

A cada jefe le pone el tramo de escalado del acto (SpEffect 7000-7200, `data/escalado.json`) y le
multiplica la vida. Unos pocos se pelean en su sitio del mundo (`data/arenas_jefes.json`): se viaja
con un flag, se resucita al jefe apagando su flag de derrota y se gana cuando el juego lo enciende.

## Banco de pruebas

Con la característica `banco` (la compilación por defecto) la DLL lee órdenes de `runtime/harness/cmd.txt`
y responde en `out.txt` (`python tools/banco.py <orden>`). Simula un mando XInput siempre conectado y el
foco de la ventana, así que el juego se puede probar en un monitor virtual (`tools/lanzar.ps1`,
`dll/pantalla`) sin tocar el ratón ni el teclado de quien use el PC. La versión de jugador
(`--no-default-features`) no lleva nada de esto; `tools/empaquetar.py` lo comprueba en el binario.

Las pruebas pisan `runtime/run.json` y `runtime/perfil.json`: hay que copiarlos antes y devolverlos.
Para no tocar la partida del mod, `lanzar.ps1 -Savefile <otra>.sl2`. `tools/medir_balance.py` mide
encuentros en el coliseo (vida tras el escalado, golpes recibidos y dados) con la orden `medir`.

## Lo que costó descubrir

- **Proyectiles.** La réplica de `BulletSpawnData` (`juego.rs`) tiene que ser `#[repr(C, align(16))]`:
  el juego la lee con instrucciones alineadas y, sin la alineación, cualquier proyectil cierra el juego
  según dónde caiga en la pila. Hay comprobaciones al compilar.
- **imgui.** Solo puede haber una `DrawListMut` de primer plano viva a la vez. Dos son un pánico, y un
  pánico dentro del juego lo cierra (0xc0000409); el gancho de pánico lo deja en `harness.log`.
- **Postura de los enemigos.** Es el super armor (`CSChrSuperArmorModule`); su toughness está a 0. Solo
  se rompe cuando un golpe la hace cruzar de positiva a 0 (dejarla en 0 justo no rompe nunca) y el
  juego la rellena al pasar `recover_time` sin daño de postura. La rotura del desvío se hace con un
  proyectil sin efectos (AtkParam_Pc 79600010, `subCategory1 = 100`, como un ataque cargado), que es lo
  que deja el crítico.
- **Mancha de runas.** El juego vuelve a poner `has_bloodstain` al cargar la zona: además de quitarla,
  se deja sin runas y en el área 99, que no existe (`BlockId::none()` significa «en todos los mapas»).
- **HKS.** Los números son float32: los ids de SpEffect por encima de 16 777 216 llegan redondeados y
  `env(GetSpEffectID)` no los encuentra. Las marcas para el comportamiento del jugador van en 7960000+.
- **Estados en los enemigos.** Un estado (sangrado, podredumbre...) solo se acumula a través del
  `spEffectId` de un AtkParam_Npc; aplicar el efecto directamente no acumula.
- **Equipo.** Quitar un objeto del inventario no lo desequipa. Los handles de gaitem cambian al cargar
  partida: el inventario se compara por id y ceniza. Al cambiar atributos el juego recalcula la vida al
  momento.
- **Posiciones.** `chr_pos` y `player_pos` usan la posición física, que comparten los personajes del
  mismo bloque; para el MSB, `PlayerIns.block_position`.
- **Mapas.** Un objeto del MSB copiado de otra sala no se dibuja (grupos de dibujado): la plantilla
  tiene que ser de la misma sala. Los evergaoles no sirven con warp directo: el combate no se arma.
- **EMEVD en C#.** `new Instruction(bank, id, params object[])` confunde el primer argumento con
  `layerMask`: usar siempre la sobrecarga con `IEnumerable<object>` (`Ins` en ModBuilder).
- **SoulsFormats** busca `oo2core_6_win64.dll` en el directorio actual: las herramientas trabajan
  desde su carpeta y la DLL se copia ahí desde la del juego.
- **Pruebas en segundo plano.** `GraphicsConfig.xml` es UTF-16 LE sin BOM (mal escrito, el juego lo
  resetea) y es global: `lanzar.ps1` lo copia y `cerrar.ps1` lo devuelve. A pantalla completa el juego
  se minimiza sin foco: siempre en ventana. En el título, nunca pulsar START en automático.
- **Interfaz del juego** (`CSMenuManImp.ui_states`): 5, 7 y 8 son el HUD; 9 la barra de jefe; 11 sale
  tras un combate; 4, 6 y 18, pantalla de carga; 23 y 28, el menú del juego.
- **Silencio.** Se baja el volumen de la sesión de audio del propio juego en Windows. Windows lo
  recuerda: el volumen original se apunta en `runtime/volumen.txt` mientras está bajado y se devuelve
  al arrancar si quedó ahí.
- **me3** copia `ER0000.sl2` a la partida del mod la primera vez; la DLL pone antes la plantilla para
  que nadie empiece con su personaje de siempre.

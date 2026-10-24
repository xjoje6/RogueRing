# Cooperativo: protocolo nuevo (19-10)

Rehacer el cooperativo sobre tres reglas, porque cada arreglo suelto abría otro fallo:

1. **Manda el anfitrión.** Lo común lo decide un solo juego. El invitado no saca conclusiones de sus
   copias de los rivales: obedece al anfitrión y, si pierde un mensaje, se pone donde está él.
2. **La muerte la hace Seamless.** Nada de invisible, invencible ni recolocar al caído.
3. **El protocolo se prueba sin el juego.** Las decisiones viven en `rogue_core` como código puro y
   cada fallo visto en partida es una prueba automática.

## Por qué fallaba

- Cada juego decidía solo con lo que veía. Las copias de los rivales del invitado se descargan al
  morir, el escalado unas veces se queda y otras no, y así salían victorias falsas, golpes que matan
  de una vez y cada jugador en un sitio.
- La muerte era nuestra: al morir, el juego te reaparece en tu última gracia (otro mapa), y cada parche
  (invisible, invencible, punto de reaparición, bloquear el mando, recolocar) chocaba con otra cosa.
- Solo se probaba jugando los dos.

## Lo que ya funciona y se queda

- La sesión (Seamless), el canal de mensajes (Steam P2P, `harness/src/coop.rs`) y el estado que
  cada juego publica varias veces por segundo.
- La misma run en todos desde la semilla del anfitrión; recompensas personales (semilla con el
  SteamID).
- Votar nodo y opción de evento: cada juego decide igual con los mismos votos (`decidir`, `sortear`).
  No ha dado fallos.
- Esperar a todos antes de votar el mapa y antes de un combate de evento.

## Quién decide qué

| Qué | Quién | Cómo lo sabe el otro |
|---|---|---|
| Nodo y opción de evento | todos, por votos (igual que ahora) | votos en el estado |
| Ir al combate | barrera: cuando todos están listos | `listo` en el estado |
| Encender cada oleada | anfitrión | `oleada` en su estado |
| Victoria o derrota | anfitrión | `resultado` en su estado (paso y cuál) |
| Fin de la run | anfitrión | `resultado = derrota` |
| Recompensas, bendiciones, tienda, equipo | cada uno | (no viaja) |
| Daño de las bendiciones de un invitado | lo aplica el anfitrión (igual que ahora) | suceso `Danio` |

El anfitrión puede decidir aunque esté muerto: sus rivales siguen en su mundo mientras mira el
combate.

## Morir

- `always_spectate_on_death = 1` en el `ersc_settings.ini` (empaquetar.py y seamless.ps1). Seamless
  hace que el muerto mire a los compañeros, con su cámara, hasta que cae todo el grupo o alguien
  descansa en una gracia.
- Cada juego publica `muerto` en su estado. El motor de bendiciones del muerto se para (no lanza
  nada).
- Todos muertos: el anfitrión publica derrota y la run se acaba en todos.
- El anfitrión gana con alguien muerto: victoria para todos. **Por probar:** cómo vuelve el muerto.
  Puede que Seamless lo reaparezca con el viaje del anfitrión al centro. Si no, el anfitrión
  «descansa» al volver (lo que pide Seamless para reaparecer a los muertos), y como último recurso el
  muerto reaparece en el centro al recibir la victoria.
- Fuera lo de antes: `caido`, el espectador propio (efecto y recolocar), `en_arena`,
  `companero_gano` y bloquear el mando. Se queda el punto de reaparición en la llegada de la arena
  (si caen todos, Seamless reaparece al grupo ahí y no en la última gracia de cada uno).

## Hecho (19-10)

- `rogue_core::protocolo` con 11 pruebas (las de abajo, salvo cofre/evento, que ya esperan a todos
  con `listo`).
- Conectado al director: el invitado cierra el combate con el resultado del anfitrión (también si
  sigue en la carga) y enciende las oleadas cuando él; el anfitrión gana con la lógica de siempre y
  pierde cuando el protocolo ve a todos muertos. Morir en cooperativo ya no acaba nada por sí solo.
- Muerto: el motor de bendiciones no hace nada. `always_spectate_on_death = 1` en el paquete y en
  seamless.ps1. El registro apunta la vida al volver de un combate ganado (para ver cómo vuelve el
  muerto).

## Seguir al anfitrión

Si el estado del invitado no cuadra con el del anfitrión durante unos segundos (otro paso, o el
anfitrión ya ha cerrado el combate en el que sigue el invitado), el invitado se pone donde está el
anfitrión: cierra el combate con el resultado del anfitrión y sigue. Así un mensaje perdido o un
atasco no deja a nadie colgado.

Tiempos límite (todos en el protocolo, con pruebas):

- Combate cerrado por el anfitrión y el invitado sigue en él: se cierra a los 3 s.
- Invitado sin llegar a la arena y el anfitrión ya peleando o fuera: se pone donde está él.
- Sin noticias del anfitrión 10 s: se le da por desconectado y la run sigue en solitario.

## Pruebas (sin el juego)

Cada una simula dos juegos con mensajes que llegan tarde o se pierden:

1. El invitado muere y el anfitrión gana: victoria para los dos.
2. El anfitrión muere y el invitado gana: victoria para los dos.
3. Mueren los dos: derrota para los dos.
4. Alguien muere justo después de ganar: sigue siendo victoria.
5. El invitado se queda en la carga: el anfitrión no gana por eso; el invitado se pone en su sitio.
6. El anfitrión se queda en la carga: el combate no empieza para nadie hasta que llega.
7. Se pierden mensajes: todos acaban en el mismo paso.
8. El anfitrión se desconecta a mitad de combate: el invitado sigue en solitario.
9. Cofre trampa y combate de evento: nadie se va solo.
10. Oleadas: el invitado enciende cada oleada cuando lo dice el anfitrión, no antes.

## Orden de trabajo

1. Este documento (revisarlo con el jugador).
2. `rogue_core::coop::protocolo`: la máquina de estados y las pruebas de arriba.
3. Conectarla al director, quitando lo viejo, y `always_spectate_on_death = 1`.
4. Una sesión de prueba con dos PCs, mirando sobre todo cómo vuelve el muerto tras una victoria.

## Prueba del 19-10 (0.5.42)

- Bien: el protocolo cerró todas las peleas en los dos juegos; el muerto mira con la cámara de
  Seamless; el invitado muerto volvió con la vida llena al ir al centro (Seamless lo reaparece al ser
  arrastrado por el viaje del anfitrión).
- Mal: el anfitrión muerto viajó al centro y siguió muerto (vida 0); en el combate siguiente nunca
  «llegó» (la llegada pide vida) y se quedó en la carga, con el invitado dentro viendo su cuerpo.
- Seamless: tras reaparecer, la cámara del invitado seguía centrada en el anfitrión (fallo conocido
  de Seamless normal).
- Arreglo (0.5.43), por capas y apuntado en el registro: al ganar, si sigue muerto, recuperación de
  gracia (evento 79609900, Bonfire-like Recovery, flag 1049300090); al volver al centro y antes del
  combate siguiente, vida al máximo a mano.

## Prueba del 19-10 (0.5.43)

- Bien: los dos reaparecen tras morir con el combate ganado por el otro (al anfitrión le hizo falta la
  capa de vida al máximo al volver; la recuperación de gracia sola no bastó).
- Mal: el anfitrión murió en la oleada 2 de la Horda y el invitado mató la 3, pero no se acabó. El
  anfitrión muerto no pasaba por la parte del combate que prepara cada oleada nueva (`oleada_lista`),
  así que nunca la veía vacía. Arreglo (0.5.44): muerto en cooperativo, el combate sigue igual en su
  juego; solo el motor de bendiciones se para.
- Cámara pegada al compañero tras reaparecer (las dos cruzadas). Es fallo de Seamless: su FAQ lo
  tiene con los evergaols («la cámara se queda en el otro jugador», sin arreglo). La cámara del
  personaje (`ChrCam`) guarda a quién mira el muerto (`death_cam_target`). Vivo 2 s con ese objetivo
  todavía, o con la cámara de muerto (tipo 7), se suelta: sin objetivo, cámara detrás del personaje y
  el tipo que tenía vivo. El registro apunta cada cambio de la cámara («cámara: …»).

## Prueba del 19-10 (0.5.44)

- Bien: el combate se acaba aunque el anfitrión muera antes de la última oleada.
- Cámara: sigue pegada al compañero en los dos sentidos. El registro del invitado muerto no apuntó
  ningún cambio en la cámara de muerto del juego (tipo 0, sin objetivo): Seamless lo hace por otro lado.
- 0.5.45, dos pruebas a la vez y todo al registro: cada segundo se busca dónde está apuntado el
  compañero (su puntero o su handle) en la cámara del personaje, en el propio personaje y en
  WorldChrMan, y se apunta cada cambio («cámara: compañero en […]»); quien murió en el combate descansa
  en gracia al volver (sin viales; si el juego los recarga igual, se devuelven los de antes durante 10 s), que es lo que saca del modo espectador de Seamless; y si 5 s
  después sigue el compañero en un hueco nuevo de la cámara, se pone uno mismo en él.

## Para probar más rápido (0.5.46, 22-10)

- Registro compartido (solo versión de pruebas): cada juego manda sus líneas nuevas a los demás cada
  2 s por un canal de Steam propio (0x524C); llegan a `runtime/harness/companero_<SteamID>.log`. Se
  manda también el arranque guardado antes de la sesión.
- Pausa en combate (versión de pruebas): «Morir ya» y «Ganar el combate» (este, solo el anfitrión o
  en solitario: el combate lo cierra él).

## Prueba del 22-10 (0.5.47)

- Cámara pegada en los dos sentidos. El rastreo (ChrCam, PlayerIns, WorldChrMan) no encontró al
  compañero ni estando muerto: Seamless lo guarda en su propia memoria. El descanso de gracia al
  volver tampoco lo arregla (los viales sí se respetaron). Ambas cosas fuera.
- Fallo: durante las cargas la vida se lee None y contaba como muerto (descansos sin haber muerto).
- 0.5.48: vivo (vida leída y > 0), cada segundo una foto de la memoria escribible de ersc.dll; a los
  3 s de morir y a los 5 s de reaparecer se compara con la foto (registro «cámara: …»: qué cambió,
  qué cambió al morir y sigue igual, y qué es un compañero: su puntero, handle o SteamID). Al
  reaparecer, lo que ahora es un compañero y antes no vuelve a lo de antes.

## Caídos, como en Nightreign (0.5.49, 22-10)

el jugador (22-10): en Seamless normal la cámara se queda en el compañero cuando el modo espectador no acaba
por su camino (matar al jefe): caer todo el grupo, evergaols. Aquí acababa siempre de otra forma.
Decisión: nadie muere de verdad en el combate cooperativo y Seamless no entra nunca en espectador.

- `player_no_dead` del juego (WorldChrManDbgFlags, la misma que usa la bendición del golpe mortal) en
  cada fotograma de la arena en cooperativo: la vida se queda en 1.
- 1 de vida durante 0,4 s: caído (director/caido.rs). Intocable, al juego solo le llegan la cámara y
  la pausa (pad::SOLO_CAMARA), bendiciones paradas, `muerto` en el protocolo (todos caídos: derrota).
- Un compañero a 2,5 m que mantiene Y 3 s lo levanta (Suceso::Levantar, protocolo 6) con el 10 % de
  la vida y sus viales. Ganado el combate, los caídos se levantan solos.
- Animaciones de Nightreign (estado NearDeath) en huecos de evento del jugador (ModBuilder, bloque
  «Caído»): 60750←17440, 60760←17460, 60780←17470, 60790←17480, 60560←17302, 60800←17600,
  60810←17650, 60811←17660, 60530←20500. Cuál hace qué se mira con «Probar animación» (pausa de
  pruebas en combate) y se pone en data/caido.json.
- `always_spectate_on_death = 0`. Fuera el rastreo de la cámara en la memoria de Seamless.
- Invocaciones de un compañero: las mueve la IA del juego en que se crean, que sigue a su jugador. Si
  se alejan más de 9 m de su dueño, vuelven a su lado; cada 10 s el registro apunta sus distancias.

### Prueba del 22-10 (0.5.49) y 0.5.50

- Mal: el compañero murió de verdad (vida 0): `player_no_dead` es de depuración y no hace nada en el
  juego normal; «Morir ya» ponía 0. Encima, el mod le pedía las animaciones de caído y al ganar le
  subía la vida estando muerto: atascado en la animación de muerte y carteles de muerte repetidos.
- 0.5.50: efecto propio con noDead (SpEffect 79619051, permanente) mientras dura el combate; «Morir ya»
  en cooperativo pone 1 de vida; muerte de verdad = la lleva el juego, sin animaciones ni vida nuestras.
- A gatas como en Nightreign: el stick izquierdo se guarda antes de bloquearlo (pad::STICK_IZQ); con
  él, el caído gira hacia donde apunta (respecto a la cámara) y repite la animación de arrastrarse
  (60530←20500), que lleva el movimiento. `lado` en data/caido.json por si izquierda y derecha salen
  al revés.

### Banco del 22-10 (0.5.51): a cuatro patas, girando sobre su eje

el jugador: «una sola animación estática a cuatro patas que gire en el sitio, sin avanzar». Comprobado en el
banco (sombra de compañero, combate del coliseo):
- Las animaciones de NR van SIN sus eventos: con ellos, la 17600 mató al personaje con el efecto de no
  morir y el modo dios puestos (es la de caído a fantasma).
- Lo que es cada una: 17440-17480 (60750-60790), caídas largas de unos 3 s tambaleándose; 17302
  (60560), otra caída; 17600 (60800), tumbado boca abajo; 20500 (60530), gatear (a cuatro patas de 0,2
  a 0,6 s); 17650/17660 (60810/60811), levantarse desde el suelo (2,4 s).
- Caer = la de gatear parada a los 0,4 s (rodillas y manos, cabeza arriba); la postura parada aguanta
  igual. El stick izquierdo la gira hacia donde apunta respecto a la cámara (derecha, izquierda, arriba
  y abajo, bien con `lado` 1), sin moverse de sitio.
- Levantar (orden `run levantame`, lo que hace Y del compañero): 60810, 10 % de vida, viales intactos.
  Ganar (`run ganar`) con el caído en el suelo: se levanta solo.
- El efecto de no morir aguanta el golpe mortal (vida 1). El modo dios del banco (`dios 1`) cura la
  vida cada fotograma: apagarlo para probar caídos.
- Banco: `anim_jugador`, `vel_jugador`, `vel_chr`, `ocultar` (sin pantallas del mod) y la sombra de pie
  en mi combate aunque no haya votado.

## Prueba del 23-10 (0.5.51) y 0.5.52

- Bien: levantar con Y funciona en los dos sentidos.
- Mal: tres muertes de verdad. Una con «Morir ya» (1 de vida) y el motor quitando ese 1 en el mismo
  instante; dos por golpes del juego en partida en línea (el efecto noDead no los paró; en el banco
  solo se vio parar uno con la opción de depuración también puesta).
- 0.5.52: (1) `set_player_hp` no baja de 1 mientras dura el «no morir» del cooperativo
  (`VIDA_MINIMA_1`); (2) tarea en DmgMan_Post (justo después del daño del fotograma, antes de procesar
  la muerte): vida 0 → 1 (`suelo_de_vida`; registro «golpe mortal parado tras el daño»). Banco:
  `solo_suelo 1` apaga noDead y la opción de depuración para probar solo esto (sin probar aún).
- Barra para los dos: `Estado.levantando` (a quién y cuánto; protocolo 7); el caído ve «X te está
  levantando» con la barra.
- Caído ignorado: SpEffect 79619052 (targetPriority -1, sightSearchEnemyRate y hearingSearchEnemyRate
  0) mientras está en el suelo.
- Invocaciones del invitado en el juego del anfitrión: copia de su NpcThinkParam (+1000000000) con
  isBuddyAI 0 (ModBuilder: 101 copias), para que peleen por su cuenta; fuera la correa. Sin probar
  (banco: `run invocar_sombra` / `run invocar_mia`).
- Fuera la recuperación de gracia por flag al reaparecer (podía recargar los viales del compañero);
  aviso «viales: suben de X a Y» si suben fuera del combate.

## Daño de verdad del juego (0.5.54, 23-10)

el jugador: usar toda la lógica del juego en lo que ya existe (proyectiles, daño). Medido en el banco:
- Un proyectil del jugador creado por el mod solo hace el daño fijo de su ataque (AtkParam_Pc) si sale
  con un hechizo como origen (BulletSpawnData.magic_id; se usa 4300). El porcentaje (atkXCorrection)
  no sirve (no hay arma o catalizador del que tomar el ataque base).
- Cambiar AtkParam con el juego en marcha (SoloParamRepository) no surte efecto: tiene que venir de
  ModBuilder.
- En la bola de fuego, la bola no hace daño: lo hace su explosión (HitBulletID). Hay que copiar la
  cadena entera del hechizo.
- ModBuilder: de cada proyectil de bendición, la cadena del hechizo original en 30 escalones de daño
  fijo (×0,2 a ×64, razón 1,22), en 79620000+ (data/balas_reales.json). El motor elige el escalón con la
  proporción daño que llega / daño fijo, que aprende por rival y proyectil midiendo lo que quita
  (registro «motor: daño de verdad»). Fuegos fatuos: converge en dos disparos (28 pedidos, 20-34).
- Daño sin proyectil (Vulnerable, quemaduras, púas, marca, daño por curación): golpe invisible
  (onda del Rompetierra sin efectos, físico) con el mismo sistema, como mucho cada 0,5 s. La bomba
  usa el meteoro con daño de verdad.
- Invitado en cooperativo: de momento, lo de antes (lo aplica el anfitrión). Falta probar si los
  proyectiles del invitado dañan a los rivales del anfitrión.

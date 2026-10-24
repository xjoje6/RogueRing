//! Director de la run dentro del juego: conecta `rogue_core::run::RunState` con el mundo por
//! event flags. Mapa → nodo (combate, tesoro, mercader, gracia...) → pantalla correspondiente →
//! mapa. Las pantallas se manejan con el mando (cruceta, A y B); también con órdenes del banco de
//! pruebas.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use eldenring::cs::{CSEventFlagMan, WorldChrMan};
use fromsoftware_shared::FromStatic;
use rogue_core::characters::{CharacterDef, CharacterFile};
use rogue_core::events::{Effect, EventDef, EventFile};
use rogue_core::relics::{OwnedRelic, RelicDef, RelicFile};
use rogue_core::rewards::{self, Item, Rarity};
use rogue_core::run::{ActDef, Encounter, EncounterKind, NodeAction, RunState};

use crate::{equip, flag_get, flag_set, overlay, pad};

mod caido;
mod combate;
mod coop;
mod equipo;
mod grupo;
mod interfaz;
mod mando;
mod nodos;
#[cfg(feature = "banco")]
mod ordenes;
mod pruebas;
mod tick;
mod ultimo;

pub use interfaz::{HistView, UiCard, UiState, ui_state};
use interfaz::{item_card, nombre_nivel, slot_name, texto_entero};
#[cfg(feature = "banco")]
pub use ordenes::command;
pub use tick::{menu_listo, tick};

/// Flag de warp al hub (pasillo de la Capilla de la Anticipación), ver data/arenas_prueba.json.
const HUB_WARP_FLAG: u32 = 1049300100;
/// Cambio de fase (18-10: a cámara lenta quedaba raro): la primera fase se queda congelada e
/// intocable entre ráfagas doradas, se va, la segunda se forma invisible otro rato entre ráfagas y
/// aparece haciendo su animación, intocable mientras.
const TRANSICION_FASE: Duration = Duration::from_millis(3000);
const FASE2_OCULTA: Duration = Duration::from_millis(2500);
const ENTRADA_FASE2: Duration = Duration::from_millis(4000);
/// Velocidad de animación de «congelado» (0 justo podría tratarse aparte).
const CONGELADO: f32 = 0.01;
/// Aparición dorada de la transición de fase (la de Godfrey en Leyndell; ModBuilder), renovada cada
/// RAFAGA_FASE para que dure toda la transición.
const SP_TRANSICION: i32 = 79_619_041;
const RAFAGA_FASE: Duration = Duration::from_millis(2200);

/// Arena grande de data/arenas_grandes.json (la variante del mapa la hace tools/ModBuilder).
struct ArenaGrande {
    area: u8,
    flag: u32,
    mapa: String,
}

fn leer_arenas(root: &Path) -> HashMap<String, ArenaGrande> {
    let v: serde_json::Value = load_json(root.join("data").join("arenas_grandes.json"));
    let mut r = HashMap::new();
    for a in v["arenas"].as_array().into_iter().flatten() {
        let m: Vec<u64> = a["map"].as_array().into_iter().flatten().filter_map(|x| x.as_u64()).collect();
        if let (Some(clave), Some(flag), 4) = (a["clave"].as_str(), a["flag"].as_u64(), m.len()) {
            let mapa = format!("m{:02}_{:02}_{:02}_{:02}", m[0], m[1], m[2], m[3]);
            r.insert(clave.to_string(), ArenaGrande { area: m[0] as u8, flag: flag as u32, mapa });
        }
    }
    r
}

/// Coliseo: dónde y en qué animación se vio por última vez moverse al enemigo.
#[derive(Clone, Copy, Debug)]
struct Vigia {
    pos: [f32; 3],
    anim: i32,
    desde: Instant,
    /// Donde apareció (dentro de la arena, con suelo navegable).
    origen: [f32; 3],
    /// Desde cuándo está fuera de la arena (encaramado a un muro o lejos del centro).
    fuera: Option<Instant>,
}

/// Lo que puede subir el enemigo sobre su punto de salida o alejarse de él antes de contar como fuera
/// de la arena, y cuánto tiempo puede estar fuera antes de volver.
const VIGIA_ALTURA: f32 = 4.0;
const VIGIA_DISTANCIA: f32 = 40.0;
/// Arena grande: lo que tiene que bajar un enemigo respecto de donde apareció para darlo por caído al vacío.
const VIGIA_CAIDA: f32 = 15.0;
const VIGIA_FUERA: Duration = Duration::from_secs(4);
/// Flags de la música de combate (tools/ModBuilder, data/musica.json): empezar, subir y parar el tema
/// con índice k son estos más k.
const MUSICA_EMPEZAR: u32 = 1_049_300_500;
const MUSICA_SUBIR: u32 = 1_049_300_600;
const MUSICA_PARAR: u32 = 1_049_300_700;

/// Temas de jefe que se pueden poner en combate (data/musica.json).
#[derive(Debug, Default, serde::Deserialize)]
struct Musica {
    temas: Vec<i32>,
    genericos: Vec<i32>,
}

/// El telón con el nombre del rival dura al menos esto, para que dé tiempo a leerlo.
const TELON_MINIMO: Duration = Duration::from_secs(4);
/// Tras llegar al coliseo, el rival aparece pasado este rato (que no pille por sorpresa).
const ESPERA_RIVAL: Duration = Duration::from_secs(3);
/// Los de la primera oleada están quietos mientras dura el telón y este rato después: al levantarse
/// están donde aparecieron (15 m), no encima del jugador.
const ARRANQUE: Duration = Duration::from_secs(2);
/// Segundos que el enemigo puede quedarse parado antes de volver a su punto de salida.
const VIGIA_PARADO: Duration = Duration::from_secs(6);
/// Con el jugador a menos de esto, el enemigo está peleando: nunca cuenta como parado. Los grandes
/// atacan sin mover su raíz o empujan contra el muro, y en algunos modelos (Dragón boquiabierto de
/// Nightreign) la animación que se lee se queda fija: los mandaba al centro en mitad del combate. Con
/// los más grandes, su raíz queda lejos del jugador aunque le estén pegando: 20 m.
const VIGIA_CERCA: f32 = 20.0;

/// Escalado de un acto (data/escalado.json, «tramos»): SpEffect de zona del juego (7000-7200).
#[derive(Clone, Copy, Debug)]
struct Tramos {
    /// Primeros combates del acto.
    debil: i32,
    /// Combates normales: de la primera fila a la última.
    normal: (i32, i32),
    /// Lo que suben los élites sobre un normal de su fila.
    elite: i32,
    jefe: i32,
    /// Multiplicador de vida de débiles, normales, élites y jefes. Con un tramo más bajo el enemigo
    /// pega menos; la vida aparte fija lo que dura cada tipo de combate.
    vida: [f64; 4],
}

impl Tramos {
    /// `t`: tramos del acto; `v`: su vida por tipo de combate (data/escalado.json).
    fn de(t: &serde_json::Value, v: &serde_json::Value) -> Self {
        let n = |k: &str, d: i64| t[k].as_i64().unwrap_or(d) as i32;
        let normal = (t["normal"][0].as_i64().unwrap_or(7000) as i32, t["normal"][1].as_i64().unwrap_or(7000) as i32);
        let vida = ["debil", "normal", "elite", "jefe"].map(|k| v[k].as_f64().unwrap_or(1.0));
        Tramos { debil: n("debil", normal.0 as i64), normal, elite: n("elite", 10), jefe: n("jefe", normal.1 as i64), vida }
    }

    fn vida(&self, kind: EncounterKind) -> f64 {
        self.vida[match kind {
            EncounterKind::Weak => 0,
            EncounterKind::Normal => 1,
            EncounterKind::Elite => 2,
            EncounterKind::Boss => 3,
        }]
    }
}

/// Probabilidad (%) de que la bendición sea mítica: cofres por acto, élites y jefes. Pocas veces.
const MITICA_COFRE: [i32; 2] = [3, 6];
const MITICA_ELITE: i32 = 6;
const MITICA_JEFE: i32 = 20;

/// Área del centro (m10_01, data/arenas_prueba.json, clave «hub»).
const HUB_AREA: u8 = 10;
/// Espera entre la muerte del boss y el viaje de vuelta (animación de muerte).
const RETURN_DELAY: Duration = Duration::from_secs(6);
/// Tras volver al hub, esperar a que termine la carga antes de mostrar la recompensa.
const HUB_SETTLE: Duration = Duration::from_secs(4);
/// Warp al Coliseo de Limgrave (aparición con entidad 45000980 puesta por tools/ModBuilder).
const COLOSSEUM_WARP_FLAG: u32 = 1049300098;
/// Área del coliseo (m45_00_00_00).
const COLOSSEUM_AREA: u8 = 45;
/// Rango de entidades de los bosses creados en el coliseo (una distinta por combate).
const ARENA_ENTITY_BASE: u32 = 79_700_000;
/// Un coliseo por combate (docs/COOP_SALAS.md): jefe colocado en una variante del MSB del coliseo,
/// apagado por el evento del mapa hasta que se enciende este flag. Lo pone tools/ModBuilder.
const JEFE_MSB: u32 = 45_000_990;
const JEFE_MSB_FLAG: u32 = 1_049_300_110;
/// Invocaciones del Invocador: entidades (una por espíritu) y efecto de mejora de las cenizas del juego
/// (BuddyParam dopingSpEffect: 290000 + nivel), con el nivel de cada acto.
const INVOCACION_BASE: u32 = 45_000_960;
const INVOCACION_MAX: u32 = 8;
/// Entidades de los aliados de bendiciones de los compañeros (los propios van en 79760000+, motor.rs).
const ALIADO_AJENO_BASE: u32 = 79_770_000;
/// Primera entidad que ya no es de invocaciones (la llegada del jefe).
const INVOCACION_TOPE: u32 = 45_000_980;
/// Invocaciones del segundo invitado en adelante (8 cada uno; libres hasta los grupos de oleada 45000900+).
const INVOCACION_MAS_INVITADOS: u32 = 45_000_920;
const SP_MEJORA_CENIZA: i32 = 290_000;
const NIVEL_CENIZA: [i32; 3] = [3, 6, 10];

/// Oleadas y huecos de un combate colocado en el MSB (eventos, flags y entidades de ModBuilder).
const OLEADAS_MSB: u32 = 5;
const HUECOS_MSB: u32 = 10;

/// Ranuras del juego (ChrAsmSlot) que usa la run.
const RIGHT_SLOTS: [u32; 3] = [1, 3, 5];
const WEAPON_SLOTS: [u32; 6] = [1, 3, 5, 0, 2, 4];
const TALISMAN_SLOTS: [u32; 4] = [17, 18, 19, 20];
/// Bit de categoría de los talismanes en los ids de inventario.
const ACCESSORY_BIT: u32 = 0x2000_0000;

/// SpEffects reservados que reescribe la DLL (tools/ModBuilder).
const SP_DMG_DYNAMIC: i32 = 79_619_000;
const SP_FIRST_HIT: i32 = 79_619_001;
const SP_VENGEANCE: i32 = 79_619_002;
const SP_DEFENSE_DYNAMIC: i32 = 79_619_003;
/// Ataque del arma de fuera del estilo (equipo.rs, compensar_arma).
const SP_COMPENSACION: i32 = 79_619_004;
/// Peso de las armas del estilo del personaje en las ofertas (el resto, 1).
const PESO_ESTILO: f64 = 4.0;
/// Icono de "Aumentar poder" (el del Favor del Árbol Áureo).
const ICON_POWER: i32 = 18040;
/// Templar en la gracia (Ancient Dragon Smithing Stone).
const ICON_TEMPLAR: i32 = 2008;
/// Daño por cada nivel de Wandering Forge por encima del máximo del arma.
const FORJA_EXTRA: f64 = 0.05;
/// Rival invisible (el estado de la Black Knife élite, SpEffect 14505 de su NpcParam).
const SP_INVISIBLE: i32 = 14505;
/// Niebla helada a sus pies (data/proyectiles.json, solo visual).
const PISADA: i32 = 79_600_032;
/// Efecto de la Lágrima dura desviadora: el HKS lo consulta para el desvío perfecto (JustGuard).
/// En el mod es permanente (tools/ModBuilder). Pasiva del Samurai.
const SP_DEFLECT: i32 = 102_000;
/// Marcas para el comportamiento del jugador (tools/hks/rogue.hks): el monje esquiva con el paso
/// rápido y al samurái le rotan las animaciones de desvío. Por debajo de 16777216 (el hks usa float).
const SP_MONJE: i32 = 7_960_000;
const SP_SAMURAI: i32 = 7_960_001;
/// Pasiva del tanque: nada lo tumba ni lo hace retroceder, salvo los agarres (tools/ModBuilder).
const SP_IMPARABLE: i32 = 79_619_040;
/// Bit de categoría de la ropa (EquipParamProtector) en los ids de inventario.
const PROTECTOR_BIT: u32 = 0x1000_0000;

/// Efectos que el HKS pone cuando un desvío ACIERTA: 102019 (SetJustGuardSucceedEffect, golpes con
/// reacción de guardia) y 102020/102022 (ExecAddDamage, golpes leves). Cada aparición cuenta.
const DEFLECT_MARKS: [i32; 3] = [102_019, 102_020, 102_022];
/// Postura que quita cada desvío del samurái (fracción de la máxima del enemigo).
const DEFLECT_POISE: f32 = 0.2;

/// Estado de las bendiciones durante un combate en el coliseo.
#[derive(Debug)]
struct CombatFx {
    since: Instant,
    last_player_hp: i32,
    last_boss_hp: i32,
    first_hit_pending: bool,
    vengeance_on: bool,
    boss_hp_cut: bool,
    last_clock: Instant,
    took_damage: bool,
    /// Samurai: desvío en curso.
    deflecting: bool,
    /// Guardia acelerada activa (para devolver la velocidad normal al salir).
    guard_fast: bool,
    /// Hasta cuándo dura la recuperación rápida tras un desvío acertado.
    deflect_fast_until: Option<Instant>,
    last_hit_taken: Option<Instant>,
    last_tick: Instant,
    /// Desde cuándo está abierta la pausa (al cerrarla, sus tiempos se corren lo que duró).
    pausa_desde: Option<Instant>,
    /// Última pisada de un rival invisible: dónde y cuándo.
    pisada: Option<([f32; 3], Instant)>,
    /// Motor de efectos con disparador (bendiciones con `efectos`/`pasivo` y mutadores del boss).
    motor: Option<crate::motor::Motor>,
    /// Señales del fotograma para el motor (las ponen los bloques de cada personaje).
    ev_deflect: bool,
    /// Estadísticas del combate: daño y curación por fuente, daño recibido y golpes.
    st_dmg: HashMap<String, f64>,
    st_heal: HashMap<String, f64>,
    st_taken: f64,
    st_hits: u32,
    /// Último golpe contado: animación de ataque en la que fue y cuándo (combate.rs).
    golpe_anim: Option<(i32, Instant)>,
    /// Vida mínima (fracción) en el combate.
    st_min: f64,
}

/// Artículo del mercader.
#[derive(Clone, Debug)]
enum Ware {
    Item(Item),
    Relic(RelicDef),
}

/// Acciones del menú principal y de la pausa.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Menu {
    Continuar,
    Nueva,
    Personajes,
    Coop,
    Compendio,
    Salir,
}

/// Menú del cooperativo (coop.rs): crear sala, unirse, salir, cambiar la contraseña y volver.
#[derive(Clone, Copy, PartialEq, Debug)]
enum MenuCoop {
    Nueva,
    Crear,
    Unirse,
    Salir,
    Clave,
    AMano,
    Volver,
}

/// Uso de un objeto de Seamless desde el menú (coop.rs): qué objeto, en qué paso va, desde cuándo, lo
/// que había en la ranura rápida, qué intento es y cuándo se pulsó A por última vez.
struct Sala {
    objeto: u32,
    paso: u8,
    desde: Instant,
    antes: Option<(u32, i32, u32, i32)>,
    intento: u8,
    ultima_a: Option<Instant>,
    /// Diagnóstico (16-10: en la partida del jugador la X no usaba el objeto): animación al pulsar X y si
    /// ya se apuntó la de después.
    anim_x: Option<(Instant, Option<i32>)>,
    anim_visto: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pausa {
    Seguir,
    Pruebas,
    Menu,
    Abandonar,
    Salir,
    /// Lo que hace la ascensión de la run y sus mutadores (texto_ascension).
    Ascension,
}

#[derive(Debug)]
enum Phase {
    /// Menú principal del mod: lo primero que se ve al cargar la partida.
    MainMenu,
    /// Pantalla de Personajes: el personaje `idx` en el mundo, con su ropa y el arma elegida.
    Personajes {
        idx: usize,
    },
    /// Elegir personaje al empezar la run (tres cartas, como StS).
    CharSelect,
    /// Bendición inicial (el Neow de StS): una común, poder + runas, o una rara con una maldición.
    Neow {
        common: Option<RelicDef>,
        rare: Option<RelicDef>,
        curse: Option<RelicDef>,
    },
    Map,
    /// `since`: cuándo empezó el viaje; `healed`: ya se preparó la llegada (escalado, vida llena
    /// y, si hace falta, colocado junto al boss); `tries`/`last_try`: intentos de colocación.
    Combat {
        enc: Encounter,
        since: Instant,
        healed: bool,
        tries: u8,
        last_try: Option<Instant>,
    },
    /// Combate en el coliseo: `ent` = entidad del boss creado (0 = aún no creado); `llegada`: cuándo
    /// terminó la carga y el jugador ya está en la arena.
    Arena {
        enc: Encounter,
        since: Instant,
        llegada: Option<Instant>,
        ent: u32,
        spawned_at: Option<Instant>,
        ready: bool,
    },
    Won {
        enc: Encounter,
        at: Instant,
    },
    Returning {
        enc: Encounter,
        at: Instant,
    },
    /// `extra`: bendiciones ganadas en el mismo combate (élite, Juramento), para la cabecera.
    Reward {
        enemy: String,
        runes: u32,
        extra: Vec<String>,
        cartas: Vec<Carta>,
    },
    /// `trap`: el cofre es una trampa (se pelea con este encuentro y el botín se cobra al ganar).
    Treasure {
        relic: Option<RelicDef>,
        runes: u32,
        trap: Option<Encounter>,
    },
    Shop {
        wares: Vec<(Ware, u32, bool)>,
    },
    Rest,
    /// Gracia, opción Templar: elegir cuál de tus bendiciones sube un nivel.
    Templar {
        opciones: Vec<OwnedRelic>,
    },
    /// Evento del "?". `result`: tras elegir, texto del resultado y pelea pendiente (si la hay).
    Event {
        ev: EventDef,
        result: Option<(String, Option<Encounter>)>,
    },
    /// Objeto que no cabe (o ceniza que hay que montar): elegir ranura. `back`: fase a la que se vuelve.
    Replace {
        item: Item,
        options: Vec<u32>,
        back: Box<Phase>,
    },
    /// Fin de la run. `new_run`: el jugador pidió otra (se crea cuando reaparece vivo).
    /// `shown`: desde cuándo se ve la pantalla (la A de "revivir" no debe contar como "Nueva run").
    Over {
        won: bool,
        new_run: bool,
        shown: Option<Instant>,
    },
}

struct Director {
    root: PathBuf,
    acts: Vec<ActDef>,
    /// Los mismos actos con las listas del cooperativo (dúos), para las runs de una sesión.
    acts_coop: Vec<ActDef>,
    pool: Vec<Item>,
    relics: RelicFile,
    events: EventFile,
    chars: Vec<CharacterDef>,
    /// Mutadores del boss (data/mutadores.json).
    mutadores: Vec<crate::motor::Mutador>,
    /// Premios de un evento que se cobran al ganar su pelea.
    pending: Vec<Effect>,
    /// Bendiciones en el combate actual (None fuera del coliseo).
    fx: Option<CombatFx>,
    run: Option<RunState>,
    phase: Phase,
    /// Tramos de escalado (SpEffect 7000-7200) de cada acto, de data/escalado.json.
    tiers: Vec<Tramos>,
    /// Coliseo: enemigo parado (ver `vigilar`).
    vigia: Option<Vigia>,
    /// Selección en la pantalla actual (índice de nodo disponible o de carta).
    sel: usize,
    last_buttons: u16,
    /// Gracia generosa: opciones ya usadas en la gracia actual (descansar, aumentar poder).
    /// Opciones de la gracia ya usadas: descansar, aumentar poder y templar.
    rest_used: [bool; 3],
    /// Lista de bendiciones abierta (desde el mapa con Y).
    show_list: bool,
    /// Bendición elegida en esa lista (su ficha a la derecha).
    lista_sel: usize,
    /// Compendio abierto (desde el selector con X) y bendición elegida en él.
    show_compendio: bool,
    /// Run guardada sin terminar, para «Continuar» desde el menú principal.
    guardada: Option<RunState>,
    /// Menú de pausa abierto (START) y su opción elegida.
    pausa: Option<usize>,
    /// Pantalla de la ascensión abierta desde la pausa.
    ver_ascension: bool,
    /// Caídos: intocable e ignorado hasta entonces (caido.rs, GRACIA).
    gracia_hasta: Option<Instant>,
    /// La gracia de la run perdida: dura hasta estar en el centro (caido.rs).
    intocable_derrota: bool,
    /// Ya se avisó de que se espera a que lleguen todos a la arena (coop.rs, esperando_llegada).
    llegada_avisada: bool,
    /// Compensación del daño de las armas de fuera del estilo: por personaje, id del arma (con afinidad,
    /// sin mejora) y multiplicador (data/compensacion_armas.json); y la que está puesta (equipo.rs).
    compensacion: HashMap<String, HashMap<i32, f64>>,
    compensacion_puesta: f64,
    compensacion_at: Option<Instant>,
    /// Invitado del cooperativo: viales, vida y acto al ganar un combate. Seamless se lo lleva con el
    /// anfitrión al volver y le recarga viales y vida (como al invocarlo); se le devuelven (tick).
    vuelta_guardada: Option<(Vec<(usize, u32, u32)>, Option<i32>, usize, Instant)>,
    /// Cooperativo: personaje elegido en la selección, a la espera de que elijan todos (coop.rs).
    elegido: Option<usize>,
    /// Pantalla que vio el mando la última vez y desde cuándo (mando.rs, `nav`): la A que llega justo al
    /// cambiar de pantalla sin pedirlo no cuenta.
    pantalla_vista: Option<(std::mem::Discriminant<Phase>, Instant)>,
    /// Cooperativo: votos del mapa completos y desde cuándo no cambian (se va cuando llevan un rato
    /// iguales; mientras, cada uno puede cambiar el suyo).
    votos_estables: Option<(String, Instant)>,
    /// Versión de pruebas (data/modo_pruebas.txt): menú de pruebas en la pausa del mapa (pruebas.rs).
    pruebas_activo: bool,
    /// Menú de pruebas abierto: pantalla, opción elegida y página; y las pantallas de atrás.
    menu_pruebas: Option<(pruebas::Pantalla, usize, usize)>,
    historial_pruebas: Vec<(pruebas::Pantalla, usize, usize)>,
    /// Compendio abierto desde el menú de pruebas: A da la bendición elegida.
    compendio_dar: bool,
    /// Combate del menú de pruebas: el escalado es el del acto de su lista, no el de la run.
    acto_prueba: Option<usize>,
    /// Hasta cuándo se cierran solos los avisos de objeto nuevo del juego (vestuario por adelantado).
    cerrar_avisos: Option<Instant>,
    /// HUD del juego oculto ahora (menú y Personajes).
    hud_oculto: bool,
    /// Viales que quedan y máximo (se leen en el hilo del juego mientras se ve el mapa).
    viales: Option<(u32, u32)>,
    /// Nivel de mejora del arma y su máximo (mapa, gracia y pausa).
    arma: Option<(u32, u32)>,
    musica: Musica,
    /// Tema que suena (índice en `musica.temas`) y si ya subió de intensidad.
    sonando: Option<(usize, bool)>,
    compendio_sel: usize,
    /// Nodo elegido en el gráfico del final.
    over_sel: usize,
    /// Cuándo quedó listo el combate actual (la barra del boss espera a que acabe la carga).
    ready_at: Option<Instant>,
    commence_shown: bool,
    /// Perfil persistente (runtime/perfil.json) y si la run actual ya se apuntó al terminar.
    perfil: rogue_core::perfil::Perfil,
    /// Build de desarrollador (ROGUE_DESARROLLO=1, «Jugar rama de pruebas (desarrollador).bat»): todo
    /// desbloqueado (armas, apariencias, invocaciones, ascensiones y compendio) sin cambiar el perfil.
    desarrollo: bool,
    recorded: bool,
    /// Personaje elegido durante una carga (tras morir el juego lleva al centro): se aplica al terminar.
    char_pendiente: Option<usize>,
    /// Desde cuándo la pantalla está despejada tras la carga del combate (para barra y letrero).
    clear_since: Option<Instant>,
    visible: bool,
    /// Invocaciones creadas en este combate a las que falta ponerles la mejora (cuando existan), las de
    /// este combate, y desde cuándo han caído todas (Relevo y Pacto de espíritu).
    invocados: Vec<u32>,
    invocacion_ents: Vec<u32>,
    invocacion_caida: Option<Instant>,
    /// Cooperativo (coop.rs): mi último voto del mapa y su paso (se sigue mandando tras decidir), el
    /// paso ya decidido, el viaje que espera el anfitrión y, siendo anfitrión, los estados del juego en
    /// el rival que se mandan a los demás.
    voto: Option<(rogue_core::coop::Paso, (i32, i32))>,
    decidido: Option<rogue_core::coop::Paso>,
    /// Acto nuevo: los viales se recargan en cuanto se está en el centro (tick).
    recargar_viales: bool,
    /// Invocaciones de los compañeros creadas en este juego: a falta de su mejora (entidad, SpEffect), y
    /// de quién son (jugador, ceniza, entidades) para la vida en el HUD.
    invocados_ajenos: Vec<(u32, i32)>,
    invocaciones_ajenas: HashMap<u64, (String, String, Vec<u32>)>,
    /// Aliados de las bendiciones de los compañeros creados en este juego (entidad, hasta cuándo,
    /// mejorado) y la siguiente entidad libre para ellos.
    aliados_ajenos: Vec<(u32, Instant, bool)>,
    siguiente_aliado: u32,
    /// Tamaño del personaje puesto fuera del motor (tick), para volver a 1 al perder la bendición.
    tamano_puesto: f32,
    /// Veces que se ha vuelto a poner el escalado a cada rival en esta oleada (`asegurar_escalado`).
    escalado_intentos: HashMap<u32, (u8, Instant)>,
    /// Última cura de estados tras ganar un combate (tick) y si ya se repitió el viaje a la arena.
    curado_victoria: Option<Instant>,
    viaje_repetido: bool,
    /// Cooperativo, caídos (caido.rs): el estado, sus reglas, desde cuándo hay 1 de vida, a quién se
    /// está levantando, si la opción de no morir es nuestra, la animación de pruebas que sigue y hasta
    /// cuándo dura su intocabilidad.
    caido: Option<caido::Caida>,
    /// Tus últimos golpes del último combate de la run (semilla y golpes): el golpe medio sigue de un
    /// combate al siguiente, no de una run a otra.
    golpes_run: (String, Vec<f64>),
    /// Rivales con barra de jefe del juego ahora (tick).
    barras_nativas: Vec<u32>,
    /// Id del nombre de cada combate en los textos del juego (data/nombres_jefe.json, ModBuilder).
    nombres_jefe: HashMap<String, i32>,
    /// Id del nombre de cada enemigo (fila de NpcParam) para las barras de los combates de varios.
    nombres_npc: HashMap<i32, i32>,
    /// Viales vistos fuera del combate y cuándo (tick: aviso si suben).
    viales_vistos: Option<(u32, u32)>,
    viales_vistos_at: Option<Instant>,
    caido_cfg: caido::CaidoCfg,
    vida_baja_desde: Option<Instant>,
    ayuda: Option<(u64, Instant)>,
    no_dead_coop: bool,
    muerte_real: bool,
    /// Caído en el suelo: desde cuándo suena su postura (para pararla) y el último giro.
    postura_desde: Option<Instant>,
    giro_at: Option<Instant>,
    /// Banco de pruebas: orden «ganar» (tick lo da por ganado).
    forzar_victoria: bool,
    /// Cooperativo (coop.rs): caído en este combate y si ya se dejó morir (caídos todos). Y los
    /// compañeros caídos que no se dibujan.
    /// Cooperativo: último combate cerrado y cómo acabó (lo publica el anfitrión; protocolo.rs).
    ultimo_coop: Option<(rogue_core::coop::Paso, rogue_core::protocolo::Resultado)>,
    /// Lo mismo para la opción de un evento, y listo para el combate que sale de él.
    voto_evento: Option<(rogue_core::coop::Paso, usize)>,
    decidido_evento: Option<rogue_core::coop::Paso>,
    listo: Option<rogue_core::coop::Paso>,
    viaje_en: Option<(u32, Instant)>,
    rival_coop: Vec<rogue_core::coop::EstadoRival>,
    /// Vida de más de los rivales por cada jugador a partir del tercero (data/escalado.json).
    coop_extra: f64,
    /// Dónde aparecen los colocados en el MSB (entidad → posición), para devolverlos allí al encenderlos.
    origenes: HashMap<u32, [f32; 3]>,
    /// Desde cuándo se ve el combate, y si los de la primera oleada ya se han soltado (ARRANQUE).
    visible_desde: Option<Instant>,
    arranque_suelto: bool,
    /// Los rivales quietos (telón, ARRANQUE, pausa) son intocables; al moverse, se les quita (tick.rs).
    rivales_intocables: bool,
    /// El combate en curso usa el jefe colocado en el MSB (si no, `spawn_chr`).
    jefe_msb: bool,
    /// Cambio de fase con un personaje por fase (`fase_en`): dónde estaba la primera, para poner ahí la
    /// segunda cuando exista.
    pos_fase: Option<[f32; 3]>,
    /// Segunda fase de un solo personaje (`renacer`): cuándo empezó (None = aún no).
    renacido: Option<Instant>,
    renacer_cerrado: bool,
    /// Cambio de fase en marcha (18-10: Godfrey pasaba de golpe a Hoarah Loux, ya atacando):
    /// desde cuándo la primera fase espera quieta, y hasta cuándo la segunda es intocable.
    transicion: Option<Instant>,
    /// Última ráfaga dorada de la transición.
    rafaga: Option<Instant>,
    /// Segunda fase formándose, invisible, hasta este momento.
    fase2_oculta: Option<Instant>,
    /// «El último en pie» de un dúo (ultimo.rs): el superviviente y su segunda fase.
    ultimo: Option<ultimo::Ultimo>,
    ultimo_escalado: bool,
    /// Entidades que se suman a la oleada en curso (la segunda fase del último en pie).
    extra_oleada: Vec<u32>,
    fase2_intocable: Option<Instant>,
    /// Arenas grandes (data/arenas_grandes.json) por clave: área, flag de viaje y nombre del mapa.
    arenas: HashMap<String, ArenaGrande>,
    /// Área y flag de viaje del combate en curso: el coliseo o una arena grande.
    area_combate: u8,
    flag_combate: u32,
    /// Combate de grupo (grupo.rs): entidades por índice de `Encounter::lista`, oleada en curso, si ya
    /// tiene su escalado y desde cuándo, vida de cada rival en el fotograma anterior, los caídos y los
    /// que ya se vieron.
    grupo: Vec<u32>,
    oleada: usize,
    oleada_lista: bool,
    oleada_desde: Option<Instant>,
    vida_previa: HashMap<u32, i32>,
    muertos: HashSet<u32>,
    vistos: HashSet<u32>,
    /// Bendiciones de más al acabar el combate: racha y contrarreloj.
    premio_extra: usize,
    /// Menú del cooperativo abierto (sobre el principal), el objeto de Seamless que se está usando y el
    /// aviso si no sirvió.
    menu_coop: bool,
    sala: Option<Sala>,
    aviso_coop: Option<String>,
    /// Selector de la contraseña de la sesión (coop.rs): índice en `CARACTERES_CLAVE` de cada letra y
    /// cuál se está cambiando.
    clave: Option<(Vec<usize>, usize)>,
    /// Si el objeto no se pudo usar solo: manos libres para usarlo desde el inventario (START), con
    /// el juego a la vista; SELECT vuelve al menú del mod.
    libre: bool,
    /// Última línea de diagnóstico del cooperativo en combate.
    diag_at: Option<Instant>,
}

static DIRECTOR: Mutex<Option<Director>> = Mutex::new(None);
/// Aviso de una bendición ganada.
pub struct Aviso {
    pub nombre: String,
    pub texto: String,
    pub icono: i32,
    pub maldita: bool,
    pub desde: Instant,
}

/// Aviso suelto (sin bendición): cambio de fase de un jefe.
pub fn aviso(nombre: &str, texto: &str) {
    if let Ok(mut t) = TOASTS.lock() {
        t.push(Aviso { nombre: nombre.into(), texto: texto.into(), icono: 0, maldita: true, desde: Instant::now() });
    }
}

/// Avisos de bendiciones ganadas. Los dibuja el overlay.
pub static TOASTS: Mutex<Vec<Aviso>> = Mutex::new(Vec::new());
/// Boss del coliseo para la barra de vida: (nombre, vida, máxima). Lo escribe el hilo del juego.
pub use caido::CAIDO_UI;
/// La sala del cooperativo en los menús (coop.rs, vista_sala): líneas ya traducidas.
pub static SALA_UI: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub static BOSS_BAR: Mutex<Option<(String, i32, i32)>> = Mutex::new(None);
/// Barras de jefe del juego puestas: de la nuestra solo va lo de alrededor (oleada, tiempo, mutadores,
/// umbrales y estados), colocado sobre las del juego. 0 = no; si no, cuántas + 256 × la del rival.
pub static BARRA_NATIVA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Rival del combate al que se viaja (se muestra sobre el telón): (tipo, nombre).
pub static COVER_TEXT: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Último combate terminado (para el resumen de la pantalla de recompensa).
pub static LAST_COMBAT: Mutex<Option<ResumenCombate>> = Mutex::new(None);
/// Segundos y daño por fuente.
pub type ResumenCombate = (f64, Vec<(String, f64)>);
/// Mutadores del boss actual (se muestran junto a su nombre).
pub static BOSS_MUTS: Mutex<String> = Mutex::new(String::new());
/// Semilla nueva al azar (seis letras), de la hora.
pub(super) fn semilla_nueva() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut x = t as u64;
    (0..6)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (b'A' + (x >> 59) as u8 % 26) as char
        })
        .collect()
}

/// Caído en un combate de cooperativo (overlay: el aviso).
/// Vida de los aliados en combate (overlay): compañeros e invocaciones.
pub static ALIADOS: Mutex<Vec<rogue_ui::aliados::Aliado>> = Mutex::new(Vec::new());
/// Tira de bendiciones del combate (overlay): (icono, estado).
pub static TIRA: Mutex<Vec<(i32, crate::motor::HudBendicion)>> = Mutex::new(Vec::new());
/// Bajo la barra del rival: sus estados y efectos (con el icono de la bendición que los puso, si es
/// cosa del motor) y los umbrales de tus bendiciones.
pub static RIVAL: Mutex<VistaRival> = Mutex::new((Vec::new(), Vec::new()));
/// (icono de la bendición, estado o efecto) y umbrales (fracción de vida, remate).
pub type VistaRival = (Vec<(i32, crate::motor::EstadoRival)>, Vec<(f32, bool)>);
/// Carta de la pantalla de recompensa.
#[derive(Clone, Debug)]
enum Carta {
    Objeto(Item),
    Bendicion(RelicDef),
}

/// Bendición en marcha en la pausa en combate.
pub struct FilaPausa {
    pub icono: i32,
    pub nombre: String,
    pub texto: String,
    /// Precio de una mítica (vacío si no tiene).
    pub contra: String,
    pub estado: crate::motor::HudBendicion,
}

/// Pausa en combate: bendiciones en marcha y el resto por nombre.
pub struct PausaCombate {
    /// «Weapon +12 / 25», junto al título.
    pub arma: String,
    pub filas: Vec<FilaPausa>,
    pub resto: Vec<String>,
}

pub static PAUSA_COMBATE: Mutex<Option<PausaCombate>> = Mutex::new(None);

/// El personaje existe y no hay carga ni viaje en curso: ya se le pueden poner atributos y equipo.
fn jugador_listo() -> bool {
    player_hp().unwrap_or(0) > 0 && crate::player_area().is_some_and(|a| a != 255) && !flag_get(HUB_WARP_FLAG) && crate::screen_fade() < 0.05
}

fn player_hp() -> Option<i32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    Some(wcm.main_player.as_ref()?.chr_ins.modules.data.hp)
}

/// Vida al máximo (cada combate empieza con la vida llena; los frascos se arrastran).
/// Las curas de los estados (los bolos sin su resistencia de después; ModBuilder, 79619043-49).
const SP_CURAS: std::ops::Range<i32> = 79_619_043..79_619_050;

/// Sin estados alterados (veneno, podredumbre, congelación...): al ganar y al empezar cada combate.
fn curar_estados() {
    for sp in SP_CURAS {
        crate::speffect_propio(sp, true);
    }
}

fn heal_player() {
    curar_estados();
    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() }
        && let Some(p) = wcm.main_player.as_mut()
    {
        let d = &mut p.chr_ins.modules.data;
        if d.hp > 0 {
            d.hp = d.max_hp;
        }
    }
}

fn load_json<T: serde::de::DeserializeOwned + Default>(path: PathBuf) -> T {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Actos del cooperativo: por tipo, los encuentros de data/coop/<acto>.json (dos jefes donde se puede)
/// y los de grupo del acto; un tipo sin ninguno se queda con los del acto en solitario.
fn actos_coop(root: &Path, acts: &[ActDef]) -> Vec<ActDef> {
    #[derive(serde::Deserialize, Default)]
    struct Lista {
        #[serde(default)]
        encounters: Vec<Encounter>,
    }
    acts.iter()
        .enumerate()
        .map(|(i, a)| {
            let coop: Lista = load_json(root.join("data").join("coop").join(format!("{}.json", i + 1)));
            let mut out = a.clone();
            // Parejas posibles: el acto sortea unas cuantas en cada run (rogue_core::run::sortear_mezclas).
            let mezclas: Lista = load_json(root.join("data").join("coop").join(format!("mezclas_{}.json", i + 1)));
            out.mezclas = mezclas.encounters.into_iter().filter(|e| e.en_coliseo()).collect();
            if coop.encounters.is_empty() {
                return out;
            }
            out.encounters = [EncounterKind::Weak, EncounterKind::Normal, EncounterKind::Elite, EncounterKind::Boss]
                .iter()
                .flat_map(|&k| {
                    // Las parejas (a mano y las mezclas de tools/mezclas_coop.py) y todo lo de solitario: los
                    // de uno y los grupos (24-10: las listas de dúos solas se repetían mucho). La vida
                    // extra por jugador la pone el combate (coop_extra).
                    // De solitario, solo los de grupo (oleadas, manadas, hordas): en cooperativo nunca sale un
                    // enemigo solo (24-10); las parejas sorteadas se suman al empezar el acto.
                    let mut v: Vec<Encounter> = coop.encounters.iter().filter(|e| e.kind == k).cloned().collect();
                    v.extend(a.encounters.iter().filter(|e| e.kind == k && e.lista().len() >= 2).cloned());
                    if v.is_empty() { a.encounters.iter().filter(|e| e.kind == k).cloned().collect() } else { v }
                })
                .collect();
            out
        })
        .collect()
}

fn load_acts(root: &Path) -> Result<Vec<ActDef>, String> {
    let dir = root.join("data").join("acts");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let t = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            serde_json::from_str::<ActDef>(&t).map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}

/// Precio en runas según rareza y acto (las runas son el oro de la run).
fn price(r: Rarity, act: usize, relic: bool) -> u32 {
    let base = match (r, relic) {
        (Rarity::Comun, false) => 250,
        (Rarity::PocoComun, false) => 450,
        (Rarity::Rara, false) => 900,
        (Rarity::Comun, true) => 400,
        (Rarity::PocoComun, true) => 650,
        (Rarity::Rara, true) => 1100,
        (Rarity::Mitica, _) => 1800,
    };
    (base as f64 * (1.0 + 0.5 * act as f64)) as u32
}

impl Director {
    fn save(&self) {
        if let Some(r) = &self.run {
            let path = run_path(&self.root);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&path, r.to_json()) {
                crate::log(&format!("run: no se pudo guardar {}: {e}", path.display()));
            }
        }
    }

    fn act(&self) -> usize {
        self.run.as_ref().map(|r| r.act_index).unwrap_or(0)
    }
}

/// Sonidos de menú del juego (eventos 79690200+k de tools/ModBuilder, mismo orden que `menuSe`).
#[derive(Clone, Copy)]
pub enum Se {
    Cursor = 0,
    Confirm = 1,
    Back = 2,
    Open = 3,
    Blessing = 4,
}

pub fn play_se(se: Se) {
    play_se_slot(se as u32);
}

pub fn play_se_slot(k: u32) {
    if let Ok(m) = unsafe { CSEventFlagMan::instance_mut() } {
        m.virtual_memory_flag.set_flag(1_049_300_160 + k, true);
    }
}

/// Carpeta raíz del mod (la padre de runtime/); la fija lib.rs al cargar la DLL.
pub static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Director con los datos del mod y una run nueva (en el selector de personaje).
fn build_director(root: &Path, seed: &str) -> Result<Director, String> {
    let mut acts = load_acts(root).map_err(|e| format!("error cargando actos: {e}"))?;
    if acts.is_empty() {
        return Err("error: no hay actos en data/acts".into());
    }
    let mut pool: Vec<Item> = load_json(root.join("data").join("objetos").join("pool.json"));
    let mut relics: RelicFile = load_json(root.join("data").join("reliquias.json"));
    // Proyectiles con daño de verdad, en escalones (motor.rs; lo escribe tools/ModBuilder).
    let reales: serde_json::Value = load_json(root.join("data").join("balas_reales.json"));
    crate::motor::poner_balas_reales(
        reales["origen_hechizo"].as_i64().unwrap_or(-1) as i32,
        reales["balas"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.parse().ok()?, serde_json::from_value(v.clone()).ok()?)))
            .collect(),
    );
    // Proyectiles que caen sobre el objetivo (motor.rs).
    let proyectiles: serde_json::Value = load_json(root.join("data").join("proyectiles.json"));
    crate::motor::poner_en_objetivo(
        proyectiles["proyectiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["en_objetivo"].as_bool() == Some(true))
            .filter_map(|p| p["id"].as_i64().map(|i| i as i32))
            .collect(),
    );
    crate::motor::poner_atras(
        proyectiles["proyectiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p["id"].as_i64()? as i32, p["atras"].as_f64()? as f32)))
            .collect(),
    );
    let mut events: EventFile = load_json(root.join("data").join("eventos.json"));
    let chars: CharacterFile = load_json(root.join("data").join("personajes.json"));
    let mut chars = chars.personajes;
    let mut mutadores: Vec<crate::motor::Mutador> = load_json::<serde_json::Value>(root.join("data").join("mutadores.json"))
        .get("mutadores")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // Textos en el idioma del juego (data/i18n): los datos se traducen al cargarlos.
    if let Some(t) = crate::idioma::traduccion(root) {
        t.actos(&mut acts);
        t.objetos(&mut pool);
        t.bendiciones(&mut relics);
        t.eventos(&mut events);
        t.personajes(&mut chars);
        for m in &mut mutadores {
            if let Some(tm) = t.mutadores.get(&m.id) {
                m.nombre = tm.nombre.clone();
                m.texto = tm.texto.clone();
            }
        }
    }
    // Cooperativo (docs/LISTAS_Y_COMBATES.md): los actos con las listas de data/coop.
    let mut acts_coop = actos_coop(root, &acts);
    if let Some(t) = crate::idioma::traduccion(root) {
        t.actos(&mut acts_coop);
    }
    let esc: serde_json::Value = load_json(root.join("data").join("escalado.json"));
    let tiers = (1..=acts.len()).map(|n| Tramos::de(&esc["tramos"][n.to_string()], &esc["vida"][n.to_string()])).collect();
    let coop_extra = esc["coop_extra"].as_f64().unwrap_or(0.5);
    let compensacion: HashMap<String, HashMap<i32, f64>> = load_json::<serde_json::Value>(root.join("data").join("compensacion_armas.json"))
        .get("personajes")
        .and_then(|v| serde_json::from_value::<HashMap<String, HashMap<String, f64>>>(v.clone()).ok())
        .map(|m| m.into_iter().map(|(pj, a)| (pj, a.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect())).collect())
        .unwrap_or_default();
    let perfil = rogue_core::perfil::Perfil::load(&root.join("runtime").join("perfil.json"));
    // También en los paquetes privados con data/desbloqueado.txt (tools/empaquetar.py), para revisar todo
    // sin ganar runs (23-10).
    let desarrollo = std::env::var("ROGUE_DESARROLLO").is_ok_and(|v| v == "1") || root.join("data").join("desbloqueado.txt").exists();
    if desarrollo {
        crate::log("desarrollo: todo desbloqueado");
    }
    let run = RunState::nueva(seed, &acts, perfil.rivales_recientes());
    // Reglas del modo (peso, escalado, viales) en memoria.
    let (npcs, lastre) = crate::globales::datos(root, &[acts.clone(), acts_coop.clone()].concat());
    crate::globales::activar(&npcs, lastre);
    Ok(Director {
        root: root.to_path_buf(),
        acts,
        acts_coop,
        pool,
        relics,
        events,
        chars,
        mutadores,
        pending: vec![],
        fx: None,
        run: Some(run),
        phase: Phase::CharSelect,
        tiers,
        vigia: None,
        sel: 0,
        last_buttons: 0,
        rest_used: [false; 3],
        show_list: false,
        lista_sel: 0,
        show_compendio: false,
        guardada: None,
        pausa: None,
        ver_ascension: false,
        gracia_hasta: None,
        intocable_derrota: false,
        llegada_avisada: false,
        vuelta_guardada: None,
        compensacion,
        compensacion_puesta: 1.0,
        compensacion_at: None,
        elegido: None,
        votos_estables: None,
        pantalla_vista: None,
        pruebas_activo: {
            let si = root.join("data").join("modo_pruebas.txt").exists();
            if si {
                crate::coop::REGISTRO_COMPARTIDO.store(true, Ordering::Relaxed);
                crate::log("versión de pruebas: menú de pruebas en la pausa del mapa; registro compartido con los compañeros");
            }
            si
        },
        menu_pruebas: None,
        historial_pruebas: vec![],
        compendio_dar: false,
        acto_prueba: None,
        cerrar_avisos: None,
        hud_oculto: false,
        viales: None,
        arma: None,
        musica: load_json(root.join("data").join("musica.json")),
        sonando: None,
        compendio_sel: 0,
        over_sel: usize::MAX,
        ready_at: None,
        commence_shown: true,
        clear_since: None,
        visible: false,
        visible_desde: None,
        arranque_suelto: true,
        rivales_intocables: false,
        origenes: HashMap::new(),
        invocados: vec![],
        invocacion_ents: vec![],
        invocacion_caida: None,
        voto: None,
        decidido: None,
        voto_evento: None,
        decidido_evento: None,
        listo: None,
        recargar_viales: false,
        invocados_ajenos: vec![],
        invocaciones_ajenas: HashMap::new(),
        aliados_ajenos: vec![],
        siguiente_aliado: ALIADO_AJENO_BASE,
        tamano_puesto: 1.0,
        escalado_intentos: HashMap::new(),
        curado_victoria: None,
        viaje_repetido: false,
        caido: None,
        golpes_run: (String::new(), vec![]),
        barras_nativas: vec![],
        nombres_jefe: {
            let v: serde_json::Value = load_json(root.join("data").join("nombres_jefe.json"));
            v["nombres"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v.as_i64()? as i32))).collect()
        },
        nombres_npc: {
            let v: serde_json::Value = load_json(root.join("data").join("nombres_jefe.json"));
            v["enemigos"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_i64()? as i32))).collect()
        },
        viales_vistos: None,
        viales_vistos_at: None,
        caido_cfg: load_json(root.join("data").join("caido.json")),
        vida_baja_desde: None,
        ayuda: None,
        no_dead_coop: false,
        muerte_real: false,
        postura_desde: None,
        giro_at: None,
        forzar_victoria: false,
        ultimo_coop: None,
        viaje_en: None,
        rival_coop: vec![],
        coop_extra,
        perfil,
        desarrollo,
        recorded: false,
        char_pendiente: None,
        jefe_msb: false,
        pos_fase: None,
        renacido: None,
        renacer_cerrado: false,
        transicion: None,
        rafaga: None,
        fase2_oculta: None,
        ultimo: None,
        ultimo_escalado: false,
        extra_oleada: Vec::new(),
        fase2_intocable: None,
        arenas: leer_arenas(root),
        area_combate: COLOSSEUM_AREA,
        flag_combate: COLOSSEUM_WARP_FLAG,
        grupo: vec![],
        oleada: 0,
        oleada_lista: false,
        oleada_desde: None,
        vida_previa: HashMap::new(),
        muertos: HashSet::new(),
        vistos: HashSet::new(),
        premio_extra: 0,
        menu_coop: false,
        sala: None,
        aviso_coop: None,
        libre: false,
        clave: None,
        diag_at: None,
    })
}

/// Semilla al azar de 6 letras.
fn random_seed() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut x = t as u64;
    (0..6)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (b'A' + (x >> 59) as u8 % 26) as char
        })
        .collect()
}

static AUTO_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static AUTO_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

fn run_path(root: &Path) -> PathBuf {
    root.join("runtime").join("run.json")
}

/// Arranque para jugadores (sin banco de pruebas): con el personaje 3 s en el mundo y sin run,
/// retoma la guardada si no terminó o empieza una nueva. `runtime/sin_autoinicio` lo desactiva.
pub fn autostart() {
    if AUTO_DONE.load(Ordering::Relaxed) {
        return;
    }
    let Some(root) = ROOT.get() else { return };
    let Ok(mut guard) = DIRECTOR.try_lock() else { return };
    if guard.is_some() || root.join("runtime").join("sin_autoinicio").exists() {
        AUTO_DONE.store(true, Ordering::Relaxed);
        return;
    }
    let in_world = player_hp().unwrap_or(0) > 0 && crate::player_area().is_some_and(|a| a != 255);
    let Ok(mut since) = AUTO_SINCE.lock() else { return };
    if !in_world {
        *since = None;
        return;
    }
    let t = *since.get_or_insert_with(Instant::now);
    if t.elapsed() < Duration::from_secs(3) {
        return;
    }
    AUTO_DONE.store(true, Ordering::Relaxed);
    let saved = std::fs::read_to_string(run_path(root)).ok().and_then(|s| RunState::from_json(&s).ok());
    let Ok(mut d) = build_director(root, &random_seed()) else { return };
    // Al menú principal: «Continuar» retoma la guardada si no terminó.
    // (y si su personaje sigue existiendo: las de personajes retirados no se pueden seguir).
    d.guardada = saved.filter(|run| !run.terminada && run.personaje.as_ref().is_some_and(|p| d.chars.iter().any(|c| &c.id == p)));
    d.phase = Phase::MainMenu;
    d.sel = 0;
    d.preparar_vestuario();
    if crate::player_area() != Some(HUB_AREA) {
        flag_set(HUB_WARP_FLAG, true);
    }
    crate::log(&format!("autoinicio: menú principal (run guardada: {})", d.guardada.as_ref().map(|r| r.seed.as_str()).unwrap_or("no")));
    *guard = Some(d);
}

/// Deja el director en la run guardada: bendiciones, pasiva, frascos y el nodo sin resolver.
fn retomar(d: &mut Director, mut run: RunState) {
    // Bendiciones de la run guardada que no existen en esta versión del mod.
    run.relics.retain(|o| d.relics.get(&o.id).is_some());
    {
        // Nodo sin resolver (el juego se cerró dentro): se vuelve a entrar en él; si no, el
        // mapa se queda sin caminos.
        let pendiente = (!run.node_done).then(|| match &run.en_curso {
            Some(e) => NodeAction::Combat(e.clone()),
            None => match run.act.map.node(run.position.0, run.position.1).map(|n| n.kind) {
                Some(rogue_core::map::NodeType::Rest) => NodeAction::Rest,
                Some(rogue_core::map::NodeType::Shop) => NodeAction::Shop,
                Some(rogue_core::map::NodeType::Treasure) => NodeAction::Treasure,
                _ => NodeAction::Event,
            },
        });
        d.run = Some(run);
        d.phase = Phase::Map;
        d.apply_relics();
        d.apply_passive();
        d.apply_flasks();
        if let Some(a) = pendiente {
            crate::log(&format!("autoinicio: se retoma el nodo sin resolver ({a:?})"));
            d.enter_action(a);
        }
    }
}

//! Motor de efectos de las bendiciones (lenguaje en rogue_core::efectos) y de los mutadores del boss.
//! Vive dentro del combate (CombatFx) y lo llama el director en cada fotograma.
//!
//! Detecta por su cuenta la rotura de postura, los estados, los frascos, los rodamientos, los
//! umbrales de vida y los golpes mortales; los golpes, el daño recibido y los desvíos se los pasa el
//! director. Es el único que escribe las filas dinámicas de daño y defensa (del jugador y
//! del boss).

use rogue_core::idioma::{t, tf};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use rogue_core::efectos::{Accion, Cuando, Efecto, Pasivo, Pila};
use rogue_core::rng::Rng;
use serde::Deserialize;

pub const SP_DMG_DYNAMIC: i32 = 79_619_000;
pub const SP_DEFENSE_DYNAMIC: i32 = 79_619_003;
pub const SP_BOSS_DEF: i32 = 79_619_030;
pub const SP_BOSS_ATK: i32 = 79_619_031;
pub const SP_PLAYER_HP: i32 = 79_619_032;
pub const SP_BOSS_HP: i32 = 79_619_033;
/// Llamas encima del rival mientras le dura una quemadura (solo se ve; tools/ModBuilder).
pub const SP_QUEMADO: i32 = 79_619_035;
/// Entidades de los aliados invocados por bendiciones.
const SUMMON_BASE: u32 = 79_760_000;
/// Nivel de ceniza espiritual +10 (vida y daño de los aliados).
pub const SUMMON_LEVEL: i32 = 290_010;

/// Proyectiles que caen del cielo (rayos): salen sobre el objetivo, no desdel jugador
/// (data/proyectiles.json, `en_objetivo`; 17-10: el Golpe de rayo caía encima del jugador).
static EN_OBJETIVO: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();

pub fn poner_en_objetivo(ids: Vec<i32>) {
    let _ = EN_OBJETIVO.set(ids);
}

/// Proyectiles que salen desde detrás del jugador (data/proyectiles.json, `atras`: metros), como en el
/// juego: la honda de roca lanza en arco y, saliendo por delante, de cerca pasaba por encima del rival
/// (23-10: las piedras del Bonk casi nunca daban).
static ATRAS: std::sync::OnceLock<HashMap<i32, f32>> = std::sync::OnceLock::new();

pub fn poner_atras(m: HashMap<i32, f32>) {
    let _ = ATRAS.set(m);
}

/// Proyectiles con daño de verdad (data/balas_reales.json, lo escribe tools/ModBuilder): de cada proyectil
/// visual, la cadena del hechizo original en escalones de daño fijo (el k-ésimo, `referencia` ×
/// `factor0` × `razon`^k, en `primero` + k × `paso`). El juego pone la defensa, la absorción de cada
/// elemento, los estados, los números y la barra (23-10: usar la lógica del juego).
#[derive(Deserialize, Clone, Debug)]
pub struct BalaReal {
    primero: i32,
    paso: i32,
    escalones: i32,
    factor0: f64,
    razon: f64,
    referencia: f64,
}

static BALAS_REALES: std::sync::OnceLock<(i32, HashMap<i32, BalaReal>)> = std::sync::OnceLock::new();

/// Hechizo de origen de los proyectiles con daño (sin él, el juego no aplica su ataque) y sus escalones.
pub fn poner_balas_reales(origen: i32, balas: HashMap<i32, BalaReal>) {
    let _ = BALAS_REALES.set((origen, balas));
}

/// El escalón de `bala` más cercano a ese daño fijo: (id, daño fijo de ese escalón).
fn escalon(bala: i32, crudo: f64) -> Option<(i32, f64)> {
    let b = BALAS_REALES.get()?.1.get(&bala)?;
    if b.referencia <= 0.0 {
        return None;
    }
    let k = ((crudo / (b.referencia * b.factor0)).max(1e-6).ln() / b.razon.ln()).round().clamp(0.0, (b.escalones - 1) as f64);
    Some((b.primero + k as i32 * b.paso, b.referencia * b.factor0 * b.razon.powf(k)))
}

/// Golpe invisible con daño de verdad (data/proyectiles.json): el daño de las bendiciones sin proyectil
/// (Vulnerable, púas, quemaduras, marca, bomba...), sobre el rival.
const GOLPE_INVISIBLE: i32 = 79_600_054;
/// Como mucho un golpe invisible cada tanto: lo que se junta entre medias va en el siguiente.
const CADA_GOLPE_INVISIBLE: Duration = Duration::from_millis(500);

/// Proporción de partida entre el daño que llega y el daño fijo (la defensa del rival); el motor la
/// corrige con cada impacto contra cada rival.
const RAZON_INICIAL: f64 = 0.5;
/// Tras disparar, cuánto se espera el impacto (para medir lo que quitó).
const VENTANA_IMPACTO: Duration = Duration::from_millis(1800);

/// ¿Daño de verdad (del juego)? En solitario, en el juego del anfitrión y en el del invitado (24-10:
/// sus proyectiles también, como los del anfitrión). Al invitado sus proyectiles le dan a su copia de los
/// rivales, que son del anfitrión: el juego manda ese daño como el de sus golpes. Si en su juego varias
/// esperas seguidas no ven bajar nada, no llega: vuelve el daño de siempre (lo aplica el anfitrión con
/// `Suceso::Danio`) el resto de la sesión (INVITADO_REAL).
fn danio_real() -> bool {
    BALAS_REALES.get().is_some()
        && (!crate::coop::hay_companeros() || crate::coop::soy_anfitrion() || INVITADO_REAL.load(std::sync::atomic::Ordering::Relaxed) != INVITADO_NO)
}

/// Invitado: ¿llega el daño de sus proyectiles de verdad? Probando, sí o no (danio_real).
static INVITADO_REAL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(INVITADO_PROBANDO);
const INVITADO_PROBANDO: u8 = 0;
const INVITADO_SI: u8 = 1;
const INVITADO_NO: u8 = 2;
/// Esperas seguidas sin ver bajar nada antes de darlo por perdido.
static INVITADO_CEROS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
const INVITADO_CEROS_MAX: u8 = 3;

/// Dispara el proyectil `id` (el visual) y, a los compañeros, su copia visual. `real`: en este juego
/// sale ese otro (un escalón con daño de verdad), con el hechizo de origen.
fn disparar(id: i32, objetivo: Option<u32>, desde: Option<[f32; 3]>, real: Option<i32>) {
    let desde = if EN_OBJETIVO.get().is_some_and(|v| v.contains(&id)) {
        objetivo.and_then(crate::chr_pos).map(|p| [p[0], p[1] + 0.1, p[2]]).or(desde)
    } else {
        desde
    };
    let _ = match real {
        Some(r) => crate::spawn_bullet_origen(r, objetivo, desde, [-1, BALAS_REALES.get().map(|b| b.0).unwrap_or(-1), -1]),
        None => crate::spawn_bullet_at(id, objetivo, desde),
    };
    if crate::coop::hay_companeros()
        && let Some(d) = desde.or_else(|| crate::player_pos().map(|p| [p[0], p[1] + 1.2, p[2]]))
    {
        crate::coop::suceso(rogue_core::coop::Suceso::Bala { id, objetivo, desde: d });
    }
}

/// De dónde sale el proyectil k de n contra el objetivo: medio metro por delante del personaje, hacia
/// el rival, a la altura del hombro (desde el centro del cuerpo parecía que salían de dentro, el jugador
/// 17-10) y, si son varios, en abanico a 0,6 m uno de otro de lado (del mismo punto se verían como uno).
fn salida(id: i32, k: u32, n: u32, objetivo: Option<u32>) -> Option<[f32; 3]> {
    let p = crate::player_pos()?;
    let t = objetivo.and_then(crate::chr_pos)?;
    let (dx, dz) = (t[0] - p[0], t[2] - p[2]);
    let l = (dx * dx + dz * dz).sqrt();
    if l < 0.01 {
        return None;
    }
    let (fx, fz) = (dx / l, dz / l);
    let o = (k as f32 - (n - 1) as f32 / 2.0) * 0.6;
    let delante = match ATRAS.get().and_then(|m| m.get(&id)) {
        Some(&a) => -a,
        None => 0.5_f32.min(l * 0.5),
    };
    Some([p[0] + fx * delante - fz * o, p[1] + 1.35 + 0.15 * (k % 2) as f32, p[2] + fz * delante + fx * o])
}

/// Mutador del boss (data/mutadores.json): lo que pagas en algunos eventos y en las ascensiones.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Mutador {
    pub id: String,
    pub nombre: String,
    pub texto: String,
    /// "plata", "oro" o "prismatico".
    pub nivel: String,
    /// Vida máxima del boss (multiplicador).
    pub vida: f64,
    /// Daño del boss (multiplicador).
    pub danio: f64,
    /// Velocidad de animación del boss.
    pub velocidad: f64,
    /// Se cura esta fracción de su vida cada 5 s.
    pub regen: f64,
    /// Por debajo de [umbral]: velocidad y daño [×vel, ×daño].
    pub furia: Option<[f64; 3]>,
    /// Una vez, al bajar de [umbral], se cura [fracción].
    pub segundo_aliento: Option<[f64; 2]>,
    /// Te devuelve esta fracción de cada golpe que le das.
    pub espinas: f64,
    /// Recibe menos daño (multiplicador de daño recibido).
    pub defensa: f64,
    /// Tamaño del boss (multiplicador).
    pub tamano: f64,
}

struct Slot {
    /// Bendición de la que viene (para las estadísticas).
    src: String,
    ef: Efecto,
    /// Pasivo de la misma bendición (para sus pilas).
    count: u32,
    last: Option<Instant>,
    used: bool,
    /// Para los umbrales: ya disparado en este cruce.
    armed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Temp {
    Danio,
    Defensa,
    Velocidad,
    BossDebil,
    BossFlojo,
    Robo,
    BossLento,
}

/// Estados del juego en el orden del módulo de resistencias: nombre y color de su barrita.
const ESTADOS: [(&str, [u8; 3]); 7] = [
    ("Poison", [150, 172, 64]),
    ("Scarlet Rot", [206, 104, 56]),
    ("Bleed", [196, 48, 48]),
    ("Death Blight", [150, 140, 150]),
    ("Frostbite", [150, 196, 232]),
    ("Sleep", [160, 150, 210]),
    ("Madness", [232, 184, 64]),
];

/// Un estado o efecto del rival bajo su barra.
#[derive(Clone, Debug)]
pub struct EstadoRival {
    /// Estado del juego (su icono, en el orden de `ESTADOS`) o, si es cosa del motor, la bendición
    /// que lo puso (su icono).
    pub estado: Option<usize>,
    pub src: String,
    pub nombre: String,
    pub color: [u8; 3],
    /// Barrita: acumulación o tiempo que le queda (0..1).
    pub lleno: f32,
    /// Segundos desde que saltó (destello); f32::MAX si no ha saltado.
    pub destello: f32,
    /// Al lado del nombre: pilas o segundos.
    pub texto: String,
}

/// Lo que se ve del rival además de su vida.
#[derive(Clone, Debug, Default)]
pub struct Rival {
    pub estados: Vec<EstadoRival>,
    pub marcas: Vec<(f32, bool)>,
}

/// Lo que el director le cuenta al motor en cada fotograma.
#[derive(Default)]
pub struct Entrada {
    pub ent: u32,
    pub hit: i32,
    /// Lo que ha perdido el rival y no es un golpe tuyo (proyectiles, estados, compañeros).
    pub otro: i32,
    pub taken: i32,
    pub deflect: bool,
    pub php: i32,
    pub pmax: i32,
    pub bhp: i32,
    pub bmax: i32,
}

/// Estado de una bendición para la tira del HUD (rogue_ui::tira): sin texto, solo anillo, pilas y luz.
#[derive(Clone, Debug, Default)]
pub struct HudBendicion {
    pub src: String,
    pub anillo: Option<f32>,
    pub lista: bool,
    pub gastada: bool,
    pub pilas: Option<(u32, u32)>,
    /// Segundos desde la última vez que saltó.
    pub destello: f32,
    /// El estado en palabras, para la pausa ("4 / 6 hits", "Back in 6 s", "Stacks 6 / 10"...).
    pub estado: String,
}

/// Lo que cuenta un contador según su disparador ("4 / 6 hits").
fn unidad(c: Cuando) -> &'static str {
    match c {
        Cuando::Golpe => t("hits"),
        Cuando::Recibir => t("hits taken"),
        Cuando::Desvio => t("deflects"),
        Cuando::Rodar => t("rolls"),
        Cuando::Ceniza => t("Ashes of War"),
        Cuando::Estado => t("ailments"),
        Cuando::Frasco => t("flasks"),
        Cuando::Cargado => t("charged attacks"),
        _ => "",
    }
}

/// Lo que el motor le devuelve: vida del boss tras sus acciones, curación hecha y peticiones para la run.
#[derive(Default)]
pub struct Salida {
    pub bhp: i32,
    pub php: i32,
    pub runas: u32,
    pub run: Vec<String>,
}

impl std::fmt::Debug for Motor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Motor({} efectos)", self.slots.len())
    }
}

/// Lo que tarda un proyectil de bendición en llegar al enemigo (a media distancia).
const IMPACTO: Duration = Duration::from_millis(550);
/// Lo mínimo entre dos disparos de una bendición de «en cada golpe» (un espadazo).
const GOLPE_MIN: Duration = Duration::from_millis(300);
/// Gastos de PF que siguen siendo la misma ceniza de guerra (sus tramos de un mismo uso).
const CENIZA_MISMA: Duration = Duration::from_millis(1500);

pub struct Motor {
    /// Cooperativo, en el juego del invitado: ataque y defensa del tramo del combate. Los rivales son
    /// copias del anfitrión y los SpEffects que les pone el invitado (tramo, ataque y defensa de los
    /// mutadores) no se quedan: lo que deberían llevar se le aplica a él (recibe más daño y hace menos).
    pub invitado: Option<(f64, f64)>,
    /// Parte de los rivales de la oleada que NO llevan el tramo en este juego (0 a 1). A veces sí se
    /// queda en las copias y entonces contaba dos veces: el rival ya pegaba con su tramo y además se le
    /// aplicaba al invitado (19-10: al amigo le mataban de un golpe y a él no). Se compensa solo
    /// esta parte. Lo pone el director en cada fotograma.
    pub invitado_falta: f64,
    slots: Vec<Slot>,
    pasivos: Vec<Pasivo>,
    pilas_def: HashMap<String, Pila>,
    pilas: HashMap<String, (u32, Option<Instant>)>,
    /// Efectos temporales: tipo, valor, hasta, desde y bendición que los puso.
    temps: Vec<(Temp, f64, Instant, Instant, String)>,
    invuln_until: Option<Instant>,
    marca: Option<(f64, Instant, f64)>,
    marca_src: String,
    burn_src: String,
    robo_src: String,
    /// Bendición de cada pasivo (mismo orden que `pasivos`).
    pasivo_src: Vec<String>,
    /// Daño al boss y curación por fuente (id de bendición) en este combate.
    pub stats_dmg: HashMap<String, f64>,
    pub stats_heal: HashMap<String, f64>,
    aplazado: Vec<(Instant, i32)>,
    /// Daño de los proyectiles de las bendiciones: llega cuando el proyectil alcanza al enemigo, no
    /// al lanzarlo (cuándo, cuánto y de qué bendición).
    impactos: Vec<(Instant, i32, String)>,
    /// Proyectiles de una misma acción que salen después del primero: (cuándo, proyectil, objetivo,
    /// cuál, cuántos).
    salidas: Vec<(Instant, i32, Option<u32>, u32, u32, Option<i32>)>,
    /// Daño de verdad: proporción entre lo que llega y el daño fijo, por rival; y los disparos cuyo
    /// impacto se está midiendo (cuándo, rival, daño fijo) con lo que ha quitado hasta ahora.
    razon_danio: HashMap<(u32, i32), f64>,
    midiendo: Vec<(Instant, u32, i32, f64)>,
    medido: f64,
    medida_desde: Option<Instant>,
    /// Daño sin proyectil que espera al siguiente golpe invisible (y de qué bendición), y el último.
    pendiente: f64,
    pendiente_src: String,
    golpe_at: Option<Instant>,

    revive: Option<(Instant, f64)>,
    /// Quemaduras activas: (fracción por segundo, hasta cuándo).
    burns: Vec<(f64, Instant)>,
    burn_acc: f64,
    /// Golpes recientes (para Ráfaga), PF del fotograma anterior, frascos bebidos y depósitos.
    recent_hits: Vec<(Instant, i32)>,
    /// Media de tus golpes en este combate (vara de medir de `danio_golpes`).
    media_golpe: f64,
    /// Golpe que se supone antes de tus primeros golpes (por acto; director, start_fx).
    pub golpe_defecto: f64,
    /// Para la tira del HUD: última vez que saltó cada bendición, sus bufos en marcha (bendición,
    /// desde, hasta) y de qué bendición es cada pila.
    disparos: HashMap<String, Instant>,
    bufos: Vec<(String, Instant, Instant)>,
    pila_src: HashMap<String, String>,
    last_fp: Option<i32>,
    fp_spent: i32,
    /// Hasta cuándo un gasto de PF sigue siendo la misma ceniza (las hay que cobran en varios tramos).
    ceniza_hasta: Option<Instant>,
    drank: bool,
    puas: f64,
    bomba: f64,
    /// Aliados invocados: (entidad, hasta cuándo, ya reforzado).
    summons: Vec<(u32, Instant, bool)>,
    /// De qué bendición sale cada aliado (su id), para su barra (director: aliados).
    summon_src: HashMap<u32, String>,
    next_summon: u32,
    last_frame: Instant,
    mutadores: Vec<Mutador>,
    mut_segundo_used: bool,
    mut_last_regen: Instant,
    // Valores base (gracia y bendiciones antiguas) y aplicados.
    base_dmg: f64,
    base_def: f64,
    /// Vida del enemigo según el acto y el tipo de combate (data/escalado.json, vida).
    base_boss_hp: f64,
    run_counters: HashMap<String, f64>,
    applied: [f64; 4],
    reapply: [bool; 4],
    speed_applied: f64,
    boss_speed_applied: f64,
    // Detección.
    start: Instant,
    /// Tiempo pasado con la pausa abierta: el reloj del motor (`ahora`) no corre en ella. Si corriera,
    /// los enfriamientos (Veil) y lo de «cada X s» se recargarían pausando.
    pausado: Duration,
    /// Pausa abierta desde: mientras dura, el reloj se queda parado ahí (si no, la pausa enseñaría
    /// los segundos y las barras avanzando, y al volver saltarían atrás).
    pausa: Option<Instant>,
    /// Si ya pasó el primer fotograma (ahí se pone `start`).
    en_marcha: bool,
    last_poise: Option<f32>,
    resist_max: Option<Vec<i32>>,
    /// Resistencia que le queda al rival a cada estado (baja con cada golpe que lo acumula y se
    /// recupera sola hasta el tope; al aparecer empieza a medias), cuándo bajó y cuándo saltó.
    resist_cur: Vec<i32>,
    /// Rival del que se vigilan postura y estados: si cambia (otra oleada, otra fase), se empieza de cero.
    vigilado: Option<u32>,
    /// Últimos golpes de verdad, para el golpe medio (mediana).
    ultimos_golpes: Vec<f64>,
    acumula_at: [Option<Instant>; 7],
    proc_at: [Option<Instant>; 7],
    last_rolling: bool,
    last_taken_at: Option<Instant>,
    no_dead_on: bool,
    rng: Rng,
    ent: u32,
    /// La vida del combate va por SpEffect (SP_BOSS_HP): también se pone a cada rival de un grupo.
    vida_rivales: bool,
    filled: bool,
    /// Tamaño del jugador (pasivos y acciones) y del boss (mutadores).
    size: f64,
    boss_size: f64,
}

impl Motor {
    /// `efectos`: (efecto, pasivo de su bendición) de todas las bendiciones que tienes.
    pub fn new(
        efectos: Vec<(String, Efecto)>,
        pasivos: Vec<(String, Pasivo)>,
        mutadores: Vec<Mutador>,
        base_dmg: f64,
        base_def: f64,
        base_boss_hp: f64,
        run_counters: HashMap<String, f64>,
    ) -> Self {
        let (pasivo_src, pasivos): (Vec<String>, Vec<Pasivo>) = pasivos.into_iter().unzip();
        let pilas_def = pasivos.iter().flat_map(|p| p.pilas.iter().cloned()).map(|p| (p.id.clone(), p)).collect();
        let pila_src = pasivos.iter().zip(&pasivo_src).flat_map(|(p, src)| p.pilas.iter().map(move |d| (d.id.clone(), src.clone()))).collect();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Motor {
            invitado: None,
            invitado_falta: 1.0,
            slots: efectos.into_iter().map(|(src, ef)| Slot { src, ef, count: 0, last: None, used: false, armed: true }).collect(),
            marca_src: String::new(),
            burn_src: String::new(),
            robo_src: String::new(),
            pasivo_src,
            stats_dmg: HashMap::new(),
            stats_heal: HashMap::new(),
            pasivos,
            pilas_def,
            pilas: HashMap::new(),
            temps: vec![],
            invuln_until: None,
            marca: None,
            aplazado: vec![],
            impactos: vec![],
            salidas: vec![],
            razon_danio: HashMap::new(),
            midiendo: vec![],
            medido: 0.0,
            medida_desde: None,
            pendiente: 0.0,
            pendiente_src: String::new(),
            golpe_at: None,
            revive: None,
            burns: vec![],
            burn_acc: 0.0,
            recent_hits: vec![],
            media_golpe: 0.0,
            golpe_defecto: 0.0,
            disparos: HashMap::new(),
            bufos: vec![],
            pila_src,
            last_fp: None,
            fp_spent: 0,
            ceniza_hasta: None,
            drank: false,
            puas: 0.0,
            bomba: 0.0,
            summons: vec![],
            summon_src: HashMap::new(),
            next_summon: SUMMON_BASE,
            last_frame: Instant::now(),
            mutadores,
            mut_segundo_used: false,
            mut_last_regen: Instant::now(),
            base_dmg,
            base_def,
            base_boss_hp,
            run_counters,
            applied: [1.0; 4],
            reapply: [false; 4],
            speed_applied: 1.0,
            boss_speed_applied: 1.0,
            start: Instant::now(),
            pausado: Duration::ZERO,
            pausa: None,
            en_marcha: false,
            last_poise: None,
            resist_max: None,
            resist_cur: vec![],
            vigilado: None,
            ultimos_golpes: vec![],
            acumula_at: [None; 7],
            proc_at: [None; 7],
            last_rolling: false,
            last_taken_at: None,
            no_dead_on: false,
            rng: Rng::named(seed, "motor"),
            ent: 0,
            vida_rivales: false,
            filled: false,
            size: 1.0,
            boss_size: 1.0,
        }
    }

    /// Al empezar el combate (el boss ya existe y los dos tienen la vida llena): vida máxima del
    /// jugador y del boss, protección contra el golpe mortal y efectos de inicio.
    pub fn inicio(&mut self, ent: u32) {
        self.ent = ent;
        self.size = self.pasivos.iter().map(|p| p.tamano).product();
        self.boss_size = self.mutadores.iter().map(|m| if m.tamano > 0.0 { m.tamano } else { 1.0 }).product();
        let run_vida: f64 =
            self.pasivos.iter().flat_map(|p| p.run_vida.iter()).map(|(k, v)| 1.0 + v * self.run_counters.get(k).copied().unwrap_or(0.0)).product();
        let vida: f64 = self.pasivos.iter().map(|p| p.vida).product::<f64>() * run_vida;
        if (vida - 1.0).abs() > 0.001 && crate::set_speffect_max_hp_rate(SP_PLAYER_HP, vida as f32) {
            crate::player_speffect(SP_PLAYER_HP, true);
        }
        let bvida: f64 = self.base_boss_hp * self.mutadores.iter().map(|m| if m.vida > 0.0 { m.vida } else { 1.0 }).product::<f64>();
        // El daño de la ascensión va con la vida del combate, que llevan todos los rivales del grupo (en
        // SP_BOSS_ATK solo lo llevaba el que golpeas).
        let danio_todos: f64 = self.mutadores.iter().filter(|m| m.id == "ascension" && m.danio > 0.0).map(|m| m.danio).product();
        // En cooperativo también en el juego del invitado: es la vida máxima que ve en la barra.
        if ((bvida - 1.0).abs() > 0.001 || (danio_todos - 1.0).abs() > 0.001)
            && crate::set_speffect_max_hp_rate(SP_BOSS_HP, bvida as f32)
            && crate::set_speffect_attack_rate(SP_BOSS_HP, danio_todos as f32)
        {
            crate::apply_speffect_to(ent, SP_BOSS_HP);
            self.vida_rivales = true;
        }
        if self.slots.iter().any(|s| s.ef.cuando == Cuando::GolpeMortal) {
            self.no_dead_on = crate::set_player_no_dead(true);
        }
        self.start = self.ahora();
    }

    /// Daño de una bendición sin proyectil: al siguiente golpe invisible (lo pone el juego).
    fn quitar(&mut self, n: f64, src: &str) {
        self.pendiente += n;
        self.pendiente_src = src.to_string();
        *self.stats_dmg.entry(src.to_string()).or_insert(0.0) += n;
    }

    /// Tus últimos golpes de verdad (para seguir con ellos en el combate siguiente de la run).
    pub fn ultimos_golpes(&self) -> Vec<f64> {
        self.ultimos_golpes.clone()
    }

    /// Empieza con los golpes del combate anterior de la run: el golpe medio ya es el tuyo desde el
    /// primer disparo (23-10).
    pub fn seguir_golpes(&mut self, golpes: Vec<f64>) {
        if golpes.is_empty() {
            return;
        }
        let mut v = golpes.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        self.media_golpe = if v.len() < 3 { v[0] } else { v[v.len() / 2] };
        self.ultimos_golpes = golpes;
    }

    /// Aliados de las bendiciones que siguen ahí, por bendición (su id): para sus barras junto a las
    /// invocaciones (23-10: el de los primeros 30 s no salía).
    pub fn aliados(&self) -> Vec<(String, Vec<u32>)> {
        let mut v: Vec<(String, Vec<u32>)> = vec![];
        for s in &self.summons {
            let src = self.summon_src.get(&s.0).cloned().unwrap_or_default();
            match v.iter_mut().find(|x| x.0 == src) {
                Some(x) => x.1.push(s.0),
                None => v.push((src, vec![s.0])),
            }
        }
        v
    }

    /// Combates de grupo (docs/LISTAS_Y_COMBATES.md): otro rival con la vida del combate.
    /// Golpe medio del jugador en este combate (0 sin golpes aún).
    pub fn media_golpe(&self) -> f64 {
        self.media_golpe
    }

    /// Tamaño del jugador ahora (1 = normal).
    pub fn tamano(&self) -> f64 {
        self.size
    }

    /// Rival al que sigue el motor (el de la barra, los umbrales y los estados).
    pub fn rival_ent(&self) -> u32 {
        self.ent
    }

    /// ¿Lleva la vida del combate cada rival (SP_BOSS_HP)?
    pub fn vida_rivales(&self) -> bool {
        self.vida_rivales
    }

    pub fn sumar_rival(&self, ent: u32) {
        if self.vida_rivales {
            crate::apply_speffect_to(ent, SP_BOSS_HP);
        }
    }

    /// Combates de grupo: el rival pasa a ser al que golpeas. Sus resistencias y su postura se leen de
    /// cero: comparadas con las del anterior parecían estados que se le metían o le saltaban (la barra
    /// enseñaba putrefacción, sangrado y sueño de golpe y se disparaban las bendiciones de estado).
    pub fn cambiar_rival(&mut self, ent: u32) {
        // El rival de antes recupera su velocidad: frenado (Manos de la tierra, mutadores) se quedaba así
        // para siempre y, si moría, su animación de muerte iba a cámara lenta (19-10).
        if (self.boss_speed_applied - 1.0).abs() > 0.001 && self.ent != ent {
            crate::set_anim_speed(self.ent, 1.0);
            self.boss_speed_applied = 1.0;
        }
        self.ent = ent;
        self.resist_max = None;
        self.resist_cur.clear();
        self.acumula_at = [None; 7];
        self.proc_at = [None; 7];
        self.last_poise = None;
    }

    /// Reloj del combate: el real menos lo que se ha estado en pausa.
    fn ahora(&self) -> Instant {
        let n = self.pausa.unwrap_or_else(Instant::now);
        n.checked_sub(self.pausado).unwrap_or(n)
    }

    /// Se abre la pausa (un jugador): el reloj del combate se para aquí.
    pub fn pausar(&mut self) {
        self.pausa.get_or_insert_with(Instant::now);
    }

    /// La pausa ha durado `d`: no cuenta para nada del combate.
    pub fn tras_pausa(&mut self, d: Duration) {
        self.pausado += d;
        self.pausa = None;
    }

    pub fn filled(&self) -> bool {
        self.filled
    }

    /// Tras `inicio`, cuando las vidas máximas nuevas ya cuentan: llenar las dos (una vez).
    pub fn llenar(&mut self) {
        if self.filled {
            return;
        }
        self.filled = true;
        crate::fill_hp(self.ent);
        if let Some((_, max)) = crate::player_hp_max() {
            crate::set_player_hp(max);
        }
    }

    pub fn fin(&mut self) {
        let ent = self.ent;
        for s in self.summons.drain(..) {
            crate::set_chr_hp(s.0, 0);
        }
        if self.no_dead_on {
            crate::set_player_no_dead(false);
            self.no_dead_on = false;
        }
        for sp in [SP_DMG_DYNAMIC, SP_DEFENSE_DYNAMIC, SP_PLAYER_HP] {
            crate::player_speffect(sp, false);
        }
        for sp in [SP_BOSS_DEF, SP_BOSS_ATK, SP_BOSS_HP] {
            crate::remove_speffect_from(ent, sp);
        }
        if self.speed_applied != 1.0 {
            crate::set_anim_speed(0, 1.0);
        }
        if (self.boss_speed_applied - 1.0).abs() > 0.001 {
            crate::set_anim_speed(ent, 1.0);
        }
        if (self.size - 1.0).abs() > 0.001 {
            crate::set_chr_scale(0, 1.0);
        }
    }

    /// Lo que se ve bajo la barra del rival (estilo Nightreign): sus estados y lo que le has metido, y
    /// en la barra, los umbrales de tus bendiciones (fracción de su vida; `true` = remate).
    pub fn rival(&self) -> Rival {
        let now = self.ahora();
        let mut estados = vec![];
        // Acumulaciones del juego: se ven mientras se le van metiendo (6 s desde la última) y un
        // momento después de saltar. Lo acumulado es lo que le falta de resistencia.
        let max = self.resist_max.clone().unwrap_or_default();
        for (i, (nombre, color)) in ESTADOS.iter().enumerate() {
            let (cur, tope) = (self.resist_cur.get(i).copied().unwrap_or(0), max.get(i).copied().unwrap_or(0));
            let destello = self.proc_at[i].map(|t| now.duration_since(t).as_secs_f32()).unwrap_or(f32::MAX);
            let reciente = self.acumula_at[i].is_some_and(|t| now.duration_since(t).as_secs_f32() < 6.0);
            // Inmune (sin tope o tope enorme): nunca se enseña.
            if tope <= 0 || tope >= 999 || (!reciente && destello > 1.5) {
                continue;
            }
            let lleno = if destello < 1.5 { 1.0 } else { (1.0 - cur as f32 / tope as f32).clamp(0.0, 1.0) };
            estados.push(EstadoRival { estado: Some(i), src: String::new(), nombre: t(nombre).into(), color: *color, lleno, destello, texto: String::new() });
        }
        // Lo que pone el motor.
        let quemaduras: Vec<&(f64, Instant)> = self.burns.iter().filter(|b| now < b.1).collect();
        if let Some(fin) = quemaduras.iter().map(|b| b.1).max() {
            let lleno = (fin.duration_since(now).as_secs_f32() / 5.0).clamp(0.0, 1.0);
            let texto = if quemaduras.len() > 1 { format!("x{}", quemaduras.len()) } else { String::new() };
            estados.push(EstadoRival {
                estado: None,
                src: self.burn_src.clone(),
                nombre: t("Burn").into(),
                color: [232, 128, 52],
                lleno,
                destello: f32::MAX,
                texto,
            });
        }
        if let Some((_, fin, acc)) = self.marca
            && now < fin
        {
            // La marca guarda daño y lo suelta al acabar: cuánto lleva y cuánto le falta.
            let texto = tf("{} in {} s", &[&(acc as i32), &fin.duration_since(now).as_secs_f32().ceil()]);
            estados.push(EstadoRival {
                estado: None,
                src: self.marca_src.clone(),
                nombre: t("Marked").into(),
                color: [196, 96, 78],
                lleno: 1.0,
                destello: f32::MAX,
                texto,
            });
        }
        for (k, nombre, color) in [
            (Temp::BossDebil, t("Exposed"), [214, 186, 120]),
            (Temp::BossFlojo, t("Weakened"), [170, 160, 140]),
            (Temp::BossLento, t("Slowed"), [140, 170, 200]),
        ] {
            if let Some(t) = self.temps.iter().filter(|t| t.0 == k && now < t.2).max_by_key(|t| t.2) {
                let total = t.2.duration_since(t.3).as_secs_f32().max(0.01);
                estados.push(EstadoRival {
                    estado: None,
                    src: t.4.clone(),
                    nombre: nombre.into(),
                    color,
                    lleno: (t.2.duration_since(now).as_secs_f32() / total).clamp(0.0, 1.0),
                    destello: f32::MAX,
                    texto: String::new(),
                });
            }
        }
        // Umbrales: remate, «con el rival por debajo de X» y disparadores de su vida.
        let mut marcas: Vec<(f32, bool)> = vec![];
        for p in &self.pasivos {
            if p.remate > 0.0 {
                marcas.push((p.remate as f32, true));
            }
            if let Some([u, _]) = p.danio_boss_vida_menor {
                marcas.push((u as f32, false));
            }
        }
        for s in &self.slots {
            if s.ef.cuando == Cuando::BossVidaBaja && s.ef.valor > 0.0 {
                marcas.push((s.ef.valor as f32, false));
            }
            if s.ef.si_boss_vida_menor > 0.0 {
                marcas.push((s.ef.si_boss_vida_menor as f32, false));
            }
        }
        marcas.retain(|m| m.0 > 0.0 && m.0 < 1.0);
        Rival { estados, marcas }
    }

    /// Estado de las bendiciones para la tira del HUD, en el orden en que las tienes. Solo las que
    /// tienen algo que contar: contador, recarga, temporizador, una vez por combate, pilas o bufo.
    pub fn hud(&self) -> Vec<HudBendicion> {
        let now = self.ahora();
        let mut v: Vec<HudBendicion> = vec![];
        fn entrada<'a>(v: &'a mut Vec<HudBendicion>, src: &str) -> &'a mut HudBendicion {
            match v.iter().position(|h| h.src == src) {
                Some(i) => &mut v[i],
                None => {
                    v.push(HudBendicion { src: src.to_string(), destello: 99.0, ..Default::default() });
                    v.last_mut().unwrap()
                }
            }
        }
        let desde = |t: Option<Instant>, d: Instant| t.map_or(now.duration_since(d), |t| now.duration_since(t)).as_secs_f64();
        for s in &self.slots {
            let ef = &s.ef;
            let estado: Option<(Option<f32>, bool, bool, String)> = if ef.una_vez {
                let texto = if s.used { t("Used this fight").to_string() } else { t("Ready. Once per fight").to_string() };
                Some((None, !s.used, s.used, texto))
            } else if ef.cada > 1 {
                let n = s.count % ef.cada;
                Some((Some(n as f32 / ef.cada as f32), false, false, format!("{n} / {} {}", ef.cada, unidad(ef.cuando)).trim_end().to_string()))
            } else if ef.cuando == Cuando::Segundos && ef.valor > 0.0 {
                let t = desde(s.last, self.start);
                Some((Some((t / ef.valor).min(1.0) as f32), false, false, tf("Next in {} s", &[&(ef.valor - t).max(0.0).ceil()])))
            } else if ef.enfriamiento >= 2.0 {
                let t = s.last.map_or(ef.enfriamiento, |t| now.duration_since(t).as_secs_f64());
                let p = (t / ef.enfriamiento).min(1.0) as f32;
                let texto = if p >= 1.0 { rogue_core::idioma::t("Ready").to_string() } else { tf("Back in {} s", &[&(ef.enfriamiento - t).ceil()]) };
                Some((Some(p), p >= 1.0, false, texto))
            } else if ef.si_sin_danio > 0.0 {
                let t = self.last_taken_at.map_or(ef.si_sin_danio, |t| now.duration_since(t).as_secs_f64());
                let p = (t / ef.si_sin_danio).min(1.0) as f32;
                let texto = if p >= 1.0 {
                    rogue_core::idioma::t("Ready").to_string()
                } else {
                    tf("Ready after {} s without being hit", &[&(ef.si_sin_danio - t).ceil()])
                };
                Some((Some(p), p >= 1.0, false, texto))
            } else {
                None
            };
            if let Some((anillo, lista, gastada, texto)) = estado {
                let h = entrada(&mut v, &s.src);
                if h.anillo.is_none() && !h.lista && !h.gastada {
                    (h.anillo, h.lista, h.gastada, h.estado) = (anillo, lista, gastada, texto);
                }
            }
        }
        for (id, (n, _)) in &self.pilas {
            if let (Some(src), Some(d)) = (self.pila_src.get(id), self.pilas_def.get(id)) {
                let h = entrada(&mut v, src);
                h.pilas = Some((*n, d.max));
                h.estado = tf("Stacks {} / {}", &[&n, &d.max]);
            }
        }
        // Bufos en marcha: el anillo es lo que les queda (los de todo el combate, solo encendidos).
        for (src, ini, fin) in &self.bufos {
            if now < *fin {
                let total = fin.duration_since(*ini).as_secs_f32();
                let h = entrada(&mut v, src);
                h.lista = true;
                h.anillo = (total < 60.0).then(|| fin.duration_since(now).as_secs_f32() / total.max(0.01));
                h.estado =
                    if total < 60.0 { tf("Active, {} s left", &[&fin.duration_since(now).as_secs_f32().ceil()]) } else { t("Active this fight").to_string() };
            }
        }
        for h in v.iter_mut() {
            if let Some(t) = self.disparos.get(&h.src) {
                h.destello = now.duration_since(*t).as_secs_f32();
            }
        }
        v
    }

    fn pila(&self, f: impl Fn(&Pila) -> f64) -> f64 {
        self.pilas.iter().filter_map(|(id, (n, _))| self.pilas_def.get(id).map(|d| f(d) * *n as f64)).sum()
    }

    fn temp(&self, k: Temp) -> f64 {
        self.temps.iter().filter(|t| t.0 == k).map(|t| t.1).product()
    }

    /// Un fotograma de combate.
    pub fn tick(&mut self, e: &Entrada) -> Salida {
        let now = self.ahora();
        // El reloj del combate empieza en el primer fotograma del motor, no en `inicio`: entre los dos
        // el rival rellena su vida un par de segundos, y lo de «al empezar» se quedaba sin saltar.
        if !self.en_marcha {
            self.en_marcha = true;
            self.start = now;
        }
        let mut out = Salida { bhp: e.bhp, php: e.php, ..Default::default() };
        let (ent, pmax, bmax) = (e.ent, e.pmax.max(1), e.bmax.max(1));
        // Caducan pilas y temporales.
        self.pilas.retain(|_, (n, until)| *n > 0 && until.is_none_or(|t| now < t));
        self.temps.retain(|t| now < t.2);
        self.bufos.retain(|b| now < b.2);

        // --- Señales que detecta el motor ---
        let mut fired: Vec<Cuando> = vec![];
        if e.hit > 0 {
            fired.push(Cuando::Golpe);
        }
        // Para «sin recibir daño» al recibir: cuenta el golpe anterior, no este.
        let taken_prev = self.last_taken_at;
        if e.taken > 0 {
            crate::log(&format!("motor: recibes {} ({} / {})", e.taken, e.php, e.pmax));
            fired.push(Cuando::Recibir);
            self.last_taken_at = Some(now);
        }
        if e.deflect {
            fired.push(Cuando::Desvio);
        }
        // Otro rival (oleada nueva, segunda fase): sus estados y su postura no se comparan con los del
        // anterior (23-10: al cambiar Godfrey de fase, seis estados «saltaron» a la vez y
        // Vulnerable hizo 18 golpes de daño).
        if self.vigilado != Some(ent) {
            self.vigilado = Some(ent);
            self.resist_max = None;
            self.resist_cur.clear();
            self.last_poise = None;
        }
        if let Some((p, max)) = crate::chr_poise(ent) {
            if self.last_poise.is_some_and(|lp| lp > 0.0) && p <= 0.0 && max > 0.0 {
                fired.push(Cuando::Postura);
            }
            self.last_poise = Some(p);
        }
        let mut procs: Vec<usize> = vec![];
        if let Some((cur, max)) = crate::chr_resist(ent) {
            let max = max.to_vec();
            if let Some(prev) = self.resist_max.as_ref() {
                procs = prev.iter().zip(&max).enumerate().filter(|(_, (a, b))| b > a).map(|(i, _)| i).collect();
            }
            for &i in &procs {
                self.proc_at[i] = Some(now);
            }
            for (i, (&c, p)) in cur.iter().zip(&self.resist_cur).enumerate() {
                if c < *p && i < 7 {
                    self.acumula_at[i] = Some(now);
                }
            }
            self.resist_cur = cur.to_vec();
            if !procs.is_empty() {
                crate::log(&format!("motor: estado {procs:?} salta en {ent}"));
                fired.push(Cuando::Estado);
            }
            self.resist_max = Some(max);
        }
        // Ráfaga: daño de los últimos 2,5 s.
        if e.hit > 0 {
            self.recent_hits.push((now, e.hit));
            // Media de los golpes de verdad: los muy pequeños (proyectiles, quemaduras) no cuentan.
            // En cooperativo, un golpe no sube la media más de 2,5 veces de una vez (ver combate.rs).
            let golpe = if self.media_golpe > 0.0 && crate::coop::hay_companeros() { (e.hit as f64).min(self.media_golpe * 2.5) } else { e.hit as f64 };
            // Mediana de los últimos 8 golpes (con menos de 3, el menor): un golpe enorme (el primero con
            // Primer corte, un crítico) no infla el daño de los proyectiles el resto del combate (el jugador,
            // 23-10: cada bola de Fuegos fatuos hizo unos 6.900 contra Godfrey).
            if self.media_golpe <= 0.0 || golpe >= self.media_golpe * 0.3 {
                self.ultimos_golpes.push(golpe);
                if self.ultimos_golpes.len() > 8 {
                    self.ultimos_golpes.remove(0);
                }
                let mut v = self.ultimos_golpes.clone();
                v.sort_by(|a, b| a.total_cmp(b));
                self.media_golpe = if v.len() < 3 { v[0] } else { v[v.len() / 2] };
            }
        }
        self.recent_hits.retain(|h| now.duration_since(h.0) < Duration::from_millis(2500));
        let burst: i32 = self.recent_hits.iter().map(|h| h.1).sum();
        // Frasco: subida de vida grande que no ha puesto nadie más (la del director llega ya descontada).
        if e.taken < 0 && -e.taken as f64 >= pmax as f64 * 0.2 {
            fired.push(Cuando::Frasco);
            self.drank = true;
            if self.pasivos.iter().any(|p| p.frascos_no_curan) {
                crate::set_player_hp(e.php + e.taken);
                out.php = e.php + e.taken;
            }
        }
        // Ceniza de guerra o hechizo: gasto de PF.
        self.fp_spent = 0;
        if let Some(fp) = crate::player_fp() {
            if let Some(prev) = self.last_fp
                && fp < prev
                && prev - fp >= 5
            {
                self.fp_spent = prev - fp;
                // Una ceniza, un disparo: el Unsheathe cobra al envainar y otra vez en el ataque que
                // sigue, y cada tramo contaba como otra ceniza (bolas y daño repetidos).
                if self.ceniza_hasta.is_none_or(|t| now >= t) {
                    fired.push(Cuando::Ceniza);
                    self.ceniza_hasta = Some(now + CENIZA_MISMA);
                }
            }
            self.last_fp = Some(fp);
        }
        if crate::pad::take_charged() {
            fired.push(Cuando::Cargado);
        }
        let rolling = crate::player_anim().is_some_and(|a| (27_000..28_000).contains(&(a % 1_000_000)));
        if rolling && !self.last_rolling {
            fired.push(Cuando::Rodar);
        }
        self.last_rolling = rolling;
        if self.no_dead_on && out.php <= 1 {
            fired.push(Cuando::GolpeMortal);
            crate::log("motor: golpe mortal evitado");
            crate::set_player_no_dead(false);
            self.no_dead_on = false;
        }

        // --- Pasivos por golpe ---
        // Quemaduras: daño continuo acumulado (se aplica en enteros).
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        self.burns.retain(|b| now < b.1);
        let burn: f64 = self.burns.iter().map(|b| b.0).sum();
        if burn > 0.0 {
            self.burn_acc += burn * bmax as f64 * dt;
            let n = self.burn_acc.floor();
            if n >= 1.0 {
                self.burn_acc -= n;
                if danio_real() {
                    // (Las estadísticas, justo debajo.)
                    self.pendiente += n;
                } else {
                    out.bhp -= n as i32;
                }
                *self.stats_dmg.entry(self.burn_src.clone()).or_insert(0.0) += n;
            }
        }
        if e.hit > 0 {
            let temp_robo: f64 = self.temps.iter().filter(|t| t.0 == Temp::Robo).map(|t| t.1).sum();
            let robo: f64 = self.pasivos.iter().map(|p| p.robo).sum::<f64>() + temp_robo;
            if robo > 0.0 {
                let n = (e.hit as f64 * robo) as i32;
                crate::heal_player_by(n);
                out.php = (out.php + n).min(pmax);
                let src = self.pasivos.iter().position(|p| p.robo > 0.0).map(|i| self.pasivo_src[i].clone()).unwrap_or_else(|| self.robo_src.clone());
                *self.stats_heal.entry(src).or_insert(0.0) += n as f64;
            }
            if let Some((frac, until, acc)) = self.marca.as_mut()
                && now < *until
            {
                *acc += e.hit as f64 * *frac;
            }
            if let Some((until, frac)) = self.revive.take()
                && now < until
            {
                let n = (pmax as f64 * frac) as i32;
                crate::heal_player_by(n);
                out.php = (out.php + n).min(pmax);
                crate::log("motor: vuelves");
            }
            // Espinas del boss (mutador).
            let esp: f64 = self.mutadores.iter().map(|m| m.espinas).sum();
            if esp > 0.0 {
                out.php = (out.php - (e.hit as f64 * esp) as i32).max(1);
                crate::set_player_hp(out.php);
            }
        }
        if e.taken > 0 {
            let parada = self.pasivos.iter().map(|p| p.parada).fold(0.0, f64::max);
            if parada > 0.0 && self.rng.next_f64() < parada {
                let n = e.taken / 2;
                crate::heal_player_by(n);
                out.php = (out.php + n).min(pmax);
                if let Some(i) = self.pasivos.iter().position(|p| p.parada > 0.0) {
                    *self.stats_heal.entry(self.pasivo_src[i].clone()).or_insert(0.0) += n as f64;
                }
            }
        }
        // Marca cumplida: el boss recibe lo guardado.
        if let Some((_, until, acc)) = self.marca
            && now >= until
        {
            if danio_real() {
                self.pendiente += acc;
            } else {
                out.bhp -= acc as i32;
            }
            *self.stats_dmg.entry(self.marca_src.clone()).or_insert(0.0) += acc;
            self.marca = None;
            crate::log(&format!("motor: marca de muerte ({})", acc as i32));
        }
        // Proyectiles que salen (los siguientes de una misma acción).
        if self.salidas.iter().any(|s| now >= s.0) {
            let ya: Vec<_> = self.salidas.iter().filter(|s| now >= s.0).copied().collect();
            self.salidas.retain(|s| now < s.0);
            for (_, b, objetivo, k, n, real) in ya {
                disparar(b, objetivo, objetivo.and_then(|_| salida(b, k, n, objetivo)), real);
            }
        }
        // Daño de verdad: lo que quita el rival y no es un golpe tuyo, mientras se espera un impacto,
        // es del proyectil; cuando acaba la espera, la proporción con su daño fijo corrige la de ese rival.
        if !self.midiendo.is_empty() {
            if e.otro > 0 && self.midiendo.iter().any(|m| m.1 == e.ent) {
                self.medido += e.otro as f64;
            }
            // Cuando ya han llegado todos o, si no paran de salir (quemaduras), cada 3 s.
            let desde = *self.medida_desde.get_or_insert(now);
            if self.midiendo.iter().all(|m| now.duration_since(m.0) > VENTANA_IMPACTO) || now.duration_since(desde) >= Duration::from_secs(3) {
                // Lo medido se reparte entre los proyectiles según su daño fijo esperado (con el daño
                // de cada uno, que se espera, se corrige la proporción de ese proyectil contra ese rival).
                let total: f64 = self.midiendo.iter().map(|m| m.3).sum();
                if crate::coop::soy_invitado() && total > 0.0 {
                    use std::sync::atomic::Ordering::Relaxed;
                    if self.medido > 0.0 {
                        INVITADO_CEROS.store(0, Relaxed);
                        if INVITADO_REAL.swap(INVITADO_SI, Relaxed) == INVITADO_PROBANDO {
                            crate::log("cooperativo: los proyectiles de verdad del invitado le quitan vida al rival del anfitrión");
                        }
                    } else if INVITADO_REAL.load(Relaxed) == INVITADO_PROBANDO && INVITADO_CEROS.fetch_add(1, Relaxed) + 1 >= INVITADO_CEROS_MAX {
                        INVITADO_REAL.store(INVITADO_NO, Relaxed);
                        crate::log("cooperativo: los proyectiles de verdad del invitado no le quitan vida al rival; vuelve el daño de siempre (lo aplica el anfitrión)");
                    }
                }
                if total > 0.0 && self.medido > 0.0 {
                    let mut por: HashMap<(u32, i32), f64> = HashMap::new();
                    for m in &self.midiendo {
                        *por.entry((m.1, m.2)).or_insert(0.0) += m.3;
                    }
                    for (clave, crudo) in por {
                        let llego = self.medido * crudo / total;
                        let r = self.razon_danio.entry(clave).or_insert(RAZON_INICIAL);
                        *r = (*r * 0.4 + (llego / crudo) * 0.6).clamp(0.01, 5.0);
                        crate::log(&format!("motor: daño de verdad ({}): {} de {} fijo, proporción {:.2} en {}", clave.1, llego as i32, crudo as i32, *r, clave.0));
                    }
                }
                self.midiendo.clear();
                self.medido = 0.0;
                self.medida_desde = None;
            }
        }
        // Golpe invisible con lo que se ha juntado.
        if self.pendiente >= 1.0 && self.golpe_at.is_none_or(|t| now.duration_since(t) >= CADA_GOLPE_INVISIBLE) {
            let razon = self.razon_danio.get(&(e.ent, GOLPE_INVISIBLE)).copied().unwrap_or(RAZON_INICIAL);
            if let Some((id, crudo)) = escalon(GOLPE_INVISIBLE, self.pendiente / razon)
                && let Some(p) = crate::chr_pos(e.ent)
            {
                let origen = [-1, BALAS_REALES.get().map(|b| b.0).unwrap_or(-1), -1];
                let _ = crate::spawn_bullet_origen(id, Some(e.ent), Some([p[0], p[1] + 1.0, p[2]]), origen);
                self.midiendo.push((now, e.ent, GOLPE_INVISIBLE, crudo));
            } else {
                // Sin escalones (datos de ModBuilder que faltan): a mano, como antes.
                out.bhp -= self.pendiente as i32;
            }
            self.pendiente = 0.0;
            self.golpe_at = Some(now);
        }
        // Proyectiles que llegan.
        let llegan: Vec<(i32, String)> = self.impactos.iter().filter(|i| now >= i.0).map(|i| (i.1, i.2.clone())).collect();
        if !llegan.is_empty() {
            self.impactos.retain(|i| now < i.0);
            for (n, src) in llegan {
                out.bhp -= n;
                *self.stats_dmg.entry(src).or_insert(0.0) += n as f64;
            }
        }
        // Daño aplazado que vence.
        let due: i32 = self.aplazado.iter().filter(|a| now >= a.0).map(|a| a.1).sum();
        if due > 0 {
            self.aplazado.retain(|a| now < a.0);
            out.php = (out.php - due).max(1);
            crate::set_player_hp(out.php);
        }
        // Revivir que no llegó a tiempo.
        if self.revive.is_some_and(|(until, _)| now >= until) {
            self.revive = None;
            crate::set_player_hp(0);
        }

        // Umbrales de vida (se rearman al volver a subir).
        let pfrac = out.php as f64 / pmax as f64;
        let bfrac = out.bhp as f64 / bmax as f64;

        // --- Efectos ---
        let dist = match (crate::chr_pos(ent), crate::player_pos()) {
            (Some(b), Some(p)) => ((p[0] - b[0]).powi(2) + (p[2] - b[2]).powi(2)).sqrt() as f64,
            _ => 999.0,
        };
        let elapsed = now.duration_since(self.start).as_secs_f64();
        let mut actions: Vec<(String, Accion)> = vec![];
        // Efectos a los que les ha fallado la probabilidad en este fotograma (para sus alternativas).
        let mut fallo_azar = vec![false; self.slots.len()];
        let mut src_previo = String::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            let previo = std::mem::replace(&mut src_previo, s.src.clone());
            let ef = &s.ef;
            if ef.alternativa {
                // Solo justo después de que al anterior de la misma bendición le falle el azar.
                let toca = i > 0 && fallo_azar[i - 1] && previo == s.src;
                if !toca {
                    if ef.cuando == Cuando::Inicio && elapsed >= 2.0 {
                        s.used = true;
                    }
                    continue;
                }
            }
            let triggered = match ef.cuando {
                Cuando::Segundos => ef.valor > 0.0 && s.last.map_or(elapsed >= ef.valor, |t| now.duration_since(t).as_secs_f64() >= ef.valor),
                Cuando::Inicio => !s.used && elapsed < 2.0,
                Cuando::VidaBaja | Cuando::BossVidaBaja => {
                    let f = if ef.cuando == Cuando::VidaBaja { pfrac } else { bfrac };
                    if f >= ef.valor {
                        s.armed = true;
                        false
                    } else {
                        std::mem::replace(&mut s.armed, false)
                    }
                }
                Cuando::Victoria => false,
                Cuando::Rafaga => burst as f64 >= bmax as f64 * ef.valor.max(0.01),
                Cuando::Estado => fired.contains(&Cuando::Estado) && ef.estado.is_none_or(|i| procs.contains(&i)),
                c => fired.contains(&c),
            };
            if !triggered {
                continue;
            }
            if ef.cuando == Cuando::Segundos {
                s.last = Some(now);
            }
            if ef.una_vez && s.used {
                continue;
            }
            if ef.enfriamiento > 0.0 && s.last.is_some_and(|t| now.duration_since(t).as_secs_f64() < ef.enfriamiento) && ef.cuando != Cuando::Segundos {
                continue;
            }
            if (ef.si_vida_menor > 0.0 && pfrac >= ef.si_vida_menor)
                || (ef.si_boss_vida_menor > 0.0 && bfrac >= ef.si_boss_vida_menor)
                || (ef.si_cerca > 0.0 && dist > ef.si_cerca)
                || (ef.si_sin_danio > 0.0
                    && (if ef.cuando == Cuando::Recibir { taken_prev } else { self.last_taken_at })
                        .is_some_and(|t| now.duration_since(t).as_secs_f64() < ef.si_sin_danio))
                || (ef.si_sin_frasco && self.drank)
            {
                continue;
            }
            s.count += 1;
            if s.count % ef.cada.max(1) != 0 {
                continue;
            }
            // «En cada golpe»: como mucho una vez por espadazo. Un golpe pesado (el martillo del
            // Bulwark) baja la vida en varios tramos y cada tramo contaba como otro golpe.
            if ef.cuando == Cuando::Golpe && ef.cada <= 1 && s.last.is_some_and(|t| now.duration_since(t) < GOLPE_MIN) {
                continue;
            }
            if !ef.alternativa && ef.prob < 1.0 && self.rng.next_f64() >= ef.prob {
                fallo_azar[i] = true;
                // Al empezar el combate se tira una sola vez (si no, se volvería a tirar en cada fotograma).
                if ef.cuando == Cuando::Inicio {
                    s.used = true;
                }
                continue;
            }
            s.used = true;
            if ef.cuando == Cuando::Rafaga {
                self.recent_hits.clear();
            }
            if ef.cuando != Cuando::Segundos {
                s.last = Some(now);
            }
            actions.extend(ef.hacer.iter().cloned().map(|a| (s.src.clone(), a)));
        }
        for (src, a) in &actions {
            // Lo que haga la acción (daño al boss, curación) se apunta a su bendición.
            let (b0, p0) = (out.bhp, out.php);
            if a.quemar > 0.0 {
                self.burn_src = src.clone();
            }
            if a.marca > 0.0 {
                self.marca_src = src.clone();
            }
            if a.robo > 0.0 {
                self.robo_src = src.clone();
            }
            self.disparos.insert(src.clone(), now);
            self.run_action(a, src, e, &mut out, now);
            if out.bhp < b0 {
                *self.stats_dmg.entry(src.clone()).or_insert(0.0) += (b0 - out.bhp) as f64;
            }
            if out.php > p0 {
                *self.stats_heal.entry(src.clone()).or_insert(0.0) += (out.php - p0) as f64;
            }
        }

        // --- Aliados invocados: reforzarlos al aparecer y despedirlos al acabar su tiempo ---
        for s in self.summons.iter_mut() {
            if !s.2 && crate::chr_loaded(s.0) {
                s.2 = crate::apply_speffect_to(s.0, SUMMON_LEVEL);
                crate::fill_hp(s.0);
            }
            if now >= s.1 {
                crate::set_chr_hp(s.0, 0);
            }
        }
        self.summons.retain(|s| now < s.1);

        // --- Pasivos de estado ---
        let remate = self.pasivos.iter().map(|p| p.remate).fold(0.0, f64::max);
        if remate > 0.0 && e.hit > 0 && (out.bhp as f64) < bmax as f64 * remate && out.bhp > 0 {
            if let Some(i) = self.pasivos.iter().position(|p| p.remate > 0.0) {
                *self.stats_dmg.entry(self.pasivo_src[i].clone()).or_insert(0.0) += out.bhp as f64;
            }
            out.bhp = 0;
            crate::log("motor: remate");
        }
        let tope = self.pasivos.iter().filter(|p| p.tope_vida > 0.0).map(|p| p.tope_vida).fold(1.0, f64::min);
        if tope < 1.0 && out.php as f64 > pmax as f64 * tope {
            out.php = (pmax as f64 * tope) as i32;
            crate::set_player_hp(out.php);
        }

        // --- Mutadores del boss ---
        let mut boss_speed = self.mutadores.iter().map(|m| if m.velocidad > 0.0 { m.velocidad } else { 1.0 }).product::<f64>();
        let mut boss_atk = self.mutadores.iter().filter(|m| m.id != "ascension").map(|m| if m.danio > 0.0 { m.danio } else { 1.0 }).product::<f64>();
        let mut boss_def = self.mutadores.iter().map(|m| if m.defensa > 0.0 { m.defensa } else { 1.0 }).product::<f64>();
        for m in &self.mutadores {
            if let Some([umbral, vel, dmg]) = m.furia
                && bfrac < umbral
            {
                boss_speed *= vel;
                boss_atk *= dmg;
            }
            if let Some([umbral, frac]) = m.segundo_aliento
                && !self.mut_segundo_used
                && bfrac < umbral
                && out.bhp > 0
            {
                self.mut_segundo_used = true;
                out.bhp = (out.bhp + (bmax as f64 * frac) as i32).min(bmax);
                crate::log(&format!("mutador: {} (segundo aliento)", m.nombre));
            }
        }
        let regen: f64 = self.mutadores.iter().map(|m| m.regen).sum();
        if regen > 0.0 && now.duration_since(self.mut_last_regen) >= Duration::from_secs(5) {
            self.mut_last_regen = now;
            if out.bhp > 0 {
                out.bhp = (out.bhp + (bmax as f64 * regen) as i32).min(bmax);
            }
        }
        boss_def *= (1.0 + self.pila(|p| p.boss_debil)) * self.temp(Temp::BossDebil);
        boss_speed *= self.temp(Temp::BossLento);
        boss_atk *= self.temp(Temp::BossFlojo);

        // --- Multiplicadores del jugador ---
        let mut dmg = self.base_dmg * self.pasivos.iter().map(|p| p.danio).product::<f64>();
        for p in &self.pasivos {
            for (k, v) in &p.run_danio {
                dmg *= 1.0 + v * self.run_counters.get(k).copied().unwrap_or(0.0);
            }
            if let Some([u, m]) = p.danio_vida_menor
                && pfrac < u
            {
                dmg *= m;
            }
            if let Some([u, m]) = p.danio_boss_vida_menor
                && bfrac < u
            {
                dmg *= m;
            }
        }
        dmg *= (1.0 + self.pila(|p| p.danio)) * self.temp(Temp::Danio);
        let mut def = self.base_def * self.pasivos.iter().map(|p| p.defensa).product::<f64>();
        def *= (1.0 - self.pila(|p| p.defensa)).max(0.1) * self.temp(Temp::Defensa);
        if let Some((tramo_atk, tramo_def)) = self.invitado {
            let f = self.invitado_falta.clamp(0.0, 1.0);
            let (tramo_atk, tramo_def) = (1.0 + f * (tramo_atk - 1.0), 1.0 + f * (tramo_def - 1.0));
            def *= boss_atk * tramo_atk;
            dmg *= boss_def / tramo_def.max(0.01);
            boss_atk = 1.0;
            boss_def = 1.0;
        }
        if self.invuln_until.is_some_and(|t| now < t) {
            def = 0.0;
        }
        let speed = self.pasivos.iter().map(|p| p.velocidad).product::<f64>() * (1.0 + self.pila(|p| p.velocidad)) * self.temp(Temp::Velocidad);

        self.write(0, SP_DMG_DYNAMIC, dmg, 0, true);
        self.write(1, SP_DEFENSE_DYNAMIC, def, 0, false);
        self.write(2, SP_BOSS_DEF, boss_def, ent, false);
        self.write(3, SP_BOSS_ATK, boss_atk, ent, true);
        if (speed - 1.0).abs() > 0.001 || (self.speed_applied - 1.0).abs() > 0.001 {
            crate::set_anim_speed(0, speed as f32);
            self.speed_applied = speed;
        }
        // Muerto, a su velocidad: que su animación de muerte no vaya lenta.
        if e.bhp <= 0 {
            boss_speed = 1.0;
        }
        if (boss_speed - 1.0).abs() > 0.001 || (self.boss_speed_applied - 1.0).abs() > 0.001 {
            crate::set_anim_speed(ent, boss_speed as f32);
            self.boss_speed_applied = boss_speed;
        }
        if out.bhp != e.bhp {
            out.bhp = out.bhp.max(0);
            crate::set_chr_hp(ent, out.bhp);
            // En cooperativo los rivales son del anfitrión: lo que escribe un invitado en su copia se
            // pierde, así que el daño de sus bendiciones se le manda (coop.rs).
            if out.bhp < e.bhp && crate::coop::soy_invitado() {
                crate::coop::suceso(rogue_core::coop::Suceso::Danio { ent, n: e.bhp - out.bhp });
            }
        }
        // Tamaños (el juego monta la matriz del modelo cada fotograma a partir de ellos).
        if (self.size - 1.0).abs() > 0.001 {
            crate::set_chr_scale(0, self.size as f32);
        }
        if (self.boss_size - 1.0).abs() > 0.001 {
            crate::set_chr_scale(ent, self.boss_size as f32);
        }
        out
    }

    /// Escribe una fila dinámica y la (re)aplica al fotograma siguiente si ha cambiado.
    fn write(&mut self, k: usize, sp: i32, v: f64, ent: u32, attack: bool) {
        let apply = |on: bool| {
            if ent == 0 {
                crate::player_speffect(sp, on);
            } else if on {
                crate::apply_speffect_to(ent, sp);
            } else {
                crate::remove_speffect_from(ent, sp);
            }
        };
        if self.reapply[k] {
            self.reapply[k] = false;
            if (self.applied[k] - 1.0).abs() > 0.001 {
                apply(true);
            }
            return;
        }
        if (v - self.applied[k]).abs() / self.applied[k].max(0.01) < 0.005 && !(v == 0.0 && self.applied[k] != 0.0) {
            return;
        }
        let ok = if attack { crate::set_speffect_attack_rate(sp, v as f32) } else { crate::set_speffect_cut_rate(sp, v as f32) };
        if ok {
            apply(false);
            self.applied[k] = v;
            self.reapply[k] = true;
        }
    }

    fn run_action(&mut self, a: &Accion, src: &str, e: &Entrada, out: &mut Salida, now: Instant) {
        let ent = e.ent;
        let (pmax, bmax) = (e.pmax.max(1) as f64, e.bmax.max(1) as f64);
        // Sin golpes todavía, el golpe de tu acto (300, 550, 1000: un 5 % de la vida de un normal; con
        // el 2 % de la vida del rival las primeras bolas de Fuegos fatuos hacían 28, 23-10).
        let golpe = if self.media_golpe > 0.0 {
            self.media_golpe
        } else if self.golpe_defecto > 0.0 {
            self.golpe_defecto
        } else {
            bmax * 0.02
        };
        let dmg =
            a.danio_boss * bmax + a.danio_golpe * e.hit.max(0) as f64 + a.danio_golpes * golpe + a.danio_vida * pmax + a.danio_recibido * e.taken.max(0) as f64;
        // Proyectil con ataque de verdad (data/proyectiles.json): su daño lo calcula el juego con tu arma y
        // la defensa del rival, como un golpe: 100 % = un golpe (23-10: usar la lógica del juego; el
        // daño puesto a mano se disparaba). En el juego de un invitado, por ahora el de siempre: falta ver
        // si sus proyectiles dañan a los rivales, que son del anfitrión.
        let n = a.balas.max(1);
        // El escalón de cada proyectil: el daño que toca, dividido por lo que llega de cada punto de daño
        // fijo contra este rival.
        let real = a.bala.filter(|_| !a.en_jugador && dmg > 0.0 && danio_real()).and_then(|b| {
            let razon = self.razon_danio.get(&(ent, b)).copied().unwrap_or(RAZON_INICIAL);
            escalon(b, dmg / n as f64 / razon)
        });
        if let Some(b) = a.bala {
            let objetivo = if a.en_jugador { None } else { Some(ent) };
            if let Some((_, crudo)) = real {
                // Para las estadísticas del final, lo que se espera (el juego no dice de quién es cada golpe).
                *self.stats_dmg.entry(src.to_string()).or_insert(0.0) += dmg;
                for k in 0..n {
                    self.midiendo.push((now + Duration::from_millis(150 * k as u64), ent, b, crudo));
                }
            }
            let real_id = real.map(|r| r.0);
            // Uno tras otro, a 150 ms (como llega su daño), no todos en el mismo instante.
            disparar(b, objetivo, objetivo.and_then(|_| salida(b, 0, n, objetivo)), real_id);
            for k in 1..n {
                self.salidas.push((now + Duration::from_millis(150 * k as u64), b, objetivo, k, n, real_id));
            }
        }
        if real.is_some() {
            // El daño lo pone el juego.
        } else if dmg > 0.0 && a.bala.is_none() && danio_real() {
            self.quitar(dmg, src);
        } else if dmg > 0.0 && a.bala.is_some() && !a.en_jugador {
            // Con proyectil, el daño llega con él (repartido entre todos, uno tras otro).
            let n = a.balas.max(1);
            for k in 0..n {
                let cuando = now + IMPACTO + Duration::from_millis(150 * k as u64);
                self.impactos.push((cuando, (dmg / n as f64) as i32, src.to_string()));
            }
        } else if dmg > 0.0 {
            out.bhp -= dmg as i32;
        }
        let heal = a.curar * pmax + a.curar_golpe * e.hit.max(0) as f64;
        if heal > 0.0 {
            crate::heal_player_by(heal as i32);
            out.php = (out.php + heal as i32).min(e.pmax);
        }
        // Lo curado: por la acción o por el frasco que la disparó.
        let curado = heal + (-e.taken).max(0) as f64;
        if a.danio_curado > 0.0 && curado > 0.0 {
            if danio_real() {
                self.quitar(curado * a.danio_curado, src);
            } else {
                out.bhp -= (curado * a.danio_curado) as i32;
            }
        }
        if let Some(sp) = a.efecto_jugador {
            crate::player_speffect(sp, true);
        }
        if let Some(sp) = a.efecto_boss {
            crate::apply_speffect_to(ent, sp);
        }
        if let Some(id) = &a.pila
            && let Some(d) = self.pilas_def.get(id)
        {
            let until = (d.segundos > 0.0).then(|| now + Duration::from_secs_f64(d.segundos));
            let entry = self.pilas.entry(id.clone()).or_insert((0, None));
            entry.0 = (entry.0 + 1).min(d.max);
            entry.1 = until;
        }
        if let Some(id) = &a.quitar_pila {
            self.pilas.remove(id);
        }
        if let Some(k) = &a.run {
            *self.run_counters.entry(k.clone()).or_insert(0.0) += 1.0;
            out.run.push(k.clone());
        }
        if a.invulnerable > 0.0 {
            self.invuln_until = Some(now + Duration::from_secs_f64(a.invulnerable));
            self.bufos.push((src.to_string(), now, now + Duration::from_secs_f64(a.invulnerable)));
        }
        let secs = Duration::from_secs_f64(a.segundos.max(0.1));
        let mut bufo = false;
        for (k, v) in [
            (Temp::Defensa, a.defensa),
            (Temp::Danio, a.danio),
            (Temp::Velocidad, a.velocidad),
            (Temp::BossDebil, a.boss_debil),
            (Temp::BossFlojo, a.boss_flojo),
            (Temp::Robo, a.robo),
            (Temp::BossLento, a.frenar_boss),
        ] {
            if v > 0.0 {
                self.temps.push((k, v, now + secs, now, src.to_string()));
                bufo = true;
            }
        }
        if bufo {
            self.bufos.retain(|b| b.0 != src);
            self.bufos.push((src.to_string(), now, now + secs));
        }
        out.runas += a.runas;
        if a.postura {
            crate::romper_postura(ent);
        }
        if a.marca > 0.0 {
            self.marca = Some((a.marca, now + secs, 0.0));
        }
        if a.aplazar > 0.0 && e.taken > 0 {
            let n = (e.taken as f64 * a.aplazar) as i32;
            crate::heal_player_by(n);
            out.php = (out.php + n).min(e.pmax);
            self.aplazado.push((now + secs, n));
        }
        if a.perdonar {
            self.aplazado.clear();
        }
        if a.recargar {
            crate::refill_player_stamina_fp();
        }
        if a.revivir > 0.0 {
            self.revive = Some((now + secs, a.revivir));
        }
        if a.tamano > 0.0 {
            self.size = a.tamano;
        }
        if a.postura_danio > 0.0 {
            crate::quitar_postura(ent, a.postura_danio as f32);
        }
        if a.quemar > 0.0 {
            self.burns.push((a.quemar, now + secs));
            crate::apply_speffect_to(e.ent, SP_QUEMADO);
        }
        if a.devolver_pf > 0.0 && self.fp_spent > 0 {
            crate::add_player_fp((self.fp_spent as f64 * a.devolver_pf) as i32);
            self.last_fp = crate::player_fp();
        }
        if a.gastar_pf > 0.0 && self.fp_spent > 0 {
            let extra = (self.fp_spent as f64 * a.gastar_pf) as i32;
            let have = crate::player_fp().unwrap_or(0);
            crate::add_player_fp(-extra.min(have));
            if extra > have {
                out.php = (out.php - (extra - have) * 5).max(1);
                crate::set_player_hp(out.php);
            }
            self.last_fp = crate::player_fp();
        }
        if a.coste_vida > 0.0 {
            out.php = (out.php - (pmax * a.coste_vida) as i32).max(1);
            crate::set_player_hp(out.php);
        }
        if a.puas > 0.0 && e.taken > 0 {
            self.puas += e.taken as f64 * a.puas;
        }
        if a.soltar_puas && self.puas >= 1.0 {
            if danio_real() {
                let p = self.puas;
                self.quitar(p, src);
            } else {
                out.bhp -= self.puas as i32;
            }
            crate::log(&format!("motor: púas ({})", self.puas as i32));
            self.puas = 0.0;
        }
        if a.bomba > 0.0 {
            let curado = heal + (-e.taken).max(0) as f64;
            self.bomba += curado * a.bomba;
            if self.bomba >= pmax {
                // El meteoro con daño de verdad (o, si no se puede, su copia sin ataque y el daño a mano).
                let razon = self.razon_danio.get(&(ent, 79_600_030)).copied().unwrap_or(RAZON_INICIAL);
                match escalon(79_600_030, self.bomba / razon).filter(|_| danio_real()) {
                    Some((id, crudo)) => {
                        self.midiendo.push((now, ent, 79_600_030, crudo));
                        disparar(79_600_030, Some(ent), None, Some(id));
                    }
                    None => {
                        out.bhp -= self.bomba as i32;
                        disparar(79_600_030, Some(ent), None, None);
                    }
                }
                crate::log(&format!("motor: bomba espiritual ({})", self.bomba as i32));
                self.bomba = 0.0;
            }
        }
        if !a.invocar.is_empty() {
            let pick = a.invocar[self.rng.range(0, a.invocar.len() as i32) as usize];
            let until = now + Duration::from_secs_f64(a.segundos.max(5.0));
            for k in 0..a.invocados.max(1) {
                let ent = self.next_summon;
                self.next_summon += 1;
                crate::spawn_chr(pick[0], pick[1], pick[2], ent, 2.0 + k as f32);
                self.summons.push((ent, until, false));
                self.summon_src.insert(ent, src.to_string());
            }
            crate::log(&format!("motor: invoca c{:04} x{}", pick[0], a.invocados.max(1)));
            // En cooperativo, los demás también lo crean (lo que crea la DLL no viaja por la red).
            if crate::coop::hay_companeros()
                && let Some(centro) = crate::player_pos()
            {
                crate::coop::suceso(rogue_core::coop::Suceso::Aliado {
                    chr: pick[0],
                    npc: pick[1],
                    think: pick[2],
                    n: a.invocados.max(1),
                    segundos: a.segundos.max(5.0),
                    centro,
                });
            }
        }
        if a.anular && e.taken > 0 {
            crate::heal_player_by(e.taken);
            out.php = (out.php + e.taken).min(e.pmax);
        }
    }

    /// Al ganar: efectos de victoria (contadores de la run y runas).
    pub fn victoria(&mut self) -> Salida {
        let mut out = Salida::default();
        let drank = self.drank;
        let acts: Vec<Accion> =
            self.slots.iter().filter(|s| s.ef.cuando == Cuando::Victoria && !(s.ef.si_sin_frasco && drank)).flat_map(|s| s.ef.hacer.clone()).collect();
        for a in acts {
            if let Some(k) = &a.run {
                out.run.push(k.clone());
            }
            out.runas += a.runas;
        }
        out
    }
}

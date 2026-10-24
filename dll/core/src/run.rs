//! Estado de una run. Estructura de StS2 (`RunState`, `ActModel.GenerateRooms`, `RunRngSet`):
//! al empezar cada acto se genera su mapa y se barajan las colas de encuentros por tipo; al entrar
//! en un nodo se saca el siguiente de la cola de su tipo. El nodo solo dice el tipo, no qué boss es.

use std::collections::HashSet;

use crate::map::{self, ActMap, ActShape, NodeType};
use crate::rng::{hash_str, Rng};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncounterKind {
    /// Primeros combates del acto (StS2: "weak encounters").
    Weak,
    Normal,
    Elite,
    Boss,
}

/// Un combate jugable: referencias a lo que el juego necesita para llevarte y detectar la victoria.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Encounter {
    pub id: String,
    pub name: String,
    pub kind: EncounterKind,
    /// Flag que la DLL enciende para que el evento EMEVD haga el warp a la arena.
    pub warp_flag: u32,
    /// Flag que el juego enciende al morir el boss.
    pub defeat_flag: u32,
    /// Etiquetas para no repetir tipos seguidos (p. ej. "caballero", "dragon").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Si la aparición del warp queda lejos del boss: punto junto a él (coordenadas locales del
    /// mapa, MSB) al que el director coloca al jugador al llegar.
    #[serde(default)]
    pub llegada: Option<[f32; 3]>,
    /// Modo coliseo: modelo (cXXXX), NpcParam e IA para crear el boss en la arena al llegar.
    #[serde(default)]
    pub chr: Option<i32>,
    #[serde(default)]
    pub npc: Option<i32>,
    #[serde(default)]
    pub think: Option<i32>,
    /// Multiplicador de vida de este encuentro, para los que se salen de lo normal de su tipo (un
    /// enemigo corriente con poca vida, un trol con demasiada).
    #[serde(default)]
    pub vida: Option<f64>,
    /// Combates de varios enemigos (docs/LISTAS_Y_COMBATES.md): uno por hueco del coliseo. Vacío =
    /// el enemigo de `chr`, `npc` y `think`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enemigos: Vec<Enemigo>,
    #[serde(default, skip_serializing_if = "Formato::es_solo")]
    pub formato: Formato,
    /// Índices de `enemigos` de cada oleada (oleadas y racha). Vacío = todos a la vez.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub oleadas: Vec<Vec<usize>>,
    /// Escolta: índice del líder en `enemigos` (al caer él, cae su gente).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lider: Option<usize>,
    /// Contrarreloj (una bendición más si cae antes) o aguante (lo que hay que sobrevivir).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segundos: Option<u32>,
    /// Mutadores propios del encuentro (data/mutadores.json): el campeón es un normal con aura.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mutadores: Vec<String>,
    /// Animación con la que entra el rival al empezar (la que le pone el evento de su mapa en el juego
    /// original; Artorias, la 20026).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_inicio: Option<i32>,
    /// Arena grande en la que se pelea (clave de data/arenas_grandes.json); None = el coliseo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arena: Option<String>,
    /// Jefe de dos fases con un personaje por fase (oleadas [[0], [1]]): la segunda sale cuando la vida
    /// de la primera baja de esta fracción (como el evento de su mapa: Radahn consorte al 65 %), no al
    /// morir; la primera se va y la segunda aparece donde estaba.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fase_en: Option<f64>,
    /// Jefe de dos fases con un solo personaje (Malenia): lo que le hace su evento al llegar al umbral.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renacer: Option<Renacer>,
    /// Dúos de jefes (cooperativo, 18-10: «el último en pie»): al caer uno, el que queda pasa a su
    /// segunda fase.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ultimo_en_pie: Vec<FaseUltimo>,
}

/// Segunda fase del superviviente de un dúo: `enemigo` es su índice en `enemigos`; o cambia de personaje
/// (`fase2`, colocado apagado en la oleada 3 + k, donde k cuenta los `fase2` anteriores) o renace en el
/// mismo (`renacer`).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FaseUltimo {
    pub enemigo: usize,
    pub fase2: Option<Enemigo>,
    pub renacer: Option<Renacer>,
}

impl Encounter {
    /// Hueco (desde el primero del encuentro) y oleada de la segunda fase de cada `ultimo_en_pie` que
    /// cambia de personaje: los huecos siguen a los de `enemigos` y las oleadas empiezan en la 3.
    pub fn huecos_fase2(&self) -> Vec<(usize, usize, Enemigo)> {
        let n = self.lista().len();
        self.ultimo_en_pie
            .iter()
            .filter_map(|u| u.fase2.map(|f| (u.enemigo, f)))
            .enumerate()
            .map(|(k, (i, f))| (i, n + k, f))
            .collect()
    }
}

/// Segunda fase de un jefe de un solo personaje: al bajar de `vida` (fracción) se vuelve intocable
/// `segundos`, recibe `efectos`, pierde `quitar`, hace `anim` y vuelve con la vida llena.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Renacer {
    pub vida: f64,
    pub efectos: Vec<i32>,
    pub quitar: Vec<i32>,
    pub anim: Option<i32>,
    pub segundos: f64,
}

/// Un enemigo de un combate de grupo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enemigo {
    pub chr: i32,
    pub npc: i32,
    pub think: i32,
    /// Animación al aparecer (la de transformación de una segunda fase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim: Option<i32>,
    /// Efecto que le pone su evento original al aparecer (la segunda fase de Messmer, 20010612: sin él su IA
    /// sigue con los ataques de la primera).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efecto: Option<i32>,
}

/// Cómo se pelea un encuentro (docs/LISTAS_Y_COMBATES.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Formato {
    /// Un enemigo.
    #[default]
    Solo,
    /// Varios a la vez: se gana cuando caen todos.
    Manada,
    /// Un líder con su gente: se gana al tumbar al líder.
    Escolta,
    /// Dos fuertes a la vez.
    Duo,
    /// Por oleadas: la siguiente sale cuando cae la anterior.
    Oleadas,
    /// Varios rivales seguidos, de uno en uno; vale doble.
    Racha,
    /// Sobrevivir `segundos` contra algo que no cae.
    Aguante,
}

impl Formato {
    pub fn es_solo(&self) -> bool {
        *self == Formato::Solo
    }
}

impl Encounter {
    /// Se pelea en el coliseo (con su variante del MSB o con `spawn_chr`), no en su arena del mundo.
    pub fn en_coliseo(&self) -> bool {
        self.chr.is_some() || !self.enemigos.is_empty()
    }

    /// Los enemigos del encuentro, uno o varios.
    pub fn lista(&self) -> Vec<Enemigo> {
        if !self.enemigos.is_empty() {
            return self.enemigos.clone();
        }
        match (self.chr, self.npc, self.think) {
            (Some(chr), npc, think) => vec![Enemigo { chr, npc: npc.unwrap_or(-1), think: think.unwrap_or(-1), anim: None, efecto: None }],
            _ => vec![],
        }
    }

    /// Índices de los enemigos de cada oleada (una sola si no hay oleadas).
    pub fn grupos(&self) -> Vec<Vec<usize>> {
        let n = self.lista().len();
        if self.oleadas.is_empty() {
            return vec![(0..n).collect()];
        }
        self.oleadas.iter().map(|o| o.iter().copied().filter(|&i| i < n).collect()).filter(|o: &Vec<usize>| !o.is_empty()).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActDef {
    pub name: String,
    pub shape: ActShape,
    /// Número de combates "débiles" al principio del acto.
    pub weak_count: usize,
    pub encounters: Vec<Encounter>,
    /// Cooperativo: parejas posibles (data/coop/mezclas_<acto>.json, tools/mezclas_coop.py). Al empezar el
    /// acto se sortean unas cuantas con la semilla de la run (`sortear_mezclas`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mezclas: Vec<Encounter>,
}

/// Veces que un mismo enemigo puede salir en las parejas sorteadas de un acto y tipo.
const MEZCLA_USOS: usize = 2;
/// Parejas sorteadas por acto y tipo, como mucho.
const MEZCLA_MAX: usize = 30;

/// Parejas de este acto y tipo para esta run (24-10: «que mezcle y genere parejas distintas cada
/// vez que alguien juega una run nueva en coop»). Tantas como enemigos distintos haya en las posibles
/// (hasta MEZCLA_MAX), cada enemigo en MEZCLA_USOS como mucho. Mismo resultado con la misma semilla: los
/// dos PCs sacan las mismas.
pub fn sortear_mezclas(posibles: &[Encounter], kind: EncounterKind, rng: &mut Rng) -> Vec<Encounter> {
    let mut bolsa: Vec<&Encounter> = posibles.iter().filter(|e| e.kind == kind).collect();
    let modelos: HashSet<i32> = bolsa.iter().flat_map(|e| e.enemigos.iter().map(|x| x.chr)).collect();
    let cuantas = modelos.len().min(MEZCLA_MAX);
    let mut usos: std::collections::HashMap<i32, usize> = Default::default();
    let mut out = vec![];
    while out.len() < cuantas && !bolsa.is_empty() {
        let e = bolsa.swap_remove(rng.range(0, bolsa.len() as i32) as usize);
        if e.enemigos.iter().any(|x| usos.get(&x.chr).copied().unwrap_or(0) >= MEZCLA_USOS) {
            continue;
        }
        for x in &e.enemigos {
            *usos.entry(x.chr).or_default() += 1;
        }
        out.push(e.clone());
    }
    out
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActState {
    pub map: ActMap,
    pub normal_queue: Vec<Encounter>,
    pub elite_queue: Vec<Encounter>,
    pub boss: Encounter,
    pub normals_used: usize,
    pub elites_used: usize,
}

/// Qué hay que hacer al entrar en un nodo. Se crea uno por nodo: el tamaño del encuentro no importa.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum NodeAction {
    Combat(Encounter),
    Rest,
    Shop,
    Treasure,
    Event,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunState {
    pub seed: String,
    pub act_index: usize,
    pub act: ActState,
    /// Nodo actual (col, fila). Fila 0 = inicio del acto.
    pub position: (i32, i32),
    /// Nodos visitados en el acto actual.
    pub path: Vec<(i32, i32)>,
    /// El nodo actual ya se ha resuelto (combate ganado, descanso usado...).
    pub node_done: bool,
    pub rng_upfront: Rng,
    pub rng_rewards: Rng,
    /// Objetos conseguidos (no se vuelven a ofrecer).
    #[serde(default)]
    pub items: Vec<(crate::rewards::Category, i32)>,
    /// Reliquias conseguidas y su nivel.
    #[serde(default)]
    pub relics: Vec<crate::relics::OwnedRelic>,
    /// Eventos ya vistos en esta run (no se repiten mientras queden otros).
    #[serde(default)]
    pub events_seen: Vec<String>,
    /// Élites derrotados en la run (Trofeo).
    #[serde(default)]
    pub elites_won: u32,
    /// Personaje elegido (id de data/personajes.json); None hasta elegirlo.
    #[serde(default)]
    pub personaje: Option<String>,
    /// Veces que se ha aumentado el poder en una gracia (+10 % de daño y de defensa cada una).
    #[serde(default)]
    pub poder: u32,
    /// Ceniza montada en el arma actual (para conservarla al subir el arma de nivel).
    #[serde(default)]
    pub weapon_gem: Option<i32>,
    /// Niveles extra del arma (Forja ambulante: uno por élite).
    #[serde(default)]
    pub forja: u32,
    /// Contadores de la run que alimentan bendiciones que crecen (Crecer, Motor, Poder maligno...).
    #[serde(default)]
    pub contadores: std::collections::HashMap<String, f64>,
    /// Mutadores que llevan los enemigos el resto de la run (eventos).
    #[serde(default)]
    pub mutadores: Vec<String>,
    /// Golpes dados en toda la run (Misión: la espátula).
    #[serde(default)]
    pub golpes: u32,
    /// Ascensión elegida (0 = normal; ver perfil::ASCENSIONES).
    #[serde(default)]
    pub ascension: u32,
    /// Run de una sesión de cooperativo: sus actos salen de las listas de data/coop (dúos).
    #[serde(default)]
    pub coop: bool,
    /// Invocación ganada en la run (ceniza de espíritu de una recompensa): sustituye a la de salida.
    #[serde(default)]
    pub invocacion: Option<crate::characters::Invocacion>,
    /// La run ya terminó (ganada o perdida): al volver a abrir el juego se empieza otra.
    #[serde(default)]
    pub terminada: bool,
    /// Rivales (ver `rival`) que han salido en la run, en orden. Pasan al perfil al acabar.
    #[serde(default)]
    pub rivales: Vec<String>,
    /// Rivales de la run anterior: mientras haya otros, no se repiten en esta (en ningún acto).
    #[serde(default)]
    pub evitar: Vec<String>,
    /// Estadísticas de cada combate de la run (para los resúmenes y para equilibrar).
    #[serde(default)]
    pub combates: Vec<CombatStats>,
    /// Un apunte por nodo recorrido (para el gráfico del final de la run).
    #[serde(default)]
    pub historia: Vec<NodeLog>,
    /// Combate del nodo actual mientras no se resuelve (para retomarlo si se cierra el juego).
    #[serde(default)]
    pub en_curso: Option<Encounter>,
    /// Sorteos del mapa: contenido del "?", evento y su resultado, cofre trampa y mutador del acto.
    /// Van aparte de `rng_rewards` para que elegir una recompensa no cambie lo que sale después. Se
    /// crea de la semilla al usarlo.
    #[serde(default)]
    pub rng_comun: Option<Rng>,
}

/// Lo que pasó en un nodo del mapa.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeLog {
    pub acto: usize,
    /// "Inicio", "Monster", "Elite", "Boss", "Rest", "Shop", "Treasure" o "Unknown".
    pub tipo: String,
    /// Vida mínima (fracción) durante el combate del nodo; 1 si no hubo combate.
    pub vida_min: f64,
    /// Índice en `combates` si hubo pelea.
    #[serde(default)]
    pub combate: Option<usize>,
    /// Bendiciones conseguidas en el nodo (ids).
    #[serde(default)]
    pub bendiciones: Vec<String>,
    /// Iconos de los objetos conseguidos (armas, cenizas).
    #[serde(default)]
    pub objetos: Vec<i32>,
}

/// Lo que pasó en un combate: de dónde salió el daño y la curación.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CombatStats {
    pub enemigo: String,
    /// "Normal", "Elite" o "Boss".
    pub tipo: String,
    pub acto: usize,
    pub segundos: f64,
    /// Daño al enemigo por fuente ("arma", "pasiva", id de bendición...), de mayor a menor.
    pub danio: Vec<(String, f64)>,
    pub curacion: Vec<(String, f64)>,
    pub recibido: f64,
    /// Vida (fracción) al acabar.
    pub vida_final: f64,
    pub golpes: u32,
    pub ganado: bool,
}

fn seed_u64(seed: &str) -> u64 {
    hash_str(seed)
}

/// Quién es el rival para el jugador: su modelo (dos entradas del juego con el mismo modelo son el
/// mismo enemigo) o, sin modelo propio, su nombre (los humanos comparten c0000).
pub fn rival(e: &Encounter) -> String {
    match e.chr {
        Some(c) if c != 0 => format!("c{c}"),
        // Grupos: cada uno es su propio rival (una manada de soldados no quita al soldado suelto).
        _ if !e.enemigos.is_empty() => format!("grupo:{}", e.id),
        _ => e.name.clone(),
    }
}

/// Saca de la bolsa sin repetir etiqueta con el último elegido (`AddWithoutRepeatingTags`) y, mientras
/// queden otros, sin repetir rival (`vistos`: los ya puestos en la run).
fn fill_queue(pool: &[Encounter], count: usize, rng: &mut Rng, out: &mut Vec<Encounter>, vistos: &mut HashSet<String>) {
    let mut bag: Vec<Encounter> = vec![];
    for _ in 0..count {
        if bag.is_empty() {
            bag = pool.to_vec();
            if bag.is_empty() {
                return;
            }
        }
        let last = out.last().cloned();
        let sin_etiqueta = |e: &Encounter| match &last {
            Some(l) => e.id != l.id && !e.tags.iter().any(|t| l.tags.contains(t)),
            None => true,
        };
        let nuevo = |e: &Encounter| !vistos.contains(&rival(e));
        // De más a menos exigente: rival nuevo y sin repetir etiqueta, rival nuevo, sin etiqueta, cualquiera.
        let filtros: [&dyn Fn(&Encounter) -> bool; 3] = [&|e| nuevo(e) && sin_etiqueta(e), &nuevo, &sin_etiqueta];
        let ok = filtros.iter().map(|f| (0..bag.len()).filter(|&i| f(&bag[i])).collect::<Vec<_>>()).find(|v| !v.is_empty()).unwrap_or_default();
        let idx = if ok.is_empty() { rng.range(0, bag.len() as i32) as usize } else { ok[rng.range(0, ok.len() as i32) as usize] };
        let e = bag.remove(idx);
        vistos.insert(rival(&e));
        out.push(e);
    }
}

/// Combates de la cola que se suelen jugar de verdad en un acto (las élites no repiten esos rivales).
const COMBATES_JUGADOS: usize = 6;

/// `previos`: rivales que ya salieron en actos anteriores.
fn start_act(seed: &str, index: usize, def: &ActDef, upfront: &mut Rng, previos: HashSet<String>) -> ActState {
    let mut map_rng = Rng::named(seed_u64(seed), &format!("act_{}_map", index + 1));
    let map = map::generate(&def.shape, &mut map_rng);
    // Cooperativo: las parejas sorteadas de esta run se suman a las de siempre.
    let mut mezclas_rng = Rng::named(seed_u64(seed), &format!("act_{}_mezclas", index + 1));
    let mut of = |k: EncounterKind| {
        let mut v = def.encounters.iter().filter(|e| e.kind == k).cloned().collect::<Vec<_>>();
        v.extend(sortear_mezclas(&def.mezclas, k, &mut mezclas_rng));
        v
    };
    let (weak, normal, elite, bosses) = (of(EncounterKind::Weak), of(EncounterKind::Normal), of(EncounterKind::Elite), of(EncounterKind::Boss));
    let mut vistos = previos.clone();
    let mut normal_queue = vec![];
    fill_queue(&weak, def.weak_count, upfront, &mut normal_queue, &mut vistos);
    fill_queue(&normal, def.shape.rooms as usize, upfront, &mut normal_queue, &mut vistos);
    // Las élites, aparte: la cola de combates es larga y casi toda no se juega; se evitan solo los
    // primeros combates y lo de actos anteriores.
    let mut vistos_elite = previos;
    vistos_elite.extend(normal_queue.iter().take(COMBATES_JUGADOS).map(rival));
    let mut elite_queue = vec![];
    fill_queue(&elite, 15, upfront, &mut elite_queue, &mut vistos_elite);
    let boss = bosses[upfront.range(0, bosses.len() as i32) as usize].clone();
    ActState { map, normal_queue, elite_queue, boss, normals_used: 0, elites_used: 0 }
}

impl RunState {
    pub fn new(seed: &str, acts: &[ActDef]) -> Self {
        Self::nueva(seed, acts, vec![])
    }

    /// Run nueva que, mientras haya otros, no repite los rivales de `evitar` (los de la run anterior).
    pub fn nueva(seed: &str, acts: &[ActDef], evitar: Vec<String>) -> Self {
        let mut upfront = Rng::named(seed_u64(seed), "upfront");
        let act = start_act(seed, 0, &acts[0], &mut upfront, evitar.iter().cloned().collect());
        let start = act.map.start;
        RunState {
            seed: seed.to_string(),
            act_index: 0,
            act,
            position: start,
            path: vec![start],
            node_done: true,
            rng_upfront: upfront,
            rng_rewards: Rng::named(seed_u64(seed), "rewards"),
            items: vec![],
            relics: vec![],
            events_seen: vec![],
            elites_won: 0,
            personaje: None,
            poder: 0,
            contadores: Default::default(),
            mutadores: vec![],
            golpes: 0,
            ascension: 0,
            terminada: false,
            rivales: vec![],
            evitar,
            combates: vec![],
            historia: vec![],
            en_curso: None,
            rng_comun: None,
            weapon_gem: None,
            forja: 0,
            coop: false,
            invocacion: None,
        }
    }

    /// Nodos a los que se puede ir ahora (hijos del actual, o el boss desde la última fila).
    pub fn available(&self) -> Vec<(i32, i32)> {
        if !self.node_done {
            return vec![];
        }
        let m = &self.act.map;
        if self.position == m.start {
            return m.first_row().map(|n| (n.col, n.row)).collect();
        }
        if self.position.1 == m.shape.rooms {
            return vec![m.boss];
        }
        m.node(self.position.0, self.position.1).map(|n| n.children.clone()).unwrap_or_default()
    }

    /// Entra en un nodo. Devuelve qué hay que hacer, o error si no es accesible.
    /// Cooperativo: el mapa, los combates y los eventos salen de la semilla de la sesión, iguales para
    /// todos, pero las recompensas (bendiciones, objetos, tienda) son de cada uno: su sorteo se mezcla con
    /// el SteamID del jugador. Sin esto, con el mismo personaje salían las mismas (16-10).
    pub fn personal(&mut self, jugador: u64) {
        self.rng_rewards = Rng::named(seed_u64(&self.seed) ^ jugador, "rewards");
    }

    /// Generador de los sorteos compartidos (ver `rng_comun`).
    pub fn comun(&mut self) -> &mut Rng {
        let seed = seed_u64(&self.seed);
        self.rng_comun.get_or_insert_with(|| Rng::named(seed, "comun"))
    }

    pub fn enter(&mut self, to: (i32, i32)) -> Result<NodeAction, String> {
        if !self.available().contains(&to) {
            return Err(format!("nodo {to:?} no accesible desde {:?}", self.position));
        }
        self.position = to;
        self.path.push(to);
        self.node_done = false;
        if to == self.act.map.boss {
            let b = self.act.boss.clone();
            self.rivales.push(rival(&b));
            return Ok(NodeAction::Combat(b));
        }
        let kind = self.act.map.node(to.0, to.1).map(|n| n.kind).ok_or("nodo inexistente")?;
        Ok(match kind {
            NodeType::Monster => NodeAction::Combat(self.next_encounter(EncounterKind::Normal).ok_or("cola de combates vacía")?),
            NodeType::Elite => NodeAction::Combat(self.next_encounter(EncounterKind::Elite).ok_or("cola de élites vacía")?),
            NodeType::Rest => NodeAction::Rest,
            NodeType::Shop => NodeAction::Shop,
            NodeType::Treasure => NodeAction::Treasure,
            NodeType::Unknown => {
                // Como el "?" de StS: casi siempre un evento, a veces otra cosa.
                match self.comun().range(0, 100) {
                    x if x < 12 => match self.next_encounter(EncounterKind::Normal) {
                        Some(e) => NodeAction::Combat(e),
                        None => NodeAction::Event,
                    },
                    x if x < 16 => NodeAction::Treasure,
                    x if x < 20 => NodeAction::Shop,
                    _ => NodeAction::Event,
                }
            }
            other => return Err(format!("tipo de nodo inesperado {other:?}")),
        })
    }

    /// Siguiente encuentro de la cola de ese tipo (élite o normal), para eventos y el "?".
    pub fn next_encounter(&mut self, kind: EncounterKind) -> Option<Encounter> {
        let e = if kind == EncounterKind::Elite {
            let e = self.act.elite_queue.get(self.act.elites_used).cloned()?;
            self.act.elites_used += 1;
            e
        } else {
            let e = self.act.normal_queue.get(self.act.normals_used).cloned()?;
            self.act.normals_used += 1;
            e
        };
        self.rivales.push(rival(&e));
        Some(e)
    }

    /// El nodo actual se ha resuelto. Si era el boss, pasa al siguiente acto (o termina: Ok(true)).
    pub fn complete(&mut self, acts: &[ActDef]) -> Result<bool, String> {
        self.node_done = true;
        self.en_curso = None;
        if self.position == self.act.map.boss {
            if self.act_index + 1 >= acts.len() {
                return Ok(true);
            }
            // Los rivales que ya salieron en esta run o en la anterior no se repiten (si hay otros).
            let vistos = self.rivales.iter().chain(&self.evitar).cloned().collect();
            self.act_index += 1;
            self.act = start_act(&self.seed, self.act_index, &acts[self.act_index], &mut self.rng_upfront, vistos);
            self.position = self.act.map.start;
            self.path = vec![self.position];
        }
        Ok(false)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(id: &str, kind: EncounterKind, tag: &str) -> Encounter {
        Encounter {
            id: id.into(),
            name: id.into(),
            kind,
            warp_flag: 0,
            defeat_flag: 0,
            tags: vec![tag.into()],
            llegada: None,
            chr: None,
            npc: None,
            think: None,
            vida: None,
            enemigos: vec![],
            formato: Formato::Solo,
            oleadas: vec![],
            lider: None,
            segundos: None,
            mutadores: vec![],
            anim_inicio: None,
            arena: None,
            fase_en: None,
            renacer: None,
            ultimo_en_pie: vec![],
        }
    }

    #[test]
    fn mezclas_distintas_por_run_e_iguales_en_los_dos_pcs() {
        let x = |chr: i32| Enemigo { chr, npc: chr * 10000, think: chr * 10000, anim: None, efecto: None };
        let mut posibles = vec![];
        for a in 0..12 {
            for b in a + 1..12 {
                let mut e = enc(&format!("m{a}_{b}"), EncounterKind::Normal, "mezcla");
                e.enemigos = vec![x(1000 + a), x(1000 + b)];
                posibles.push(e);
            }
        }
        let sorteo = |semilla: &str| {
            let mut r = Rng::named(seed_u64(semilla), "act_1_mezclas");
            sortear_mezclas(&posibles, EncounterKind::Normal, &mut r).into_iter().map(|e| e.id).collect::<Vec<_>>()
        };
        let (a, b, c) = (sorteo("RUN1"), sorteo("RUN1"), sorteo("RUN2"));
        assert_eq!(a, b, "misma semilla, mismas parejas (los dos PCs)");
        assert_ne!(a, c, "otra run, otras parejas");
        assert!(!a.is_empty() && a.len() <= 12);
        // Nadie en más de MEZCLA_USOS parejas, y sin parejas repetidas.
        let mut usos = std::collections::HashMap::new();
        let mezclas: Vec<&Encounter> = a.iter().map(|id| posibles.iter().find(|e| &e.id == id).unwrap()).collect();
        for e in &mezclas {
            for y in &e.enemigos {
                *usos.entry(y.chr).or_insert(0) += 1;
            }
        }
        assert!(usos.values().all(|&n| n <= MEZCLA_USOS));
        assert_eq!(a.iter().collect::<HashSet<_>>().len(), a.len());
        // Sin posibles de ese tipo, nada.
        let mut r = Rng::named(1, "x");
        assert!(sortear_mezclas(&posibles, EncounterKind::Elite, &mut r).is_empty());
    }

    #[test]
    fn grupos_y_oleadas() {
        let mut e = enc("g", EncounterKind::Normal, "g");
        assert!(!e.en_coliseo());
        let x = Enemigo { chr: 4311, npc: 43110000, think: 43110000, anim: None, efecto: None };
        e.enemigos = vec![x; 5];
        assert!(e.en_coliseo());
        assert_eq!(e.grupos(), vec![vec![0, 1, 2, 3, 4]]);
        e.oleadas = vec![vec![0, 1], vec![2, 3, 9], vec![]];
        assert_eq!(e.grupos(), vec![vec![0, 1], vec![2, 3]]);
        assert_eq!(rival(&e), "grupo:g");
        // Se guarda en run.json sin los campos vacíos y vuelve igual.
        let s = serde_json::to_string(&e).unwrap();
        assert!(!s.contains("lider") && !s.contains("mutadores"));
        assert_eq!(serde_json::from_str::<Encounter>(&s).unwrap(), e);
    }

    #[test]
    fn coop_mapa_comun_y_recompensas_de_cada_uno() {
        let mut a = RunState::nueva("CO1", &acts(), vec![]);
        let mut b = RunState::nueva("CO1", &acts(), vec![]);
        a.personal(76561198000000001);
        b.personal(76561198000000002);
        assert_eq!(a.act.map.nodes.len(), b.act.map.nodes.len());
        assert_eq!(a.act.map.boss, b.act.map.boss);
        assert_eq!(a.comun().range(0, 1_000_000), b.comun().range(0, 1_000_000));
        let ra: Vec<i32> = (0..8).map(|_| a.rng_rewards.range(0, 1000)).collect();
        let rb: Vec<i32> = (0..8).map(|_| b.rng_rewards.range(0, 1000)).collect();
        assert_ne!(ra, rb);
    }

    pub(super) fn acts() -> Vec<ActDef> {
        let mut e = vec![];
        for i in 0..4 {
            e.push(enc(&format!("w{i}"), EncounterKind::Weak, &format!("tw{}", i % 2)));
            e.push(enc(&format!("n{i}"), EncounterKind::Normal, &format!("tn{}", i % 3)));
            e.push(enc(&format!("e{i}"), EncounterKind::Elite, &format!("te{i}")));
        }
        e.push(enc("b0", EncounterKind::Boss, "b"));
        e.push(enc("b1", EncounterKind::Boss, "b"));
        let a = ActDef { name: "Acto".into(), shape: ActShape::rogue_ring_default(), weak_count: 3, encounters: e, mezclas: vec![] };
        vec![a.clone(), a]
    }

    /// Juega una run entera eligiendo siempre el primer nodo disponible.
    #[test]
    fn run_completa_y_guardado() {
        let acts = acts();
        let mut r = RunState::new("ELDEN", &acts);
        let mut combates = 0;
        loop {
            let next = r.available()[0];
            if let NodeAction::Combat(_) = r.enter(next).unwrap() {
                combates += 1;
            }
            // Guardar y cargar a mitad no cambia nada.
            let r2 = RunState::from_json(&r.to_json()).unwrap();
            assert_eq!(r2.to_json(), r.to_json());
            r = r2;
            if r.complete(&acts).unwrap() {
                break;
            }
        }
        assert!(combates >= 4);
    }

    #[test]
    fn misma_semilla_mismos_encuentros() {
        let acts = acts();
        let a = RunState::new("MARIKA", &acts);
        let b = RunState::new("MARIKA", &acts);
        assert_eq!(a.to_json(), b.to_json());
        // Sin etiqueta repetida seguida en la cola normal.
        for w in a.act.normal_queue.windows(2) {
            assert!(!w[1].tags.iter().any(|t| w[0].tags.contains(t)));
        }
    }

    #[test]
    fn no_se_puede_saltar_nodos() {
        let acts = acts();
        let mut r = RunState::new("RADAGON", &acts);
        assert!(r.enter(r.act.map.boss).is_err());
    }
}

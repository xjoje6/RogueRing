//! Los ficheros de data/ se editan a mano: estas pruebas comprueban que lo que unos nombran existe en
//! los otros y que los textos siguen las reglas de las cartas. Las imágenes salen del juego
//! (extract/ui); si no están extraídas, esa parte no se comprueba.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rogue_core::characters::CharacterFile;
use rogue_core::events::{Effect, EventFile};
use rogue_core::relics::RelicFile;
use rogue_core::rewards::{Category, Item, Rarity};
use rogue_core::run::{rival, ActDef, Encounter, EncounterKind, Formato, RunState};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn leer<T: serde::de::DeserializeOwned>(rel: &str) -> T {
    let p = root().join(rel);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Texto de carta: sin guion largo ni exclamaciones, sin espacios de más.
fn texto_ok(donde: &str, t: &str, fallos: &mut Vec<String>) {
    if t.contains('\u{2014}') || t.contains('!') || t.contains("  ") || t != t.trim() {
        fallos.push(format!("{donde}: {t:?}"));
    }
}

/// Carpeta de imágenes extraídas del juego, si existe.
fn imagenes(sub: &str) -> Option<PathBuf> {
    let d = root().join("extract").join("ui").join(sub);
    d.is_dir().then_some(d)
}

/// ¿Hay imágenes extraídas y falta esta?
fn falta(dir: &Option<PathBuf>, nombre: impl std::fmt::Display) -> bool {
    dir.as_ref().is_some_and(|d| !d.join(format!("{nombre}.png")).exists())
}

fn acabar(fallos: Vec<String>) {
    assert!(fallos.is_empty(), "{} fallos:\n{}", fallos.len(), fallos.join("\n"));
}

#[test]
fn bendiciones() {
    let rel: RelicFile = leer("data/reliquias.json");
    let chars: CharacterFile = leer("data/personajes.json");
    let personajes: HashSet<&str> = chars.personajes.iter().map(|c| c.id.as_str()).collect();
    let iconos = imagenes("iconos");
    let mut ids = HashSet::new();
    let mut fallos = vec![];
    // ModBuilder lee «campos» de cada nivel (sus efectos en SpEffectParam), aunque esté vacío.
    let crudo: Value = leer("data/reliquias.json");
    for r in crudo["reliquias"].as_array().into_iter().flatten() {
        if r["niveles"].as_array().into_iter().flatten().any(|n| n.get("campos").is_none()) {
            fallos.push(format!("{}: nivel sin «campos» (ModBuilder se para)", r["id"]));
        }
    }
    for r in &rel.reliquias {
        if !ids.insert(&r.id) {
            fallos.push(format!("{}: id repetido", r.id));
        }
        texto_ok(&r.id, &r.nombre, &mut fallos);
        if r.niveles.is_empty() || r.niveles.iter().any(|n| n.texto.is_empty()) {
            fallos.push(format!("{}: nivel sin texto", r.id));
        }
        for n in &r.niveles {
            texto_ok(&r.id, &n.texto, &mut fallos);
        }
        // Las míticas enseñan su precio; las demás no tienen.
        match (&r.contra, r.rareza == Rarity::Mitica) {
            (Some(c), true) if !c.is_empty() => texto_ok(&r.id, c, &mut fallos),
            (None, false) => {}
            _ => fallos.push(format!("{}: precio (contra) y rareza no cuadran", r.id)),
        }
        if r.personaje.as_ref().is_some_and(|p| !personajes.contains(p.as_str())) {
            fallos.push(format!("{}: personaje desconocido {:?}", r.id, r.personaje));
        }
        if falta(&iconos, r.icono) {
            fallos.push(format!("{}: falta el icono {}", r.id, r.icono));
        }
    }
    // Cada personaje tiene sus propias.
    for c in &chars.personajes {
        let n = rel.reliquias.iter().filter(|r| r.personaje.as_deref() == Some(c.id.as_str()) && !r.pendiente).count();
        if n < 5 {
            fallos.push(format!("{}: solo {n} bendiciones propias", c.id));
        }
    }
    acabar(fallos);
}

#[test]
fn personajes_y_objetos() {
    let chars: CharacterFile = leer("data/personajes.json");
    let pool: Vec<Item> = leer("data/objetos/pool.json");
    let iconos = imagenes("iconos");
    let mut fallos = vec![];
    let mut ids = HashSet::new();
    for it in &pool {
        if !ids.insert(it.id) {
            fallos.push(format!("objeto {} repetido", it.id));
        }
        texto_ok(&format!("objeto {}", it.id), &it.nombre, &mut fallos);
        if falta(&iconos, it.icono) {
            fallos.push(format!("objeto {} ({}): falta el icono {}", it.id, it.nombre, it.icono));
        }
    }
    for c in &chars.personajes {
        texto_ok(&c.id, &c.lema, &mut fallos);
        texto_ok(&c.id, &c.pasiva, &mut fallos);
        if c.armas.len() != 3 {
            fallos.push(format!("{}: {} armas (de salida, a las 3 victorias y a las 10)", c.id, c.armas.len()));
        }
        for a in &c.armas {
            match pool.iter().find(|i| i.id == *a) {
                Some(i) if i.categoria == Category::Arma => {
                    // Con lo que suben los atributos para empuñarla, sigue cerca del nivel 150.
                    let mut con = c.clone();
                    con.atributos = c.atributos_para(i.req);
                    if con.level() > 175 {
                        fallos.push(format!("{}: con {} pasa a nivel {}", c.id, i.nombre, con.level()));
                    }
                }
                _ => fallos.push(format!("{}: el arma {a} no está en el pool", c.id)),
            }
        }
        if c.ceniza.is_some_and(|g| !pool.iter().any(|i| i.id == g && i.categoria == Category::Ceniza)) {
            fallos.push(format!("{}: la ceniza {:?} no está en el pool", c.id, c.ceniza));
        }
    }
    acabar(fallos);
}

#[test]
fn eventos() {
    let ev: EventFile = leer("data/eventos.json");
    let rel: RelicFile = leer("data/reliquias.json");
    let muts: Value = leer("data/mutadores.json");
    let niveles: HashSet<&str> = muts["mutadores"].as_array().unwrap().iter().filter_map(|m| m["nivel"].as_str()).collect();
    let artes = imagenes("carga");
    let mut fallos = vec![];
    for e in &ev.eventos {
        texto_ok(&e.id, &e.titulo, &mut fallos);
        if falta(&artes, &e.imagen) {
            fallos.push(format!("{}: falta el arte {}", e.id, e.imagen));
        }
        let efectos = e.opciones.iter().flat_map(|o| o.efectos.iter().chain(o.azar.iter().flat_map(|b| b.efectos.iter())));
        for f in efectos {
            match f {
                Effect::Maldicion(id) if !rel.get(id).is_some_and(|r| r.maldita) => fallos.push(format!("{}: {id} no es una maldición", e.id)),
                Effect::ReliquiaFija(id) if rel.get(id).is_none() => fallos.push(format!("{}: no existe la bendición {id}", e.id)),
                Effect::Mutador(n) if !niveles.contains(n.as_str()) => fallos.push(format!("{}: no hay mutadores de nivel {n}", e.id)),
                _ => {}
            }
        }
    }
    for m in muts["mutadores"].as_array().unwrap() {
        let id = m["id"].as_str().unwrap_or("?");
        texto_ok(id, m["nombre"].as_str().unwrap_or(""), &mut fallos);
        texto_ok(id, m["texto"].as_str().unwrap_or(""), &mut fallos);
    }
    acabar(fallos);
}

#[test]
fn actos_y_arenas() {
    let esc: Value = leer("data/escalado.json");
    let jefes: Value = leer("data/arenas_jefes.json");
    let flags: HashSet<u64> = jefes["arenas"].as_array().unwrap().iter().filter_map(|a| a["flag"].as_u64()).collect();
    let mut fallos = vec![];
    for n in 1..=ACTOS {
        let act: ActDef = leer(&format!("data/acts/{n}.json"));
        let mut ids = HashSet::new();
        for e in &act.encounters {
            if !ids.insert(&e.id) {
                fallos.push(format!("acto {n}: encuentro {} repetido", e.id));
            }
            if !coliseo_o_arena(e, &flags) {
                fallos.push(format!("acto {n}: {} ({}) sin chr/npc/think ni arena", e.name, e.id));
            }
            fallos.extend(grupo_mal(e).map(|m| format!("acto {n}: {} ({}) {m}", e.name, e.id)));
        }
        let de = |k: EncounterKind| act.encounters.iter().filter(|e| e.kind == k).count();
        if de(EncounterKind::Boss) == 0 || de(EncounterKind::Elite) == 0 || de(EncounterKind::Weak) < act.weak_count {
            fallos.push(format!("acto {n}: faltan jefes, élites o débiles"));
        }
        let t = &esc["tramos"][n.to_string()];
        if t["debil"].as_i64().is_none() || t["normal"].as_array().is_none_or(|v| v.len() != 2) || t["jefe"].as_i64().is_none() {
            fallos.push(format!("escalado.json: tramos del acto {n} incompletos"));
        }
        if ["debil", "normal", "elite", "jefe"].iter().any(|k| esc["vida"][n.to_string()][k].as_f64().is_none()) {
            fallos.push(format!("escalado.json: falta la vida por tipo del acto {n}"));
        }
    }
    acabar(fallos);
}

/// Actos de una run (data/acts/1.json ... ACTOS.json).
const ACTOS: usize = 3;

/// En el coliseo hace falta qué crear (cada enemigo con su NpcParam e IA; chr 0 son los humanos, c0000);
/// en su sitio del mundo, cómo llegar.
fn coliseo_o_arena(e: &Encounter, flags: &HashSet<u64>) -> bool {
    let lista = e.lista();
    let coliseo = !lista.is_empty() && lista.iter().all(|x| x.chr >= 0 && x.npc > 0 && x.think > 0);
    coliseo || flags.contains(&(e.warp_flag as u64))
}

/// Combates de grupo: hasta 10 enemigos y 5 oleadas (huecos y flags del coliseo), escolta con líder,
/// aguante y contrarreloj con sus segundos, oleadas que nombran enemigos que existen.
fn grupo_mal(e: &Encounter) -> Option<String> {
    let n = e.lista().len();
    if n > 10 {
        return Some(format!("tiene {n} enemigos (máximo 10)"));
    }
    if e.grupos().len() > 5 {
        return Some("tiene más de 5 oleadas".into());
    }
    if e.oleadas.iter().flatten().any(|&i| i >= n) {
        return Some("una oleada nombra un enemigo que no está".into());
    }
    match e.formato {
        Formato::Escolta if e.lider.is_none_or(|l| l >= n) => Some("escolta sin líder".into()),
        Formato::Aguante if e.segundos.is_none() => Some("aguante sin segundos".into()),
        Formato::Oleadas | Formato::Racha if e.oleadas.len() < 2 => Some("oleadas sin oleadas".into()),
        Formato::Manada | Formato::Duo | Formato::Escolta if n < 2 => Some("grupo de uno".into()),
        _ => None,
    }
}

/// Las listas del cooperativo (data/coop) son encuentros del coliseo bien formados, con dos rivales.
#[test]
fn listas_del_cooperativo() {
    let mut fallos = vec![];
    for n in 1..=ACTOS {
        let v: Value = leer(&format!("data/coop/{n}.json"));
        let encs: Vec<Encounter> = serde_json::from_value(v["encounters"].clone()).unwrap();
        if encs.is_empty() {
            fallos.push(format!("coop {n}: vacío"));
        }
        for e in &encs {
            if !coliseo_o_arena(e, &HashSet::new()) {
                fallos.push(format!("coop {n}: {} sin enemigos del coliseo", e.id));
            }
            fallos.extend(grupo_mal(e).map(|m| format!("coop {n}: {} {m}", e.id)));
        }
    }
    acabar(fallos);
}

/// Un mismo rival (mismo modelo, aunque sean entradas distintas del juego) no sale dos veces en los
/// combates que se suelen jugar de un acto, ni una élite repite a uno de ellos.
#[test]
fn rivales_sin_repetir() {
    let acts: Vec<ActDef> = (1..=ACTOS).map(|n| leer(&format!("data/acts/{n}.json"))).collect();
    let mut fallos = vec![];
    for k in 0..300 {
        let run = RunState::new(&format!("PRUEBA{k}"), &acts);
        let combates: Vec<String> = run.act.normal_queue.iter().take(6).map(rival).collect();
        let elites: Vec<String> = run.act.elite_queue.iter().take(3).map(rival).collect();
        let todos: Vec<&String> = combates.iter().chain(&elites).collect();
        if todos.iter().collect::<HashSet<_>>().len() != todos.len() {
            fallos.push(format!("PRUEBA{k}: rival repetido en {todos:?}"));
        }
    }
    acabar(fallos);
}

/// La run siguiente prefiere rivales que no salieron en la anterior: con los 6 débiles del acto 1,
/// los 3 primeros combates son los otros 3.
#[test]
fn rivales_de_la_run_anterior() {
    let acts: Vec<ActDef> = (1..=ACTOS).map(|n| leer(&format!("data/acts/{n}.json"))).collect();
    let mut fallos = vec![];
    for k in 0..200 {
        let antes = RunState::new(&format!("ANTES{k}"), &acts);
        let vistos: Vec<String> = antes.act.normal_queue.iter().take(acts[0].weak_count).map(rival).collect();
        let despues = RunState::nueva(&format!("DESPUES{k}"), &acts, vistos.clone());
        let repetidos: Vec<String> = despues.act.normal_queue.iter().take(acts[0].weak_count).map(rival).filter(|r| vistos.contains(r)).collect();
        if !repetidos.is_empty() {
            fallos.push(format!("DESPUES{k}: repite {repetidos:?} de {vistos:?}"));
        }
    }
    acabar(fallos);
}

/// Textos del código que pasan por idioma::t / tf (o por las ayudas de botones), en inglés.
fn textos_de_pantalla() -> HashSet<String> {
    let mut out = HashSet::new();
    // Solo el código de cada crate (src), no las pruebas.
    let mut pila: Vec<PathBuf> = ["core", "harness", "ui"].iter().map(|c| root().join("dll").join(c).join("src")).collect();
    while let Some(d) = pila.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                if !p.ends_with("target") {
                    pila.push(p);
                }
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let s = std::fs::read_to_string(&p).unwrap();
            for pre in ["t(", "tf("] {
                let mut resto = s.as_str();
                while let Some(i) = resto.find(pre) {
                    let antes = resto[..i].chars().last();
                    resto = &resto[i + pre.len()..];
                    if antes.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    // El texto puede ir en la línea siguiente si rustfmt parte la llamada.
                    let Some(r) = resto.trim_start().strip_prefix('"') else { continue };
                    resto = r;
                    // Hasta la comilla que cierra (las escapadas no cierran).
                    let mut lit = String::new();
                    let mut cs = resto.chars();
                    while let Some(c) = cs.next() {
                        match c {
                            '\\' => lit.push(cs.next().unwrap_or('\\')),
                            '"' => break,
                            c => lit.push(c),
                        }
                    }
                    out.insert(lit);
                }
            }
        }
    }
    out
}

/// La traducción al español (data/i18n/es.json) lo cubre todo: cada texto de pantalla del código,
/// cada bendición con sus niveles y su precio, cada evento con sus opciones y suertes, personajes,
/// mutadores, objetos y enemigos. Y sin guiones largos.
#[test]
fn traduccion_completa() {
    let t: rogue_core::idioma::Traduccion = leer("data/i18n/es.json");
    let crudo = std::fs::read_to_string(root().join("data/i18n/es.json")).unwrap();
    let mut fallos = vec![];
    if crudo.contains('\u{2014}') {
        fallos.push("es.json tiene guiones largos".into());
    }
    let claves = textos_de_pantalla();
    // Si la búsqueda en el código fallara, la prueba no comprobaría nada.
    assert!(claves.len() > 150, "solo {} textos de pantalla en el código", claves.len());
    for k in claves {
        if t.ui.get(&k).is_none_or(|v| v.trim().is_empty()) {
            fallos.push(format!("ui sin traducir: {k:?}"));
        }
    }
    let rel: RelicFile = leer("data/reliquias.json");
    for r in &rel.reliquias {
        match t.bendiciones.get(&r.id) {
            None => fallos.push(format!("bendición sin traducir: {}", r.id)),
            Some(b) => {
                if b.niveles.len() != r.niveles.len() {
                    fallos.push(format!("{}: {} niveles traducidos de {}", r.id, b.niveles.len(), r.niveles.len()));
                }
                if b.contra.is_some() != r.contra.is_some() {
                    fallos.push(format!("{}: el precio no cuadra con el inglés", r.id));
                }
            }
        }
    }
    let ev: EventFile = leer("data/eventos.json");
    for e in &ev.eventos {
        match t.eventos.get(&e.id) {
            None => fallos.push(format!("evento sin traducir: {}", e.id)),
            Some(te) => {
                if te.opciones.len() != e.opciones.len() {
                    fallos.push(format!("{}: {} opciones traducidas de {}", e.id, te.opciones.len(), e.opciones.len()));
                }
                for (k, (o, to)) in e.opciones.iter().zip(&te.opciones).enumerate() {
                    if o.azar.len() != to.azar.len() {
                        fallos.push(format!("{} opción {k}: {} suertes traducidas de {}", e.id, to.azar.len(), o.azar.len()));
                    }
                }
            }
        }
    }
    let chars: CharacterFile = leer("data/personajes.json");
    for c in &chars.personajes {
        if !t.personajes.contains_key(&c.id) {
            fallos.push(format!("personaje sin traducir: {}", c.id));
        }
    }
    let muts: Value = leer("data/mutadores.json");
    for m in muts["mutadores"].as_array().unwrap() {
        let id = m["id"].as_str().unwrap();
        if !t.mutadores.contains_key(id) {
            fallos.push(format!("mutador sin traducir: {id}"));
        }
    }
    let pool: Vec<Item> = leer("data/objetos/pool.json");
    for it in &pool {
        if !t.objetos.contains_key(&it.id.to_string()) {
            fallos.push(format!("objeto sin nombre en español: {} {}", it.id, it.nombre));
        }
        if !t.ui.contains_key(&it.tipo) {
            fallos.push(format!("tipo sin traducir: {}", it.tipo));
        }
    }
    for n in 1..=ACTOS {
        let act: ActDef = leer(&format!("data/acts/{n}.json"));
        if !t.ui.contains_key(&act.name) {
            fallos.push(format!("acto sin traducir: {}", act.name));
        }
        for e in &act.encounters {
            if e.name.trim().is_empty() {
                fallos.push(format!("acto {n}: encuentro {} sin nombre", e.id));
            } else if !t.enemigos.contains_key(&e.name) {
                fallos.push(format!("enemigo sin nombre en español: {}", e.name));
            }
        }
        // Y los dúos del cooperativo.
        let v: Value = leer(&format!("data/coop/{n}.json"));
        for e in v["encounters"].as_array().into_iter().flatten() {
            let nombre = e["name"].as_str().unwrap_or("");
            if nombre.trim().is_empty() || !t.enemigos.contains_key(nombre) {
                fallos.push(format!("coop {n}: {} sin nombre en español ({nombre:?})", e["id"]));
            }
        }
    }
    acabar(fallos);
}

#[test]
fn musica() {
    let m: Value = leer("data/musica.json");
    let temas: Vec<i64> = m["temas"].as_array().unwrap().iter().filter_map(|v| v.as_i64()).collect();
    let mut fallos = vec![];
    // Los flags de cada tema van de 100 en 100 (tools/ModBuilder).
    if temas.is_empty() || temas.len() > 100 {
        fallos.push(format!("musica.json: {} temas (entre 1 y 100)", temas.len()));
    }
    for g in m["genericos"].as_array().unwrap().iter().filter_map(|v| v.as_i64()) {
        if !temas.contains(&g) {
            fallos.push(format!("musica.json: el genérico {g} no está en temas"));
        }
    }
    acabar(fallos);
}

#[test]
fn iconos_de_bendiciones_sin_repetir() {
    let rel: RelicFile = leer("data/reliquias.json");
    let pool: Vec<Item> = leer("data/objetos/pool.json");
    let objetos: HashSet<i32> = pool.iter().map(|i| i.icono).collect();
    let mut vistos = std::collections::HashMap::new();
    let mut fallos = vec![];
    for r in &rel.reliquias {
        if let Some(otra) = vistos.insert(r.icono, &r.id) {
            fallos.push(format!("{} y {otra} comparten el icono {}", r.id, r.icono));
        }
        // Tampoco el de un arma o una ceniza: en las recompensas se confundirían.
        if objetos.contains(&r.icono) {
            fallos.push(format!("{}: su icono {} es el de un objeto del pool", r.id, r.icono));
        }
    }
    acabar(fallos);
}

//! Lo que la interfaz dibuja: copia del estado del director en datos puros.

use super::*;
use rogue_core::idioma::{t, td, tf};
use rogue_ui::evento::Modo;

pub(super) fn slot_name(slot: u32) -> String {
    match slot {
        0 | 2 | 4 => tf("Left hand {}", &[&(slot / 2 + 1)]),
        1 | 3 | 5 => tf("Right hand {}", &[&(slot / 2 + 1)]),
        17..=20 => tf("Talisman {}", &[&(slot - 16)]),
        _ => tf("Slot {}", &[&slot]),
    }
}

/// Un nodo de la historia para el gráfico del final (iconos por id; los carga el overlay).
#[derive(Clone, Debug)]
pub struct HistView {
    pub acto: usize,
    pub tipo: String,
    pub vida_min: f64,
    pub bendiciones: Vec<(i32, bool)>,
    pub objetos: Vec<i32>,
    pub detalle: Option<rogue_ui::grafico::Detalle>,
}

/// La run guardada para el menú principal (iconos por id; los carga el overlay).
#[derive(Clone)]
pub struct RunGuardada {
    pub run: RunState,
    pub act_name: String,
    pub personaje: String,
    pub arma_icono: i32,
    pub arma_nombre: String,
    /// (icono, maldita) de lo que lleva.
    pub bendiciones: Vec<(i32, bool)>,
}

/// Carta para la interfaz (datos puros; el overlay pone la textura del icono).
pub struct UiCard {
    pub cat: String,
    pub nombre: String,
    pub tipo: String,
    pub rareza: Option<Rarity>,
    pub icono: i32,
    pub pie: String,
    pub apagada: bool,
    /// Lo malo que trae (míticas), en rojo bajo el texto.
    pub contra: String,
    /// Explicación al elegirla: (título, texto).
    pub nota: Option<(String, String)>,
    /// Arma doble: el icono sale dos veces.
    pub doble: bool,
}

/// Cooperativo en el mapa: votos de los demás (nodo, nombre), el mío, a quién se espera, sus cursores
/// y si aún no se puede votar (alguno sigue con lo del nodo anterior).
pub type VistaCoop = (Vec<((i32, i32), String)>, Option<(i32, i32)>, Vec<String>, Vec<((i32, i32), String)>, bool);

/// Lo que la interfaz necesita para dibujar este fotograma (una copia por fotograma).
#[allow(clippy::large_enum_variant)]
pub enum UiState {
    None,
    Map {
        run: RunState,
        act_name: String,
        selected: Option<(i32, i32)>,
        relics: Vec<(String, String, i32, bool)>,
        /// Viales que quedan y máximo.
        viales: Option<(u32, u32)>,
        /// Nivel de mejora del arma y máximo.
        arma: Option<(u32, u32)>,
        /// Nombre del arma que llevas.
        arma_nombre: Option<String>,
        /// Invocador: la invocación que lleva.
        invocacion: Option<String>,
        /// Cooperativo: votos de los demás, el mío y a quién se espera (None fuera de cooperativo).
        coop: Option<VistaCoop>,
    },
    Cards {
        cabecera: String,
        titulo: String,
        linea: String,
        instruccion: String,
        cards: Vec<UiCard>,
        selected: usize,
        salir: Option<String>,
        ayuda: Vec<(String, String)>,
        destacada: Option<UiCard>,
        /// Arte de fondo (extract/ui/carga) en lugar del mapa: la tienda de Kalé.
        fondo: Option<String>,
    },
    Over {
        won: bool,
        act: usize,
        nodes: usize,
        items: usize,
        seed: String,
        personaje: String,
        golpes: u32,
        elites: u32,
        miticas: Vec<String>,
        relics: Vec<(String, i32, bool)>,
        historia: Vec<HistView>,
        marcador: i32,
        sel: usize,
    },
    Event {
        cabecera: String,
        titulo: String,
        texto: String,
        imagen: String,
        /// (texto, detalle, apagada, explicación de la maldición que da).
        opciones: Vec<(String, String, bool, Option<String>)>,
        selected: usize,
        /// Evento, pausa o menú sencillo (rogue_ui::evento::Modo).
        modo: rogue_ui::evento::Modo,
    },
    /// Menú principal: las opciones y, a la derecha, la run guardada.
    Titulo {
        /// (texto, detalle, apagada) de cada opción.
        opciones: Vec<(String, String, bool)>,
        selected: usize,
        /// Cifras del perfil o la bienvenida.
        texto: String,
        guardada: Option<RunGuardada>,
    },
    /// (nombre, texto, icono, maldita) de lo que llevas.
    Blessings {
        items: Vec<(String, String, i32, bool)>,
        /// La elegida (su ficha a la derecha).
        sel: usize,
    },
    /// Pantalla de Personajes: (nombre, tipo, candado, icono) de las armas iniciales y la elegida. El
    /// candado dice cuántas victorias faltan; None si ya se puede elegir.
    Personajes {
        nombre: String,
        lema: String,
        pasiva: String,
        armas: Vec<(String, String, Option<String>, i32)>,
        arma_sel: usize,
        /// (nombre, candado) de sus apariencias: la de siempre primero.
        apariencias: Vec<(String, Option<String>)>,
        apariencia_sel: usize,
        /// (nombre, candado) de sus invocaciones, si tiene, y la elegida.
        invocaciones: Vec<(String, Option<String>)>,
        invocacion_sel: usize,
        idx: usize,
        total: usize,
    },
    /// (nombre, texto, icono, rareza, etiqueta, vista) de todas las bendiciones.
    Compendio {
        items: Vec<(String, String, i32, String, String, bool)>,
        sel: usize,
    },
}

pub(super) fn item_card(it: &Item) -> UiCard {
    let cat = match it.categoria {
        rewards::Category::Arma => t("WEAPON"),
        rewards::Category::Ceniza => t("ASH OF WAR"),
        rewards::Category::Hechizo => t("SPELL"),
        rewards::Category::Talisman => t("TALISMAN"),
        rewards::Category::Invocacion => t("SPIRIT ASH"),
    };
    // El tipo de arma es también un dato (estilos de cada personaje): se traduce solo al enseñarlo.
    let tipo = if it.doble { tf("Dual {}", &[&td(&it.tipo)]) } else { td(&it.tipo) };
    let cat = if it.doble { t("DUAL WEAPON") } else { cat };
    UiCard {
        cat: cat.into(),
        nombre: it.nombre.clone(),
        tipo,
        rareza: Some(it.rareza),
        icono: it.icono,
        pie: String::new(),
        apagada: false,
        doble: it.doble,
        contra: String::new(),
        nota: None,
    }
}

pub(super) fn relic_card(r: &RelicDef, nivel: usize, cat: &str) -> UiCard {
    let texto = r.niveles.get(nivel).map(|l| l.texto.clone()).unwrap_or_default();
    let cat = if r.rareza == Rarity::Mitica { t("MYTHIC BLESSING") } else { cat };
    UiCard {
        cat: cat.into(),
        nombre: r.nombre.clone(),
        tipo: texto,
        contra: r.contra.clone().unwrap_or_default(),
        rareza: Some(r.rareza),
        icono: r.icono,
        pie: String::new(),
        apagada: false,
        doble: false,
        nota: None,
    }
}

impl Director {
    /// Qué hace la maldición que da una opción de evento (directa o en alguna de sus suertes).
    pub(super) fn maldicion_de(&self, o: &rogue_core::events::EventOption) -> Option<String> {
        let r = o.efectos.iter().chain(o.azar.iter().flat_map(|b| b.efectos.iter())).find_map(|e| match e {
            Effect::Maldicion(id) => self.relics.get(id),
            _ => None,
        })?;
        Some(format!("{}: {}", r.nombre, texto_entero(r)))
    }
}

/// Nombre con el nivel detrás cuando pasa del primero («Anvil II»).
pub(super) fn nombre_nivel(r: &RelicDef, nivel: usize) -> String {
    match nivel {
        0 => r.nombre.clone(),
        1 => format!("{} II", r.nombre),
        2 => format!("{} III", r.nombre),
        n => format!("{} {}", r.nombre, n + 1),
    }
}

/// Texto de una bendición con su precio detrás (listas y compendio).
pub(super) fn texto_entero(r: &RelicDef) -> String {
    texto_nivel(r, 0)
}

/// Texto de una bendición en un nivel, con su precio detrás.
pub(super) fn texto_nivel(r: &RelicDef, nivel: usize) -> String {
    let t = r.niveles.get(nivel).or(r.niveles.first()).map(|l| l.texto.clone()).unwrap_or_default();
    match &r.contra {
        Some(c) => format!("{t} {c}"),
        None => t,
    }
}

/// Ayuda de botones (la etiqueta, en el idioma del juego).
pub(super) fn ayuda(xs: &[(&str, &str)]) -> Vec<(String, String)> {
    xs.iter().map(|(a, b)| (a.to_string(), td(b))).collect()
}

/// Copia del estado para la interfaz (sin bloquear el hilo del juego si está ocupado).
/// Lo que se ve; en las pantallas de elegir (recompensa, tesoro, tienda...), con el arma que llevas en
/// la línea de debajo del título, para elegir sabiéndolo (23-10).
pub fn ui_state() -> Option<UiState> {
    let mut s = ui_state_base()?;
    if let UiState::Cards { linea, .. } = &mut s {
        let arma = DIRECTOR.try_lock().ok().and_then(|g| {
            let d = g.as_ref()?;
            let nombre = d.current_weapon_name()?;
            Some(match d.arma {
                Some((n, _)) if n > 0 => format!("{nombre} +{n}"),
                _ => nombre,
            })
        });
        if let Some(a) = arma {
            let txt = tf("Your weapon: {}", &[&a]);
            *linea = if linea.is_empty() { txt } else { format!("{linea}   {txt}") };
        }
    }
    Some(s)
}

fn ui_state_base() -> Option<UiState> {
    let g = DIRECTOR.try_lock().ok()?;
    let d = g.as_ref()?;
    let run = d.run.clone()?;
    let runas = crate::runes().unwrap_or(0);
    if let Some((titulo, opciones, selected)) = d.vista_pruebas() {
        return Some(UiState::Event { cabecera: t("TESTS").into(), titulo, texto: String::new(), imagen: String::new(), opciones, selected, modo: Modo::Menu });
    }
    if d.ver_ascension {
        return Some(UiState::Event {
            cabecera: t("ASCENSION").into(),
            titulo: tf("Ascension {}", &[&run.ascension]),
            texto: d.texto_ascension(),
            imagen: String::new(),
            opciones: vec![],
            selected: 0,
            modo: Modo::Menu,
        });
    }
    if let Some(psel) = d.pausa {
        let texto = tf("Act {}. Seed {}.", &[&(run.act_index + 1), &run.seed]);
        let opciones = d.pausa_opciones().into_iter().map(|o| (o.0, o.1, o.2, None)).collect();
        // En combate, en lugar del arte va el detalle de las bendiciones (overlay, PAUSA_COMBATE).
        let imagen = if d.fx.as_ref().is_some_and(|f| f.motor.is_some()) { String::new() } else { "MENU_Load_00004_PS5".into() };
        return Some(UiState::Event { cabecera: t("PAUSE").into(), titulo: t("Paused").into(), texto, imagen, opciones, selected: psel, modo: Modo::Pausa });
    }
    Some(match &d.phase {
        Phase::MainMenu if d.clave.is_some() => {
            let texto = d.vista_clave().unwrap_or_default();
            UiState::Event { cabecera: t("CO-OP").into(), titulo: t("Session password").into(), texto, imagen: String::new(), opciones: vec![], selected: 0, modo: Modo::Menu }
        }
        Phase::MainMenu if d.menu_coop && !d.show_compendio => {
            let (titulo, texto) = d.estado_sala();
            let opciones = d.coop_opciones().into_iter().map(|o| (o.0, o.1, o.2, None)).collect();
            UiState::Event { cabecera: t("CO-OP").into(), titulo, texto, imagen: String::new(), opciones, selected: d.sel, modo: Modo::Menu }
        }
        Phase::MainMenu if !d.show_compendio => {
            let victorias: u32 = d.perfil.victorias.values().sum();
            let mut texto = if d.perfil.runs == 0 {
                t("A boss rush across the Lands Between. Pick a character, cross the map, fight what waits.").to_string()
            } else {
                tf("Runs: {}. Wins: {}.", &[&d.perfil.runs, &victorias])
            };
            if d.desarrollo {
                texto = format!("{texto} {}", t("Developer build: everything unlocked."));
            }
            let opciones = d.menu_opciones().into_iter().map(|o| (o.0, o.1, o.2)).collect();
            let guardada = d.guardada.as_ref().map(|g| {
                let c = g.personaje.as_deref().and_then(|id| d.chars.iter().find(|c| c.id == id));
                let arma = c.and_then(|c| d.pool.iter().find(|i| i.id == d.arma_de(c)));
                RunGuardada {
                    run: g.clone(),
                    act_name: d.acts.get(g.act_index).map(|a| a.name.clone()).unwrap_or_default(),
                    personaje: c.map(|c| c.nombre.clone()).unwrap_or_default(),
                    arma_icono: arma.map(|i| i.icono).unwrap_or(0),
                    arma_nombre: arma.map(|i| i.nombre.clone()).unwrap_or_default(),
                    bendiciones: g.relics.iter().filter_map(|o| d.relics.get(&o.id)).map(|r| (r.icono, r.maldita)).collect(),
                }
            });
            UiState::Titulo { opciones, selected: d.sel, texto, guardada }
        }
        Phase::Personajes { idx } => {
            let c = d.chars.get(*idx).cloned()?;
            let elegida = d.arma_de(&c);
            let v = d.victorias(&c);
            UiState::Personajes {
                nombre: c.nombre.clone(),
                lema: c.lema.clone(),
                pasiva: c.pasiva.clone(),
                armas: c
                    .armas
                    .iter()
                    .enumerate()
                    .map(|(k, a)| {
                        let (nombre, tipo, icono) = d.pool.iter().find(|i| i.id == *a).map(|i| (i.nombre.clone(), td(&i.tipo), i.icono)).unwrap_or_default();
                        let pide = CharacterDef::victorias_para(k);
                        let candado = (!c.arma_libre(k, v)).then(|| tf("Win {} runs with the {} ({}/{})", &[&pide, &c.nombre, &v.min(pide), &pide]));
                        (nombre, tipo, candado, icono)
                    })
                    .collect(),
                arma_sel: c.armas.iter().position(|a| *a == elegida).unwrap_or(0),
                apariencias: std::iter::once((t("Default").to_string(), None))
                    .chain(c.apariencias.iter().map(|a| {
                        let candado = (v < a.victorias).then(|| tf("Win {} runs with the {} ({}/{})", &[&a.victorias, &c.nombre, &v.min(a.victorias), &a.victorias]));
                        (a.nombre.clone(), candado)
                    }))
                    .collect(),
                apariencia_sel: d
                    .perfil
                    .apariencias
                    .get(&c.id)
                    .and_then(|id| c.apariencias.iter().position(|a| &a.id == id && v >= a.victorias))
                    .map(|k| k + 1)
                    .unwrap_or(0),
                invocaciones: c
                    .invocaciones
                    .iter()
                    .enumerate()
                    .map(|(k, i)| {
                        let pide = CharacterDef::victorias_para(k);
                        let candado = (v < pide).then(|| tf("Win {} runs with the {} ({}/{})", &[&pide, &c.nombre, &v.min(pide), &pide]));
                        (i.nombre.clone(), candado)
                    })
                    .collect(),
                invocacion_sel: c
                    .invocacion(d.perfil.invocaciones.get(&c.id).map(String::as_str), v)
                    .and_then(|i| c.invocaciones.iter().position(|x| x.id == i.id))
                    .unwrap_or(0),
                idx: *idx,
                total: d.chars.len(),
            }
        }
        _ if d.show_compendio => UiState::Compendio {
            items: d
                .relics
                .reliquias
                .iter()
                .map(|r| {
                    let rareza = if r.maldita {
                        t("Curse")
                    } else {
                        match r.rareza {
                            Rarity::Comun => t("Common"),
                            Rarity::PocoComun => t("Uncommon"),
                            Rarity::Rara => t("Rare"),
                            Rarity::Mitica => t("Mythic"),
                        }
                    };
                    let etiqueta = r.personaje.as_deref().and_then(|p| d.chars.iter().find(|c| c.id == p)).map(|c| c.nombre.clone()).unwrap_or_default();
                    (r.nombre.clone(), texto_entero(r), r.icono, rareza.to_string(), etiqueta, d.desarrollo || d.compendio_dar || d.perfil.vistas.contains(&r.id))
                })
                .collect(),
            sel: d.compendio_sel,
        },
        Phase::Map if d.show_list => UiState::Blessings {
            items: run
                .relics
                .iter()
                .filter_map(|o| d.relics.get(&o.id).map(|r| (nombre_nivel(r, o.nivel), texto_nivel(r, o.nivel), r.icono, r.maldita)))
                .collect(),
            sel: d.lista_sel,
        },
        Phase::Map => {
            let act_name = d.acts.get(run.act_index).map(|a| a.name.clone()).unwrap_or_default();
            let selected = d.available_sorted().get(d.sel).copied();
            let relics = run
                .relics
                .iter()
                .filter_map(|o| d.relics.get(&o.id).map(|r| (nombre_nivel(r, o.nivel), texto_nivel(r, o.nivel), r.icono, r.maldita)))
                .collect();
            let coop = d.coop_activo().then(|| {
                let (votos, espera, cursores) = d.votos_coop();
                let paso = d.run.as_ref().map(|r| rogue_core::coop::Paso { acto: r.act_index, nodo: r.path.len() });
                let bloqueo = d.esperando_mapa();
                let esperar = !bloqueo.is_empty();
                (votos, d.voto.filter(|v| Some(v.0) == paso).map(|v| v.1), if esperar { bloqueo } else { espera }, cursores, esperar)
            });
            UiState::Map { run, act_name, selected, relics, viales: d.viales, arma: d.arma, arma_nombre: d.current_weapon_name(), invocacion: d.invocacion_actual().map(|i| i.nombre), coop }
        }
        Phase::Reward { enemy, runes, extra, cartas } => UiState::Cards {
            cabecera: t("DEFEATED").into(),
            titulo: enemy.clone(),
            linea: std::iter::once(tf("+ {} runes", &[runes])).chain(extra.iter().map(|b| format!("+ {b}"))).collect::<Vec<_>>().join("   "),
            instruccion: t("Pick one. It stays with you for the rest of the run.").into(),
            cards: cartas
                .iter()
                .map(|c| match c {
                    Carta::Objeto(it) => d.offer_card(it),
                    Carta::Bendicion(r) => relic_card(r, 0, t("BLESSING")),
                })
                .collect(),
            selected: d.sel,
            salir: Some(t("Skip").into()),
            ayuda: ayuda(&[("A", "Choose"), ("B", "Skip")]),
            destacada: None,
            fondo: None,
        },
        Phase::Treasure { relic, runes, trap } => UiState::Cards {
            cabecera: t("TREASURE").into(),
            titulo: t("A Chest").into(),
            linea: if trap.is_some() { tf("+ {} runes", &[&(runes * 3 / 2)]) } else { tf("+ {} runes", &[runes]) },
            instruccion: match (trap.is_some(), d.coop_activo() && d.listo.is_some() && d.listo == d.paso_actual()) {
                // Cooperativo: listo para el combate del cofre; a quién se espera.
                (true, true) if !d.esperando_listos().is_empty() => tf("Waiting for {}", &[&d.esperando_listos().join(", ")]),
                (true, _) => t("The chest moves. You'll have to earn it.").into(),
                _ => t("What's inside is yours.").into(),
            },
            cards: relic.iter().map(|r| relic_card(r, 0, t("BLESSING"))).collect(),
            selected: d.sel,
            // Dejarlo: el normal da las runas sin la bendición; el trampa, nada y sin pelear.
            salir: d.cofre_se_deja(trap.is_some()).then(|| t("Leave it").into()),
            ayuda: if d.cofre_se_deja(trap.is_some()) {
                ayuda(&[("A", if trap.is_some() { t("Fight") } else { t("Take") }), ("B", "Leave it")])
            } else {
                ayuda(&[("A", if trap.is_some() { t("Fight") } else { t("Take") })])
            },
            destacada: None,
            fondo: None,
        },
        Phase::Shop { wares } => UiState::Cards {
            cabecera: t("MERCHANT").into(),
            titulo: "Kalé".into(),
            linea: tf("You have {} runes", &[&runas]),
            instruccion: t("Kalé: \"Have a look. Everything here has a price.\"").into(),
            cards: wares
                .iter()
                .map(|(w, cost, sold)| {
                    let mut c = match w {
                        Ware::Item(it) => d.offer_card(it),
                        Ware::Relic(r) => relic_card(r, 0, t("BLESSING")),
                    };
                    c.pie = if *sold { t("Sold").into() } else { tf("{} runes", &[cost]) };
                    c.apagada = *sold || runas < *cost;
                    c
                })
                .collect(),
            selected: d.sel,
            salir: Some(t("Leave").into()),
            ayuda: ayuda(&[("A", "Buy"), ("B", "Leave")]),
            destacada: None,
            // Un mercader nómada junto a su hoguera: que no se confunda con elegir una recompensa.
            fondo: Some("MENU_Load_00006_PS5".into()),
        },
        Phase::Neow { common, rare, curse } => {
            let mut cards = vec![];
            cards.push(match common {
                Some(r) => relic_card(r, 0, t("BLESSING")),
                None => UiCard {
                    cat: t("BLESSING").into(),
                    nombre: t("Nothing").into(),
                    tipo: String::new(),
                    rareza: None,
                    icono: 0,
                    pie: String::new(),
                    apagada: true,
                    doble: false,
                    contra: String::new(),
                    nota: None,
                },
            });
            cards.push(UiCard {
                cat: t("GRACE").into(),
                nombre: t("Increase power").into(),
                tipo: t("10% more damage and defense. And 200 runes.").into(),
                rareza: None,
                icono: ICON_POWER,
                pie: String::new(),
                apagada: false,
                doble: false,
                contra: String::new(),
                nota: None,
            });
            let mut trato = match rare {
                Some(r) => relic_card(r, 0, t("DEAL")),
                None => UiCard {
                    cat: t("DEAL").into(),
                    nombre: t("Nothing").into(),
                    tipo: String::new(),
                    rareza: None,
                    icono: 0,
                    pie: String::new(),
                    apagada: true,
                    doble: false,
                    contra: String::new(),
                    nota: None,
                },
            };
            if let Some(c) = curse {
                trato.pie = tf("Curse: {}", &[&c.nombre]);
                trato.nota = Some((tf("Curse: {}", &[&c.nombre]), texto_entero(c)));
            }
            cards.push(trato);
            UiState::Cards {
                cabecera: t("BEFORE YOU BEGIN").into(),
                titulo: t("A Blessing").into(),
                linea: String::new(),
                instruccion: t("Pick one. The third one comes with a curse.").into(),
                cards,
                selected: d.sel,
                salir: None,
                ayuda: ayuda(&[("A", "Choose")]),
                destacada: None,
                fondo: None,
            }
        }
        Phase::CharSelect => UiState::Cards {
            cabecera: t("NEW RUN").into(),
            titulo: t("Choose Who").into(),
            linea: if run.ascension == 0 {
                tf("Seed {}", &[&run.seed])
            } else {
                tf("Ascension {}: {}", &[&run.ascension, &t(rogue_core::perfil::ASCENSIONES[run.ascension as usize - 1])])
            },
            instruccion: match (d.elegido.and_then(|i| d.chars.get(i)), d.faltan_en_seleccion()) {
                (Some(c), f) if !f.is_empty() => tf("{} chosen. Waiting for {} to choose. B: change.", &[&c.nombre, &f.join(", ")]),
                (None, f) if !f.is_empty() => tf("Waiting for {} to choose.", &[&f.join(", ")]),
                _ => t("Each one fights their own way.").into(),
            },
            cards: d
                .chars
                .iter()
                .map(|c| {
                    let w = d.pool.iter().find(|i| i.id == d.arma_de(c));
                    UiCard {
                        cat: c.lema.to_uppercase(),
                        nombre: c.nombre.clone(),
                        tipo: c.pasiva.clone(),
                        rareza: None,
                        icono: w.map(|w| w.icono).unwrap_or(0),
                        pie: {
                            let arma = w.map(|w| if c.doble { tf("Dual {}", &[&w.nombre]) } else { w.nombre.clone() }).unwrap_or_default();
                            let v = d.perfil.victorias.get(&c.id).copied().unwrap_or(0);
                            let a = d.max_ascension(&c.id);
                            if v > 0 || a > 0 { tf("{}  ·  {} wins  ·  asc. {}", &[&arma, &v, &a]) } else { arma }
                        },
                        apagada: false,
                        doble: c.doble,
                        contra: String::new(),
                        nota: None,
                    }
                })
                .collect(),
            selected: d.elegido.unwrap_or(d.sel),
            salir: None,
            ayuda: {
                let mut v = vec![("A", "Choose"), ("B", "Main menu"), ("X", "Compendium")];
                if d.chars.iter().any(|c| d.max_ascension(&c.id) > 0) && !(crate::coop::hay_companeros() && !crate::coop::soy_anfitrion()) {
                    v.extend([("RB", "Ascension"), ("LB", "Lower")]);
                }
                ayuda(&v)
            },
            destacada: None,
            fondo: None,
        },
        Phase::Rest => {
            let mut cards = vec![UiCard {
                cat: t("GRACE").into(),
                nombre: t("Rest").into(),
                tipo: t("Flasks refilled and full HP.").into(),
                rareza: None,
                icono: 15, // Frasco de lágrimas carmesí
                // Lo que más importa al decidir: cuántos viales quedan.
                pie: d
                    .viales
                    .map(|(n, max)| if n < max { tf("Flasks {} / {} to {} / {}", &[&n, &max, &max, &max]) } else { tf("Flasks {} / {}", &[&n, &max]) })
                    .unwrap_or_default(),
                apagada: d.rest_used[0],
                doble: false,
                contra: String::new(),
                nota: None,
            }];
            cards.push(UiCard {
                cat: t("GRACE").into(),
                nombre: t("Increase power").into(),
                tipo: t("10% more damage and defense for the rest of the run.").into(),
                rareza: None,
                icono: ICON_POWER,
                pie: tf("Power {} to {}", &[&run.poder, &(run.poder + 1)]),
                apagada: d.rest_used[1],
                doble: false,
                contra: String::new(),
                nota: None,
            });
            let templables = d.temperable().len();
            cards.push(UiCard {
                cat: t("GRACE").into(),
                nombre: t("Temper").into(),
                tipo: t("One of your blessings goes up a level.").into(),
                rareza: None,
                icono: ICON_TEMPLAR,
                pie: if templables == 0 { t("Nothing to temper").into() } else { tf("{} can be tempered", &[&templables]) },
                apagada: d.rest_used[2] || templables == 0,
                doble: false,
                contra: String::new(),
                nota: None,
            });
            UiState::Cards {
                cabecera: t("GRACE").into(),
                titulo: t("A Moment of Calm").into(),
                linea: d.viales.map(|(n, max)| tf("Flasks {} / {}", &[&n, &max])).unwrap_or_default(),
                instruccion: match (run.relics.iter().any(|o| o.id == "gracia_generosa"), d.rest_used.iter().filter(|&&u| u).count()) {
                    (true, 0) => t("Two things before moving on."),
                    (true, _) => t("One more thing before moving on."),
                    _ => t("Just one thing before moving on."),
                }
                .into(),
                cards,
                selected: d.sel,
                salir: None,
                ayuda: ayuda(&[("A", "Choose")]),
                destacada: None,
                fondo: None,
            }
        }
        Phase::Templar { opciones } => UiState::Cards {
            cabecera: t("GRACE").into(),
            titulo: t("Temper").into(),
            linea: String::new(),
            instruccion: t("Pick a blessing. It goes up a level.").into(),
            cards: opciones
                .iter()
                .filter_map(|o| d.relics.get(&o.id).map(|r| (o, r)))
                .map(|(o, r)| {
                    let mut c = relic_card(r, o.nivel + 1, t("BLESSING"));
                    c.pie = tf("Level {} to {}", &[&(o.nivel + 1), &(o.nivel + 2)]);
                    c
                })
                .collect(),
            selected: d.sel,
            salir: Some(t("Back").into()),
            ayuda: ayuda(&[("A", "Temper"), ("B", "Back")]),
            destacada: None,
            fondo: None,
        },
        Phase::Event { ev, result } => {
            let (texto, opciones) = match result {
                None => {
                    // Cooperativo: quién ha votado cada opción y a quién se espera.
                    let (votos, espera) = d.votos_evento();
                    let mut texto = ev.texto.clone();
                    if d.coop_activo() {
                        let linea = if espera.is_empty() { t("Vote an option: it is chosen when everyone has voted.").to_string() } else { tf("Waiting for {}", &[&espera.join(", ")]) };
                        texto.push_str(&format!("\n\n{linea}"));
                    }
                    let opciones = ev
                        .opciones
                        .iter()
                        .enumerate()
                        .map(|(k, o)| {
                            let quien: Vec<&str> = votos.iter().filter(|v| v.0 == k).map(|v| v.1.as_str()).collect();
                            let detalle = if quien.is_empty() { o.detalle.clone() } else { format!("{} [{}]", o.detalle, quien.join(", ")) };
                            (o.texto.clone(), detalle, !d.option_enabled(o), d.maldicion_de(o))
                        })
                        .collect();
                    (texto, opciones)
                }
                Some((res, enc)) => {
                    let seguir = if enc.is_some() { t("Fight") } else { t("Continue") };
                    let mut res = res.clone();
                    // Cooperativo: el combate espera a que pulsen todos.
                    if enc.is_some() && d.coop_activo() {
                        let espera = d.esperando_listos();
                        let linea = if espera.is_empty() { t("You fight when everyone is ready.").to_string() } else { tf("Waiting for {}", &[&espera.join(", ")]) };
                        res.push_str(&format!("\n\n{linea}"));
                    }
                    (res, vec![(seguir.to_string(), String::new(), false, None)])
                }
            };
            UiState::Event { cabecera: t("ENCOUNTER").into(), titulo: ev.titulo.clone(), texto, imagen: ev.imagen.clone(), opciones, selected: d.sel, modo: Modo::Encuentro }
        }
        Phase::Replace { item, options, .. } => {
            let gem = item.categoria == rewards::Category::Ceniza;
            let cards = options
                .iter()
                .map(|&slot| {
                    let mut c = match equip::slot_item(slot).and_then(|(_, id)| d.pool_item(id)) {
                        Some(it) => item_card(it),
                        None => UiCard {
                            cat: t("EMPTY").into(),
                            nombre: t("Nothing").into(),
                            tipo: String::new(),
                            rareza: None,
                            icono: 0,
                            pie: String::new(),
                            apagada: false,
                            doble: false,
                            contra: String::new(),
                            nota: None,
                        },
                    };
                    c.pie = slot_name(slot);
                    c
                })
                .collect();
            UiState::Cards {
                cabecera: if gem { t("ASH OF WAR").into() } else { t("NO ROOM").into() },
                titulo: item.nombre.clone(),
                linea: td(&item.tipo),
                instruccion: if gem { t("Choose the weapon that takes it.").into() } else { t("It doesn't fit. Choose what to drop.").into() },
                cards,
                selected: d.sel,
                salir: Some(t("Leave it").into()),
                ayuda: ayuda(&[("A", "Swap"), ("B", "Leave it")]),
                destacada: Some(item_card(item)),
                fondo: None,
            }
        }
        Phase::Over { won, .. } => UiState::Over {
            won: *won,
            act: run.act_index + 1,
            nodes: run.path.len().saturating_sub(1),
            items: run.items.len(),
            seed: run.seed.clone(),
            personaje: d.character().map(|c| c.nombre.clone()).unwrap_or_default(),
            golpes: run.golpes,
            elites: run.elites_won,
            miticas: run.relics.iter().filter_map(|o| d.relics.get(&o.id)).filter(|r| r.rareza == Rarity::Mitica).map(|r| r.nombre.clone()).collect(),
            relics: run.relics.iter().filter_map(|o| d.relics.get(&o.id)).map(|r| (r.nombre.clone(), r.icono, r.maldita)).collect(),
            historia: d.hist_view(&run),
            marcador: d.character().and_then(|c| d.pool.iter().find(|i| i.id == d.arma_de(c))).map(|i| i.icono).unwrap_or(0),
            sel: d.over_sel,
        },
        _ => UiState::None,
    })
}

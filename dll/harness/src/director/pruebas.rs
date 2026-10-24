//! Menú de pruebas (solo en la versión de pruebas: `data/modo_pruebas.txt`, que empaquetar.py pone en
//! los paquetes privados). Desde la pausa en el mapa: pelear contra el rival que quieras, darte
//! bendiciones (el compendio entero; A la da), objetos del pool y runas (17-10: «una forma de
//! enfrentarme a quien yo quiera y ponerme las recompensas que quiera»).
//!
//! En cooperativo el rival lo elige el anfitrión y va a los demás como `Suceso::Rival`: cada juego
//! entra en el mismo combate. Las bendiciones y los objetos son de cada uno.

use super::*;
use rogue_core::coop::Suceso;
use rogue_core::idioma::{t, td, tf};
use rogue_core::rewards::Category;
use rogue_core::run::Encounter;

/// Filas por página en las listas largas (rivales, objetos).
const POR_PAGINA: usize = 8;
/// Runas que da «Runas».
const RUNAS: u32 = 20_000;

#[derive(Clone, PartialEq)]
pub(super) enum Pantalla {
    Raiz,
    /// Listas: (acto, de cooperativo).
    Actos,
    Tipos { acto: usize, coop: bool },
    Rivales { acto: usize, coop: bool, tipo: Option<EncounterKind>, pagina: usize },
    Categorias,
    Tipos2 { cat: Category },
    Objetos { cat: Category, tipo: String, pagina: usize },
}

#[derive(Clone, PartialEq)]
enum Accion {
    Ir(Pantalla),
    Pelear(Encounter),
    Dar(Item),
    Bendiciones,
    Runas,
    Pagina(usize),
    Volver,
}

/// Una opción: (texto, nota al elegirla, apagada, qué hace).
type Opcion = (String, String, bool, Accion);

fn tipo_rival(k: EncounterKind) -> &'static str {
    match k {
        EncounterKind::Weak => t("weak"),
        EncounterKind::Normal => t("normal"),
        EncounterKind::Elite => t("elite"),
        EncounterKind::Boss => t("boss"),
    }
}

fn nombre_cat(c: Category) -> &'static str {
    match c {
        Category::Arma => t("Weapons"),
        Category::Ceniza => t("Ashes of War"),
        Category::Hechizo => t("Spells"),
        Category::Talisman => t("Talismans"),
        Category::Invocacion => t("Spirit Ashes"),
    }
}

/// Página de una lista larga, con «Más» y «Volver».
fn paginar(todas: Vec<Opcion>, pagina: usize) -> Vec<Opcion> {
    let paginas = todas.len().div_ceil(POR_PAGINA).max(1);
    let pagina = pagina.min(paginas - 1);
    let mut v: Vec<Opcion> = todas.into_iter().skip(pagina * POR_PAGINA).take(POR_PAGINA).collect();
    if paginas > 1 {
        v.push((tf("More ({}/{})", &[&(pagina + 1), &paginas]), String::new(), false, Accion::Pagina((pagina + 1) % paginas)));
    }
    v.push((t("Back").into(), String::new(), false, Accion::Volver));
    v
}

impl Director {
    /// ¿Versión de pruebas?
    pub(super) fn modo_pruebas(&self) -> bool {
        self.pruebas_activo
    }

    fn lista_actos(&self, coop: bool) -> &Vec<ActDef> {
        if coop { &self.acts_coop } else { &self.acts }
    }

    fn opciones_pruebas(&self, p: &Pantalla) -> Vec<Opcion> {
        let invitado = self.coop_activo() && !crate::coop::soy_anfitrion();
        match p {
            Pantalla::Raiz => vec![
                (
                    t("Choose a rival").into(),
                    if invitado { t("The host chooses the rival.").into() } else { String::new() },
                    invitado,
                    Accion::Ir(Pantalla::Actos),
                ),
                (t("Give a blessing").into(), t("All of them. A gives the one you pick.").into(), false, Accion::Bendiciones),
                (t("Give an item").into(), String::new(), false, Accion::Ir(Pantalla::Categorias)),
                (tf("Runes +{}", &[&RUNAS]), String::new(), false, Accion::Runas),
                (t("Back").into(), String::new(), false, Accion::Volver),
            ],
            Pantalla::Actos => {
                let coop = self.run.as_ref().is_some_and(|r| r.coop);
                // Primero las listas del modo de esta run.
                let mut v = vec![];
                for lista in [coop, !coop] {
                    for (k, a) in self.lista_actos(lista).iter().enumerate() {
                        let nota = if lista { t("Co-op list (duos)") } else { t("Solo list") };
                        v.push((format!("{} {}{}", t("Act"), k + 1, if lista != coop { format!(" ({nota})") } else { String::new() }), format!("{} · {}", a.encounters.len(), nota), false, Accion::Ir(Pantalla::Tipos { acto: k, coop: lista })));
                    }
                }
                v.push((t("Back").into(), String::new(), false, Accion::Volver));
                v
            }
            Pantalla::Tipos { acto, coop } => {
                let encs = self.lista_actos(*coop).get(*acto).map(|a| a.encounters.clone()).unwrap_or_default();
                let mut v = vec![(t("All").to_string(), format!("{}", encs.len()), false, Accion::Ir(Pantalla::Rivales { acto: *acto, coop: *coop, tipo: None, pagina: 0 }))];
                for k in [EncounterKind::Weak, EncounterKind::Normal, EncounterKind::Elite, EncounterKind::Boss] {
                    let n = encs.iter().filter(|e| e.kind == k).count();
                    if n > 0 {
                        v.push((tipo_rival(k).to_string(), format!("{n}"), false, Accion::Ir(Pantalla::Rivales { acto: *acto, coop: *coop, tipo: Some(k), pagina: 0 })));
                    }
                }
                v.push((t("Back").into(), String::new(), false, Accion::Volver));
                v
            }
            Pantalla::Rivales { acto, coop, tipo, pagina } => {
                let todas = self
                    .lista_actos(*coop)
                    .get(*acto)
                    .map(|a| a.encounters.iter().filter(|e| tipo.is_none_or(|k| e.kind == k)).cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| (e.name.clone(), format!("{} · {}", tipo_rival(e.kind), e.id), false, Accion::Pelear(e)))
                    .collect();
                paginar(todas, *pagina)
            }
            Pantalla::Categorias => {
                let mut v = vec![];
                for c in [Category::Arma, Category::Ceniza, Category::Hechizo, Category::Talisman, Category::Invocacion] {
                    let n = self.pool.iter().filter(|i| i.categoria == c).count();
                    if n > 0 {
                        v.push((nombre_cat(c).to_string(), format!("{n}"), false, Accion::Ir(Pantalla::Tipos2 { cat: c })));
                    }
                }
                v.push((t("Back").into(), String::new(), false, Accion::Volver));
                v
            }
            Pantalla::Tipos2 { cat } => {
                let mut tipos: Vec<String> = self.pool.iter().filter(|i| i.categoria == *cat).map(|i| i.tipo.clone()).collect();
                tipos.sort();
                tipos.dedup();
                let todas = tipos
                    .into_iter()
                    .map(|tp| {
                        let n = self.pool.iter().filter(|i| i.categoria == *cat && i.tipo == tp).count();
                        (td(&tp), format!("{n}"), false, Accion::Ir(Pantalla::Objetos { cat: *cat, tipo: tp, pagina: 0 }))
                    })
                    .collect();
                // Página fija: los tipos se ven todos con «Más».
                paginar(todas, self.menu_pruebas.as_ref().map(|m| m.2).unwrap_or(0))
            }
            Pantalla::Objetos { cat, tipo, pagina } => {
                let todas = self
                    .pool
                    .iter()
                    .filter(|i| i.categoria == *cat && i.tipo == *tipo)
                    .map(|i| (i.nombre.clone(), String::new(), false, Accion::Dar(i.clone())))
                    .collect();
                paginar(todas, *pagina)
            }
        }
    }

    /// Para la interfaz: (título, opciones (texto, detalle, apagada, nota), elegida).
    pub(super) fn vista_pruebas(&self) -> Option<(String, Vec<(String, String, bool, Option<String>)>, usize)> {
        let (p, sel, _) = self.menu_pruebas.as_ref()?;
        let titulo = match p {
            Pantalla::Raiz => t("Tests").to_string(),
            Pantalla::Actos | Pantalla::Tipos { .. } | Pantalla::Rivales { .. } => t("Choose a rival").to_string(),
            _ => t("Give an item").to_string(),
        };
        let ops = self
            .opciones_pruebas(p)
            .into_iter()
            .map(|(texto, nota, apagada, a)| {
                // En las listas cortas, el número va de detalle; en las de rivales y objetos, como nota.
                let larga = matches!(a, Accion::Pelear(_) | Accion::Dar(_));
                if larga || nota.is_empty() { (texto, String::new(), apagada, (!nota.is_empty()).then_some(nota)) } else { (texto, nota, apagada, None) }
            })
            .collect();
        Some((titulo, ops, *sel))
    }

    pub(super) fn abrir_pruebas(&mut self) {
        self.menu_pruebas = Some((Pantalla::Raiz, 0, 0));
        self.historial_pruebas.clear();
    }

    /// Mando en el menú de pruebas.
    pub(super) fn mando_pruebas(&mut self, up: bool, down: bool, a: bool, b: bool, lb: bool, rb: bool) {
        let Some((p, sel, pag)) = self.menu_pruebas.clone() else { return };
        let ops = self.opciones_pruebas(&p);
        // LB y RB: página anterior y siguiente de las listas largas.
        if (lb || rb)
            && let Some(Accion::Pagina(sig)) = ops.iter().map(|o| o.3.clone()).find(|x| matches!(x, Accion::Pagina(_)))
        {
            let paginas = self.paginas_pruebas(&p);
            let k = if rb { sig } else { (pag + paginas - 1) % paginas.max(1) };
            play_se(Se::Cursor);
            let np = match p {
                Pantalla::Rivales { acto, coop, tipo, .. } => Pantalla::Rivales { acto, coop, tipo, pagina: k },
                Pantalla::Objetos { cat, tipo, .. } => Pantalla::Objetos { cat, tipo, pagina: k },
                otra => otra,
            };
            self.menu_pruebas = Some((np, 0, k));
            return;
        }
        let n = ops.len().max(1);
        let mut nsel = sel.min(n - 1);
        if up {
            nsel = (nsel + n - 1) % n;
        }
        if down {
            nsel = (nsel + 1) % n;
        }
        if nsel != sel {
            play_se(Se::Cursor);
            self.menu_pruebas = Some((p.clone(), nsel, pag));
        }
        if b {
            play_se(Se::Back);
            self.volver_pruebas();
            return;
        }
        if !a {
            return;
        }
        let Some((_, _, apagada, accion)) = ops.get(nsel).cloned() else { return };
        if apagada {
            return;
        }
        play_se(Se::Confirm);
        match accion {
            Accion::Ir(np) => {
                self.historial_pruebas.push((p, nsel, pag));
                self.menu_pruebas = Some((np, 0, 0));
            }
            Accion::Pagina(k) => {
                let np = match p {
                    Pantalla::Rivales { acto, coop, tipo, .. } => Pantalla::Rivales { acto, coop, tipo, pagina: k },
                    Pantalla::Objetos { cat, tipo, .. } => Pantalla::Objetos { cat, tipo, pagina: k },
                    otra => otra,
                };
                self.menu_pruebas = Some((np, 0, k));
            }
            Accion::Volver => self.volver_pruebas(),
            Accion::Pelear(enc) => {
                self.menu_pruebas = None;
                let msg = self.pelear_prueba(enc);
                crate::log(&format!("pruebas: {msg}"));
            }
            Accion::Dar(it) => {
                self.menu_pruebas = None;
                let msg = self.acquire(it, Phase::Map);
                crate::log(&format!("pruebas: {msg}"));
                self.save();
            }
            Accion::Bendiciones => {
                self.menu_pruebas = None;
                self.compendio_dar = true;
                self.show_compendio = true;
            }
            Accion::Runas => {
                let r = crate::runes().unwrap_or(0);
                crate::set_runes(r + RUNAS);
                crate::log(&format!("pruebas: runas {r} -> {}", r + RUNAS));
            }
        }
    }

    /// Páginas de la pantalla (las listas largas tienen más de una).
    fn paginas_pruebas(&self, p: &Pantalla) -> usize {
        let n = match p {
            Pantalla::Rivales { acto, coop, tipo, .. } => {
                self.lista_actos(*coop).get(*acto).map(|a| a.encounters.iter().filter(|e| tipo.is_none_or(|k| e.kind == k)).count()).unwrap_or(0)
            }
            Pantalla::Objetos { cat, tipo, .. } => self.pool.iter().filter(|i| i.categoria == *cat && i.tipo == *tipo).count(),
            Pantalla::Tipos2 { cat } => {
                let mut v: Vec<&String> = self.pool.iter().filter(|i| i.categoria == *cat).map(|i| &i.tipo).collect();
                v.sort();
                v.dedup();
                v.len()
            }
            _ => 0,
        };
        n.div_ceil(POR_PAGINA).max(1)
    }

    fn volver_pruebas(&mut self) {
        self.menu_pruebas = self.historial_pruebas.pop();
    }

    /// Compendio en modo «dar»: la bendición elegida pasa a ser tuya.
    pub(super) fn dar_bendicion_prueba(&mut self) {
        let Some(r) = self.relics.reliquias.get(self.compendio_sel).cloned() else { return };
        crate::log(&format!("pruebas: bendición {}", r.id));
        self.gain_relic(&r);
        self.save();
    }

    /// Pelear contra `enc` desde el mapa, en el nodo en que estás. En cooperativo (solo el anfitrión)
    /// los demás entran en el mismo combate.
    fn pelear_prueba(&mut self, enc: Encounter) -> String {
        if !matches!(self.phase, Phase::Map) {
            return "solo desde el mapa".into();
        }
        if self.coop_activo() {
            crate::coop::suceso(Suceso::Rival { id: enc.id.clone() });
        }
        self.acto_prueba = self.acto_de(&enc.id);
        self.enter_action(NodeAction::Combat(enc))
    }

    /// Acto de la lista en que está el encuentro (el mismo en todos los juegos).
    fn acto_de(&self, id: &str) -> Option<usize> {
        [&self.acts, &self.acts_coop].into_iter().find_map(|l| l.iter().position(|a| a.encounters.iter().chain(&a.mezclas).any(|e| e.id == id)))
    }

    /// Cooperativo: el anfitrión ha elegido un rival en su menú de pruebas.
    pub(super) fn rival_recibido(&mut self, id: &str) {
        if !matches!(self.phase, Phase::Map) {
            crate::log(&format!("pruebas: rival {id} del anfitrión fuera del mapa; no se entra"));
            return;
        }
        let Some(enc) = self.acts.iter().chain(self.acts_coop.iter()).flat_map(|a| a.encounters.iter().chain(&a.mezclas)).find(|e| e.id == id).cloned() else {
            crate::log(&format!("pruebas: no hay encuentro {id}"));
            return;
        };
        self.menu_pruebas = None;
        self.pausa = None;
        self.acto_prueba = self.acto_de(id);
        let msg = self.enter_action(NodeAction::Combat(enc));
        crate::log(&format!("pruebas: rival del anfitrión: {msg}"));
    }
}

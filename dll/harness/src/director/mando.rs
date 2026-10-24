//! Mando en las pantallas del mod (cruceta, A, B, X, Y, START y LB/RB).

use super::*;

/// Tras cambiar de pantalla, la A no cuenta durante este rato (era para la pantalla de antes).
const A_TRAS_CAMBIO: Duration = Duration::from_millis(400);

impl Director {
    /// Entrada del mando en las pantallas (flancos de subida de los botones).
    pub(super) fn nav(&mut self, buttons: u16) {
        let sel_before = self.sel;
        let pressed = buttons & !self.last_buttons;
        self.last_buttons = buttons;
        let (left, right, mut a, b) = (pressed & 0x0005 != 0, pressed & 0x000A != 0, pressed & 0x1000 != 0, pressed & 0x2000 != 0);
        // Pantalla recién cambiada: la A de ese momento era para la de antes (24-10: al invitado le
        // llevó el anfitrión a la selección de personaje mientras pulsaba A en el menú, y se le eligió uno).
        let pantalla = std::mem::discriminant(&self.phase);
        if self.pantalla_vista.is_none_or(|(p, _)| p != pantalla) {
            self.pantalla_vista = Some((pantalla, Instant::now()));
        }
        if a && self.pantalla_vista.is_some_and(|(_, t)| t.elapsed() < A_TRAS_CAMBIO) {
            a = false;
            crate::log("mando: A ignorada (la pantalla acaba de cambiar)");
        }
        // Usando un objeto de Seamless (coop.rs): el menú espera; lo que se pulsa ahora es del mod.
        if self.sala.is_some() {
            return;
        }
        // Selector de la contraseña de la sesión (coop.rs).
        if self.clave.is_some() && matches!(self.phase, Phase::MainMenu) {
            self.nav_clave(pressed);
            return;
        }
        // Manos libres del cooperativo: el mando es del juego; SELECT vuelve al menú del mod.
        if self.libre {
            if pressed & 0x0020 != 0 || !matches!(self.phase, Phase::MainMenu) {
                self.libre = false;
                play_se(Se::Back);
            }
            return;
        }
        // Menú del cooperativo: B vuelve al principal.
        if self.menu_coop && b && matches!(self.phase, Phase::MainMenu) {
            self.menu_coop = false;
            self.sel = self.menu_opciones().iter().position(|o| o.3 == Menu::Coop).unwrap_or(0);
            play_se(Se::Back);
            return;
        }
        // Personajes: izquierda/derecha cambian de personaje, arriba/abajo de arma, LB/RB de apariencia, X
        // de invocación; B vuelve.
        if let Phase::Personajes { idx } = self.phase {
            let n = self.chars.len().max(1);
            let mut nidx = idx;
            if pressed & 0x0004 != 0 {
                nidx = (idx + n - 1) % n;
            }
            if pressed & 0x0008 != 0 {
                nidx = (idx + 1) % n;
            }
            if nidx != idx {
                play_se(Se::Cursor);
                self.phase = Phase::Personajes { idx: nidx };
                let m = self.probar_personaje(nidx);
                crate::log(&format!("personajes: {m}"));
            } else if pressed & 0x0003 != 0
                && let Some(c) = self.chars.get(idx).cloned()
            {
                // Solo las desbloqueadas; las demás se ven con su candado.
                let v = self.victorias(&c);
                let libres: Vec<usize> = (0..c.armas.len()).filter(|&k| c.arma_libre(k, v)).collect();
                let k = libres.len().max(1);
                let elegida = self.arma_de(&c);
                let actual = libres.iter().position(|&i| c.armas[i] == elegida).unwrap_or(0);
                let nueva = if pressed & 0x0001 != 0 { (actual + k - 1) % k } else { (actual + 1) % k };
                let nueva = libres.get(nueva).copied().unwrap_or(0);
                if c.armas.get(nueva).is_some_and(|&a| a != elegida) {
                    play_se(Se::Cursor);
                    self.perfil.armas.insert(c.id.clone(), c.armas[nueva]);
                    self.perfil.save(&self.root.join("runtime").join("perfil.json"));
                    let m = self.probar_personaje(idx);
                    crate::log(&format!("personajes: {m}"));
                }
            }
            if nidx == idx
                && pressed & 0x0300 != 0
                && let Some(c) = self.chars.get(idx).cloned()
            {
                // La de siempre (None) y las desbloqueadas.
                let v = self.victorias(&c);
                let mut libres: Vec<Option<String>> = vec![None];
                libres.extend(c.apariencias.iter().filter(|a| v >= a.victorias).map(|a| Some(a.id.clone())));
                let actual = self.perfil.apariencias.get(&c.id).cloned();
                let i = libres.iter().position(|x| *x == actual).unwrap_or(0);
                let k = libres.len();
                let nueva = libres[if pressed & 0x0100 != 0 { (i + k - 1) % k } else { (i + 1) % k }].clone();
                if nueva != actual {
                    play_se(Se::Cursor);
                    match nueva {
                        Some(id) => self.perfil.apariencias.insert(c.id.clone(), id),
                        None => self.perfil.apariencias.remove(&c.id),
                    };
                    self.perfil.save(&self.root.join("runtime").join("perfil.json"));
                    let m = self.probar_personaje(idx);
                    crate::log(&format!("personajes: {m}"));
                }
            }
            if nidx == idx
                && pressed & 0x4000 != 0
                && let Some(c) = self.chars.get(idx).cloned()
                && c.invocaciones.len() > 1
            {
                // Solo las desbloqueadas, como las armas.
                let v = self.victorias(&c);
                let libres: Vec<usize> = (0..c.invocaciones.len()).filter(|&k| v >= CharacterDef::victorias_para(k)).collect();
                let actual = c.invocacion(self.perfil.invocaciones.get(&c.id).map(String::as_str), v).map(|i| i.id.clone());
                let i = libres.iter().position(|&k| Some(&c.invocaciones[k].id) == actual.as_ref()).unwrap_or(0);
                let nueva = &c.invocaciones[libres[(i + 1) % libres.len()]];
                if Some(&nueva.id) != actual.as_ref() {
                    play_se(Se::Cursor);
                    self.perfil.invocaciones.insert(c.id.clone(), nueva.id.clone());
                    self.perfil.save(&self.root.join("runtime").join("perfil.json"));
                    crate::log(&format!("personajes: {} invoca {}", c.nombre, nueva.nombre));
                }
            }
            if b {
                play_se(Se::Back);
                self.phase = Phase::MainMenu;
                self.sel = self.menu_opciones().iter().position(|o| o.3 == Menu::Personajes).unwrap_or(0);
            }
            return;
        }
        // Pantalla de la ascensión (desde la pausa): A, B o START vuelven a la pausa.
        if self.ver_ascension {
            if a || b || pressed & 0x0010 != 0 {
                self.ver_ascension = false;
                play_se(Se::Back);
            }
            return;
        }
        // Menú de pruebas (versión de pruebas): por encima de todo.
        if self.menu_pruebas.is_some() {
            self.mando_pruebas(left, right, a, b, pressed & 0x0100 != 0, pressed & 0x0200 != 0);
            return;
        }
        // Pausa (START): por encima de todo; B o START la cierran.
        let start = pressed & 0x0010 != 0;
        if let Some(psel) = self.pausa {
            let ops = self.pausa_opciones();
            let n = ops.len();
            let mut nsel = psel;
            if left {
                nsel = (psel + n - 1) % n;
            }
            if right {
                nsel = (psel + 1) % n;
            }
            if nsel != psel {
                play_se(Se::Cursor);
                self.pausa = Some(nsel);
            }
            if b || start {
                self.cerrar_pausa();
                play_se(Se::Back);
            } else if a && !ops[nsel].2 {
                play_se(Se::Confirm);
                let msg = self.confirmar_pausa(ops[nsel].3);
                crate::log(&format!("pausa: {msg}"));
            }
            return;
        }
        if start && !matches!(self.phase, Phase::MainMenu | Phase::Personajes { .. } | Phase::Over { .. }) && !self.show_compendio {
            self.pausa = Some(0);
            play_se(Se::Open);
            return;
        }
        // Selector: LB/RB cambian la ascensión (hasta la desbloqueada con ese personaje). En cooperativo
        // la elige el anfitrión (coop.rs, ascension_anfitrion).
        if matches!(self.phase, Phase::CharSelect) && self.elegido.is_none() && !(crate::coop::hay_companeros() && !crate::coop::soy_anfitrion()) {
            let max = self.chars.get(self.sel).map(|c| self.max_ascension(&c.id)).unwrap_or(0);
            if let Some(run) = self.run.as_mut() {
                let before = run.ascension;
                if pressed & 0x0200 != 0 {
                    run.ascension = (run.ascension + 1).min(max);
                }
                if pressed & 0x0100 != 0 {
                    run.ascension = run.ascension.saturating_sub(1);
                }
                run.ascension = run.ascension.min(max);
                if run.ascension != before {
                    play_se(Se::Cursor);
                }
            }
        }
        if let Phase::Over { won, new_run: false, .. } = self.phase
            && !self.recorded
        {
            self.recorded = true;
            if let Some(run) = &self.run {
                let r = rogue_core::perfil::Resumen {
                    personaje: run.personaje.clone().unwrap_or_default(),
                    ganada: won,
                    acto: run.act_index + 1,
                    ascension: run.ascension,
                    semilla: run.seed.clone(),
                    golpes: run.golpes,
                    bendiciones: run.relics.iter().map(|o| o.id.clone()).collect(),
                    rivales: run.rivales.clone(),
                };
                self.perfil.registrar(r);
                self.perfil.save(&self.root.join("runtime").join("perfil.json"));
                if let Some(run) = self.run.as_mut() {
                    run.terminada = true;
                }
                self.save();
                crate::log(&format!("perfil: run apuntada ({})", if won { "ganada" } else { "perdida" }));
            }
        }
        if let Phase::Over { won, new_run: false, shown } = self.phase {
            // Cruceta: elegir nodo del gráfico (empieza en el último).
            let n = self.run.as_ref().map(|r| r.historia.len()).unwrap_or(0);
            if n > 0 {
                if self.over_sel >= n {
                    self.over_sel = n - 1;
                }
                if left && self.over_sel > 0 {
                    self.over_sel -= 1;
                    play_se(Se::Cursor);
                }
                if right && self.over_sel + 1 < n {
                    self.over_sel += 1;
                    play_se(Se::Cursor);
                }
            }
            if a && shown.is_some_and(|t| t.elapsed() > Duration::from_millis(1200)) {
                self.phase = Phase::Over { won, new_run: true, shown };
            }
            return;
        }
        // Compendio: X lo abre desde el selector; la cruceta mueve y B o X lo cierran.
        if self.show_compendio {
            let n = self.relics.reliquias.len().max(1);
            let cols = rogue_ui::bendiciones::COMPENDIO_COLS;
            let before = self.compendio_sel;
            if pressed & 0x0004 != 0 {
                self.compendio_sel = (self.compendio_sel + n - 1) % n;
            }
            if pressed & 0x0008 != 0 {
                self.compendio_sel = (self.compendio_sel + 1) % n;
            }
            if pressed & 0x0001 != 0 && self.compendio_sel >= cols {
                self.compendio_sel -= cols;
            }
            if pressed & 0x0002 != 0 && self.compendio_sel + cols < n {
                self.compendio_sel += cols;
            }
            if self.compendio_sel != before {
                play_se(Se::Cursor);
            }
            if b || pressed & 0x4000 != 0 {
                self.show_compendio = false;
                self.compendio_dar = false;
                play_se(Se::Back);
            } else if a && self.compendio_dar {
                self.dar_bendicion_prueba();
            }
            return;
        }
        if pressed & 0x4000 != 0 && matches!(self.phase, Phase::CharSelect | Phase::MainMenu) {
            self.show_compendio = true;
            play_se(Se::Open);
            return;
        }
        // Cooperativo, ya elegido: B lo deshace (se sigue en la selección); lo demás espera.
        if self.elegido.is_some() && matches!(self.phase, Phase::CharSelect) {
            if b {
                self.elegido = None;
                play_se(Se::Back);
            }
            return;
        }
        // Selector de personaje: B vuelve al menú principal sin empezar la run.
        if b && matches!(self.phase, Phase::CharSelect) {
            play_se(Se::Back);
            self.phase = Phase::MainMenu;
            self.sel = self.menu_opciones().iter().position(|o| o.3 == Menu::Nueva).unwrap_or(0);
            return;
        }
        // Lista de bendiciones: Y la abre desde el mapa; B o Y la cierran.
        let y = pressed & 0x8000 != 0;
        if self.show_list {
            if b || y {
                self.show_list = false;
                play_se(Se::Back);
            } else if pressed & 0x000F != 0 {
                // Cruceta: se mueve por la rejilla (izquierda y derecha de una en una, arriba y abajo
                // de fila en fila).
                let n = self.run.as_ref().map(|r| r.relics.len()).unwrap_or(0).max(1);
                let cols = rogue_ui::bendiciones::LLEVAS_COLS;
                let antes = self.lista_sel;
                if pressed & 0x0004 != 0 {
                    self.lista_sel = self.lista_sel.saturating_sub(1);
                }
                if pressed & 0x0008 != 0 {
                    self.lista_sel = (self.lista_sel + 1).min(n - 1);
                }
                if pressed & 0x0001 != 0 && self.lista_sel >= cols {
                    self.lista_sel -= cols;
                }
                if pressed & 0x0002 != 0 && self.lista_sel + cols < n {
                    self.lista_sel += cols;
                }
                if self.lista_sel != antes {
                    play_se(Se::Cursor);
                }
            }
            return;
        }
        if y && matches!(self.phase, Phase::Map) {
            self.show_list = true;
            self.lista_sel = 0;
            play_se(Se::Open);
            return;
        }
        let n = self.slots();
        if n == 0 {
            return;
        }
        if left {
            self.sel = (self.sel + n - 1) % n;
        }
        if right {
            self.sel = (self.sel + 1) % n;
        }
        // B: ir directo a "Saltar"/"Salir" en las pantallas que lo tienen.
        let cofre_dejable = matches!(&self.phase, Phase::Treasure { trap, .. } if self.cofre_se_deja(trap.is_some()));
        if b && (cofre_dejable || matches!(self.phase, Phase::Reward { .. } | Phase::Shop { .. } | Phase::Replace { .. } | Phase::Templar { .. })) {
            self.sel = n - 1;
        }
        if left || right {
            play_se(Se::Cursor);
        }
        if a {
            play_se(Se::Confirm);
            let msg = self.confirm();
            crate::log(&format!("mando: {msg}"));
        } else if b && self.sel != sel_before {
            play_se(Se::Back);
        }
    }
}

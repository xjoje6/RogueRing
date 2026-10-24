//! Combates de varios enemigos (docs/LISTAS_Y_COMBATES.md): manada, escolta, dúo, oleadas, racha y
//! aguante. Los enemigos de una oleada están colocados en la variante del coliseo, apagados hasta que
//! se enciende el flag de su oleada (sin variante, se crean con `spawn_chr`). El motor de bendiciones
//! sigue a uno: al último que ha perdido vida mientras atacas, o al primero vivo.

use super::*;
use rogue_core::run::Formato;

/// Separación entre los de una oleada creados con `spawn_chr` (los colocados van en filas de cuatro).
const SEPARACION: f32 = 3.5;

impl Director {
    /// Entidades del combate y su primera oleada encendida. Devuelve el primer rival.
    pub(super) fn preparar_grupo(&mut self, enc: &Encounter) -> u32 {
        let n = enc.lista().len().max(1) as u32;
        self.grupo = if self.jefe_msb {
            (0..n).map(|i| JEFE_MSB + i).collect()
        } else {
            let k = self.run.as_ref().map(|r| (r.act_index * 100 + r.path.len()) as u32).unwrap_or(0);
            (0..n).map(|i| ARENA_ENTITY_BASE + k * 16 + i).collect()
        };
        self.oleada = 0;
        self.oleada_lista = false;
        self.oleada_desde = None;
        self.pos_fase = None;
        self.renacido = None;
        self.renacer_cerrado = false;
        self.transicion = None;
        self.rafaga = None;
        self.fase2_oculta = None;
        self.ultimo = None;
        self.ultimo_escalado = false;
        self.extra_oleada.clear();
        self.fase2_intocable = None;
        self.vida_previa.clear();
        self.muertos.clear();
        self.vistos.clear();
        self.premio_extra = 0;
        self.arranque_suelto = false;
        self.encender_oleada(enc, 0);
        self.de_oleada(enc).first().copied().unwrap_or(0)
    }

    /// Enciende la oleada `w`: su flag (enemigos colocados) o los crea (`spawn_chr`).
    pub(super) fn encender_oleada(&mut self, enc: &Encounter, w: usize) {
        let (lista, grupos) = (enc.lista(), enc.grupos());
        let Some(ola) = grupos.get(w) else { return };
        if self.jefe_msb {
            flag_set(JEFE_MSB_FLAG + w as u32, true);
            for &i in ola {
                if let Some(&ent) = self.grupo.get(i) {
                    if let Some(&p) = self.origenes.get(&ent) {
                        crate::set_chr_pos(ent, p);
                    }
                    crate::set_chr_disabled(ent, false);
                }
            }
        } else {
            let k = ola.len() as f32;
            for (j, &i) in ola.iter().enumerate() {
                let (Some(e), Some(&ent)) = (lista.get(i), self.grupo.get(i)) else { continue };
                let dx = (j as f32 - (k - 1.0) / 2.0) * SEPARACION;
                crate::spawn_chr_en(e.chr, e.npc, e.think, ent, dx, 15.0);
            }
        }
        crate::log(&format!("coliseo: {} oleada {}/{} ({} enemigos)", enc.name, w + 1, grupos.len(), ola.len()));
    }

    /// Colocados en el MSB: apagados los de las oleadas que aún no tocan (cada fotograma, porque van
    /// apareciendo según carga el mapa).
    pub(super) fn apagar_pendientes(&self, enc: &Encounter) {
        if !self.jefe_msb {
            return;
        }
        for ola in enc.grupos().iter().skip(self.oleada + 1) {
            for &i in ola {
                if let Some(&ent) = self.grupo.get(i) {
                    crate::set_chr_disabled(ent, true);
                }
            }
        }
    }

    /// Invocador: su invocación a su lado al empezar el combate (la de antes, si sigue, cae). La mejora se
    /// le pone en cuanto existe (tick).
    ///
    /// En cooperativo las crea siempre el anfitrión: lo que crea un invitado solo existe en su juego, y
    /// con las mismas entidades chocaba con las del anfitrión (16-10: con dos Invocadores solo
    /// salía la del anfitrión). Cada invitado tiene sus entidades (`entidades_invocacion`).
    pub(super) fn invocar(&mut self) {
        let Some(inv) = self.invocacion_actual() else { return };
        // Manada y Legión: más de su clase, cada tanda un poco más atrás.
        let owned = self.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
        let tandas = 1 + self.relics.dll_sum(&owned, "invocacion_extra").max(0.0) as usize;
        self.invocados.clear();
        self.invocacion_caida = None;
        let coop = self.coop_activo();
        let (base, max) = entidades_invocacion(if coop && crate::coop::soy_invitado() { crate::coop::indice_invitado(crate::coop::yo()) } else { None });
        self.invocacion_ents = crear_invocacion(&inv.posiciones, (inv.chr, inv.npc, inv.think), tandas, base, max, None);
        self.invocados = self.invocacion_ents.clone();
        if coop {
            crate::coop::suceso(rogue_core::coop::Suceso::Invocar {
                chr: inv.chr,
                npc: inv.npc,
                think: inv.think,
                posiciones: inv.posiciones.clone(),
                tandas,
                sp: self.sp_ceniza(),
                centro: crate::player_pos().unwrap_or_default(),
                nombre: inv.nombre.clone(),
            });
        }
        crate::log(&format!("invocación: {} ({} espíritus, {base}+)", inv.nombre, self.invocacion_ents.len()));
    }

    /// La invocación de un compañero, en este juego: junto a él, en sus entidades (las del anfitrión o
    /// las de su puesto de invitado) y con su mejora.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn invocar_para(&mut self, de: u64, chr: (i32, i32, i32), posiciones: &[[f32; 2]], tandas: usize, sp: i32, centro: [f32; 3], ceniza: String) {
        let companero = crate::coop::companeros().into_iter().find(|c| c.jugador.steam == de);
        let es_anfitrion = companero.as_ref().is_some_and(|c| c.jugador.anfitrion);
        let (base, max) = entidades_invocacion(if es_anfitrion { None } else { crate::coop::indice_invitado(de) });
        let chr = sin_companero(chr);
        let ents = crear_invocacion(posiciones, chr, tandas, base, max, Some(centro));
        crate::log(&format!("invocación de {de}: {} espíritus ({base}+)", ents.len()));
        self.invocados_ajenos.extend(ents.iter().map(|&e| (e, sp)));
        let jugador = companero.map(|c| c.jugador.nombre).unwrap_or_default();
        self.invocaciones_ajenas.insert(de, (jugador, ceniza, ents));
    }

    /// Mejora de ceniza de mi invocación en este acto (SpEffect).
    pub(super) fn sp_ceniza(&self) -> i32 {
        let acto = self.run.as_ref().map(|r| r.act_index).unwrap_or(0).min(NIVEL_CENIZA.len() - 1);
        // Ceniza templada (+3) y Legión (-3), entre 0 y 10 como las cenizas del juego.
        let owned = self.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
        let nivel = (NIVEL_CENIZA[acto] + self.relics.dll_sum(&owned, "invocacion_nivel") as i32).clamp(0, 10);
        SP_MEJORA_CENIZA + nivel
    }

    /// Mejora del acto a las invocaciones que ya existen.
    pub(super) fn mejorar_invocados(&mut self) {
        // Las que el anfitrión creó para un invitado llevan la mejora que pidió él.
        self.invocados_ajenos.retain(|&(e, sp)| !(crate::chr_loaded(e) && crate::apply_speffect_to(e, sp)));
        if self.invocados.is_empty() {
            return;
        }
        let sp = self.sp_ceniza();
        self.invocados.retain(|&e| !(crate::chr_loaded(e) && crate::apply_speffect_to(e, sp)));
    }

    /// Cuando caen todas las invocaciones: Pacto de espíritu cura y Relevo las vuelve a llamar pasado su
    /// tiempo.
    pub(super) fn vigilar_invocacion(&mut self) {
        if self.invocacion_ents.is_empty() {
            return;
        }
        let owned = self.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
        let vivas = self.invocacion_ents.iter().any(|&e| crate::chr_hp(e).is_some_and(|h| h > 0));
        match self.invocacion_caida {
            None if !vivas && self.invocados.is_empty() => {
                self.invocacion_caida = Some(Instant::now());
                let cura = self.relics.dll_sum(&owned, "invocacion_cura");
                if cura > 0.0
                    && let Some((_, max)) = crate::player_hp_max()
                {
                    crate::heal_player_by((max as f64 * cura) as i32);
                }
                crate::log("invocación: ha caído");
            }
            Some(t) => {
                let revive = self.relics.dll_sum(&owned, "invocacion_revive");
                if revive > 0.0 && t.elapsed().as_secs_f64() >= revive {
                    self.invocar();
                }
            }
            None => {}
        }
    }

    /// Entidades de la oleada en curso (con las que se le suman, como la segunda fase del último en pie).
    pub(super) fn de_oleada(&self, enc: &Encounter) -> Vec<u32> {
        let mut v: Vec<u32> = enc.grupos().get(self.oleada).map(|o| o.iter().filter_map(|&i| self.grupo.get(i).copied()).collect()).unwrap_or_default();
        v.extend(self.extra_oleada.iter().copied());
        v
    }

    /// Todos los de la oleada existen ya.
    pub(super) fn oleada_cargada(&self, enc: &Encounter) -> bool {
        let ents = self.de_oleada(enc);
        !ents.is_empty() && ents.iter().all(|&e| crate::chr_loaded(e))
    }

    /// Vivos de la oleada. Los que todavía no existen cuentan como vivos; uno que ya se vio y
    /// desaparece, como muerto.
    pub(super) fn vivos(&mut self, enc: &Encounter) -> Vec<u32> {
        let mut v = vec![];
        for e in self.de_oleada(enc) {
            if self.muertos.contains(&e) {
                continue;
            }
            match crate::chr_hp(e) {
                Some(h) if h <= 0 => {
                    self.muertos.insert(e);
                }
                Some(_) => {
                    self.vistos.insert(e);
                    v.push(e);
                }
                None if self.vistos.contains(&e) => {
                    self.muertos.insert(e);
                }
                None => v.push(e),
            }
        }
        v
    }

    /// Escalado del acto y vida del combate a los de la oleada (menos `primero`, que ya lo tiene).
    pub(super) fn escalar_oleada(&mut self, enc: &Encounter, primero: u32) {
        self.escalado_intentos.clear();
        let tier = self.tramo(enc.kind);
        for e in self.de_oleada(enc) {
            if e == primero {
                continue;
            }
            crate::apply_speffect_to(e, tier);
            if let Some(m) = self.fx.as_ref().and_then(|f| f.motor.as_ref()) {
                m.sumar_rival(e);
            }
        }
        self.oleada_lista = true;
        self.oleada_desde = Some(Instant::now());
    }

    /// Comprueba que cada rival de la oleada lleva su tramo y la vida del combate, y lo vuelve a poner si
    /// no. En cooperativo, todos en su copia (ver `escalar_oleada` y tick.rs).
    pub(super) fn asegurar_escalado(&mut self, enc: &Encounter) {
        let tier = self.tramo(enc.kind);
        let vida = self.fx.as_ref().and_then(|f| f.motor.as_ref()).is_some_and(|m| m.vida_rivales());
        for e in self.de_oleada(enc) {
            let sps = crate::chr_speffects(e);
            if sps.is_empty() {
                continue;
            }
            let falta_tramo = !sps.contains(&tier);
            let falta_vida = vida && !sps.contains(&crate::motor::SP_BOSS_HP);
            if falta_tramo || falta_vida {
                // Seis intentos por rival y oleada, separados 0,4 s: recién encendidos, a veces el juego
                // aún no los tiene listos y los intentos seguidos se gastaban en tres fotogramas (el jugador,
                // 19-10: Seguidores ancestrales en cooperativo con su vida base, 316 en vez de 1900). En el
                // juego del invitado no se queda nunca (son del anfitrión): por eso el tope.
                let (n, ultimo) = self.escalado_intentos.entry(e).or_insert((0, Instant::now() - Duration::from_secs(1)));
                if *n >= 6 || ultimo.elapsed() < Duration::from_millis(400) {
                    continue;
                }
                *n += 1;
                *ultimo = Instant::now();
                if *n == 1 {
                    crate::log(&format!("escalado: a {e} le faltaba {} (se pone otra vez)", if falta_tramo { "el tramo" } else { "la vida" }));
                } else if *n == 6 && !crate::coop::invitado_en_sala() {
                    crate::log(&format!("escalado: a {e} no se le queda {} tras 6 intentos", if falta_tramo { "el tramo" } else { "la vida" }));
                }
                if falta_tramo {
                    crate::apply_speffect_to(e, tier);
                }
                if falta_vida {
                    crate::apply_speffect_to(e, crate::motor::SP_BOSS_HP);
                }
            }
        }
    }

    /// A quién sigue el motor: al último que ha perdido vida mientras atacas o, si el de ahora ha
    /// caído, al primero vivo. Al cambiar, el motor parte de la vida que tenía el nuevo (así el golpe
    /// que lo ha cambiado cuenta).
    pub(super) fn objetivo(&mut self, actual: u32, vivos: &[u32]) -> u32 {
        let mut nuevo = actual;
        let mut previa = None;
        for &e in vivos {
            let Some(h) = crate::chr_hp(e) else { continue };
            if let Some(p) = self.vida_previa.insert(e, h)
                && h < p
                && e != actual
                && crate::golpe_mio(e)
            {
                nuevo = e;
                previa = Some(p);
            }
        }
        if !vivos.contains(&nuevo) {
            nuevo = vivos.first().copied().unwrap_or(actual);
            previa = self.vida_previa.get(&nuevo).copied();
        }
        if nuevo != actual
            && let Some(fx) = self.fx.as_mut()
        {
            fx.last_boss_hp = previa.unwrap_or(i32::MAX);
            if let Some(m) = fx.motor.as_mut() {
                m.cambiar_rival(nuevo);
            }
        }
        nuevo
    }

    /// Barra de un combate de grupo o con tiempo: la vida de la oleada sumada y, en el nombre, cuántos
    /// quedan, la oleada o los segundos.
    pub(super) fn barra_grupo(&self, enc: &Encounter, ent: u32) -> Option<(String, i32, i32)> {
        let ents = self.de_oleada(enc);
        let (mut hp, mut max, mut quedan) = (0, 0, 0);
        for e in if ents.len() > 1 { ents.clone() } else { vec![ent] } {
            if let Some((h, m)) = crate::chr_hp_max(e) {
                // El que se ha ido con vida (el último en pie al transformarse) ya no cuenta.
                if h > 0 && self.muertos.contains(&e) {
                    continue;
                }
                hp += h.max(0);
                max += m;
                quedan += (h > 0) as usize;
            }
        }
        if max == 0 {
            return None;
        }
        let olas = enc.grupos().len();
        let pasado = self.ready_at.map(|t| t.elapsed().as_secs() as u32).unwrap_or(0);
        let nombre = match (enc.formato, enc.segundos) {
            (Formato::Aguante, Some(s)) => rogue_core::idioma::tf("{} · hold on {} s", &[&enc.name, &s.saturating_sub(pasado)]),
            (_, Some(s)) if pasado < s => rogue_core::idioma::tf("{} · {} s", &[&enc.name, &(s - pasado)]),
            _ if olas > 1 => rogue_core::idioma::tf("{} · wave {} of {}", &[&enc.name, &(self.oleada + 1), &olas]),
            _ if ents.len() > 1 => format!("{} · {}", enc.name, quedan),
            _ => enc.name.clone(),
        };
        Some((nombre, hp, max))
    }
}

/// Entidades de las invocaciones: las del anfitrión (o sin cooperativo), luego las del primer invitado,
/// sin llegar a la de la llegada del jefe (45000980), y las de los demás invitados en
/// INVOCACION_MAS_INVITADOS (con tres o más jugadores el segundo invitado se quedaba con 4 y el tercero sin
/// ninguna). (primera, cuántas)
fn entidades_invocacion(invitado: Option<usize>) -> (u32, u32) {
    match invitado {
        None => (INVOCACION_BASE, INVOCACION_MAX),
        Some(0) => {
            let base = INVOCACION_BASE + INVOCACION_MAX;
            (base, INVOCACION_TOPE.saturating_sub(base).min(INVOCACION_MAX))
        }
        Some(k) => (INVOCACION_MAS_INVITADOS + INVOCACION_MAX * (k as u32 - 1), INVOCACION_MAX),
    }
}

/// Las invocaciones de un compañero, en el juego del anfitrión (las que pelean de verdad): su IA de
/// compañero sigue al jugador de este juego y solo pelea cuando él pelea (23-10: las del invitado
/// no atacaban si el anfitrión no atacaba, y la correa no lo arreglaba). Con la copia de su IA sin modo
/// compañero (NpcThinkParam + THINK_SIN_COMPANERO, isBuddyAI 0; tools/ModBuilder) pelean por su cuenta.
/// En el juego del invitado las del anfitrión son solo para verlas: se quedan como están.
pub(super) fn sin_companero((chr, npc, think): (i32, i32, i32)) -> (i32, i32, i32) {
    const THINK_SIN_COMPANERO: i32 = 1_000_000_000;
    if crate::coop::soy_anfitrion() && think > 0 && think < THINK_SIN_COMPANERO { (chr, npc, think + THINK_SIN_COMPANERO) } else { (chr, npc, think) }
}

/// Crea los espíritus de una invocación en `base..base+max` (los que siguen vivos ahí caen antes),
/// junto al jugador o junto a `centro`.
fn crear_invocacion(posiciones: &[[f32; 2]], (chr, npc, think): (i32, i32, i32), tandas: usize, base: u32, max: u32, centro: Option<[f32; 3]>) -> Vec<u32> {
    for k in 0..max {
        if crate::chr_hp(base + k).is_some_and(|h| h > 0) {
            crate::set_chr_hp(base + k, 0);
        }
    }
    let mut ents = vec![];
    'tandas: for t in 0..tandas {
        for p in posiciones {
            let ent = base + ents.len() as u32;
            if ent >= base + max {
                break 'tandas;
            }
            crate::spawn_chr_junto(chr, npc, think, ent, centro, p[0] + t as f32 * 1.4, p[1] - t as f32 * 1.6);
            ents.push(ent);
        }
    }
    ents
}

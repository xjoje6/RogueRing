//! «El último en pie» (18-10): en un dúo de jefes, al caer uno el que queda pasa a su segunda fase.
//! O cambia de personaje (`fase2`: el de la segunda fase está colocado apagado en la oleada 3 + k de la
//! variante, ver `Encounter::huecos_fase2`), con la misma transición dorada que los jefes de dos fases, o
//! renace en el mismo cuerpo (`renacer`: Malenia).

use super::*;
use rogue_core::run::FaseUltimo;

/// En qué va la segunda fase del superviviente.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Paso {
    /// Congelado e intocable entre ráfagas doradas (TRANSICION_FASE).
    Transicion,
    /// La segunda fase existe ya: invisible y congelada entre ráfagas (FASE2_OCULTA desde que carga).
    Oculta,
    /// Visible, haciendo su animación de transformación e intocable (ENTRADA_FASE2).
    Entrada,
    /// Renace en el mismo cuerpo: intocable `segundos` y luego con la vida llena.
    Renace,
    Hecho,
}

pub(super) struct Ultimo {
    superviviente: u32,
    /// Índice del superviviente en `enemigos`.
    indice: usize,
    paso: Paso,
    desde: Instant,
    rafaga: Instant,
    /// La segunda fase (otra entidad), cuando la hay.
    nuevo: Option<u32>,
    pos: Option<[f32; 3]>,
}

impl Director {
    /// Cada fotograma del combate: si en la primera oleada de un dúo con `ultimo_en_pie` queda uno solo,
    /// empieza su segunda fase; después la lleva paso a paso.
    pub(super) fn ultimo_tick(&mut self, enc: &Encounter, vivos: &[u32]) {
        if enc.ultimo_en_pie.is_empty() {
            return;
        }
        if self.ultimo.is_none() {
            if self.oleada != 0 || !self.oleada_lista || vivos.len() != 1 {
                return;
            }
            let ola = self.de_oleada(enc);
            if ola.len() < 2 || !ola.iter().any(|e| self.muertos.contains(e)) {
                return;
            }
            let sup = vivos[0];
            let Some(indice) = self.grupo.iter().position(|&e| e == sup) else { return };
            let Some(fase) = enc.ultimo_en_pie.iter().find(|u| u.enemigo == indice).cloned() else {
                // Sin segunda fase para el que queda: sigue el combate como siempre.
                self.ultimo = Some(Ultimo { superviviente: sup, indice, paso: Paso::Hecho, desde: Instant::now(), rafaga: Instant::now(), nuevo: None, pos: None });
                return;
            };
            self.empezar_ultimo(enc, sup, indice, &fase);
            return;
        }
        let Some(mut u) = self.ultimo.take() else { return };
        if u.paso != Paso::Hecho {
            // Mientras dura, nadie lo devuelve a su sitio por quieto.
            if let Some(v) = self.vigia.as_mut() {
                v.desde = Instant::now();
            }
            self.paso_ultimo(enc, &mut u);
        }
        self.ultimo = Some(u);
    }

    fn paso_ultimo(&mut self, enc: &Encounter, u: &mut Ultimo) {
        let fase = enc.ultimo_en_pie.iter().find(|f| f.enemigo == u.indice).cloned().unwrap_or_default();
        match u.paso {
            Paso::Transicion => {
                if u.desde.elapsed() < TRANSICION_FASE {
                    rafaga_dorada(&mut u.rafaga, &[u.superviviente]);
                    return;
                }
                // Se va el superviviente y se enciende su segunda fase donde estaba.
                let sup = u.superviviente;
                u.pos = crate::chr_pos(sup);
                crate::set_chr_disabled(sup, true);
                self.muertos.insert(sup);
                let hueco = enc.huecos_fase2().into_iter().enumerate().find(|(_, (i, _, _))| *i == u.indice);
                let Some((k, (_, hueco, _))) = hueco else {
                    crate::log(&format!("último en pie: {} sin hueco para la segunda fase", enc.name));
                    u.paso = Paso::Hecho;
                    return;
                };
                let nuevo = JEFE_MSB + hueco as u32;
                flag_set(JEFE_MSB_FLAG + 3 + k as u32, true);
                crate::set_chr_disabled(nuevo, false);
                u.nuevo = Some(nuevo);
                u.paso = Paso::Oculta;
                u.desde = Instant::now();
                self.extra_oleada.push(nuevo);
                crate::log(&format!("último en pie: {} se transforma ({sup} -> {nuevo})", enc.name));
            }
            Paso::Oculta => {
                let Some(nuevo) = u.nuevo else { return };
                if !crate::chr_loaded(nuevo) {
                    u.desde = Instant::now();
                    return;
                }
                // Recién cargada: en su sitio, invisible, quieta, intocable y escalada como el combate.
                if !self.ultimo_escalado {
                    self.ultimo_escalado = true;
                    if let Some(p) = u.pos {
                        crate::set_chr_pos(nuevo, p);
                    }
                    let tier = self.tramo(enc.kind);
                    crate::apply_speffect_to(nuevo, tier);
                    if let Some(m) = self.fx.as_ref().and_then(|f| f.motor.as_ref()) {
                        m.sumar_rival(nuevo);
                    }
                }
                if u.desde.elapsed() < FASE2_OCULTA {
                    crate::set_chr_render(nuevo, false);
                    crate::set_anim_speed(nuevo, CONGELADO);
                    crate::set_chr_invincible(nuevo, true);
                    crate::fill_hp(nuevo);
                    rafaga_dorada(&mut u.rafaga, &[nuevo]);
                    return;
                }
                crate::set_chr_render(nuevo, true);
                crate::set_anim_speed(nuevo, 1.0);
                crate::fill_hp(nuevo);
                crate::remove_speffect_from(nuevo, SP_TRANSICION);
                crate::apply_speffect_to(nuevo, SP_TRANSICION);
                if let Some(sp) = fase.fase2.and_then(|f| f.efecto) {
                    crate::apply_speffect_to(nuevo, sp);
                }
                if let Some(a) = fase.fase2.and_then(|f| f.anim) {
                    crate::log(&format!("último en pie: animación {a}: {}", crate::chr_request_anim(nuevo, a)));
                }
                u.paso = Paso::Entrada;
                u.desde = Instant::now();
            }
            Paso::Entrada => {
                if u.desde.elapsed() >= ENTRADA_FASE2 {
                    if let Some(n) = u.nuevo {
                        crate::set_chr_invincible(n, false);
                        crate::remove_speffect_from(n, SP_TRANSICION);
                    }
                    u.paso = Paso::Hecho;
                }
            }
            Paso::Renace => {
                let segundos = fase.renacer.as_ref().map(|r| r.segundos).unwrap_or(4.0).max(1.0);
                if u.desde.elapsed().as_secs_f64() >= segundos {
                    crate::fill_hp(u.superviviente);
                    crate::set_chr_invincible(u.superviviente, false);
                    u.paso = Paso::Hecho;
                }
            }
            Paso::Hecho => {}
        }
    }

    fn empezar_ultimo(&mut self, enc: &Encounter, sup: u32, indice: usize, fase: &FaseUltimo) {
        crate::set_chr_invincible(sup, true);
        crate::aviso_fase();
        let ahora = Instant::now();
        let paso = if let Some(r) = fase.renacer.as_ref() {
            for &sp in &r.quitar {
                crate::remove_speffect_from(sup, sp);
            }
            for &sp in &r.efectos {
                crate::apply_speffect_to(sup, sp);
            }
            if let Some(a) = r.anim {
                crate::chr_request_anim(sup, a);
            }
            Paso::Renace
        } else if fase.fase2.is_some() && self.jefe_msb {
            crate::set_anim_speed(sup, CONGELADO);
            crate::apply_speffect_to(sup, SP_TRANSICION);
            Paso::Transicion
        } else {
            crate::set_chr_invincible(sup, false);
            Paso::Hecho
        };
        crate::log(&format!("último en pie: queda {sup} en {} ({paso:?})", enc.name));
        self.ultimo_escalado = false;
        self.ultimo = Some(Ultimo { superviviente: sup, indice, paso, desde: ahora, rafaga: ahora, nuevo: None, pos: None });
    }
}

/// Otra ráfaga dorada cada RAFAGA_FASE: quitar y volver a poner el efecto lo hace salir de nuevo.
fn rafaga_dorada(ultima: &mut Instant, ents: &[u32]) {
    if ultima.elapsed() >= RAFAGA_FASE {
        *ultima = Instant::now();
        for &e in ents {
            crate::remove_speffect_from(e, SP_TRANSICION);
            crate::apply_speffect_to(e, SP_TRANSICION);
        }
    }
}

//! Cooperativo: quién decide cómo acaba cada combate (docs/COOP_PROTOCOLO.md).
//!
//! Manda el anfitrión: los rivales son suyos, así que solo su juego sabe de verdad si quedan. Él
//! decide la victoria (con la lógica de siempre del director: oleadas, escolta, aguante, fases) y la
//! derrota (todos muertos), y lo publica en su estado con el paso del combate. El invitado nunca saca
//! conclusiones de sus copias de los rivales: enciende cada oleada cuando la enciende el anfitrión y
//! cierra el combate con el resultado del anfitrión. Si el anfitrión se desconecta, el invitado sigue
//! en solitario y decide él.
//!
//! La muerte la hace Seamless (`always_spectate_on_death = 1`): el muerto mira a los compañeros hasta
//! que acaba el combate. Aquí solo cuenta para saber si queda alguien en pie.
//!
//! Todo es puro (sin el juego): `decidir` recibe lo que ve cada juego y devuelve qué hacer, y las
//! pruebas de abajo simulan dos juegos con mensajes que llegan tarde o se pierden.

use serde::{Deserialize, Serialize};

use crate::coop::Paso;

/// Cómo acaba un combate.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resultado {
    Victoria,
    /// Todos muertos: se acaba la run.
    Derrota,
}

/// Lo que cada juego cuenta del combate en que está (va en `coop::Estado`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Combate {
    /// Paso del combate (acto y nodo).
    pub paso: Paso,
    /// Ya en la arena con la carga terminada.
    pub llegado: bool,
    pub muerto: bool,
    /// Oleada encendida (0 la primera). La del anfitrión manda.
    pub oleada: usize,
}

/// Un compañero, tal como llega por la red.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Otro {
    pub anfitrion: bool,
    /// Con noticias recientes.
    pub conectado: bool,
    pub combate: Option<Combate>,
    /// Último combate cerrado por él y cómo acabó (el anfitrión lo sigue mandando después).
    pub ultimo: Option<(Paso, Resultado)>,
}

/// Lo que ve este juego del combate en que está.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Local {
    pub anfitrion: bool,
    pub combate: Combate,
    /// Segundos desde que llegó a la arena (anfitrión: para no esperar para siempre a quien no llega).
    pub en_arena: f64,
    /// Solo el anfitrión: la lógica del director ya da el combate por ganado.
    pub ganado: bool,
}

/// Qué tiene que hacer el director.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accion {
    /// Seguir peleando.
    Seguir,
    /// Invitado: encender esta oleada (el anfitrión ya la tiene).
    Oleada(usize),
    /// Cerrar el combate así.
    Terminar(Resultado),
    /// Sin anfitrión: el director decide solo, como en solitario.
    Solo,
}

/// Segundos que el anfitrión cuenta como «en pie» a un compañero que aún no ha llegado a la arena.
/// Pasados, si no llega, ya no cuenta (se quedó colgado en la carga).
pub const ESPERA_LLEGADA: f64 = 40.0;

/// Qué hacer este fotograma.
pub fn decidir(local: &Local, otros: &[Otro]) -> Accion {
    let paso = local.combate.paso;
    if local.anfitrion {
        // Ganar manda sobre morir: si el combate ya está ganado, es victoria aunque luego caiga alguien
        // (la nube del Avatar putrefacto mataba tras ganar).
        if local.ganado {
            return Accion::Terminar(Resultado::Victoria);
        }
        let alguien_en_pie = !local.combate.muerto
            || otros.iter().filter(|o| o.conectado).any(|o| match o.combate {
                Some(c) if c.paso == paso => !c.muerto && (c.llegado || local.en_arena < ESPERA_LLEGADA),
                // Aún sin noticias de este combate (viene de camino): se le espera un poco.
                _ => o.ultimo.map(|u| u.0) != Some(paso) && local.en_arena < ESPERA_LLEGADA,
            });
        return if alguien_en_pie { Accion::Seguir } else { Accion::Terminar(Resultado::Derrota) };
    }
    let Some(anf) = otros.iter().find(|o| o.anfitrion && o.conectado) else { return Accion::Solo };
    // El anfitrión ya cerró este combate: igual aquí.
    if let Some((_, r)) = anf.ultimo.filter(|u| u.0 == paso) {
        return Accion::Terminar(r);
    }
    // El anfitrión va por delante (otro paso posterior) sin decir cómo acabó este: si sigue en la run,
    // fue victoria (una derrota la acaba).
    if let Some(c) = anf.combate {
        if (c.paso.acto, c.paso.nodo) > (paso.acto, paso.nodo) {
            return Accion::Terminar(Resultado::Victoria);
        }
        if c.paso == paso && c.oleada > local.combate.oleada {
            return Accion::Oleada(c.oleada);
        }
    }
    Accion::Seguir
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Paso = Paso { acto: 0, nodo: 3 };

    /// Un juego simulado: lo que publica (con retraso de red) y lo que decide.
    #[derive(Clone)]
    struct Juego {
        anfitrion: bool,
        combate: Combate,
        en_arena: f64,
        ganado: bool,
        ultimo: Option<(Paso, Resultado)>,
        /// Oleadas encendidas en su mundo y rivales vivos en la que va (solo los del anfitrión cuentan).
        cerrado: Option<Resultado>,
    }

    impl Juego {
        fn nuevo(anfitrion: bool) -> Self {
            Juego {
                anfitrion,
                combate: Combate { paso: P, llegado: true, muerto: false, oleada: 0 },
                en_arena: 0.0,
                ganado: false,
                ultimo: None,
                cerrado: None,
            }
        }
        fn visto(&self, conectado: bool) -> Otro {
            Otro {
                anfitrion: self.anfitrion,
                conectado,
                combate: if self.cerrado.is_some() { None } else { Some(self.combate) },
                ultimo: self.ultimo,
            }
        }
        /// Un fotograma con lo que sabe de los demás (ya con el retraso que toque).
        fn paso(&mut self, otros: &[Otro]) -> Accion {
            if self.cerrado.is_some() {
                return Accion::Seguir;
            }
            let a = decidir(&Local { anfitrion: self.anfitrion, combate: self.combate, en_arena: self.en_arena, ganado: self.ganado }, otros);
            match a {
                Accion::Terminar(r) => {
                    self.cerrado = Some(r);
                    self.ultimo = Some((self.combate.paso, r));
                }
                Accion::Oleada(n) => self.combate.oleada = n,
                _ => {}
            }
            a
        }
    }

    /// Simula `n` fotogramas de dos juegos; los mensajes llegan con `retraso` fotogramas y se pierden
    /// los de los fotogramas de `perdidos`. `antes` cambia el mundo en cada fotograma.
    fn simular(a: &mut Juego, b: &mut Juego, n: usize, retraso: usize, perdidos: &[usize], mut antes: impl FnMut(usize, &mut Juego, &mut Juego)) {
        let mut hist_a: Vec<Otro> = vec![];
        let mut hist_b: Vec<Otro> = vec![];
        let mut ultimo_a: Option<Otro> = None;
        let mut ultimo_b: Option<Otro> = None;
        for t in 0..n {
            antes(t, a, b);
            hist_a.push(a.visto(true));
            hist_b.push(b.visto(true));
            if t >= retraso && !perdidos.contains(&t) {
                ultimo_a = Some(hist_a[t - retraso].clone());
                ultimo_b = Some(hist_b[t - retraso].clone());
            }
            a.en_arena += 0.1;
            b.en_arena += 0.1;
            let de_b: Vec<Otro> = ultimo_b.iter().cloned().collect();
            let de_a: Vec<Otro> = ultimo_a.iter().cloned().collect();
            a.paso(&de_b);
            b.paso(&de_a);
        }
    }

    #[test]
    fn muere_el_invitado_y_gana_el_anfitrion() {
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 200, 3, &[], |t, h, g| {
            if t == 20 {
                g.combate.muerto = true;
            }
            if t == 80 {
                h.ganado = true;
            }
        });
        assert_eq!(h.cerrado, Some(Resultado::Victoria));
        assert_eq!(g.cerrado, Some(Resultado::Victoria));
    }

    #[test]
    fn muere_el_anfitrion_y_gana_el_invitado() {
        // El anfitrión, muerto, sigue viendo a sus rivales: cuando el invitado los mata, gana él.
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 200, 3, &[], |t, h, _| {
            if t == 20 {
                h.combate.muerto = true;
            }
            if t == 90 {
                h.ganado = true;
            }
        });
        assert_eq!(h.cerrado, Some(Resultado::Victoria));
        assert_eq!(g.cerrado, Some(Resultado::Victoria));
    }

    #[test]
    fn mueren_los_dos() {
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 200, 3, &[], |t, h, g| {
            if t == 20 {
                g.combate.muerto = true;
            }
            if t == 50 {
                h.combate.muerto = true;
            }
        });
        assert_eq!(h.cerrado, Some(Resultado::Derrota));
        assert_eq!(g.cerrado, Some(Resultado::Derrota));
    }

    #[test]
    fn morir_tras_ganar_sigue_siendo_victoria() {
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 200, 3, &[], |t, h, g| {
            if t == 30 {
                h.ganado = true;
                h.combate.muerto = true;
                g.combate.muerto = true;
            }
        });
        assert_eq!(h.cerrado, Some(Resultado::Victoria));
        assert_eq!(g.cerrado, Some(Resultado::Victoria));
    }

    #[test]
    fn el_invitado_nunca_decide_con_sus_copias() {
        // Sus copias pueden descargarse o fallar: mientras el anfitrión no cierre, el invitado sigue.
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 300, 3, &[], |t, _, g| {
            if t == 10 {
                g.combate.muerto = true;
            }
        });
        assert_eq!(h.cerrado, None);
        assert_eq!(g.cerrado, None);
    }

    #[test]
    fn invitado_colgado_en_la_carga() {
        // No llega nunca: el anfitrión no gana ni pierde por eso. Si el anfitrión muere, se le espera
        // un rato (puede estar llegando) y luego es derrota.
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        g.combate.llegado = false;
        simular(&mut h, &mut g, 300, 3, &[], |t, h, _| {
            if t == 20 {
                h.combate.muerto = true;
            }
        });
        assert_eq!(h.cerrado, None, "aún dentro de la espera de llegada");
        simular(&mut h, &mut g, 200, 3, &[], |_, _, _| {});
        assert_eq!(h.cerrado, Some(Resultado::Derrota));
        assert_eq!(g.cerrado, Some(Resultado::Derrota), "el colgado se pone donde está el anfitrión");
    }

    #[test]
    fn mensajes_perdidos_y_tarde() {
        let perdidos: Vec<usize> = (0..400).filter(|t| t % 3 != 0).collect();
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 400, 25, &perdidos, |t, h, _| {
            if t == 100 {
                h.ganado = true;
            }
        });
        assert_eq!(g.cerrado, Some(Resultado::Victoria));
    }

    #[test]
    fn el_anfitrion_va_por_delante() {
        // El invitado se perdió el cierre y el anfitrión ya está en el combate siguiente.
        let mut g = Juego::nuevo(false);
        let h = Otro {
            anfitrion: true,
            conectado: true,
            combate: Some(Combate { paso: Paso { acto: 0, nodo: 4 }, llegado: true, muerto: false, oleada: 0 }),
            ultimo: None,
        };
        assert_eq!(g.paso(&[h]), Accion::Terminar(Resultado::Victoria));
    }

    #[test]
    fn sin_anfitrion_decide_el_invitado() {
        let g = Juego::nuevo(false);
        let h = Otro { anfitrion: true, conectado: false, combate: Some(g.combate), ultimo: None };
        let l = Local { anfitrion: false, combate: g.combate, en_arena: 5.0, ganado: false };
        assert_eq!(decidir(&l, &[h]), Accion::Solo);
        assert_eq!(decidir(&l, &[]), Accion::Solo);
    }

    #[test]
    fn oleadas_las_enciende_el_anfitrion() {
        let (mut h, mut g) = (Juego::nuevo(true), Juego::nuevo(false));
        simular(&mut h, &mut g, 100, 3, &[], |t, h, _| {
            if t == 30 {
                h.combate.oleada = 1;
            }
            if t == 60 {
                h.combate.oleada = 2;
            }
        });
        assert_eq!(g.combate.oleada, 2);
        assert_eq!(g.cerrado, None);
    }

    #[test]
    fn el_resultado_viejo_no_cierra_el_combate_nuevo() {
        let g = Juego::nuevo(false);
        let h = Otro {
            anfitrion: true,
            conectado: true,
            combate: Some(g.combate),
            ultimo: Some((Paso { acto: 0, nodo: 2 }, Resultado::Victoria)),
        };
        let l = Local { anfitrion: false, combate: g.combate, en_arena: 1.0, ganado: false };
        assert_eq!(decidir(&l, &[h]), Accion::Seguir);
    }
}

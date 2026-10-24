//! Cooperativo: caer y levantar, como en Nightreign (22-10). Nadie muere de verdad: con la opción
//! `player_no_dead` del juego la vida se queda en 1, así que Seamless no entra en su modo espectador
//! (que dejaba la cámara pegada al compañero; docs/COOP_PROTOCOLO.md). Con 1 de vida se cae: intocable,
//! solo la cámara (pad::SOLO_CAMARA) y sin bendiciones. Un compañero cerca que mantiene Y lo levanta
//! con una parte de la vida y los viales que tenía. Caídos todos, derrota (protocolo.rs, `muerto`);
//! ganado el combate, los caídos se levantan solos. Animaciones y tiempos en data/caido.json.

use super::*;
use rogue_core::coop::Suceso;
use rogue_core::idioma::{t, tf};

/// Botón para levantar a un compañero: Y (triángulo), el de interactuar del juego.
const BOTON_LEVANTAR: u16 = 0x8000;
/// Efecto que no deja morir (noDead; tools/ModBuilder), puesto mientras dura el combate cooperativo.
const SP_NO_MORIR: i32 = 79_619_051;
/// Caído: los enemigos no lo buscan ni lo prefieren como objetivo (tools/ModBuilder: targetPriority -1,
/// sin que lo vean ni lo oigan; 23-10) y no recibe daño. Sigue mientras se levanta y `GRACIA`
/// después, para poder moverse y rodar (23-10: lo remataban nada más levantarse).
const SP_IGNORADO: i32 = 79_619_052;
const GRACIA: Duration = Duration::from_secs(2);
/// Con 1 de vida tanto tiempo, caído (un golpe mortal evitado por una bendición cura justo después).
const VIDA_MINIMA_DURANTE: Duration = Duration::from_millis(400);

/// data/caido.json: animaciones (ModBuilder pone las de Nightreign en huecos de evento de ER) y reglas.
#[derive(serde::Deserialize, Clone)]
#[serde(default)]
pub(super) struct CaidoCfg {
    /// Al caer, en el suelo y al levantarse; la de quien ayuda (0, ninguna). La del suelo se para a los
    /// `congelar` segundos: una postura fija a cuatro patas (22-10).
    caer: i32,
    suelo: i32,
    congelar: f64,
    levantarse: i32,
    ayudar: i32,
    /// Lo que dura la caída antes de pasar a la del suelo, y el levantarse.
    caer_segundos: f64,
    levantarse_segundos: f64,
    /// Segundos manteniendo Y, a cuántos metros como mucho y con qué parte de la vida se levanta.
    segundos: f64,
    distancia: f32,
    vida: f64,
    /// En el suelo, el stick izquierdo lo gira sobre su eje hacia donde apunta (grados por segundo), sin
    /// moverlo de sitio. `lado`: izquierda y derecha del stick (1 o -1, según los ejes del juego).
    giro: f32,
    lado: f32,
}

impl Default for CaidoCfg {
    fn default() -> Self {
        CaidoCfg {
            caer: 60530,
            suelo: 60530,
            congelar: 0.4,
            giro: 120.0,
            lado: 1.0,
            levantarse: 60810,
            ayudar: 0,
            caer_segundos: 0.0,
            levantarse_segundos: 2.5,
            segundos: 3.0,
            distancia: 2.5,
            vida: 0.10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Caida {
    Cayendo(Instant),
    Suelo,
    Levantando(Instant),
}

/// Lo que se ve en pantalla (overlay.rs): texto y avance de 0 a 1 (negativo, sin barra).
pub static CAIDO_UI: Mutex<Option<(String, f32)>> = Mutex::new(None);

fn ui(v: Option<(String, f32)>) {
    if let Ok(mut u) = CAIDO_UI.try_lock() {
        *u = v;
    }
}

fn anim_actual() -> Option<i32> {
    crate::player_anim().map(|a| a % 1_000_000)
}

impl Director {
    /// Fuera del combate a efectos del protocolo: caído y sin levantarse aún.
    pub(super) fn caido_fuera(&self) -> bool {
        matches!(self.caido, Some(Caida::Cayendo(_) | Caida::Suelo))
    }

    pub(super) fn caido_cfg_vida(&self) -> f64 {
        self.caido_cfg.vida
    }

    /// Cada fotograma (también fuera del cooperativo: recoge la opción de no morir).
    pub(super) fn caido_tick(&mut self) {
        // Fuera del combate (abandonar, el menú, la sesión que se corta): nadie se queda en el suelo.
        if self.caido_fuera() && !matches!(self.phase, Phase::Arena { .. }) {
            self.levantarse(None, "fuera del combate");
        }
        // También tras ganar, hasta volver al centro (23-10: caído al acabar el combate, un golpe
        // que quedaba lo mató de verdad y Seamless lo metió en su modo espectador).
        let en_arena = matches!(self.phase, Phase::Arena { llegada: Some(_), .. } | Phase::Won { .. } | Phase::Returning { .. });
        // Nadie muere en el combate cooperativo. La bendición del golpe mortal también usa la opción y la
        // apaga al gastarse (motor.rs): se vuelve a poner en cada fotograma.
        let solo_suelo = crate::SOLO_SUELO.load(Ordering::Relaxed);
        if en_arena && self.coop_activo() {
            crate::set_player_no_dead(!solo_suelo);
            if solo_suelo {
                crate::speffect_propio(SP_NO_MORIR, false);
            } else if !crate::chr_speffects(0).contains(&SP_NO_MORIR) {
                let ok = crate::speffect_propio(SP_NO_MORIR, true);
                if !self.no_dead_coop {
                    crate::log(&format!("caído: efecto de no morir puesto ({ok})"));
                }
            }
            self.no_dead_coop = true;
        } else if self.no_dead_coop {
            crate::set_player_no_dead(false);
            crate::speffect_propio(SP_NO_MORIR, false);
            self.no_dead_coop = false;
        }
        crate::VIDA_MINIMA_1.store(self.no_dead_coop, Ordering::Relaxed);
        let parados = crate::GOLPES_PARADOS.swap(0, Ordering::Relaxed);
        if parados > 0 {
            crate::log(&format!("caído: golpe mortal parado tras el daño ({parados})"));
        }
        // Muerte de verdad (algo se ha saltado el efecto): la lleva el juego, sin animaciones ni vida
        // nuestras encima (22-10: atascado en la de muerte y carteles de muerte al ganar).
        if player_hp() == Some(0) {
            if self.caido_fuera() || !self.muerte_real {
                crate::log("caído: muerte de verdad; la lleva el juego");
            }
            self.muerte_real = true;
            if self.caido.is_some() {
                self.caido = None;
                pad::SOLO_CAMARA.store(false, Ordering::Relaxed);
                crate::set_anim_speed(0, 1.0);
                crate::speffect_propio(SP_IGNORADO, false);
            }
            ui(None);
            return;
        }
        self.muerte_real = false;
        if self.caido.is_none()
            && let Some(t) = self.gracia_hasta
        {
            let en_centro = self.intocable_derrota && crate::player_area() == Some(HUB_AREA) && player_hp().unwrap_or(0) > 0;
            if Instant::now() >= t || en_centro {
                self.intocable_derrota = false;
                self.fin_gracia();
            } else {
                crate::set_player_invincible(true);
            }
        }
        match self.caido {
            None => {
                ui(None);
                if !(en_arena && self.coop_activo()) {
                    self.vida_baja_desde = None;
                    return;
                }
                if player_hp() == Some(1) {
                    let desde = *self.vida_baja_desde.get_or_insert_with(Instant::now);
                    if desde.elapsed() >= VIDA_MINIMA_DURANTE {
                        self.caer();
                    }
                } else {
                    self.vida_baja_desde = None;
                    self.ayudar_tick();
                }
            }
            Some(c) => {
                crate::set_player_invincible(true);
                let cfg = self.caido_cfg.clone();
                match c {
                    Caida::Cayendo(desde) => {
                        if desde.elapsed().as_secs_f64() >= cfg.caer_segundos {
                            self.caido = Some(Caida::Suelo);
                        }
                        self.ui_caido();
                    }
                    Caida::Suelo => {
                        // Postura fija: la del suelo parada a los `congelar` s (si algo la cambia, otra vez).
                        if anim_actual() != Some(cfg.suelo) {
                            crate::set_anim_speed(0, 1.0);
                            crate::player_request_anim(cfg.suelo);
                            self.postura_desde = Some(Instant::now());
                        } else if self.postura_desde.is_some_and(|t| t.elapsed().as_secs_f64() >= cfg.congelar) {
                            crate::set_anim_speed(0, 0.0);
                        }
                        // Con el stick, gira sobre su eje hacia donde apunta, sin moverse de sitio.
                        let dt = self.giro_at.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0).min(0.1);
                        self.giro_at = Some(Instant::now());
                        let (sx, sy) = pad::stick_izquierdo();
                        if sx.hypot(sy) > 0.35
                            && let Some(p) = crate::punto_del_stick(sx, sy, cfg.lado)
                        {
                            crate::girar_hacia(p, cfg.giro.to_radians() * dt);
                        }
                        self.ui_caido();
                    }
                    Caida::Levantando(desde) => {
                        ui(None);
                        if desde.elapsed().as_secs_f64() >= cfg.levantarse_segundos {
                            self.caido = None;
                            pad::SOLO_CAMARA.store(false, Ordering::Relaxed);
                            // Intocable un poco más (`GRACIA`); con la run perdida, hasta la siguiente.
                            let fin = Instant::now() + GRACIA;
                            self.gracia_hasta = Some(self.gracia_hasta.map_or(fin, |t| t.max(fin)));
                            crate::log(&format!("caído: de pie, vida {:?}", crate::player_hp_max()));
                        }
                    }
                }
            }
        }
    }

    fn caer(&mut self) {
        let cfg = &self.caido_cfg;
        self.caido = Some(Caida::Cayendo(Instant::now()));
        self.vida_baja_desde = None;
        self.ayuda = None;
        pad::SOLO_CAMARA.store(true, Ordering::Relaxed);
        crate::set_player_invincible(true);
        crate::speffect_propio(SP_IGNORADO, true);
        crate::set_anim_speed(0, 1.0);
        crate::player_request_anim(cfg.caer);
        // En el suelo, con media vida por dentro (sigue intocable): con 1, cualquier daño que se colara
        // (un estado, un golpe en el fotograma en que se gana) lo mataba de verdad. Al levantarse, la de
        // levantarse (`levantarse`).
        if let Some((_, max)) = crate::player_hp_max() {
            crate::set_player_hp((max / 2).max(1));
        }
        // Si la de caer es la misma que la del suelo, se para desde aquí.
        self.postura_desde = Some(Instant::now());
        crate::log(&format!("caído: al suelo (vida {:?}, viales {:?})", crate::player_hp_max(), equip::hp_flasks()));
    }

    /// Se levanta: con `vida` (parte de la máxima) si se la dan, o tal cual (fin del combate).
    pub(super) fn levantarse(&mut self, vida: Option<f64>, por: &str) {
        if !self.caido_fuera() {
            return;
        }
        if let (Some(v), Some((_, max))) = (vida, crate::player_hp_max()) {
            crate::set_player_hp(((max as f64 * v).round() as i32).max(1));
        }
        crate::set_anim_speed(0, 1.0);
        crate::player_request_anim(self.caido_cfg.levantarse);
        self.caido = Some(Caida::Levantando(Instant::now()));
        crate::log(&format!("caído: se levanta ({por}), vida {:?}, viales {:?}", crate::player_hp_max(), equip::hp_flasks()));
    }

    /// Se acaba la gracia tras levantarse (o la run perdida que la alargaba): otra vez se le ve y se le daña.
    pub(super) fn fin_gracia(&mut self) {
        if self.gracia_hasta.take().is_none() {
            return;
        }
        crate::speffect_propio(SP_IGNORADO, false);
        if !matches!(self.phase, Phase::Won { .. } | Phase::Returning { .. }) {
            crate::set_player_invincible(false);
        }
        crate::log("caído: fin de la gracia");
    }

    /// Run perdida en cooperativo: intocable e ignorado hasta que está fuera del coliseo, en el centro
    /// (23-10: se quedaba en la arena con los enemigos y lo volvían a matar; el invitado no viaja
    /// solo y empezar otra run no lo sacaba de allí).
    pub(super) fn intocable_hasta_la_siguiente(&mut self) {
        crate::speffect_propio(SP_IGNORADO, true);
        crate::set_player_invincible(true);
        self.gracia_hasta = Some(Instant::now() + Duration::from_secs(3600));
        self.intocable_derrota = true;
    }

    /// Aviso del caído: si un compañero lo está levantando, su nombre y su barra (la que ve él).
    fn ui_caido(&self) {
        let yo = crate::coop::yo();
        let quien = crate::coop::companeros()
            .into_iter()
            .find_map(|c| c.estado.as_ref()?.levantando.filter(|l| l.0 == yo).map(|l| (c.jugador.nombre.clone(), l.1)));
        match quien {
            Some((nombre, avance)) => ui(Some((tf("{} is raising you", &[&nombre]), avance))),
            None => ui(Some((t("You have fallen. A companion can raise you.").into(), -1.0))),
        }
    }

    /// Para el estado del cooperativo: a quién estoy levantando y cuánto llevo.
    pub(super) fn levantando_ahora(&self) -> Option<(u64, f32)> {
        let (s, desde) = self.ayuda?;
        Some((s, (desde.elapsed().as_secs_f64() / self.caido_cfg.segundos.max(0.1)).min(1.0) as f32))
    }

    /// Un compañero me ha levantado (Suceso::Levantar).
    pub(super) fn levantado_por(&mut self, de: u64, a: u64) {
        if a != crate::coop::yo() {
            return;
        }
        let nombre = crate::coop::companeros().into_iter().find(|c| c.jugador.steam == de).map(|c| c.jugador.nombre).unwrap_or_default();
        let vida = self.caido_cfg.vida;
        self.levantarse(Some(vida), &format!("lo levanta {nombre}"));
    }

    /// De pie en la arena: si hay un compañero caído cerca, mantener Y lo levanta.
    fn ayudar_tick(&mut self) {
        let cfg = self.caido_cfg.clone();
        let mio = crate::player_pos();
        let paso = self.paso();
        // El caído más cercano (con tres o más jugadores puede haber varios).
        let cerca = crate::coop::companeros()
            .into_iter()
            .filter_map(|c| {
                c.estado.as_ref()?.combate.as_ref().filter(|k| Some(k.paso) == paso && k.muerto)?;
                let p = crate::pos_jugador(c.jugador.steam)?;
                let m = mio?;
                let d = (p[0] - m[0]).hypot(p[2] - m[2]);
                (d <= cfg.distancia && (p[1] - m[1]).abs() < 2.0).then_some((d, c.jugador.steam, c.jugador.nombre))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s, n)| (s, n));
        let Some((steam, nombre)) = cerca else {
            self.ayuda = None;
            ui(None);
            return;
        };
        let pulsado = pad::UI_BUTTONS.load(Ordering::Relaxed) & BOTON_LEVANTAR != 0;
        match self.ayuda {
            Some((s, desde)) if s == steam && pulsado => {
                let avance = (desde.elapsed().as_secs_f64() / cfg.segundos.max(0.1)) as f32;
                if avance >= 1.0 {
                    crate::coop::suceso(Suceso::Levantar { a: steam });
                    crate::log(&format!("caído: levanto a {nombre}"));
                    self.ayuda = None;
                    ui(None);
                    return;
                }
                ui(Some((tf("Raising {}", &[&nombre]), avance)));
            }
            _ if pulsado => {
                self.ayuda = Some((steam, Instant::now()));
                if cfg.ayudar > 0 {
                    crate::player_request_anim(cfg.ayudar);
                }
                ui(Some((tf("Raising {}", &[&nombre]), 0.0)));
            }
            _ => {
                self.ayuda = None;
                ui(Some((tf("Hold Y to raise {}", &[&nombre]), -1.0)));
            }
        }
    }
}

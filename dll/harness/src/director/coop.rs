//! Cooperativo en el director: votos del mapa, viajes y lo que se cuenta a los demás (crate::coop
//! para el canal y rogue_core::coop para cómo se decide).
//!
//! - Mapa: como en Slay the Spire 2. Cada uno vota un nodo y nadie entra hasta que han votado todos;
//!   si no coinciden, se sortea entre los votos (el mismo resultado en todos los juegos).
//! - Viajes: solo el anfitrión viaja. A los demás los lleva Seamless con él: un invitado que viaja
//!   por su cuenta se queda cargando para siempre (16-10). El anfitrión espera `ESPERA_VIAJE`
//!   para que los demás tengan ya su coliseo preparado (la variante del MSB) cuando los arrastre.
//! - Barra del rival: sus estados del juego los manda el anfitrión (los demás no los leen bien).

use super::*;
use rogue_core::coop::{self as reglas, Estado, EstadoRival as EstadoCoop, Paso};
use rogue_core::idioma::{t, tf};

/// Votos de los demás en el mapa (nodo, nombre), a quién se espera y dónde tienen el cursor.
pub(super) type VotosCoop = (Vec<((i32, i32), String)>, Vec<String>, Vec<((i32, i32), String)>);

/// Objetos de Seamless Co-op 2.0.1 (sus propios ids, que da al descansar en una gracia): crear sala
/// (Vasija pequeña), unirse (Efigie de Malenia) y salir (Niebla de separación). Medidos en el juego.
const OBJ_VASIJA: u32 = 8_380_001;
const OBJ_EFIGIE: u32 = 8_380_002;
const OBJ_NIEBLA: u32 = 8_380_004;

/// Lo que espera el anfitrión antes de viajar al coliseo tras decidir el nodo, y lo máximo que espera
/// a que los demás vayan también hacia el combate (en un evento, cada uno lee el resultado a su ritmo).
const ESPERA_VIAJE: Duration = Duration::from_millis(1500);
const ESPERA_MAX_VIAJE: Duration = Duration::from_secs(60);

/// Letras de la contraseña elegida en el juego: sin las que se confunden (O/0, I/1).
pub(super) const CARACTERES_CLAVE: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
/// Largo de la contraseña elegida en el juego.
const LARGO_CLAVE: usize = 6;

/// Veces que se intenta usar la Vasija o la Niebla si la sesión no cambia (16-10: en la partida de
/// el jugador el primer «usar» no siempre llegaba).
const INTENTOS_SALA: u8 = 3;

/// Vida de las invocaciones del Invocador que hay ahora (las de todos: entidades 45000960 a 45000979),
/// redondeada al 5 % para no mandar un mensaje por cada golpe.
fn vida_invocaciones() -> Vec<(u32, i32)> {
    (INVOCACION_BASE..INVOCACION_TOPE)
        .filter_map(|e| {
            let (h, m) = crate::chr_hp_max(e)?;
            let v = if h <= 0 { 0 } else { ((h as i64 * 20 / m.max(1) as i64).max(1) * m as i64 / 20) as i32 };
            Some((e, v))
        })
        .collect()
}

/// Qué está haciendo un compañero que no está votando en el mapa (su `Estado::fase`).
/// Lo más que espera el anfitrión a que todos lleguen a la arena (esperando_llegada).
const ESPERA_LLEGADA_MAX: Duration = Duration::from_secs(30);

/// Votos del mapa iguales este rato antes de irse (coop_tick).
const VOTOS_ESTABLES: Duration = Duration::from_millis(1500);

fn haciendo(fase: &str) -> &'static str {
    match fase {
        "recompensa" => t("choosing a reward"),
        "evento" => t("in an event"),
        "tienda" => t("at the shop"),
        "gracia" => t("at a grace"),
        "combate" => t("fighting"),
        "mapa" => t("looking at the map"),
        "personaje" => t("choosing a character"),
        "elegido" => t("character chosen"),
        "inicio" => t("starting the run"),
        _ => t("in the menus"),
    }
}

impl Director {
    /// Run de cooperativo con compañeros en la sesión (o la sombra del banco de pruebas).
    pub(super) fn coop_activo(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.coop) && crate::coop::hay_companeros()
    }

    /// Paso de la run: acto y nodos recorridos.
    pub(super) fn paso_actual(&self) -> Option<Paso> {
        self.paso()
    }

    pub(super) fn paso(&self) -> Option<Paso> {
        self.run.as_ref().map(|r| Paso { acto: r.act_index, nodo: r.path.len() })
    }

    /// Lo que este juego cuenta del combate en que está (protocolo.rs).
    pub(super) fn combate_coop(&self) -> Option<rogue_core::protocolo::Combate> {
        let paso = self.paso()?;
        match &self.phase {
            Phase::Arena { llegada, .. } => Some(rogue_core::protocolo::Combate {
                paso,
                llegado: llegada.is_some(),
                muerto: player_hp() == Some(0) || self.caido_fuera(),
                oleada: self.oleada,
            }),
            _ => None,
        }
    }

    /// Qué dice el protocolo del cooperativo sobre el combate en curso (protocolo.rs). `ganado`: la
    /// lógica de siempre del director lo da por ganado (solo cuenta en el anfitrión). Fuera del
    /// cooperativo, `Solo`.
    pub(super) fn protocolo(&self, ganado: bool, en_arena: f64) -> rogue_core::protocolo::Accion {
        use rogue_core::protocolo::{Accion, Local, Otro, decidir};
        let (Some(combate), true) = (self.combate_coop(), self.coop_activo()) else { return Accion::Solo };
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        let otros: Vec<Otro> = crate::coop::companeros()
            .into_iter()
            .map(|c| {
                let e = c.estado.filter(|e| e.semilla == semilla && e.version == reglas::VERSION);
                Otro {
                    anfitrion: c.jugador.anfitrion,
                    conectado: c.conectado && e.is_some(),
                    combate: e.as_ref().and_then(|e| e.combate),
                    ultimo: e.as_ref().and_then(|e| e.ultimo),
                }
            })
            .collect();
        decidir(&Local { anfitrion: crate::coop::soy_anfitrion(), combate, en_arena, ganado }, &otros)
    }

    /// Combate de un evento en cooperativo: listo; se va cuando lo están todos (`coop_tick`).
    pub(super) fn listo_combate(&mut self) -> String {
        let Some(paso) = self.paso() else { return "sin run".into() };
        if self.listo == Some(paso) {
            return "ya estás listo".into();
        }
        self.listo = Some(paso);
        play_se(Se::Confirm);
        "cooperativo: listo para el combate".into()
    }

    /// A quién se espera para el combate de un evento.
    pub(super) fn esperando_listos(&self) -> Vec<String> {
        let paso = self.paso();
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        crate::coop::companeros()
            .into_iter()
            .filter(|c| c.estado.as_ref().is_some_and(|e| c.conectado && e.semilla == semilla && e.listo != paso))
            .map(|c| c.jugador.nombre)
            .collect()
    }

    /// Evento en cooperativo: se vota la opción como el nodo del mapa; se resuelve cuando han votado
    /// todos (`coop_tick`).
    pub(super) fn votar_evento(&mut self, k: usize) -> String {
        let Some(paso) = self.paso() else { return "sin run".into() };
        if self.voto_evento.is_some_and(|(p, _)| p == paso) {
            return "ya has votado".into();
        }
        self.voto_evento = Some((paso, k));
        play_se(Se::Confirm);
        format!("cooperativo: voto de evento {k}")
    }

    /// Votos del evento en curso para la pantalla: (opción, nombre), el mío incluido, y a quién se
    /// espera.
    pub(super) fn votos_evento(&self) -> (Vec<(usize, String)>, Vec<String>) {
        if !self.coop_activo() {
            return (vec![], vec![]);
        }
        let paso = self.paso();
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        let mut votos: Vec<(usize, String)> = self.voto_evento.filter(|v| Some(v.0) == paso).map(|v| (v.1, t("You").to_string())).into_iter().collect();
        let mut espera = vec![];
        for c in crate::coop::companeros() {
            let Some(e) = c.estado.filter(|e| c.conectado && e.semilla == semilla) else { continue };
            match e.voto_evento.filter(|v| Some(v.0) == paso) {
                Some((_, k)) => votos.push((k, c.jugador.nombre.clone())),
                None => espera.push(c.jugador.nombre.clone()),
            }
        }
        (votos, espera)
    }

    /// Aliados en combate para el HUD (rogue_ui::aliados): los compañeros que también pelean y los
    /// espíritus de tu invocación (16-10).
    pub(super) fn aliados(&self) -> Vec<rogue_ui::aliados::Aliado> {
        if !matches!(self.phase, Phase::Arena { .. } | Phase::Combat { .. }) {
            return vec![];
        }
        let mut v = vec![];
        if self.coop_activo() {
            for c in crate::coop::companeros() {
                if let Some(vida) = c.estado.filter(|e| c.conectado && e.fase == "combate").and_then(|e| e.vida) {
                    v.push(rogue_ui::aliados::Aliado { nombre: c.jugador.nombre, vidas: vec![vida], companero: true });
                }
            }
        }
        // Solo los espíritus que existen y siguen vivos: los huecos de la ceniza que no salen o ya han
        // caído se dibujaban como barras vacías (19-10).
        let vivos = |ents: &[u32]| -> Vec<(i32, i32)> { ents.iter().filter_map(|&e| crate::chr_hp_max(e)).filter(|v| v.0 > 0).collect() };
        let vidas = vivos(&self.invocacion_ents);
        if !vidas.is_empty() {
            let nombre = self.invocacion_actual().map(|i| i.nombre).unwrap_or_default();
            v.push(rogue_ui::aliados::Aliado { nombre, vidas, companero: false });
        }
        // Los aliados de mis bendiciones (Espíritu errante...) y los de las de los compañeros.
        if let Some(m) = self.fx.as_ref().and_then(|f| f.motor.as_ref()) {
            for (src, ents) in m.aliados() {
                let vidas = vivos(&ents);
                if !vidas.is_empty() {
                    let nombre = self.relics.reliquias.iter().find(|r| r.id == src).map(|r| r.nombre.clone()).unwrap_or_default();
                    v.push(rogue_ui::aliados::Aliado { nombre, vidas, companero: false });
                }
            }
        }
        let ajenos: Vec<u32> = self.aliados_ajenos.iter().map(|a| a.0).collect();
        let vidas = vivos(&ajenos);
        if !vidas.is_empty() {
            v.push(rogue_ui::aliados::Aliado { nombre: t("Companion's allies").into(), vidas, companero: false });
        }
        // Las invocaciones de los compañeros (su copia en este juego), mientras existan.
        if self.coop_activo() {
            for (jugador, ceniza, ents) in self.invocaciones_ajenas.values() {
                let vidas = vivos(ents);
                if !vidas.is_empty() {
                    v.push(rogue_ui::aliados::Aliado { nombre: format!("{ceniza} ({jugador})"), vidas, companero: false });
                }
            }
        }
        v
    }

    /// ¿Van ya todos los compañeros de esta run hacia el combate (su director en «combate» en mi
    /// paso)? Hasta entonces el anfitrión no viaja: Seamless se los llevaría a mitad de una pantalla.
    fn todos_al_combate(&self) -> bool {
        let paso = self.paso();
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        crate::coop::companeros()
            .into_iter()
            .filter_map(|c| c.estado.filter(|e| c.conectado && e.semilla == semilla))
            .all(|e| e.fase == "combate" && Some(e.paso) == paso)
    }

    /// Compañeros que aún no han acabado lo del nodo (recompensa, gracia, evento, tienda...), con lo que
    /// hacen: hasta que estén todos en el mapa no se vota (17-10: «esperas a que tu compañero
    /// elija recompensa, gracia...», todo por partes para que no se desincronice).
    pub(super) fn esperando_mapa(&self) -> Vec<String> {
        if !self.coop_activo() || !matches!(self.phase, Phase::Map) {
            return vec![];
        }
        let Some(paso) = self.paso() else { return vec![] };
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        crate::coop::companeros()
            .into_iter()
            .filter_map(|c| c.estado.filter(|e| c.conectado && e.semilla == semilla).map(|e| (c.jugador.nombre, e)))
            .filter(|(_, e)| {
                let listo = (e.paso.acto, e.paso.nodo) > (paso.acto, paso.nodo)
                    || (e.paso == paso && (e.fase == "mapa" || e.voto.is_some_and(|v| v.0 == paso)));
                !listo
            })
            .map(|(n, e)| format!("{n} ({})", haciendo(&e.fase)))
            .collect()
    }

    /// Mapa en cooperativo: el voto se puede cambiar hasta que han votado todos (23-10); se va
    /// cuando los votos de todos llevan un rato iguales (`coop_tick`, VOTOS_ESTABLES).
    pub(super) fn votar(&mut self, to: (i32, i32)) -> String {
        let Some(paso) = self.paso() else { return "sin run".into() };
        if !self.esperando_mapa().is_empty() {
            play_se(Se::Back);
            return "cooperativo: aún no se vota (esperando a los demás)".into();
        }
        match self.voto.filter(|(p, _)| *p == paso) {
            Some((_, n)) if n == to => return "ya has votado ese".into(),
            Some(_) if self.votos_estables.is_some() => {
                play_se(Se::Back);
                return "cooperativo: ya han votado todos".into();
            }
            _ => {}
        }
        self.voto = Some((paso, to));
        play_se(Se::Confirm);
        format!("cooperativo: voto {to:?}")
    }

    /// Viaje al coliseo (`COLOSSEUM_WARP_FLAG`) o al hub (`HUB_WARP_FLAG`). En una sesión solo viaja
    /// el anfitrión, y al coliseo con `ESPERA_VIAJE` de margen.
    pub(super) fn viajar(&mut self, flag: u32) {
        if !crate::coop::hay_companeros() {
            flag_set(flag, true);
            return;
        }
        if !crate::coop::soy_anfitrion() {
            crate::log("cooperativo: el invitado no viaja; lo trae Seamless con el anfitrión");
            return;
        }
        if flag != HUB_WARP_FLAG {
            self.viaje_en = Some((flag, Instant::now() + ESPERA_VIAJE));
        } else {
            flag_set(flag, true);
        }
    }

    /// Votos de los compañeros para dibujarlos en el mapa: (nodo, nombre), a quién se espera (con lo
    /// que está haciendo) y el cursor de los que miran el mapa en este mismo paso.
    pub(super) fn votos_coop(&self) -> VotosCoop {
        if !self.coop_activo() {
            return (vec![], vec![], vec![]);
        }
        let paso = self.paso();
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        let (mut votos, mut espera, mut cursores) = (vec![], vec![], vec![]);
        for c in crate::coop::companeros() {
            // En otra run (otra semilla): sus votos no cuentan; se avisa.
            if let Some(e) = c.estado.as_ref().filter(|e| c.conectado && e.semilla.is_some() && e.semilla != semilla && e.fase != "menu") {
                let _ = e;
                espera.push(format!("{} ({})", c.jugador.nombre, t("in another run")));
                continue;
            }
            let Some(e) = c.estado.filter(|e| c.conectado && e.semilla == semilla) else { continue };
            let nombre = c.jugador.nombre.clone();
            if let Some((_, n)) = e.voto.filter(|v| Some(v.0) == paso) {
                votos.push((n, nombre));
                continue;
            }
            let en_mapa = e.fase == "mapa" && Some(e.paso) == paso;
            if let Some(n) = e.cursor.filter(|_| en_mapa) {
                cursores.push((n, nombre.clone()));
            }
            espera.push(format!("{nombre} ({})", haciendo(if en_mapa { "mapa" } else { &e.fase })));
        }
        (votos, espera, cursores)
    }

    /// Cada fotograma: publicar el estado, decidir el nodo cuando han votado todos y el viaje que
    /// espera el anfitrión.
    pub(super) fn coop_tick(&mut self) {
        self.sala_tick();
        if let Some((flag, en)) = self.viaje_en
            && Instant::now() >= en
            && (flag == HUB_WARP_FLAG || en.elapsed() > ESPERA_MAX_VIAJE || self.todos_al_combate())
        {
            self.viaje_en = None;
            flag_set(flag, true);
        }
        let fase = match &self.phase {
            Phase::Map => "mapa",
            Phase::Arena { .. } | Phase::Combat { .. } | Phase::Won { .. } => "combate",
            Phase::Returning { .. } | Phase::Reward { .. } | Phase::Replace { .. } => "recompensa",
            Phase::Event { .. } => "evento",
            Phase::Shop { .. } => "tienda",
            Phase::Rest => "gracia",
            // Empezando una run (personaje y bendición inicial): con esta semilla entran los demás. En el
            // menú principal la semilla es la de la run anterior (17-10: el invitado cogió esa).
            // CharSelect, aparte: ahí se esperan todos antes de elegir (nodos.rs, faltan_en_seleccion).
            Phase::CharSelect if self.elegido.is_some() => "elegido",
            Phase::CharSelect => "personaje",
            Phase::Neow { .. } => "inicio",
            _ => "menu",
        };
        let paso = self.paso().unwrap_or_default();
        // El voto no se borra al decidir: los demás lo necesitan aunque yo ya vaya por el combate.
        let en_mapa = matches!(self.phase, Phase::Map);
        let mio = Estado {
            version: reglas::VERSION,
            nombre: String::new(),
            fase: fase.into(),
            paso,
            semilla: self.run.as_ref().map(|r| r.seed.clone()),
            voto: self.voto.filter(|_| self.run.is_some()),
            voto_evento: self.voto_evento.filter(|_| self.run.is_some()),
            tamano: Some(self.fx.as_ref().and_then(|f| f.motor.as_ref()).map(|m| m.tamano()).unwrap_or_else(|| self.tamano_pasivo()) as f32)
                .filter(|s| (s - 1.0).abs() > 0.001),
            sucesos: vec![],
            combate: self.combate_coop(),
            ultimo: self.ultimo_coop,
            listo: self.listo.filter(|_| self.run.is_some()),
            levantando: self.levantando_ahora(),
            invocaciones: if crate::coop::soy_anfitrion() && fase == "combate" { vida_invocaciones() } else { vec![] },
            vida: crate::player_hp_max().map(|(hp, max)| ((hp.max(0) as i64 * 50 / max.max(1) as i64) as i32 * max / 50, max)),
            cursor: if en_mapa { self.available_sorted().get(self.sel).copied() } else { None },
            rival: (crate::coop::soy_anfitrion() && fase == "combate").then(|| self.rival_coop.clone()),
            ascension: self.run.as_ref().filter(|_| crate::coop::soy_anfitrion()).map(|r| r.ascension),
        };
        crate::coop::publicar(mio.clone());
        for (de, s) in crate::coop::sucesos_recibidos() {
            self.suceso_recibido(de, s);
        }
        self.seguir_anfitrion();
        self.ascension_anfitrion();
        self.elegir_si_todos();
        // Con Seamless cargado no salen las bendiciones de golpe mortal: Seamless anula la opción de no
        // morir que usan, también jugando solo con su lanzador (repaso del 18-10).
        self.relics.coop = self.run.as_ref().is_some_and(|r| r.coop) || crate::seamless_cargado();
        // Invitado: sus copias de las invocaciones, con la vida de las del anfitrión (17-10: en su
        // juego había muerto y en el del invitado seguía viva).
        if !crate::coop::soy_anfitrion()
            && let Some(host) = crate::coop::companeros().into_iter().find(|c| c.jugador.anfitrion && c.conectado)
            && let Some(e) = host.estado
        {
            for (ent, vida) in e.invocaciones {
                if let Some((h, m)) = crate::chr_hp_max(ent)
                    && h > 0
                    && (vida == 0 || (h - vida).abs() > m / 20)
                {
                    crate::set_chr_hp(ent, vida);
                }
            }
        }
        // Aliados de bendiciones de los compañeros: su mejora al aparecer y fuera al acabar su tiempo.
        let ahora = Instant::now();
        for a in self.aliados_ajenos.iter_mut() {
            if !a.2 && crate::chr_loaded(a.0) {
                a.2 = crate::apply_speffect_to(a.0, crate::motor::SUMMON_LEVEL);
                crate::fill_hp(a.0);
            }
            if ahora >= a.1 {
                crate::set_chr_hp(a.0, 0);
            }
        }
        self.aliados_ajenos.retain(|a| ahora < a.1);
        // Tamaño de los compañeros: el juego de cada uno solo escala su propio personaje (16-10:
        // con Diminuto, el compañero lo veía de su tamaño). Y los caídos no se ven. Su personaje se
        // busca por su SteamID.
        for c in crate::coop::companeros() {
            let e = c.estado.as_ref().filter(|_| c.conectado);
            crate::set_jugador_scale(c.jugador.steam, e.and_then(|e| e.tamano).unwrap_or(1.0));
        }
        if !self.coop_activo() {
            return;
        }
        let otros: Vec<(u64, Estado, bool)> =
            crate::coop::companeros().into_iter().filter_map(|c| Some((c.jugador.steam, c.estado?, c.conectado))).collect();
        let semilla = mio.semilla.clone().unwrap_or_default();
        if matches!(self.phase, Phase::Event { result: None, .. }) && self.decidido_evento != Some(paso) {
            let Some(votos) = reglas::completos((crate::coop::yo(), &mio), &otros, |e| e.voto_evento) else { return };
            let Some(k) = reglas::sortear(&votos, &semilla, "evento", paso) else { return };
            crate::log(&format!("cooperativo: votos del evento {votos:?} -> opción {k}"));
            self.decidido_evento = Some(paso);
            let msg = self.resolver_evento(k);
            crate::log(&msg);
            return;
        }
        // Combate que sale de un evento: se va cuando han pulsado «Pelear» todos (16-10: cada uno
        // iba a su ritmo y se desincronizaban).
        if let Phase::Event { result: Some((_, Some(enc))), .. } = &self.phase
            && self.listo == Some(paso)
            && reglas::completos((crate::coop::yo(), &mio), &otros, |e| e.listo.map(|p| (p, ()))).is_some()
        {
            let enc = enc.clone();
            crate::log("cooperativo: todos listos para el combate del evento");
            self.listo = None;
            let msg = self.start_combat(enc);
            crate::log(&msg);
            return;
        }
        // Cofre trampa: igual, se va cuando han pulsado «Pelear» todos (19-10: el anfitrión se fue
        // solo y el invitado, al pulsar después, se quedó esperando un viaje que ya no iba a llegar).
        if let Phase::Treasure { relic, runes, trap: Some(enc) } = &self.phase
            && self.listo == Some(paso)
            && reglas::completos((crate::coop::yo(), &mio), &otros, |e| e.listo.map(|p| (p, ()))).is_some()
        {
            let (relic, runes, enc) = (relic.clone(), *runes, enc.clone());
            crate::log("cooperativo: todos listos para el combate del cofre");
            self.listo = None;
            let msg = self.pelear_cofre(relic, runes, enc);
            crate::log(&msg);
            return;
        }
        if !en_mapa || self.decidido == Some(paso) {
            return;
        }
        crate::coop::pista_disponibles(self.available_sorted());
        let Some(votos) = reglas::votos_completos((crate::coop::yo(), &mio), &otros) else {
            self.votos_estables = None;
            return;
        };
        // Con todos los votos, un momento de margen por si a alguien le llega tarde el cambio de otro:
        // se va cuando llevan VOTOS_ESTABLES iguales, y así todos deciden con los mismos.
        let clave = format!("{votos:?}");
        match &self.votos_estables {
            Some((k, desde)) if *k == clave => {
                if desde.elapsed() < VOTOS_ESTABLES {
                    return;
                }
            }
            _ => {
                self.votos_estables = Some((clave, Instant::now()));
                return;
            }
        }
        self.votos_estables = None;
        let Some(destino) = reglas::decidir(&votos, &semilla, paso) else { return };
        crate::log(&format!("cooperativo: votos {votos:?} -> {destino:?}"));
        self.decidido = Some(paso);
        let msg = self.travel(destino);
        crate::log(&msg);
    }

    /// Barra del rival: el anfitrión guarda sus estados del juego para mandarlos; los demás cambian
    /// los suyos por los del anfitrión (los efectos de las bendiciones siguen siendo de cada uno).
    pub(super) fn rival_coop(&mut self, estados: &mut Vec<(i32, crate::motor::EstadoRival)>) {
        if !crate::coop::hay_companeros() {
            return;
        }
        if crate::coop::soy_anfitrion() {
            self.rival_coop = estados
                .iter()
                .filter_map(|(_, e)| {
                    Some(EstadoCoop { estado: e.estado?, nombre: e.nombre.clone(), color: e.color, lleno: e.lleno, destello: e.destello, texto: e.texto.clone() })
                })
                .collect();
            return;
        }
        let Some(suyos) = crate::coop::companeros().into_iter().find(|c| c.jugador.anfitrion && c.conectado).and_then(|c| c.estado?.rival) else {
            return;
        };
        estados.retain(|(_, e)| e.estado.is_none());
        estados.extend(suyos.into_iter().map(|s| {
            (0, crate::motor::EstadoRival { estado: Some(s.estado), src: String::new(), nombre: s.nombre, color: s.color, lleno: s.lleno, destello: s.destello, texto: s.texto })
        }));
    }

    /// Lo que pasa en el juego de un compañero y hay que hacer aquí: el anfitrión aplica el daño de sus
    /// bendiciones y crea sus invocaciones; todos ven sus proyectiles.
    fn suceso_recibido(&mut self, de: u64, s: reglas::Suceso) {
        let anfitrion = crate::coop::soy_anfitrion();
        match s {
            reglas::Suceso::Danio { ent, n } if anfitrion => {
                if let Some(h) = crate::chr_hp(ent).filter(|&h| h > 0) {
                    // No es un golpe del anfitrión (combate.rs).
                    crate::coop::eco(ent, n);
                    crate::set_chr_hp(ent, (h - n).max(0));
                }
            }
            reglas::Suceso::Bala { id, objetivo, desde } => {
                let _ = crate::spawn_bullet_at(id, objetivo, Some(desde));
            }
            reglas::Suceso::Invocar { chr, npc, think, posiciones, tandas, sp, centro, nombre } => {
                self.invocar_para(de, (chr, npc, think), &posiciones, tandas, sp, centro, nombre);
            }
            reglas::Suceso::Aliado { chr, npc, think, n, segundos, centro } => {
                let hasta = Instant::now() + Duration::from_secs_f64(segundos);
                for k in 0..n {
                    let ent = self.siguiente_aliado;
                    self.siguiente_aliado = if ent + 1 >= ALIADO_AJENO_BASE + 1000 { ALIADO_AJENO_BASE } else { ent + 1 };
                    let (chr, npc, think) = super::grupo::sin_companero((chr, npc, think));
                    crate::spawn_chr_junto(chr, npc, think, ent, Some(centro), 0.0, 2.0 + k as f32);
                    self.aliados_ajenos.push((ent, hasta, false));
                }
                crate::log(&format!("aliado de {de}: c{chr:04} x{n}, {segundos} s"));
            }
            reglas::Suceso::Rival { id } => self.rival_recibido(&id),
            reglas::Suceso::Levantar { a } => self.levantado_por(de, a),
            _ => {}
        }
    }

    /// Invitado: cuando el anfitrión empieza una run, se entra en ella con su semilla si se está en el
    /// menú o eligiendo personaje con otra (17-10: cada uno acabó en una run distinta).
    fn seguir_anfitrion(&mut self) {
        if crate::coop::soy_anfitrion() || self.sala.is_some() {
            return;
        }
        let Some(semilla) = crate::coop::semilla_anfitrion() else { return };
        if self.run.as_ref().is_some_and(|r| r.coop && r.seed == semilla) {
            return;
        }
        match self.phase {
            // Desde cualquier menú (23-10: mirando los personajes no le llevaba a la run).
            Phase::MainMenu | Phase::Personajes { .. } | Phase::Over { .. } => {
                crate::log(&format!("cooperativo: el anfitrión ha empezado la run {semilla}; se entra en ella"));
                self.menu_coop = false;
                self.clave = None;
                self.show_compendio = false;
                self.pausa = None;
                let msg = self.nueva_run();
                crate::log(&msg);
            }
            Phase::CharSelect => {
                crate::log(&format!("cooperativo: la semilla pasa a la del anfitrión ({semilla})"));
                let mut run = RunState::nueva(&semilla, &self.acts_coop, vec![]);
                run.coop = true;
                run.personal(crate::coop::yo());
                self.reiniciar_coop();
                self.run = Some(run);
                self.save();
            }
            _ => {}
        }
    }

    /// Invitado: la ascensión es la del anfitrión, también si la cambia mientras se elige personaje. Si
    /// ya se había elegido, lo que va al empezar se pone ahora: los viales y la maldición.
    fn ascension_anfitrion(&mut self) {
        if crate::coop::soy_anfitrion() {
            return;
        }
        let Some((semilla, asc)) = crate::coop::ascension_anfitrion() else { return };
        let Some(run) = self.run.as_mut().filter(|r| r.coop && r.seed == semilla && r.ascension != asc) else { return };
        let antes = run.ascension;
        run.ascension = asc;
        crate::log(&format!("cooperativo: ascensión {asc}, la del anfitrión (antes {antes})"));
        if run.personaje.is_some() {
            self.apply_flasks();
            if antes < 6 && asc >= 6 && let Some(m) = self.maldicion_ascension() {
                crate::log(&m);
            }
        }
        self.save();
    }

    /// Selector de la contraseña: cruceta izquierda y derecha, qué letra; arriba y abajo, cuál; A la
    /// guarda en el INI de Seamless y B vuelve sin cambiarla.
    pub(super) fn nav_clave(&mut self, pressed: u16) {
        let Some((codigo, cur)) = self.clave.as_mut() else { return };
        let n = CARACTERES_CLAVE.len();
        let antes = (codigo.clone(), *cur);
        if pressed & 0x0004 != 0 {
            *cur = (*cur + codigo.len() - 1) % codigo.len();
        }
        if pressed & 0x0008 != 0 {
            *cur = (*cur + 1) % codigo.len();
        }
        if pressed & 0x0001 != 0 {
            codigo[*cur] = (codigo[*cur] + 1) % n;
        }
        if pressed & 0x0002 != 0 {
            codigo[*cur] = (codigo[*cur] + n - 1) % n;
        }
        if (codigo.clone(), *cur) != antes {
            play_se(Se::Cursor);
        }
        if pressed & 0x1000 != 0 {
            let texto: String = codigo.iter().map(|&i| CARACTERES_CLAVE[i] as char).collect();
            let ok = crate::poner_clave_seamless(&texto);
            crate::log(&format!("cooperativo: contraseña guardada ({})", if ok { "ok" } else { "error" }));
            self.aviso_coop = Some(if ok { t("Password saved. It is used the next time you open the game.") } else { t("Could not save the password.") }.into());
            self.clave = None;
            play_se(Se::Confirm);
        } else if pressed & 0x2000 != 0 {
            self.clave = None;
            play_se(Se::Back);
        }
    }

    /// Lo que enseña el selector de la contraseña: las letras, la elegida entre corchetes.
    pub(super) fn vista_clave(&self) -> Option<String> {
        let (codigo, cur) = self.clave.as_ref()?;
        let letras: Vec<String> =
            codigo.iter().enumerate().map(|(k, &i)| if k == *cur { format!("[{}]", CARACTERES_CLAVE[i] as char) } else { format!(" {} ", CARACTERES_CLAVE[i] as char) }).collect();
        Some(format!(
            "{}\n\n{}\n{}",
            letras.join(" "),
            t("D-pad left and right: choose a letter. Up and down: change it."),
            t("A: save. B: back. Everyone in the session needs the same password.")
        ))
    }

    /// Run nueva o de cooperativo: votos de la run anterior fuera (la semilla de la sesión se repite).
    pub(super) fn reiniciar_coop(&mut self) {
        self.voto = None;
        self.decidido = None;
        self.voto_evento = None;
        self.decidido_evento = None;
        self.listo = None;
        self.invocaciones_ajenas.clear();
        self.viaje_en = None;
        // El resultado del último combate es de la run anterior: con el mismo acto y nodo, al invitado le
        // daba por ganado el primer combate de la nueva sin entrar (23-10).
        self.ultimo_coop = None;
    }

    /// Compañeros de la sala que aún no han elegido personaje en esta run (en un menú, en otra run o
    /// eligiendo): hasta que elijan todos nadie sigue (23-10: uno se iba a la bendición inicial
    /// o al mapa y el otro se quedaba sin poder elegir).
    pub(super) fn faltan_en_seleccion(&self) -> Vec<String> {
        if !crate::coop::hay_companeros() {
            return vec![];
        }
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        crate::coop::companeros()
            .into_iter()
            .filter(|c| c.conectado)
            .filter(|c| !c.estado.as_ref().is_some_and(|e| e.version != reglas::VERSION || (e.semilla == semilla && e.fase != "personaje" && e.fase != "menu")))
            .map(|c| c.jugador.nombre)
            .collect()
    }

    /// Anfitrión, al empezar un combate: los rivales siguen quietos e intocables hasta que todos han
    /// terminado de cargar y están en la arena (24-10: el anfitrión los veía parados y les pegaba
    /// mientras al invitado aún le cargaba). Si alguien no llega en ESPERA_LLEGADA_MAX, se empieza.
    pub(super) fn esperando_llegada(&mut self) -> bool {
        if !crate::coop::soy_anfitrion() || !self.coop_activo() {
            return false;
        }
        let Some(paso) = self.paso() else { return false };
        let semilla = self.run.as_ref().map(|r| r.seed.clone());
        let faltan: Vec<String> = crate::coop::companeros()
            .into_iter()
            .filter(|c| c.conectado)
            .filter(|c| c.estado.as_ref().is_some_and(|e| e.version == reglas::VERSION && e.semilla == semilla))
            .filter(|c| !c.estado.as_ref().and_then(|e| e.combate).is_some_and(|k| k.paso == paso && k.llegado))
            .map(|c| c.jugador.nombre)
            .collect();
        if faltan.is_empty() {
            if self.llegada_avisada {
                self.llegada_avisada = false;
                crate::log("cooperativo: han llegado todos; empieza el combate");
            }
            return false;
        }
        if self.visible_desde.is_some_and(|t| t.elapsed() > ESPERA_LLEGADA_MAX) {
            crate::log(&format!("cooperativo: {} no ha llegado a tiempo; se empieza sin esperar", faltan.join(", ")));
            self.llegada_avisada = false;
            return false;
        }
        if !self.llegada_avisada {
            self.llegada_avisada = true;
            crate::log(&format!("cooperativo: se espera a que llegue {}", faltan.join(", ")));
            super::aviso(t("Waiting"), &tf("{} is still loading. The fight starts when everyone is here.", &[&faltan.join(", ")]));
        }
        true
    }

    /// Selección de personaje en cooperativo: con el mío elegido, se sigue cuando han elegido todos.
    fn elegir_si_todos(&mut self) {
        let Some(i) = self.elegido.filter(|_| matches!(self.phase, Phase::CharSelect)) else { return };
        if !self.faltan_en_seleccion().is_empty() {
            return;
        }
        self.elegido = None;
        if let Some(c) = self.chars.get(i).cloned() {
            let msg = self.choose_character(&c);
            crate::log(&format!("cooperativo: han elegido todos; {msg}"));
        }
    }

    /// La sala, en los menús hasta que empieza la run (23-10): quién está y qué hace.
    pub(super) fn vista_sala(&self) -> Vec<String> {
        let en_menus = matches!(self.phase, Phase::MainMenu | Phase::Personajes { .. } | Phase::CharSelect | Phase::Neow { .. } | Phase::Over { .. });
        if !en_menus || crate::sesion().is_none() {
            return vec![];
        }
        let mut v = vec![if crate::coop::soy_anfitrion() { t("Session (you are the host)").to_string() } else { t("Session (you are a guest)").to_string() }];
        for c in crate::coop::companeros() {
            let que = match c.estado.as_ref().filter(|_| c.conectado) {
                None => t("connecting").to_string(),
                Some(e) if e.version != reglas::VERSION => t("other version of the mod").to_string(),
                Some(e) => haciendo(&e.fase).to_string(),
            };
            v.push(format!("{}: {que}", c.jugador.nombre));
        }
        if let Some(a) = self.run.as_ref().filter(|r| r.coop && r.ascension > 0 && matches!(self.phase, Phase::CharSelect | Phase::Neow { .. })).map(|r| r.ascension) {
            v.push(tf("Ascension {}", &[&a]));
        }
        v
    }

    /// Menú del cooperativo: (texto, detalle, apagada, acción). Con compañeros en la sala, la run se
    /// empieza desde aquí.
    pub(super) fn coop_opciones(&self) -> Vec<(String, String, bool, MenuCoop)> {
        let en_sala = crate::coop::hay_companeros() || crate::sesion().is_some();
        let mut v = vec![];
        // La run la empieza el anfitrión; los invitados entran solos en ella (`seguir_anfitrion`).
        if crate::coop::hay_companeros() && !crate::coop::invitado_en_sala() {
            v.push((t("New co-op run").into(), t("A new seed, the same for everyone. Each one picks a character.").into(), false, MenuCoop::Nueva));
        }
        v.extend([
            (t("Create session").into(), t("Opens your world. Your friends join with the same password.").into(), en_sala, MenuCoop::Crear),
            (t("Join a session").into(), t("Looks for your friend's session with the same password.").into(), en_sala, MenuCoop::Unirse),
            (t("Leave the session").into(), t("Each one goes back to their own world.").into(), !en_sala, MenuCoop::Salir),
            (t("Change password").into(), t("Choose it here. Everyone in the session needs the same one.").into(), false, MenuCoop::Clave),
        ]);
        if self.aviso_coop.is_some() {
            v.push((t("Do it by hand").into(), t("The game shows up: use the item from the inventory (START). SELECT comes back here.").into(), false, MenuCoop::AMano));
        }
        v.extend([
            (t("Back").into(), String::new(), false, MenuCoop::Volver),
        ]);
        v
    }

    /// Título y texto del menú del cooperativo: con quién estás.
    pub(super) fn estado_sala(&self) -> (String, String) {
        if let Some(s) = &self.sala {
            let texto = match s.objeto {
                OBJ_VASIJA => t("Opening the session..."),
                OBJ_EFIGIE => t("Looking for your friend's session..."),
                _ => t("Leaving the session..."),
            };
            return (t("Session").into(), texto.into());
        }
        if let Some(a) = &self.aviso_coop {
            return (t("Session").into(), a.clone());
        }
        let otros: Vec<String> = crate::coop::companeros().into_iter().map(|c| c.jugador.nombre).collect();
        match (crate::sesion(), otros.is_empty()) {
            (Some((_, anfitrion, _)), false) => {
                let rol = if anfitrion { t("You are the host.") } else { t("You are a guest.") };
                (tf("Session with {}", &[&otros.join(", ")]), format!("{rol} {}", t("New runs share the seed; on the map everyone votes.")))
            }
            (Some(_), true) => (t("Session open").into(), t("Waiting for your friends: they choose Join a session with the same password.").into()),
            (None, _) => (t("No session").into(), t("Create one or join a friend's. Everyone needs the same password.").into()),
        }
    }

    pub(super) fn elegir_coop(&mut self, op: MenuCoop) -> String {
        self.aviso_coop = None;
        match op {
            MenuCoop::Nueva => {
                self.menu_coop = false;
                self.nueva_run()
            }
            MenuCoop::Crear => self.usar_objeto_seamless(OBJ_VASIJA),
            MenuCoop::Unirse => self.usar_objeto_seamless(OBJ_EFIGIE),
            MenuCoop::Salir => self.usar_objeto_seamless(OBJ_NIEBLA),
            MenuCoop::Clave => {
                // Se empieza con la que hay si se puede escribir con estas letras; si no, una al azar.
                let actual: Option<Vec<usize>> = crate::clave_seamless()
                    .filter(|c| c.len() == LARGO_CLAVE)
                    .and_then(|c| c.bytes().map(|b| CARACTERES_CLAVE.iter().position(|&x| x == b.to_ascii_uppercase())).collect());
                let codigo = actual.unwrap_or_else(|| {
                    let s = semilla_nueva();
                    s.bytes().map(|b| (b as usize * 7) % CARACTERES_CLAVE.len()).collect()
                });
                self.clave = Some((codigo, 0));
                "cooperativo: elegir contraseña".into()
            }
            MenuCoop::AMano => {
                self.libre = true;
                "cooperativo: manos libres".into()
            }
            MenuCoop::Volver => {
                self.menu_coop = false;
                self.sel = self.menu_opciones().iter().position(|o| o.3 == Menu::Coop).unwrap_or(0);
                "menú principal".into()
            }
        }
    }

    /// Usa un objeto de Seamless por el jugador (si no lo tiene, se le da) sin salir del menú: `sala_tick`
    /// lo pone en la ranura rápida, pulsa usar y acepta su diálogo con pulsaciones forzadas, que llegan
    /// al juego por detrás de la pantalla del mod (el mando del jugador no). Ni el personaje ni el
    /// diálogo de Seamless se ven (16-10: todo en menús).
    fn usar_objeto_seamless(&mut self, objeto: u32) -> String {
        if !crate::seamless_cargado() {
            return "sin Seamless".into();
        }
        let id = 0x4000_0000 | objeto;
        if crate::equip::find_item(id).is_none() {
            let r = crate::equip::give(id, 1, -1);
            crate::log(&format!("cooperativo: objeto de Seamless {objeto} dado ({r:?})"));
        }
        self.sala = Some(Sala { objeto, paso: 0, desde: Instant::now(), antes: None, intento: 1, ultima_a: None, anim_x: None, anim_visto: false });
        format!("cooperativo: usando el objeto {objeto}")
    }

    /// Pasos del uso: ranura y botón de usar (0,8 s); A cada 1,2 s entre 2,5 y 6 s para aceptar su
    /// diálogo (si no hay diálogo, el personaje salta, pero no se ve); y esperar a que la sesión cambie
    /// (8 s; unirse, 30 s, porque busca la sala y carga el mundo del anfitrión). Si no cambia, otra vez,
    /// salvo al unirse (repetirlo cortaría la búsqueda).
    pub(super) fn sala_tick(&mut self) {
        let Some(s) = self.sala.as_mut() else { return };
        let pasado = s.desde.elapsed();
        let sesion = crate::sesion();
        let hecho = match s.objeto {
            OBJ_VASIJA => sesion.is_some(),
            OBJ_NIEBLA => sesion.is_none(),
            _ => sesion.is_some_and(|x| !x.1),
        };
        let limite = Duration::from_secs(if s.objeto == OBJ_EFIGIE { 30 } else { 8 });
        if let Some((t, antes)) = s.anim_x
            && !s.anim_visto
            && t.elapsed() >= Duration::from_millis(700)
        {
            s.anim_visto = true;
            crate::log(&format!("cooperativo: animación al pulsar X {antes:?}, a los 0,7 s {:?}", crate::player_anim()));
        }
        match s.paso {
            0 if pasado >= Duration::from_millis(800) => {
                match crate::equip::poner_rapido(0, 0x4000_0000 | s.objeto) {
                    Ok(antes) => {
                        s.antes = Some(antes);
                        crate::log(&format!("cooperativo: ranura rápida antes {antes:?}, ahora {:?}", crate::equip::estado_rapido(0)));
                        crate::pad::press_forzado(crate::pad::button("X").unwrap_or(0), 200);
                        s.anim_x = Some((Instant::now(), crate::player_anim()));
                        s.anim_visto = false;
                    }
                    Err(e) => crate::log(&format!("cooperativo: no se pudo poner el objeto: {e}")),
                }
                s.paso = 1;
            }
            1 if hecho || pasado >= limite => {
                if let Some(antes) = s.antes.take() {
                    crate::equip::restaurar_rapido(0, antes);
                }
                if !hecho && s.objeto != OBJ_EFIGIE && s.intento < INTENTOS_SALA {
                    crate::log(&format!("cooperativo: el objeto {} no hizo nada (intento {}); otra vez", s.objeto, s.intento));
                    s.intento += 1;
                    s.paso = 0;
                    s.desde = Instant::now();
                    s.ultima_a = None;
                    return;
                }
                crate::log(&format!("cooperativo: objeto {} {}; sesión {sesion:?}", s.objeto, if hecho { "usado" } else { "sin efecto" }));
                if !hecho {
                    self.aviso_coop = Some(
                        match s.objeto {
                            OBJ_VASIJA => t("Could not open the session. Try again."),
                            OBJ_EFIGIE => t("No session found. Check that your friend has opened it and that you both use the same password."),
                            _ => t("Could not leave the session. Try again."),
                        }
                        .into(),
                    );
                }
                self.sala = None;
                self.menu_coop = true;
                self.sel = self.coop_opciones().iter().position(|o| !o.2).unwrap_or(0);
            }
            1 if pasado >= Duration::from_millis(2500)
                && pasado < Duration::from_secs(6)
                && s.ultima_a.is_none_or(|a| a.elapsed() >= Duration::from_millis(1200)) =>
            {
                crate::pad::press_forzado(crate::pad::button("A").unwrap_or(0), 150);
                s.ultima_a = Some(Instant::now());
            }
            _ => {}
        }
    }
}

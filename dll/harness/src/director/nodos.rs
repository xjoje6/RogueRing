//! Mapa y nodos: menú principal, pausa, viajes, eventos, tienda, gracia, tesoro y recompensas.

use super::*;
use rogue_core::idioma::{t, tf};

impl Director {
    /// Nodos disponibles ordenados de izquierda a derecha (orden de la cruceta).
    pub(super) fn available_sorted(&self) -> Vec<(i32, i32)> {
        let mut v = self.run.as_ref().map(|r| r.available()).unwrap_or_default();
        v.sort_by_key(|p| p.0);
        v
    }

    /// Opciones del menú principal: (texto, detalle, apagada, acción).
    pub(super) fn menu_opciones(&self) -> Vec<(String, String, bool, Menu)> {
        let mut v = vec![];
        // Invitado en una sala: las runs las lleva el anfitrión (17-10). Se entra solo en la que
        // empiece (`seguir_anfitrion`) y solo se puede retomar la suya.
        let invitado = crate::coop::invitado_en_sala();
        if let Some(run) = &self.guardada {
            let pj = run.personaje.as_deref().and_then(|id| self.chars.iter().find(|c| c.id == id)).map(|c| c.nombre.clone()).unwrap_or_default();
            let ajena = invitado && crate::coop::semilla_del_anfitrion().as_deref() != Some(run.seed.as_str());
            let detalle = if ajena { t("In a session you can only continue the host's run.").into() } else { tf("{}, act {}. Seed {}.", &[&pj, &(run.act_index + 1), &run.seed]) };
            v.push((t("Continue").into(), detalle, ajena, Menu::Continuar));
        }
        if invitado {
            v.push((t("New run").into(), t("The host starts the run; you join it on your own.").into(), true, Menu::Nueva));
        } else {
            let nueva = if self.guardada.is_some() { t("Your current run will be lost.") } else { t("Choose who fights.") };
            v.push((t("New run").into(), nueva.into(), false, Menu::Nueva));
        }
        v.push((t("Characters").into(), t("Outfits and starting weapons.").into(), false, Menu::Personajes));
        let coop = match (crate::seamless_cargado(), crate::sesion()) {
            (false, _) => (t("Start the game with Play RogueRing (Seamless).").to_string(), true),
            (true, Some((n, ..))) if n >= 2 => (tf("Session of {} players. The host starts the runs.", &[&n]), false),
            (true, _) => (t("Create a session or join a friend's.").to_string(), false),
        };
        v.push((t("Co-op").into(), coop.0, coop.1, Menu::Coop));
        v.push((t("Compendium").into(), t("Every blessing you have found.").into(), false, Menu::Compendio));
        v.push((t("Quit game").into(), String::new(), false, Menu::Salir));
        v
    }

    /// Opciones de la pausa: (texto, detalle, apagada, acción).
    pub(super) fn pausa_opciones(&self) -> Vec<(String, String, bool, Pausa)> {
        let peleando = matches!(self.phase, Phase::Arena { .. } | Phase::Combat { .. } | Phase::Won { .. } | Phase::Returning { .. });
        let menu = if peleando { t("Finish the fight first.") } else { t("Your run is saved.") };
        vec![
            (t("Resume").into(), if peleando && self.coop_activo() { t("Online, the fight goes on.").into() } else { String::new() }, false, Pausa::Seguir),
            (
                self.run.as_ref().map(|r| tf("Ascension {}", &[&r.ascension])).unwrap_or_default(),
                t("What it changes in this run, and its mutators.").into(),
                false,
                Pausa::Ascension,
            ),
            // Versión de pruebas: rivales y recompensas a elegir, desde el mapa.
            (t("Tests").into(), t("Fight whoever you want and take any reward.").into(), !matches!(self.phase, Phase::Map), Pausa::Pruebas),
            (t("Main menu").into(), menu.into(), peleando, Pausa::Menu),
            (t("Abandon run").into(), t("The run ends here.").into(), false, Pausa::Abandonar),
            (t("Quit game").into(), t("Your run is saved.").into(), false, Pausa::Salir),
        ]
        .into_iter()
        .filter(|o| !matches!(o.3, Pausa::Pruebas) || self.modo_pruebas())
        .filter(|o| !matches!(o.3, Pausa::Ascension) || self.run.as_ref().is_some_and(|r| r.ascension > 0))
        .collect()
    }

    pub(super) fn cerrar_pausa(&mut self) {
        self.pausa = None;
        if let Phase::Arena { ent, .. } = self.phase {
            crate::set_anim_speed(ent, 1.0);
            for &e in &self.grupo {
                crate::set_anim_speed(e, 1.0);
            }
            crate::set_player_invincible(false);
        }
    }

    pub(super) fn confirmar_pausa(&mut self, p: Pausa) -> String {
        match p {
            Pausa::Seguir => {
                self.cerrar_pausa();
                "se sigue".into()
            }
            Pausa::Ascension => {
                self.ver_ascension = true;
                "ascensión".into()
            }
            Pausa::Pruebas => {
                self.cerrar_pausa();
                self.abrir_pruebas();
                "menú de pruebas".into()
            }
            Pausa::Menu => {
                self.cerrar_pausa();
                self.save();
                self.guardada = self.run.clone().filter(|r| r.personaje.is_some() && !r.terminada);
                self.phase = Phase::MainMenu;
                self.sel = 0;
                "menú principal".into()
            }
            Pausa::Abandonar => {
                self.cerrar_pausa();
                self.end_fx();
                self.fx = None;
                // Si el boss sigue ahí, que no te mate mientras se ve la pantalla de fin.
                crate::set_player_invincible(true);
                self.phase = Phase::Over { won: false, new_run: false, shown: None };
                self.sel = 0;
                self.save();
                "run abandonada".into()
            }
            Pausa::Salir => {
                self.save();
                crate::salir_del_juego();
                "saliendo del juego".into()
            }
        }
    }

    /// Las tres opciones de la bendición inicial.
    pub(super) fn neow_offer(&mut self) -> Phase {
        let Some(run) = self.run.as_mut() else { return Phase::Map };
        let pj = run.personaje.clone();
        let common = self.relics.roll(&run.relics, Rarity::Comun, &mut run.rng_rewards, pj.as_deref());
        let mut owned = run.relics.clone();
        if let Some(c) = &common {
            owned.push(OwnedRelic { id: c.id.clone(), nivel: 0 });
        }
        let rare = self.relics.roll(&owned, Rarity::Rara, &mut run.rng_rewards, pj.as_deref());
        let curses: Vec<&RelicDef> = self.relics.reliquias.iter().filter(|r| r.maldita).collect();
        let curse = if curses.is_empty() { None } else { Some(curses[run.rng_rewards.range(0, curses.len() as i32) as usize].clone()) };
        Phase::Neow { common, rare, curse }
    }

    /// Antes de viajar al coliseo: la variante de su MSB con el jefe de este encuentro ya colocado
    /// (un coliseo por combate, docs/COOP_SALAS.md) y su flag apagado, para que espere apagado. Sin
    /// variante, o con `runtime/jefe_spawn`, vuelve el coliseo normal y el jefe se crea con
    /// `spawn_chr` como antes.
    pub(super) fn preparar_coliseo(&mut self, enc: &Encounter) {
        // Todas las oleadas apagadas (las de un combate anterior se quedaban encendidas).
        for w in 0..OLEADAS_MSB {
            flag_set(JEFE_MSB_FLAG + w, false);
        }
        let dir = self.root.join("mod").join("map");
        // Arena grande: su variante sobre el mapa de la arena, y se viaja allí.
        self.area_combate = COLOSSEUM_AREA;
        self.flag_combate = COLOSSEUM_WARP_FLAG;
        if let Some(a) = enc.arena.as_ref().and_then(|k| self.arenas.get(k)) {
            let clave = enc.arena.clone().unwrap_or_default();
            let variante = dir.join("rogue_arenas").join(&clave).join(format!("{}.msb.dcx", enc.id));
            let copiado = std::fs::copy(&variante, dir.join("mapstudio").join(format!("{}.msb.dcx", a.mapa)));
            if let Err(e) = &copiado {
                crate::log(&format!("arena {clave}: no se pudo copiar {}: {e}", variante.display()));
            } else {
                self.area_combate = a.area;
                self.flag_combate = a.flag;
                self.jefe_msb = true;
                self.origenes.clear();
                crate::log(&format!("arena grande {clave} ({}): jefe colocado en el MSB", a.mapa));
                return;
            }
        }
        let variante = dir.join("rogue_coliseo").join(format!("{}.msb.dcx", enc.id));
        let usar = !self.root.join("runtime").join("jefe_spawn").exists() && variante.exists();
        let origen = if usar { variante } else { dir.join("rogue_coliseo").join("base.msb.dcx") };
        let copiado = std::fs::copy(&origen, dir.join("mapstudio").join("m45_00_00_00.msb.dcx"));
        if let Err(e) = &copiado {
            crate::log(&format!("coliseo: no se pudo copiar {}: {e}", origen.display()));
        }
        self.jefe_msb = usar && copiado.is_ok();
        self.origenes.clear();
        crate::log(&format!("coliseo: jefe {}", if self.jefe_msb { "colocado en el MSB" } else { "con spawn_chr" }));
    }

    /// Viaja al combate de este encuentro (coliseo o arena original).
    pub(super) fn start_combat(&mut self, enc: Encounter) -> String {
        // Muerto todavía: con vida 0 no se da nunca por llegado a la arena (19-10: se quedó en la
        // carga para siempre).
        self.revivir(2);
        // La oleada es la de este combate desde ya: el anfitrión la publica en cuanto empieza y, con la
        // del anterior, el invitado encendía una oleada que no existe y apagaba a los rivales (23-10:
        // el Bastardo leonino quieto en su juego).
        self.oleada = 0;
        self.oleada_lista = false;
        if enc.en_coliseo() {
            self.viaje_repetido = false;
            self.forzar_victoria = false;
            self.preparar_coliseo(&enc);
            self.viajar(self.flag_combate);
            let msg = format!("combate en el coliseo: {} ({:?})", enc.name, enc.kind);
            self.phase = Phase::Arena { enc, since: Instant::now(), llegada: None, ent: 0, spawned_at: None, ready: false };
            msg
        } else {
            flag_set(enc.defeat_flag, false);
            flag_set(enc.warp_flag, true);
            let msg = format!("combate: {} ({:?})", enc.name, enc.kind);
            self.phase = Phase::Combat { enc, since: Instant::now(), healed: false, tries: 0, last_try: None };
            msg
        }
    }

    /// Tras usar una opción de la gracia: se sigue en ella si quedan elecciones (Generous Grace da dos)
    /// o se termina el nodo.
    pub(super) fn tras_gracia(&mut self) {
        let generosa = self.run.as_ref().is_some_and(|r| self.relics.has(&r.relics, "descanso_templar"));
        let usadas = self.rest_used.iter().filter(|&&u| u).count();
        if usadas < if generosa { 2 } else { 1 } {
            self.phase = Phase::Rest;
            self.sel = self.rest_used.iter().position(|&u| !u).unwrap_or(0);
        } else {
            self.rest_used = [false; 3];
            self.finish_node();
        }
    }

    /// Una opción de evento se puede elegir si alcanzan las runas y, si templa, hay qué templar.
    pub(super) fn option_enabled(&self, o: &rogue_core::events::EventOption) -> bool {
        crate::runes().unwrap_or(0) >= o.cost() && (!o.needs_temper() || !self.temperable().is_empty())
    }

    /// Aplica efectos de evento. Un objeto pasa por `acquire` (puede pedir ranura); luego `back`.
    pub(super) fn apply_effects(&mut self, effects: Vec<Effect>, back: Phase) -> String {
        let mut log = vec![];
        let mut item: Option<Item> = None;
        for e in effects {
            match e {
                Effect::Runas(n) if n >= 0 => log.push(tf("You gain {} runes.", &[&self.gain_runes(n as u32)])),
                Effect::Runas(n) => {
                    let have = crate::runes().unwrap_or(0);
                    crate::set_runes(have.saturating_sub((-n) as u32));
                    log.push(tf("You lose {} runes.", &[&-n]));
                }
                Effect::Reliquia(rar) => {
                    let owned = self.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
                    let got = self.run.as_mut().and_then(|r| self.relics.roll(&owned, rar, &mut r.rng_rewards, r.personaje.as_deref()));
                    if let Some(r) = got {
                        log.push(format!("{}: {}.", if r.rareza == Rarity::Mitica { t("Mythic blessing") } else { t("Blessing") }, r.nombre));
                        self.gain_relic(&r);
                    }
                }
                Effect::Maldicion(id) | Effect::ReliquiaFija(id) => {
                    let owned = self.run.as_ref().is_some_and(|r| r.relics.iter().any(|o| o.id == id));
                    if let (false, Some(r)) = (owned, self.relics.get(&id).cloned()) {
                        log.push(format!("{}: {}.", if r.maldita { t("Curse") } else { t("Blessing") }, r.nombre));
                        self.gain_relic(&r);
                    }
                }
                Effect::Objeto(rar) => {
                    // Como en las recompensas: sin lo que ya llevas, y las armas de tu estilo pesan más.
                    let fuera = self.no_ofrecer();
                    let pool: Vec<Item> = self.offer_pool().into_iter().filter(|it| it.rareza == rar && !fuera.contains(&(it.categoria, it.id))).collect();
                    if let (Some(run), false) = (self.run.as_mut(), pool.is_empty()) {
                        let total: f64 = pool.iter().map(|it| it.peso.max(0.0001)).sum();
                        let mut r = run.rng_rewards.next_f64() * total;
                        let k = pool.iter().position(|it| {
                            r -= it.peso.max(0.0001);
                            r < 0.0
                        });
                        item = Some(pool[k.unwrap_or(pool.len() - 1)].clone());
                    }
                }
                Effect::Templar => {
                    let t = self.temperable();
                    if let (Some(run), false) = (self.run.as_mut(), t.is_empty()) {
                        let o = t[run.rng_rewards.range(0, t.len() as i32) as usize].clone();
                        self.temper(&o.id);
                        let nombre = self.relics.get(&o.id).map(|r| r.nombre.clone()).unwrap_or(o.id.clone());
                        log.push(tf("Tempered: {}.", &[&nombre]));
                    }
                }
                Effect::Combate(_) => {}
                Effect::Mutador(nivel) => {
                    let pick = self.run.as_mut().and_then(|run| {
                        let cands: Vec<&crate::motor::Mutador> = self.mutadores.iter().filter(|m| m.nivel == nivel && !run.mutadores.contains(&m.id)).collect();
                        // En cooperativo, del sorteo común: los rivales son de todos.
                        let rng = if run.coop { run.comun() } else { &mut run.rng_rewards };
                        (!cands.is_empty()).then(|| cands[rng.range(0, cands.len() as i32) as usize].clone())
                    });
                    if let (Some(m), Some(run)) = (pick, self.run.as_mut()) {
                        log.push(tf("Enemies: {}. {}", &[&m.nombre, &m.texto]));
                        run.mutadores.push(m.id.clone());
                    }
                }
            }
        }
        match item {
            Some(it) => {
                let nombre = it.nombre.clone();
                let _ = self.acquire(it, back);
                log.push(tf("You get {}.", &[&nombre]));
            }
            None => self.phase = back,
        }
        log.join(" ")
    }

    pub(super) fn finish_node(&mut self) {
        let acts = if self.run.as_ref().is_some_and(|r| r.coop) { &self.acts_coop } else { &self.acts };
        if let Some(run) = self.run.as_mut() {
            let _ = run.complete(acts);
        }
        self.phase = Phase::Map;
        self.sel = 0;
        self.save();
    }

    #[cfg(feature = "banco")]
    pub(super) fn status(&self) -> String {
        let Some(r) = &self.run else { return serde_json::json!({ "run": null }).to_string() };
        let avail: Vec<_> = self
            .available_sorted()
            .into_iter()
            .map(|(c, row)| {
                let kind =
                    if (c, row) == r.act.map.boss { "Boss".to_string() } else { r.act.map.node(c, row).map(|n| format!("{:?}", n.kind)).unwrap_or_default() };
                serde_json::json!([c, row, kind])
            })
            .collect();
        let fase = match &self.phase {
            Phase::Reward { enemy, cartas, .. } => {
                let nombres: Vec<&str> = cartas
                    .iter()
                    .map(|c| match c {
                        Carta::Objeto(i) => i.nombre.as_str(),
                        Carta::Bendicion(r) => r.nombre.as_str(),
                    })
                    .collect();
                format!("Reward({enemy}: {})", nombres.join(" | "))
            }
            p => format!("{p:?}").chars().take(90).collect(),
        };
        serde_json::json!({
            "semilla": r.seed, "acto": r.act_index + 1, "posicion": r.position, "fase": fase, "pausa": self.pausa.is_some(),
            "seleccion": self.sel, "disponibles": avail, "boss_del_acto": r.act.boss.name,
            "objetos": r.items.len(), "reliquias": r.relics.iter().map(|o| format!("{}:{}", o.id, o.nivel + 1)).collect::<Vec<_>>(),
            "runas": crate::runes(),
            // En combate: cómo está cada bendición (lo que dice la pausa).
            "tira": self.fx.as_ref().and_then(|f| f.motor.as_ref()).map(|m| m.hud().into_iter().map(|h| format!("{}: {}", h.src, h.estado)).collect::<Vec<_>>()),
        })
        .to_string()
    }

    /// Viajar al nodo elegido.
    /// Resuelve la opción `k` del evento en curso (en cooperativo, la que salió de los votos).
    pub(super) fn resolver_evento(&mut self, k: usize) -> String {
        let Phase::Event { ev, result: None } = &self.phase else { return "sin evento".into() };
        let ev = ev.clone();
        let Some(o) = ev.opciones.get(k).cloned() else { return "sin opción".into() };
        let pagable = self.option_enabled(&o);
        let run = self.run.as_mut().unwrap();
        // El evento es común: su resultado sale igual a todos (y el sorteo común sigue a la par).
        let (mut effects, mut texto) = o.resolve(run.comun());
        if !pagable {
            // En cooperativo la opción la deciden los votos: quien no puede pagarla pelea igual, sin
            // coste ni premio.
            effects.retain(|e| matches!(e, Effect::Combate(_)));
            texto.push_str(&format!("\n\n{}", t("You could not pay for it: nothing happens to you.")));
        }
        // Con pelea: se paga lo que cuesta ya y el resto se cobra al ganar.
        let fight = effects.iter().find_map(|e| if let Effect::Combate(k) = e { Some(*k) } else { None });
        let enc = fight.and_then(|k| run.next_encounter(k));
        let (now, later): (Vec<Effect>, Vec<Effect>) = if enc.is_some() {
            effects.into_iter().filter(|e| !matches!(e, Effect::Combate(_))).partition(|e| matches!(e, Effect::Runas(n) if *n < 0))
        } else {
            (effects.into_iter().filter(|e| !matches!(e, Effect::Combate(_))).collect(), vec![])
        };
        self.pending = later;
        self.sel = 0;
        let back = Phase::Event { ev, result: Some((texto, enc)) };
        let msg = self.apply_effects(now, back);
        if !msg.is_empty() {
            let target = match &mut self.phase {
                Phase::Replace { back, .. } => back.as_mut(),
                p => p,
            };
            if let Phase::Event { result: Some((t, _)), .. } = target {
                t.push_str(&format!("\n\n{msg}"));
            }
        }
        self.save();
        msg
    }

    /// Run nueva desde el menú principal o el del cooperativo. En una sesión de Seamless con compañeros,
    /// todos con la misma semilla y las listas de dúos.
    pub(super) fn nueva_run(&mut self) -> String {
        // Invitado en una sala: solo la del anfitrión (`seguir_anfitrion` la abre con su semilla).
        if crate::coop::invitado_en_sala() && crate::coop::semilla_anfitrion().is_none() {
            return "las runs las empieza el anfitrión".into();
        }
        self.reiniciar_coop();
        self.elegido = None;
        // Cooperativo: la semilla la saca el anfitrión, nueva cada vez, y los demás entran con la suya
        // (16-10: salía siempre la de la sesión). Un invitado espera a que el anfitrión empiece.
        let semilla = if !crate::coop::hay_companeros() {
            None
        } else if crate::coop::soy_anfitrion() {
            Some(crate::coop::semilla_sesion().filter(|s| s == "COSOMBRA").unwrap_or_else(semilla_nueva))
        } else {
            match crate::coop::semilla_anfitrion() {
                Some(s) => Some(s),
                None => {
                    self.aviso_coop = Some(t("Wait until the host starts the run.").into());
                    self.menu_coop = true;
                    self.sel = 0;
                    return "esperando al anfitrión".into();
                }
            }
        };
        if let Some(semilla) = semilla {
            let mut run = RunState::nueva(&semilla, &self.acts_coop, vec![]);
            run.coop = true;
            run.personal(crate::coop::yo());
            self.run = Some(run);
            crate::log(&format!("cooperativo: run nueva con la semilla de la sesión {semilla} y las listas de dúos"));
        }
        self.guardada = None;
        self.limpiar_personaje();
        crate::set_runes(0);
        crate::clear_bloodstain();
        self.phase = Phase::CharSelect;
        self.sel = 0;
        self.save();
        "run nueva".into()
    }

    pub(super) fn travel(&mut self, to: (i32, i32)) -> String {
        let run = self.run.as_mut().unwrap();
        let tipo = run.act.map.nodes.iter().find(|n| (n.col, n.row) == to).map(|n| format!("{:?}", n.kind)).unwrap_or_default();
        run.historia.push(rogue_core::run::NodeLog { acto: run.act_index + 1, tipo, vida_min: 1.0, ..Default::default() });
        let action = match run.enter(to) {
            Ok(a) => a,
            Err(e) => return format!("error: {e}"),
        };
        self.enter_action(action)
    }

    /// Qué pasa al entrar en un nodo (también lo usa `run forzar`).
    pub(super) fn enter_action(&mut self, action: NodeAction) -> String {
        let act = self.act();
        let run = self.run.as_mut().unwrap();
        self.sel = 0;
        let msg = match action {
            NodeAction::Combat(enc) if enc.en_coliseo() => {
                run.en_curso = Some(enc.clone());
                // Modo coliseo: viajar a la arena; el boss se enciende (o se crea) al llegar.
                self.preparar_coliseo(&enc);
                self.viajar(self.flag_combate);
                let msg = format!("combate en el coliseo: {} ({:?})", enc.name, enc.kind);
                self.phase = Phase::Arena { enc, since: Instant::now(), llegada: None, ent: 0, spawned_at: None, ready: false };
                msg
            }
            NodeAction::Combat(enc) => {
                run.en_curso = Some(enc.clone());
                // El boss puede estar muerto de otra run: resucitarlo antes de ir.
                flag_set(enc.defeat_flag, false);
                flag_set(enc.warp_flag, true);
                let msg = format!("combate: {} ({:?})", enc.name, enc.kind);
                self.phase = Phase::Combat { enc, since: Instant::now(), healed: false, tries: 0, last_try: None };
                msg
            }
            NodeAction::Treasure => {
                // Rareza de la reliquia según el acto (en el 3 ya pueden salir raras).
                let rar = match (act, run.rng_rewards.range(0, 100)) {
                    (0, x) if x < 75 => Rarity::Comun,
                    (0, _) => Rarity::PocoComun,
                    (1, x) if x < 60 => Rarity::PocoComun,
                    (_, x) if x < 50 => Rarity::PocoComun,
                    _ => Rarity::Rara,
                };
                // Muy de vez en cuando, una mítica.
                let mitica = run.rng_rewards.range(0, 100) < MITICA_COFRE[act.min(1)];
                let relic = mitica
                    .then(|| self.relics.roll(&run.relics, Rarity::Mitica, &mut run.rng_rewards, run.personaje.as_deref()))
                    .flatten()
                    .or_else(|| self.relics.roll(&run.relics, rar, &mut run.rng_rewards, run.personaje.as_deref()));
                let runes = (300.0 * (1.0 + 0.6 * act as f64)) as u32;
                // Un cofre de cada cinco es una trampa: élite del acto y el botín, mejorado, al ganar.
                // La trampa es compartida (trae un combate de los dos).
                let trap = if run.comun().range(0, 100) < 20 { run.next_encounter(EncounterKind::Elite) } else { None };
                self.phase = Phase::Treasure { relic, runes, trap };
                "tesoro".into()
            }
            NodeAction::Shop => {
                let pool = self.offer_pool();
                let dobles = !self.character().is_some_and(|c| c.sin_dobles);
                let fuera = self.no_ofrecer();
                let run = self.run.as_mut().unwrap();
                let mut offer = rewards::generate(&pool, EncounterKind::Elite, act, &mut run.rng_rewards, &fuera);
                rewards::roll_doubles(&mut offer.choices, &mut run.rng_rewards, dobles);
                let fp = self.relics.dll_factor(&run.relics, "precio") * if run.ascension >= 7 { 1.5 } else { 1.0 };
                let mut wares: Vec<(Ware, u32, bool)> = offer
                    .choices
                    .into_iter()
                    .map(|it| {
                        let p = (price(it.rareza, act, false) as f64 * fp) as u32;
                        (Ware::Item(it), p, false)
                    })
                    .collect();
                if let Some(r) = self.relics.roll(&run.relics, Rarity::PocoComun, &mut run.rng_rewards, run.personaje.as_deref()) {
                    let p = (price(r.rareza, act, true) as f64 * fp) as u32;
                    wares.push((Ware::Relic(r), p, false));
                }
                self.phase = Phase::Shop { wares };
                "mercader".into()
            }
            NodeAction::Rest => {
                self.phase = Phase::Rest;
                "gracia".into()
            }
            NodeAction::Event => {
                let seen = run.events_seen.clone();
                match self.events.pick(run.act_index, &seen, run.comun()) {
                    Some(ev) => {
                        run.events_seen.push(ev.id.clone());
                        let msg = format!("evento: {}", ev.titulo);
                        self.phase = Phase::Event { ev, result: None };
                        msg
                    }
                    None => {
                        let _ = run.complete(if run.coop { &self.acts_coop } else { &self.acts });
                        "sin eventos: nodo completado".into()
                    }
                }
            }
        };
        self.save();
        msg
    }

    /// Número de huecos seleccionables en la pantalla actual (cartas + salir).
    /// Cofre trampa: su combate, con el botín (mejorado) para después.
    pub(super) fn pelear_cofre(&mut self, relic: Option<RelicDef>, runes: u32, enc: Encounter) -> String {
        self.pending = vec![Effect::Runas((runes * 3 / 2) as i32)];
        if let Some(r) = relic {
            self.pending.push(Effect::ReliquiaFija(r.id));
        }
        self.start_combat(enc)
    }

    /// ¿Se puede dejar este cofre? El trampa en cooperativo no: su combate es de todos.
    pub(super) fn cofre_se_deja(&self, trampa: bool) -> bool {
        !(trampa && self.coop_activo())
    }

    pub(super) fn slots(&self) -> usize {
        match &self.phase {
            Phase::Map => self.available_sorted().len(),
            Phase::Reward { cartas, .. } => cartas.len() + 1,
            Phase::Treasure { trap, .. } => 1 + usize::from(self.cofre_se_deja(trap.is_some())),
            Phase::MainMenu if self.menu_coop => self.coop_opciones().len(),
            Phase::MainMenu => self.menu_opciones().len(),
            Phase::CharSelect => self.chars.len(),
            Phase::Neow { .. } => 3,
            Phase::Shop { wares } => wares.len() + 1,
            Phase::Replace { options, .. } => options.len() + 1,
            Phase::Event { ev, result: None } => ev.opciones.len(),
            Phase::Event { result: Some(_), .. } => 1,
            Phase::Rest => 3,
            Phase::Templar { opciones } => opciones.len() + 1,
            _ => 0,
        }
    }

    /// A sobre el hueco `sel` de la pantalla actual.
    pub(super) fn confirm(&mut self) -> String {
        let sel = self.sel;
        match std::mem::replace(&mut self.phase, Phase::Map) {
            Phase::Neow { common, rare, curse } => {
                let msg = match sel {
                    0 => match common {
                        Some(r) => {
                            self.gain_relic(&r);
                            format!("bendición inicial: {}", r.nombre)
                        }
                        None => "sin bendición".into(),
                    },
                    1 => {
                        if let Some(r) = self.run.as_mut() {
                            r.poder += 1;
                        }
                        let n = self.gain_runes(200);
                        format!("poder +1 y {n} runas")
                    }
                    _ => {
                        let mut v = vec![];
                        for r in [rare, curse].into_iter().flatten() {
                            v.push(r.nombre.clone());
                            self.gain_relic(&r);
                        }
                        format!("trato: {}", v.join(" + "))
                    }
                };
                self.phase = Phase::Map;
                self.sel = 0;
                self.save();
                msg
            }
            Phase::MainMenu if self.menu_coop => {
                self.phase = Phase::MainMenu;
                match self.coop_opciones().get(sel).filter(|o| !o.2).map(|o| o.3) {
                    Some(op) => self.elegir_coop(op),
                    None => "no disponible".into(),
                }
            }
            Phase::MainMenu => {
                self.phase = Phase::MainMenu;
                match self.menu_opciones().get(sel).filter(|o| !o.2).map(|o| o.3) {
                    Some(Menu::Continuar) => match self.guardada.take() {
                        Some(run) => {
                            let msg = format!("se retoma la run {}", run.seed);
                            retomar(self, run);
                            msg
                        }
                        None => "no hay run guardada".into(),
                    },
                    Some(Menu::Nueva) => self.nueva_run(),
                    Some(Menu::Compendio) => {
                        self.show_compendio = true;
                        "compendio".into()
                    }
                    Some(Menu::Personajes) => {
                        self.phase = Phase::Personajes { idx: 0 };
                        self.probar_personaje(0)
                    }
                    Some(Menu::Coop) => {
                        self.menu_coop = true;
                        self.sel = 0;
                        "cooperativo: menú".into()
                    }
                    Some(Menu::Salir) => {
                        crate::salir_del_juego();
                        "saliendo del juego".into()
                    }
                    _ => "no disponible".into(),
                }
            }
            // Cooperativo: se queda elegido y se sigue cuando han elegido todos (coop.rs, elegir_si_todos).
            Phase::CharSelect if self.coop_activo() => {
                self.phase = Phase::CharSelect;
                self.elegido = Some(sel);
                format!("cooperativo: elegido {:?}; se espera a los demás", self.chars.get(sel).map(|c| c.nombre.clone()))
            }
            Phase::CharSelect => match self.chars.get(sel).cloned() {
                Some(c) => self.choose_character(&c),
                None => {
                    self.phase = Phase::CharSelect;
                    "sin personaje".into()
                }
            },
            Phase::Map => match self.available_sorted().get(sel).copied() {
                // En cooperativo se vota: se entra cuando han votado todos (coop.rs).
                Some(to) if self.coop_activo() => self.votar(to),
                Some(to) => self.travel(to),
                None => "sin nodos".into(),
            },
            Phase::Reward { cartas, .. } => {
                self.sel = 0;
                let msg = match cartas.get(sel).cloned() {
                    Some(Carta::Objeto(it)) => self.acquire(it, Phase::Map),
                    Some(Carta::Bendicion(r)) => {
                        self.gain_relic(&r);
                        format!("bendición: {}", r.nombre)
                    }
                    None => "saltado".into(),
                };
                self.save();
                msg
            }
            Phase::Event { ev, result: None } => {
                let Some(o) = ev.opciones.get(sel).cloned() else {
                    self.phase = Phase::Event { ev, result: None };
                    return "sin opción".into();
                };
                let disponible = self.option_enabled(&o);
                self.phase = Phase::Event { ev, result: None };
                if !disponible {
                    return "opción no disponible".into();
                }
                // En cooperativo se vota: se resuelve cuando han votado todos (coop.rs).
                if self.coop_activo() {
                    return self.votar_evento(sel);
                }
                self.resolver_evento(sel)
            }
            Phase::Event { ev, result: Some((texto, enc)) } => {
                // En cooperativo, el combate espera a que estén listos todos (coop.rs).
                if enc.is_some() && self.coop_activo() {
                    self.phase = Phase::Event { ev, result: Some((texto, enc)) };
                    return self.listo_combate();
                }
                let _ = (ev, texto);
                match enc {
                    Some(enc) => self.start_combat(enc),
                    None => {
                        self.finish_node();
                        "fin del evento".into()
                    }
                }
            }
            Phase::Replace { item, options, back } => {
                let msg = match options.get(sel) {
                    Some(&slot) => self.put(&item, slot),
                    None => format!("{} stays behind", item.nombre),
                };
                self.phase = *back;
                self.sel = 0;
                self.save();
                msg
            }
            Phase::Treasure { runes, trap, .. } if sel >= 1 => {
                // Dejarlo (19-10): el normal da sus runas sin la bendición; el trampa, nada.
                let msg = if trap.is_some() { "cofre trampa: lo dejas".to_string() } else { format!("cofre: lo dejas ({} runas)", self.gain_runes(runes)) };
                self.finish_node();
                msg
            }
            Phase::Treasure { relic, runes, trap: Some(enc) } => {
                // En cooperativo el combate del cofre es de todos: espera a que pulsen «Pelear» (coop.rs).
                if self.coop_activo() {
                    self.phase = Phase::Treasure { relic, runes, trap: Some(enc) };
                    return self.listo_combate();
                }
                self.pelear_cofre(relic, runes, enc)
            }
            Phase::Treasure { relic, runes, trap: None } => {
                let got = self.gain_runes(runes);
                let msg = match &relic {
                    Some(r) => {
                        self.gain_relic(r);
                        format!("tesoro: {} y {got} runas", r.nombre)
                    }
                    None => format!("tesoro: {got} runas"),
                };
                self.finish_node();
                msg
            }
            Phase::Shop { mut wares } => {
                if sel >= wares.len() {
                    self.finish_node();
                    return "sales del mercader".into();
                }
                let (ware, cost, sold) = wares[sel].clone();
                let have = crate::runes().unwrap_or(0);
                let msg = if sold {
                    "ya comprado".to_string()
                } else if have < cost {
                    format!("no alcanza: {have}/{cost}")
                } else {
                    crate::set_runes(have - cost);
                    wares[sel].2 = true;
                    match ware {
                        Ware::Item(it) => {
                            // Si no cabe, se elige ranura y luego se vuelve a la tienda (ya con la venta marcada).
                            let r = self.acquire(it.clone(), Phase::Shop { wares: wares.clone() });
                            if matches!(self.phase, Phase::Replace { .. }) {
                                self.save();
                                return format!("comprado: {} ({cost}); {r}", it.nombre);
                            }
                            format!("comprado: {} ({cost}); {r}", it.nombre)
                        }
                        Ware::Relic(r) => {
                            self.gain_relic(&r);
                            format!("comprado: {} ({cost})", r.nombre)
                        }
                    }
                };
                self.phase = Phase::Shop { wares };
                self.save();
                msg
            }
            Phase::Rest => {
                if self.rest_used.get(sel).copied().unwrap_or(true) {
                    self.phase = Phase::Rest;
                    return "ya usado".into();
                }
                let msg = match sel {
                    0 => {
                        let ok = crate::rest_at_grace();
                        format!("descanso en gracia ({})", if ok { "ok" } else { "error" })
                    }
                    1 => {
                        // Aumentar poder: +10 % de daño y de defensa para el resto de la run.
                        let p = self.run.as_mut().map(|r| {
                            r.poder += 1;
                            r.poder
                        });
                        format!("poder {}", p.unwrap_or(0))
                    }
                    _ => {
                        let opciones = self.temperable();
                        if opciones.is_empty() {
                            self.phase = Phase::Rest;
                            return "nada que templar".into();
                        }
                        self.phase = Phase::Templar { opciones };
                        self.sel = 0;
                        return "templar: elegir bendición".into();
                    }
                };
                self.rest_used[sel.min(2)] = true;
                self.tras_gracia();
                msg
            }
            Phase::Templar { opciones } => {
                let Some(o) = opciones.get(sel).cloned() else {
                    // Volver a las opciones de la gracia sin gastar esta.
                    self.phase = Phase::Rest;
                    self.sel = 2;
                    return "templar: vuelta".into();
                };
                self.temper(&o.id);
                self.rest_used[2] = true;
                self.save();
                self.tras_gracia();
                format!("templada: {} (nivel {})", o.id, o.nivel + 2)
            }
            other => {
                self.phase = other;
                "nada".into()
            }
        }
    }
}

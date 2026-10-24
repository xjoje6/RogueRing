//! Personaje y equipo de la run: atributos, ropa, armas, cenizas, bendiciones, frascos y runas.

use super::*;

/// Personajes que llevan el arma a dos manos (el Samurái por su desvío; el Grandullón, sus colosales).
const A_DOS_MANOS: [&str; 2] = ["samurai", "tanque"];

impl Director {
    /// Runs ganadas con este personaje (desbloquean sus otras armas iniciales, apariencias e
    /// invocaciones). En la build de desarrollador, las que hagan falta para todo.
    pub(super) fn victorias(&self, c: &CharacterDef) -> u32 {
        if self.desarrollo {
            return 99;
        }
        self.perfil.victorias.get(&c.id).copied().unwrap_or(0)
    }

    /// Ascensión más alta que se puede elegir con este personaje (en desarrollo, todas).
    pub(super) fn max_ascension(&self, id: &str) -> u32 {
        if self.desarrollo {
            return rogue_core::perfil::ASCENSIONES.len() as u32;
        }
        self.perfil.max_ascension(id)
    }

    /// Arma inicial del personaje: la elegida en Personajes si ya está desbloqueada.
    pub(super) fn arma_de(&self, c: &CharacterDef) -> i32 {
        c.arma(self.perfil.armas.get(&c.id).copied(), self.victorias(c))
    }

    /// Run nueva: fuera todo lo que otra run (o una sesión anterior) pudo dejar en el personaje: los
    /// efectos de todas las bendiciones, no solo las de la última run (una run retomada y luego
    /// sustituida deja los suyos), las pasivas, el tamaño y la ventana de desvío.
    pub(super) fn limpiar_personaje(&self) {
        for r in &self.relics.reliquias {
            for nivel in 0..r.niveles.len().max(1) {
                if let Some(sp) = self.relics.speffect(&r.id, nivel) {
                    crate::player_speffect(sp, false);
                }
            }
        }
        crate::player_speffect(SP_DEFLECT, false);
        crate::player_speffect(SP_MONJE, false);
        crate::player_speffect(SP_SAMURAI, false);
        crate::player_speffect(SP_IMPARABLE, false);
        crate::scale_deflect_window(1.0);
        crate::set_chr_scale(0, 1.0);
    }

    /// Aplica al jugador los SpEffects de sus bendiciones (nivel actual).
    pub(super) fn apply_relics(&self) {
        let Some(run) = &self.run else { return };
        for o in &run.relics {
            if let Some(sp) = self.relics.speffect(&o.id, o.nivel) {
                crate::player_speffect(sp, true);
            }
        }
    }

    pub(super) fn gain_relic(&mut self, r: &RelicDef) {
        /// Bendiciones que cambian el tamaño del personaje.
        const TAMANO: [&str; 3] = ["goliat", "cientifico_loco", "pequenajo"];
        // Lo que cambie ahora los viales (Frasco de más...) vale: ya no se devuelven los de antes del viaje.
        self.vuelta_guardada = None;
        if let Some(run) = self.run.as_mut() {
            run.relics.push(OwnedRelic { id: r.id.clone(), nivel: 0 });
            if let Some(h) = run.historia.last_mut() {
                h.bendiciones.push(r.id.clone());
            }
        }
        if self.perfil.vistas.insert(r.id.clone()) {
            self.perfil.save(&self.root.join("runtime").join("perfil.json"));
        }
        if let Ok(mut t) = TOASTS.lock() {
            t.push(Aviso { nombre: r.nombre.clone(), texto: texto_entero(r), icono: r.icono, maldita: r.maldita, desde: Instant::now() });
            play_se(Se::Blessing);
            // La tela de la ropa del Monje y del Invocador no cambia de tamaño con el personaje: se
            // queda rota a propósito y se avisa con un guiño (17-10).
            let pj = self.run.as_ref().and_then(|x| x.personaje.clone());
            if TAMANO.contains(&r.id.as_str()) && matches!(pj.as_deref(), Some("monje" | "invocador")) {
                t.push(Aviso {
                    nombre: rogue_core::idioma::t("Tailor's note").into(),
                    texto: rogue_core::idioma::t("Cloth does not change size. We tried. It did not go well.").into(),
                    icono: r.icono,
                    maldita: false,
                    desde: Instant::now(),
                });
            }
        }
        self.apply_relics();
        if r.niveles.first().is_some_and(|l| l.dll.contains_key("frascos") || l.dll.contains_key("sin_frascos")) {
            self.apply_flasks();
        }
        // Al coger: runas, bendiciones al azar, Caja de Pandora o maldiciones (el precio de Caos).
        let Some(al) = r.niveles.first().and_then(|l| l.al_coger.clone()) else { return };
        if al.runas > 0 {
            self.gain_runes(al.runas);
        }
        let pj = self.run.as_ref().and_then(|x| x.personaje.clone());
        if al.pandora {
            // Todas las bendiciones (menos maldiciones y la propia caja) se cambian por raras al azar.
            let quitadas: Vec<OwnedRelic> = match self.run.as_mut() {
                Some(run) => {
                    let (fuera, quedan): (Vec<_>, Vec<_>) =
                        run.relics.drain(..).partition(|o| o.id != r.id && self.relics.get(&o.id).is_some_and(|d| !d.maldita));
                    run.relics = quedan;
                    fuera
                }
                None => vec![],
            };
            crate::log(&format!("Caja de Pandora: {} bendiciones cambiadas", quitadas.len()));
            for _ in 0..quitadas.len() {
                let pick = self.run.as_mut().and_then(|run| self.relics.roll_any(&run.relics, Some(Rarity::Rara), &mut run.rng_rewards, pj.as_deref()));
                if let Some(nr) = pick {
                    self.gain_relic(&nr);
                }
            }
            self.apply_relics();
        }
        let rar: Option<Rarity> = al.bendiciones_rareza.as_deref().and_then(|s| serde_json::from_value(serde_json::Value::String(s.into())).ok());
        for _ in 0..al.bendiciones {
            let pick = self.run.as_mut().and_then(|run| self.relics.roll_any(&run.relics, rar, &mut run.rng_rewards, pj.as_deref()));
            if let Some(nr) = pick {
                self.gain_relic(&nr);
            }
        }
        for _ in 0..al.maldiciones {
            let pick = self.run.as_mut().and_then(|run| {
                let cands: Vec<&RelicDef> = self.relics.reliquias.iter().filter(|c| c.maldita && !run.relics.iter().any(|o| o.id == c.id)).collect();
                (!cands.is_empty()).then(|| cands[run.rng_rewards.range(0, cands.len() as i32) as usize].clone())
            });
            if let Some(m) = pick {
                self.gain_relic(&m);
            }
        }
    }

    /// Tamaño que dan las bendiciones pasivas de la run (Diminuto, Goliat); 1 = normal.
    pub(super) fn tamano_pasivo(&self) -> f64 {
        let Some(run) = &self.run else { return 1.0 };
        run.relics
            .iter()
            .filter_map(|o| self.relics.get(&o.id).and_then(|r| r.niveles.get(o.nivel)))
            .flat_map(|l| l.pasivo.iter().map(|p| p.tamano))
            .product()
    }

    /// Frascos carmesí: los del personaje, + Frasco de más, o ninguno con Sin frascos.
    pub(super) fn apply_flasks(&self) {
        let Some(run) = &self.run else { return };
        let base = self.character().map(|c| c.frascos).filter(|&n| n > 0).unwrap_or(4) as f64;
        let menos = if run.ascension >= 3 { 1.0 } else { 0.0 };
        let n = if self.relics.has(&run.relics, "sin_frascos") { 0.0 } else { (base + self.relics.dll_sum(&run.relics, "frascos") - menos).max(1.0) };
        crate::set_hp_flasks(n as u8);
    }

    /// Runas ganadas, con los multiplicadores de las reliquias (p. ej. Bolsa de Kalé).
    pub(super) fn gain_runes(&self, base: u32) -> u32 {
        let f = self.run.as_ref().map(|r| self.relics.dll_factor(&r.relics, "runas") * if r.ascension >= 9 { 0.6 } else { 1.0 }).unwrap_or(1.0);
        let n = (base as f64 * f) as u32;
        if n > 0 && !crate::equip::add_runes(n) {
            crate::set_runes(crate::runes().unwrap_or(0).saturating_add(n));
        }
        n
    }

    /// Objeto del pool que corresponde a un id de inventario (armas: sin afinidad ni mejora).
    pub(super) fn pool_item(&self, inv_id: u32) -> Option<&Item> {
        let (cat, id) = if inv_id & ACCESSORY_BIT != 0 {
            (rewards::Category::Talisman, (inv_id & 0x0FFF_FFFF) as i32)
        } else if inv_id & 0xF000_0000 == 0 {
            (rewards::Category::Arma, (inv_id - inv_id % 10000) as i32)
        } else {
            return None;
        };
        self.pool.iter().find(|p| p.categoria == cat && p.id == id)
    }

    /// Ranuras de arma con un arma en la que encaja la ceniza.
    pub(super) fn gem_slots(&self, gem: &Item) -> Vec<u32> {
        WEAPON_SLOTS
            .into_iter()
            .filter(|&s| equip::slot_item(s).and_then(|(_, id)| self.pool_item(id)).is_some_and(|w| !w.unica && gem.encaja.contains(&w.tipo)))
            .collect()
    }

    /// Lo que no se vuelve a ofrecer: lo conseguido en la run y lo que llevas puesto (las armas de las
    /// manos, la ceniza montada y tu invocación, también las de salida; 24-10: te salía en las
    /// recompensas lo que ya llevabas).
    pub(super) fn no_ofrecer(&self) -> Vec<(rewards::Category, i32)> {
        let mut v = self.run.as_ref().map(|r| r.items.clone()).unwrap_or_default();
        for slot in 0..6 {
            if let Some((_, id)) = equip::slot_item(slot)
                && id & 0xF000_0000 == 0
            {
                v.push((rewards::Category::Arma, (id - id % 10000) as i32));
            }
        }
        if let Some(g) = self.run.as_ref().and_then(|r| r.weapon_gem) {
            v.push((rewards::Category::Ceniza, g));
        }
        if let Some(inv) = self.invocacion_actual() {
            v.extend(self.pool.iter().filter(|i| i.categoria == rewards::Category::Invocacion && i.invocacion.as_ref().is_some_and(|x| x.chr == inv.chr)).map(|i| (i.categoria, i.id)));
        }
        v
    }

    /// Pool para generar ofertas: sin cenizas que no encajen en nada de lo que llevas. Cualquier arma para
    /// cualquier personaje, las de su estilo con PESO_ESTILO (24-10: siempre salían las mismas;
    /// las de fuera de estilo llevan su compensación de daño, `compensar_arma`).
    pub(super) fn offer_pool(&self) -> Vec<Item> {
        let estilo = self.character().map(|c| c.estilo.clone()).unwrap_or_default();
        self.pool
            .iter()
            .filter(|it| it.categoria != rewards::Category::Ceniza || !self.gem_slots(it).is_empty())
            // Cenizas de espíritu: solo para quien invoca.
            .filter(|it| it.categoria != rewards::Category::Invocacion || self.character().is_some_and(|c| !c.invocaciones.is_empty()))
            .map(|it| Item { peso: if it.categoria == rewards::Category::Arma && estilo.contains(&it.tipo) { PESO_ESTILO } else { 1.0 }, ..it.clone() })
            .collect()
    }

    /// Arma de fuera del estilo en la mano derecha: su ataque, al 90 % del que daría con los atributos
    /// repartidos a su favor (data/compensacion_armas.json, tools/compensacion_armas.py). Las del estilo,
    /// tal cual. Se mira cada segundo (tick).
    pub(super) fn compensar_arma(&mut self) {
        let pj = self.run.as_ref().and_then(|r| r.personaje.clone());
        let id = equip::slot_item(1).map(|(_, id)| id).filter(|id| id & 0xF000_0000 == 0);
        let f = match (pj, id) {
            (Some(pj), Some(id)) => self.compensacion.get(&pj).and_then(|m| m.get(&((id - id % 100) as i32))).copied().unwrap_or(1.0),
            _ => 1.0,
        };
        if (f - self.compensacion_puesta).abs() < 0.001 {
            return;
        }
        crate::player_speffect(SP_COMPENSACION, false);
        if (f - 1.0).abs() > 0.001 && crate::set_speffect_attack_rate(SP_COMPENSACION, f as f32) {
            crate::player_speffect(SP_COMPENSACION, true);
        }
        crate::log(&format!("arma: compensación x{f:.3} ({:?})", id.and_then(|i| self.pool_item(i)).map(|w| w.nombre.clone())));
        self.compensacion_puesta = f;
    }

    /// Da y equipa un objeto elegido. Si no hay hueco (o es una ceniza con varias armas posibles)
    /// pasa a la pantalla de elegir ranura y después vuelve a `back`.
    pub(super) fn acquire(&mut self, it: Item, back: Phase) -> String {
        if let Some(run) = self.run.as_mut() {
            run.items.push((it.categoria, it.id));
            if let Some(h) = run.historia.last_mut() {
                h.objetos.push(it.icono);
            }
        }
        if it.categoria == rewards::Category::Arma {
            let r = self.swap_weapon(&it);
            self.phase = back;
            return r;
        }
        if it.categoria == rewards::Category::Invocacion {
            if let Some(run) = self.run.as_mut() {
                // Con el nombre de la carta (ya en el idioma del juego).
                run.invocacion = it.invocacion.clone().map(|i| rogue_core::characters::Invocacion { nombre: it.nombre.clone(), ..i });
            }
            self.phase = back;
            return format!("{}: summoned from now on", it.nombre);
        }
        if it.categoria == rewards::Category::Ceniza {
            let slots = self.gem_slots(&it);
            let r = if slots.is_empty() {
                format!("{}: doesn't fit your weapon", it.nombre)
            } else {
                slots.iter().map(|&s| self.put(&it, s)).collect::<Vec<_>>().join("; ")
            };
            self.phase = back;
            return r;
        }
        let options: Vec<u32> = match it.categoria {
            rewards::Category::Arma => RIGHT_SLOTS.to_vec(),
            rewards::Category::Talisman => TALISMAN_SLOTS.to_vec(),
            rewards::Category::Ceniza => self.gem_slots(&it),
            rewards::Category::Hechizo | rewards::Category::Invocacion => vec![],
        };
        if options.is_empty() {
            self.phase = back;
            return format!("{}: nowhere to put it", it.nombre);
        }
        let direct = match it.categoria {
            rewards::Category::Ceniza if options.len() == 1 => Some(options[0]),
            rewards::Category::Ceniza => None,
            _ => options.iter().copied().find(|&s| equip::slot_item(s).is_none()),
        };
        match direct {
            Some(slot) => {
                let r = self.put(&it, slot);
                self.phase = back;
                r
            }
            None => {
                let msg = format!("{}: elegir ranura", it.nombre);
                self.phase = Phase::Replace { item: it, options, back: Box::new(back) };
                self.sel = 0;
                msg
            }
        }
    }

    /// Una sola arma por personaje: la nueva (en una mano o, si es doble, en las dos) sustituye a
    /// todas las que hubiera, que desaparecen del inventario.
    /// Nivel de mejora del arma según el acto (equilibrado para personajes de nivel 150).
    pub(super) fn weapon_level(&self, unica: bool) -> u32 {
        let (nivel, max) = self.nivel_y_tope(unica);
        nivel.min(max)
    }

    /// Nivel que le toca al arma (acto + Wandering Forge, sin tope) y el máximo del juego.
    fn nivel_y_tope(&self, unica: bool) -> (u32, u32) {
        let act = self.act().min(2);
        let forja = self.run.as_ref().map(|r| r.forja).unwrap_or(0);
        // Por acto, sin saltos (24-10: de +12 a +25 al empezar el acto 2 el daño se disparaba):
        // normales +8/+17/+25, únicas +3/+7/+10.
        if unica { ([3, 7, 10][act] + forja, 10) } else { ([8, 17, 25][act] + forja, 25) }
    }

    /// Niveles de Wandering Forge por encima del máximo del arma equipada. El juego no tiene armas
    /// por encima de +25 (+10 las únicas): cada uno de esos niveles es un `FORJA_EXTRA` de daño.
    pub(super) fn forja_extra(&self) -> u32 {
        let Some((_, cur)) = equip::slot_item(1) else { return 0 };
        let Some(base) = self.pool_item(cur) else { return 0 };
        let (nivel, max) = self.nivel_y_tope(base.unica);
        nivel.saturating_sub(max)
    }

    /// Nivel de mejora del arma equipada (con los de Wandering Forge que pasan del máximo) y su máximo.
    pub(super) fn arma_nivel(&self) -> Option<(u32, u32)> {
        let (_, cur) = equip::slot_item(1)?;
        let base = self.pool_item(cur)?;
        Some((cur % 100 + self.forja_extra(), if base.unica { 10 } else { 25 }))
    }

    /// Rehace el arma actual al nivel del acto, con su ceniza (al empezar un acto nuevo): qué pasó
    /// (para el registro) y el nivel nuevo.
    pub(super) fn reforge(&mut self) -> Option<(String, u32)> {
        let (_, cur) = equip::slot_item(1)?;
        let base = self.pool_item(cur)?.clone();
        let level = self.weapon_level(base.unica);
        if cur % 100 == level {
            return None;
        }
        let doble = equip::slot_item(0).is_some_and(|(_, l)| l - l % 10000 == cur - cur % 10000);
        let gem = self.run.as_ref().and_then(|r| r.weapon_gem);
        let msg = self.swap_weapon(&Item { doble, ..base });
        if let Some(g) = gem {
            let slots: &[u32] = if doble { &[1, 0] } else { &[1] };
            for &s in slots {
                let _ = equip::mount_gem(s, g);
            }
            if let Some(r) = self.run.as_mut() {
                r.weapon_gem = Some(g);
            }
        }
        Some((format!("{msg} to +{level}"), level))
    }

    pub(super) fn swap_weapon(&mut self, it: &Item) -> String {
        let id = it.id as u32 + self.weapon_level(it.unica);
        if let Some(r) = self.run.as_mut() {
            r.weapon_gem = None;
        }
        let old: Vec<(u32, u32)> = WEAPON_SLOTS.iter().filter_map(|&s| equip::slot_item(s)).collect();
        let old_idx: Vec<u32> = old.iter().map(|o| o.0).collect();
        // Las de pareja (puños, garras, artes marciales) van una sola y a dos manos.
        let doble = it.doble && !it.en_pareja();
        let hands: &[u32] = if doble { &[1, 0] } else { &[1] };
        for &slot in hands {
            let r = equip::give_new(id, -1).and_then(|idx| equip::equip_idx(slot, idx));
            if let Err(e) = r {
                return format!("error equipando {}: {e}", it.nombre);
            }
        }
        // Las demás ranuras de arma, con los puños (si no, el modelo sigue mostrando la vieja).
        for &slot in WEAPON_SLOTS.iter().filter(|s| !hands.contains(s)) {
            if equip::slot_item(slot).is_some() {
                let _ = equip::unequip(slot);
            }
        }
        for i in old_idx {
            let _ = equip::remove_idx(i, 1);
        }
        // Mano 1 activa y reequipar para que el modelo lo refleje.
        equip::select_first_weapons();
        for &slot in hands {
            if let Some((idx, _)) = equip::slot_item(slot) {
                let _ = equip::equip_idx(slot, idx);
            }
        }
        // A dos manos al cogerla: las de pareja y, con el Samurái y el Grandullón, todas (19-10: las
        // que antes le salían dobles le llegaban a una mano; la pasiva solo lo ponía al empezar el combate).
        if it.en_pareja() || (!doble && self.character().is_some_and(|c| A_DOS_MANOS.contains(&c.id.as_str()))) {
            equip::set_two_handed(true);
        }
        format!("{}{} equipped", if doble { "Dual " } else { "" }, it.nombre)
    }

    /// Carta de una oferta: dice qué arma sustituye o a cuál va la ceniza.
    pub(super) fn offer_card(&self, it: &Item) -> UiCard {
        let mut c = item_card(it);
        if let Some(w) = self.current_weapon_name() {
            match it.categoria {
                rewards::Category::Arma => c.pie = rogue_core::idioma::tf("Replaces {}", &[&w]),
                rewards::Category::Ceniza => c.pie = rogue_core::idioma::tf("For your {}", &[&w]),
                _ => {}
            }
        }
        if it.categoria == rewards::Category::Invocacion
            && let Some(n) = self.invocacion_actual().map(|i| i.nombre)
        {
            c.pie = rogue_core::idioma::tf("Replaces {}", &[&n]);
        }
        c
    }

    /// Invocación de la run: la ganada en una recompensa o, si no, la elegida en Personajes.
    pub(super) fn invocacion_actual(&self) -> Option<rogue_core::characters::Invocacion> {
        if let Some(i) = self.run.as_ref().and_then(|r| r.invocacion.clone()) {
            return Some(i);
        }
        let c = self.character()?;
        c.invocacion(self.perfil.invocaciones.get(&c.id).map(String::as_str), self.victorias(c)).cloned()
    }

    /// Nombre del arma que llevas en la mano derecha (para las cartas).
    pub(super) fn current_weapon_name(&self) -> Option<String> {
        equip::slot_item(1).and_then(|(_, id)| self.pool_item(id)).map(|w| w.nombre.clone())
    }

    /// Pone el objeto en la ranura; lo que hubiera ahí desaparece del inventario.
    pub(super) fn put(&mut self, it: &Item, slot: u32) -> String {
        let r = match it.categoria {
            rewards::Category::Ceniza => {
                let r = equip::mount_gem(slot, it.id);
                if r.is_ok()
                    && let Some(run) = self.run.as_mut()
                {
                    run.weapon_gem = Some(it.id);
                }
                r
            }
            _ => {
                let id = if it.categoria == rewards::Category::Talisman { ACCESSORY_BIT | it.id as u32 } else { it.id as u32 };
                let old = equip::slot_item(slot);
                equip::give_new(id, -1).and_then(|idx| equip::equip_idx(slot, idx)).and_then(|_| old.map(|(i, _)| equip::remove_idx(i, 1)).unwrap_or(Ok(())))
            }
        };
        match r {
            Ok(()) => format!("{} in {}", it.nombre, slot_name(slot)),
            Err(e) => format!("error equipando {}: {e}", it.nombre),
        }
    }

    /// Marcas del personaje que lee tools/hks/rogue.hks (esquiva de la Duquesa, desvíos, Imparable).
    pub(super) fn marcas_personaje(&self) -> Vec<i32> {
        match self.character().map(|c| c.id.as_str()) {
            Some("monje") => vec![SP_MONJE],
            Some("samurai") => vec![SP_SAMURAI, SP_DEFLECT],
            Some("tanque") => vec![SP_IMPARABLE],
            _ => vec![],
        }
    }

    /// Pasiva del personaje que se hace con efectos del juego (Samurai: desvío perfecto siempre).
    pub(super) fn apply_passive(&self) {
        let id = self.character().map(|c| c.id.as_str()).unwrap_or("");
        crate::player_speffect(SP_MONJE, id == "monje");
        crate::player_speffect(SP_SAMURAI, id == "samurai");
        crate::player_speffect(SP_IMPARABLE, id == "tanque");
        if id == "samurai" {
            crate::player_speffect(SP_DEFLECT, true);
            // Mano de hierro: ventana de desvío un 50 % más larga (duración de sus SpEffects).
            let mano = self.run.as_ref().is_some_and(|r| r.relics.iter().any(|o| o.id == "mano_hierro"));
            crate::scale_deflect_window(if mano { 1.5 } else { 1.0 });
        }
        // A dos manos: el Samurái y el Grandullón siempre (el desvío pide el arma bien agarrada; las colosales
        // se llevan así, 24-10) y cualquiera con un arma de pareja (puños, garras, artes marciales),
        // salvo con arma doble.
        let pareja = equip::slot_item(1).and_then(|(_, cur)| self.pool_item(cur)).is_some_and(|i| i.en_pareja());
        if (A_DOS_MANOS.contains(&id) || pareja) && !equip::left_hand_armed() {
            equip::set_two_handed(true);
        }
    }

    pub(super) fn character(&self) -> Option<&CharacterDef> {
        let id = self.run.as_ref()?.personaje.as_deref()?;
        self.chars.iter().find(|c| c.id == id)
    }

    /// Aplica el personaje elegido: atributos fijos, su arma (sola o doble) y su ceniza.
    /// Da a la partida, una vez, la ropa y las armas iniciales de todos los personajes que falten. Se
    /// hace por detrás del menú principal: el juego enseña el aviso de objeto nuevo la primera vez y se
    /// cierra solo (cerrar_avisos). Así Personajes solo equipa y no salen avisos ni copias.
    pub(super) fn preparar_vestuario(&mut self) {
        let inv: Vec<u32> = equip::inventory().iter().map(|e| e.2).collect();
        let mut dadas = 0;
        for c in self.chars.clone() {
            // Su ropa de siempre y la de sus apariencias.
            let ropa: Vec<i32> = c.ropa.iter().chain(c.apariencias.iter().flat_map(|a| a.ropa.iter())).copied().collect();
            for &r in &ropa {
                let id = PROTECTOR_BIT | r as u32;
                if r > 0 && !inv.contains(&id) && equip::give(id, 1, -1).is_ok() {
                    dadas += 1;
                }
            }
            for &a in &c.armas {
                let a = a as u32;
                if !inv.iter().any(|&i| i < PROTECTOR_BIT && i - i % 10_000 == a) && equip::give(a, 1, -1).is_ok() {
                    dadas += 1;
                }
            }
        }
        if dadas > 0 {
            self.cerrar_avisos = Some(Instant::now() + Duration::from_secs(8));
        }
        crate::log(&format!("vestuario: {dadas} objetos nuevos"));
    }

    /// Pantalla de Personajes: le pone al jugador la ropa y el arma elegida del personaje `idx`.
    pub(super) fn probar_personaje(&mut self, idx: usize) -> String {
        let Some(c) = self.chars.get(idx).cloned() else { return "sin personaje".into() };
        let mut log = vec![c.nombre.clone(), self.vestir(&c)];
        // El arma: la del inventario (cualquier mejora o afinidad), en la mano derecha; si no está,
        // como en la run.
        let arma = self.arma_de(&c) as u32;
        let ya = equip::inventory().iter().find(|e| e.2 < PROTECTOR_BIT && e.2 - e.2 % 10_000 == arma).map(|e| e.0);
        match ya {
            Some(idx) => log.push(format!("{:?}", equip::equip_idx(1, idx))),
            None => {
                if let Some(mut w) = self.pool.iter().find(|i| i.id == arma as i32).cloned() {
                    w.doble = c.doble;
                    log.push(self.swap_weapon(&w));
                }
            }
        }
        crate::mirar_a_camara();
        log.join("; ")
    }

    /// Elige personaje. La run y las pantallas siguen al momento; lo que toca al personaje en el juego
    /// (atributos, arma, frascos, pasiva) espera a que termine la carga si hace falta: tras morir, la run
    /// nueva lleva al centro y el personaje no existe hasta que acaba (char_pendiente, lo aplica tick).
    pub(super) fn choose_character(&mut self, c: &CharacterDef) -> String {
        if let Some(run) = self.run.as_mut() {
            run.personaje = Some(c.id.clone());
            run.historia.push(rogue_core::run::NodeLog { acto: 1, tipo: "Inicio".into(), vida_min: 1.0, ..Default::default() });
        }
        let mut log = vec![if jugador_listo() {
            self.aplicar_personaje(c)
        } else {
            self.char_pendiente = self.chars.iter().position(|x| x.id == c.id);
            format!("{}: se aplica al terminar la carga", c.nombre)
        }];
        // Ascensión 6: una maldición al empezar.
        if self.run.as_ref().is_some_and(|r| r.ascension >= 6) {
            log.extend(self.maldicion_ascension());
        }
        self.phase = self.neow_offer();
        self.sel = 0;
        self.save();
        log.join("; ")
    }

    /// Pone la ropa del personaje (cabeza, torso, brazos, piernas) y quita la que llevaba: la de su
    /// apariencia elegida si ya está desbloqueada. Un -1 deja la ranura vacía (las apariencias de
    /// Nightreign son el héroe entero en el torso).
    pub(super) fn vestir(&self, c: &CharacterDef) -> String {
        let ropa = c.ropa_con(self.perfil.apariencias.get(&c.id).map(String::as_str), self.victorias(c));
        let mut puestas = 0;
        for (i, &id) in ropa.iter().enumerate().take(4) {
            let slot = 12 + i as u32;
            if id < 0 {
                if equip::slot_item(slot).is_some() {
                    let _ = equip::unequip(slot);
                }
                puestas += 1;
                continue;
            }
            if id == 0 {
                continue;
            }
            let inv_id = PROTECTOR_BIT | id as u32;
            let vieja = equip::slot_item(slot);
            if vieja.is_some_and(|(_, item)| item == inv_id) {
                puestas += 1;
                continue;
            }
            // La que ya está en el inventario (vestuario por adelantado); si no, una nueva.
            let ya = equip::inventory().iter().find(|e| e.2 == inv_id).map(|e| e.0);
            let vieja = vieja.filter(|_| ya.is_none());
            match ya.map(Ok).unwrap_or_else(|| equip::give_new(inv_id, -1)).and_then(|idx| equip::equip_idx(slot, idx)) {
                Ok(()) => {
                    puestas += 1;
                    if let Some((idx, _)) = vieja {
                        let _ = equip::remove_idx(idx, 1);
                    }
                }
                Err(e) => crate::log(&format!("ropa {id}: {e}")),
            }
        }
        format!("ropa {puestas}/{}", ropa.len())
    }

    /// Lo que el personaje elegido cambia en el juego: atributos fijos, su arma (sola o doble) y su
    /// ceniza, frascos, pasiva, y las bendiciones que ya tenga la run.
    pub(super) fn aplicar_personaje(&mut self, c: &CharacterDef) -> String {
        let arma = self.arma_de(c);
        // Lo justo para empuñar el arma elegida (las desbloqueables piden algo más).
        let req = self.pool.iter().find(|i| i.id == arma).map(|i| i.req).unwrap_or([0; 5]);
        let lvl = crate::set_attributes(c.atributos_para(req));
        let talis = equip::clear_talismans();
        let mut log = vec![format!("{} (nivel {}, {talis} talismanes fuera)", c.nombre, lvl.unwrap_or(0))];
        log.push(self.vestir(c));
        if let Some(mut w) = self.pool.iter().find(|i| i.id == arma).cloned() {
            w.doble = c.doble;
            log.push(self.swap_weapon(&w));
            if let Some(g) = c.ceniza.and_then(|g| self.pool.iter().find(|i| i.categoria == rewards::Category::Ceniza && i.id == g).cloned()) {
                for s in self.gem_slots(&g) {
                    log.push(self.put(&g, s));
                }
            }
        }
        if c.frascos > 0 {
            crate::set_hp_flasks(c.frascos as u8);
            crate::rest_at_grace();
        }
        self.apply_passive();
        self.apply_relics();
        self.apply_flasks();
        log.join("; ")
    }

    pub(super) fn temperable(&self) -> Vec<OwnedRelic> {
        self.run.as_ref().map(|r| r.relics.iter().filter(|o| self.can_temper(o)).cloned().collect()).unwrap_or_default()
    }

    /// Sube un nivel la reliquia (cambia su SpEffect por el del nivel siguiente).
    pub(super) fn temper(&mut self, id: &str) {
        if let Some(o) = self.run.as_ref().and_then(|r| r.relics.iter().find(|x| x.id == id).cloned()) {
            if let Some(sp) = self.relics.speffect(&o.id, o.nivel) {
                crate::player_speffect(sp, false);
            }
            if let Some(x) = self.run.as_mut().and_then(|r| r.relics.iter_mut().find(|x| x.id == id)) {
                x.nivel += 1;
            }
            self.apply_relics();
        }
    }

    pub(super) fn can_temper(&self, o: &OwnedRelic) -> bool {
        self.relics.get(&o.id).is_some_and(|d| o.nivel + 1 < d.niveles.len())
    }
}

impl Director {
    /// Ascensión 6: una maldición al azar de las que no se llevan.
    pub(super) fn maldicion_ascension(&mut self) -> Option<String> {
        let pick = self.run.as_mut().and_then(|run| {
            let curses: Vec<&RelicDef> = self.relics.reliquias.iter().filter(|r| r.maldita && !r.pendiente && !run.relics.iter().any(|o| o.id == r.id)).collect();
            (!curses.is_empty()).then(|| curses[run.rng_rewards.range(0, curses.len() as i32) as usize].clone())
        })?;
        self.gain_relic(&pick);
        Some(format!("maldición de la ascensión: {}", pick.nombre))
    }
}

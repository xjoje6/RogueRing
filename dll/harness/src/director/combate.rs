//! Combate: escalado del rival, bendiciones con disparador, pasivas por fotograma y estadísticas.

use super::*;

impl Director {
    /// Coliseo: hay enemigos que se apagan si se les lleva al borde de la arena (Perfumista Tricia) y
    /// otros que se salen de ella (los dragones se posan en los muros). Si pasa `VIGIA_PARADO` sin
    /// moverse ni cambiar de animación (y sin el jugador cerca, `VIGIA_CERCA`), o `VIGIA_FUERA` fuera
    /// de la arena, vuelve a donde apareció y retoma la pelea desde allí.
    /// Rival invisible (la Black Knife élite): en su sitio del juego la delatan sus pisadas en la nieve;
    /// en el coliseo no hay nieve, así que deja un soplo de niebla helada a sus pies cuando se mueve.
    pub(super) fn pisadas(&mut self, ent: u32) {
        let Some(fx) = self.fx.as_mut() else { return };
        if !crate::chr_speffects(ent).contains(&SP_INVISIBLE) {
            return;
        }
        let Some(p) = crate::chr_pos(ent) else { return };
        let toca = fx.pisada.is_none_or(|(q, t)| t.elapsed() >= Duration::from_millis(450) && ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt() >= 0.6);
        if toca {
            let _ = crate::spawn_bullet_at(PISADA, None, Some([p[0], p[1] + 0.05, p[2]]));
            fx.pisada = Some((p, Instant::now()));
        }
    }

    pub(super) fn vigilar(&mut self, ent: u32) {
        let (Some(p), Some(a)) = (crate::chr_pos(ent), crate::chr_anim(ent)) else { return };
        let grande = self.area_combate != COLOSSEUM_AREA;
        let v = self.vigia.get_or_insert(Vigia { pos: p, anim: a, desde: Instant::now(), origen: p, fuera: None });
        // Arena grande: sitio de sobra para alejarse, volar o quedarse quieto (19-10). Solo vuelve si se
        // cae de la plataforma.
        if grande {
            if p[1] < v.origen[1] - VIGIA_CAIDA {
                let ok = crate::set_chr_pos(ent, v.origen);
                crate::log(&format!("arena grande: el enemigo se ha caído; vuelve a su sitio ({ok})"));
                (v.pos, v.desde, v.fuera) = (v.origen, Instant::now(), None);
            }
            return;
        }
        let lejos = (p[0] - v.origen[0]).hypot(p[2] - v.origen[2]) > VIGIA_DISTANCIA;
        // Peleando (el jugador cerca): la altura no cuenta. Los grandes que saltan o vuelan (Caligo)
        // pasaban de la altura y volvían al centro en mitad del combate (16-10).
        let peleando = crate::player_pos().is_some_and(|j| (p[0] - j[0]).hypot(p[2] - j[2]) < VIGIA_CERCA);
        if (p[1] > v.origen[1] + VIGIA_ALTURA && !peleando) || lejos {
            if v.fuera.get_or_insert_with(Instant::now).elapsed() > VIGIA_FUERA {
                let ok = crate::set_chr_pos(ent, v.origen);
                crate::log(&format!("coliseo: el enemigo se ha salido de la arena; vuelve a su sitio ({ok})"));
                (v.pos, v.desde, v.fuera) = (v.origen, Instant::now(), None);
            }
            return;
        }
        v.fuera = None;
        let d2 = (p[0] - v.pos[0]).powi(2) + (p[1] - v.pos[1]).powi(2) + (p[2] - v.pos[2]).powi(2);
        if d2 > 0.25 || a != v.anim || peleando {
            (v.pos, v.anim, v.desde) = (p, a, Instant::now());
        } else if v.desde.elapsed() > VIGIA_PARADO {
            let ok = crate::set_chr_pos(ent, v.origen);
            crate::log(&format!("coliseo: el enemigo lleva {}s parado; vuelve a su sitio ({ok})", VIGIA_PARADO.as_secs()));
            (v.pos, v.desde) = (v.origen, Instant::now());
        }
    }

    /// Música del combate del coliseo: el tema propio del jefe o uno genérico de minijefe, siempre el
    /// mismo para el mismo encuentro.
    pub(super) fn empezar_musica(&mut self, enc: &Encounter) {
        let m = &self.musica;
        let pos = |id: i32| m.temas.iter().position(|&t| t == id);
        let propio = enc.chr.and_then(|c| pos(c * 100));
        let generico = || {
            let n = m.genericos.len().max(1) as u64;
            m.genericos.get((rogue_core::rng::hash_str(&enc.id) % n) as usize).and_then(|&id| pos(id))
        };
        if let Some(k) = propio.or_else(generico) {
            flag_set(MUSICA_EMPEZAR + k as u32, true);
            self.sonando = Some((k, false));
        }
    }

    /// Por debajo de media vida del rival, el tema sube de intensidad (una vez).
    pub(super) fn subir_musica(&mut self) {
        if let Some((k, false)) = self.sonando {
            flag_set(MUSICA_SUBIR + k as u32, true);
            self.sonando = Some((k, true));
        }
    }

    pub(super) fn parar_musica(&mut self) {
        if let Some((k, _)) = self.sonando.take() {
            flag_set(MUSICA_PARAR + k as u32, true);
        }
    }

    /// Tramo de escalado del combate actual: sube con la fila del mapa dentro del acto.
    pub(super) fn tramo(&self, kind: EncounterKind) -> i32 {
        let Some(t) = self.tiers.get(self.acto_prueba.unwrap_or(self.act())) else { return 7000 };
        let (fila, filas) = self.run.as_ref().map(|r| (r.position.1, r.act.map.nodes.iter().map(|n| n.row).max().unwrap_or(1))).unwrap_or((0, 1));
        let x = (fila as f32 / filas.max(1) as f32).clamp(0.0, 1.0);
        let normal = t.normal.0 + (((t.normal.1 - t.normal.0) as f32 * x / 10.0).round() as i32) * 10;
        match kind {
            EncounterKind::Weak => t.debil,
            EncounterKind::Normal => normal,
            EncounterKind::Elite => (normal + t.elite).min(t.jefe),
            EncounterKind::Boss => t.jefe,
        }
    }

    /// Al empezar el combate: multiplicadores de daño de la run (Trofeo, Coleccionista) y Primer corte.
    pub(super) fn start_fx(&mut self, enc: &Encounter) {
        let kind = enc.kind;
        let Some(run) = &self.run else { return };
        let owned = &run.relics;
        let n_blessings = owned.iter().filter(|o| self.relics.get(&o.id).is_some_and(|r| !r.maldita)).count() as f64;
        let power = 1.1f64.powi(run.poder as i32);
        let (danio_pj, recibido_pj) = self.character().map(|c| (c.danio, c.recibido)).unwrap_or((1.0, 1.0));
        // Wandering Forge por encima del máximo del arma: daño en lugar de niveles que el juego no tiene.
        let forja = 1.0 + FORJA_EXTRA * self.forja_extra() as f64;
        let dyn_rate = power * danio_pj * forja * (1.0 + self.relics.dll_sum(owned, "coleccionista") * n_blessings);
        // El motor escribe las filas dinámicas de daño y defensa (base: gracia y Coleccionista).
        let mut efectos = vec![];
        let mut pasivos = vec![];
        for o in owned {
            if let Some(l) = self.relics.get(&o.id).and_then(|r| r.niveles.get(o.nivel)) {
                efectos.extend(l.efectos.iter().cloned().map(|e| (o.id.clone(), e)));
                pasivos.extend(l.pasivo.iter().cloned().map(|p| (o.id.clone(), p)));
            }
        }
        let mut muts: Vec<crate::motor::Mutador> = run.mutadores.iter().filter_map(|id| self.mutadores.iter().find(|m| &m.id == id).cloned()).collect();
        // Mutadores del propio encuentro: el campeón es un enemigo normal con aura (docs/LISTAS_Y_COMBATES.md).
        muts.extend(enc.mutadores.iter().filter_map(|id| self.mutadores.iter().find(|m| &m.id == id).cloned()));
        // Ascensión: vida y daño de los enemigos, y mutadores extra en élites y jefes.
        let asc = run.ascension;
        let (vida, danio) = rogue_core::perfil::factores_ascension(asc);
        if asc >= 1 {
            muts.push(crate::motor::Mutador { id: "ascension".into(), vida, danio, ..Default::default() });
        }
        let last_act = run.act_index + 1 >= self.acts.len();
        if let Some(m) = self.mutador_ascension(run, kind, last_act) {
            muts.push(m);
        }
        // Precio de algunas míticas: enemigos que pegan más (sin nombre en la barra) o con más vida.
        let danio_enemigos = self.relics.dll_factor(owned, "danio_enemigos");
        if (danio_enemigos - 1.0).abs() > 0.001 {
            muts.push(crate::motor::Mutador { id: "bendiciones".into(), danio: danio_enemigos, ..Default::default() });
        }
        if let Ok(mut b) = BOSS_MUTS.lock() {
            *b = muts.iter().filter(|m| !m.nombre.is_empty()).map(|m| m.nombre.clone()).collect::<Vec<_>>().join("  ·  ");
        }
        // Cooperativo (docs/LISTAS_Y_COMBATES.md): las listas de dúos ya son para dos, con el escalado de
        // Seamless a cero (lanzador); `coop_extra` más de vida por cada jugador además del primero (el jugador,
        // 23-10: que con dos ya se note, no a partir de tres).
        let coop = if run.coop { 1.0 + self.coop_extra * crate::coop::jugadores().saturating_sub(1) as f64 } else { 1.0 };
        let bhp_mult = self.tiers.get(self.acto_prueba.unwrap_or(run.act_index)).map(|t| t.vida(kind)).unwrap_or(1.0)
            * self.relics.dll_factor(owned, "vida_enemigos")
            * enc.vida.unwrap_or(1.0)
            * coop;
        let mut motor = crate::motor::Motor::new(efectos, pasivos, muts, dyn_rate, recibido_pj / power, bhp_mult, run.contadores.clone());
        motor.golpe_defecto = [300.0, 550.0, 1000.0][self.acto_prueba.unwrap_or(run.act_index).min(2)];
        // Tus golpes del combate anterior de esta run (no de otras).
        if self.golpes_run.0 == run.seed {
            motor.seguir_golpes(self.golpes_run.1.clone());
        }
        // Invitado: el tramo no se queda en sus copias de los rivales (son del anfitrión); se le aplica a
        // él en el motor (repaso del 18-10, punto 10).
        if run.coop && crate::coop::hay_companeros() && !crate::coop::soy_anfitrion() {
            let tier = self.tramo(kind);
            motor.invitado = crate::tramo_rates(tier).map(|(a, d)| (a as f64, d as f64));
            crate::log(&format!("cooperativo: invitado, tramo {tier} {:?} sobre el jugador", motor.invitado));
        }
        let first = self.relics.dll_factor(owned, "primer_golpe");
        let first_hit_pending = first > 1.0001 && crate::set_speffect_attack_rate(SP_FIRST_HIT, first as f32);
        if first_hit_pending {
            crate::player_speffect(SP_FIRST_HIT, true);
        }
        let v = self.relics.dll_factor(owned, "venganza");
        if v > 1.0001 {
            crate::set_speffect_attack_rate(SP_VENGEANCE, v as f32);
        }
        self.apply_passive();
        let php = crate::player_hp_max().map(|h| h.0).unwrap_or(0);
        crate::log(&format!("bendiciones: daño x{dyn_rate:.2}, primer golpe x{first:.1}"));
        self.fx = Some(CombatFx {
            since: Instant::now(),
            last_player_hp: php,
            last_boss_hp: i32::MAX,
            first_hit_pending,
            vengeance_on: false,
            boss_hp_cut: false,
            last_clock: Instant::now(),
            took_damage: false,
            deflecting: false,
            guard_fast: false,
            deflect_fast_until: None,
            last_hit_taken: None,
            last_tick: Instant::now(),
            pausa_desde: None,
            pisada: None,
            motor: Some(motor),
            ev_deflect: false,
            st_dmg: HashMap::new(),
            st_heal: HashMap::new(),
            st_taken: 0.0,
            st_hits: 0,
            golpe_anim: None,
            st_min: 1.0,
        });
    }

    /// Cada fotograma del combate: Primer corte, Espinas, Venganza, Reloj de arena y Marca del cazador.
    /// Cooperativo: apunta cómo acabó este combate (lo publica el estado; el del anfitrión manda).
    pub(super) fn anotar_resultado(&mut self, r: rogue_core::protocolo::Resultado) {
        if self.coop_activo()
            && let Some(run) = self.run.as_ref()
        {
            self.ultimo_coop = Some((rogue_core::coop::Paso { acto: run.act_index, nodo: run.path.len() }, r));
        }
    }

    /// Cooperativo: si sigue muerto tras un combate ganado, a reaparecer. Por capas, porque no se puede
    /// probar sin dos PCs: `paso` 0 al ganar (recuperación de gracia, lo que espera Seamless), 1 al
    /// volver al centro y 2 antes del combate siguiente (vida al máximo a mano). Apunta cuál hizo falta.
    pub(super) fn revivir(&mut self, paso: u8) {
        if !self.coop_activo() || crate::player_hp_max().is_none_or(|(h, _)| h > 0) {
            return;
        }
        match paso {
            // Al ganar: lo hace Seamless con el viaje al centro. La recuperación de gracia (un flag de
            // evento) podía saltar también en el juego del compañero y recargarle los viales (23-10).
            0 => crate::log("cooperativo: muerto tras ganar; reaparece con el viaje"),
            _ => {
                if let Some((_, max)) = crate::player_hp_max() {
                    crate::set_player_hp(max);
                }
                crate::log(&format!("cooperativo: seguía muerto ({}); vida al máximo", if paso == 1 { "al volver" } else { "antes del combate" }));
            }
        }
    }

    /// Cierra el combate como diga el protocolo del cooperativo: victoria (la del anfitrión, en el
    /// juego del invitado) o derrota (todos muertos: se acaba la run).
    pub(super) fn cerrar_combate(&mut self, enc: &Encounter, r: rogue_core::protocolo::Resultado) {
        self.anotar_resultado(r);
        match r {
            rogue_core::protocolo::Resultado::Victoria => {
                self.motor_victoria();
                self.record_combat(enc, true);
                self.end_fx();
                crate::log(&format!("cooperativo: {} ganado (lo dice el anfitrión)", enc.name));
                self.revivir(0);
                let v = self.caido_cfg_vida();
                self.levantarse(Some(v), "combate ganado");
                self.phase = Phase::Won { enc: enc.clone(), at: Instant::now() };
            }
            rogue_core::protocolo::Resultado::Derrota => {
                self.end_fx();
                self.record_combat(enc, false);
                self.fx = None;
                crate::log(&format!("run terminada: todos muertos contra {}", enc.name));
                self.intocable_hasta_la_siguiente();
                self.levantarse(None, "fin de la run");
                // Los rivales, fuera, y todos al centro ya (el invitado, con Seamless): con el final en
                // pantalla seguían pegando detrás, y al invitado que empezaba otra run en la arena lo
                // mataban de verdad (23-10: modo espectador de Seamless y la cámara rota).
                for e in self.grupo.clone() {
                    crate::set_chr_disabled(e, true);
                }
                self.viajar(HUB_WARP_FLAG);
                self.phase = Phase::Over { won: false, new_run: false, shown: None };
                self.sel = 0;
                self.save();
            }
        }
    }

    pub(super) fn tick_fx(&mut self, ent: u32, kind: EncounterKind) {
        // Muerto (mirando a los compañeros en cooperativo): tus bendiciones no hacen nada (19-10:
        // el cuerpo invisible seguía lanzando bolas de fuego).
        if player_hp() == Some(0) || self.caido_fuera() {
            return;
        }
        let owned = self.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
        let espinas = self.relics.dll_sum(&owned, "espinas");
        let vida_jefe = self.relics.dll_factor(&owned, "vida_jefe");
        let reloj = self.relics.dll_sum(&owned, "reloj");
        let venganza = self.relics.has(&owned, "venganza");
        let is_samurai = self.character().is_some_and(|c| c.id == "samurai");
        let is_tanque = self.character().is_some_and(|c| c.id == "tanque");
        let pacto = self.relics.dll_factor(&owned, "pacto");
        let is_monje = self.character().is_some_and(|c| c.id == "monje");
        let regen_mult = self.relics.dll_factor(&owned, "regeneracion");
        let Some(fx) = self.fx.as_mut() else { return };
        // Vuelta de la pausa: lo que duró no cuenta (si no, Hourglass o Veil se recargarían pausando).
        if let Some(t) = fx.pausa_desde.take() {
            let d = t.elapsed();
            fx.since += d;
            fx.last_clock += d;
            fx.last_hit_taken = fx.last_hit_taken.map(|x| x + d);
            fx.deflect_fast_until = fx.deflect_fast_until.map(|x| x + d);
            if let Some(m) = fx.motor.as_mut() {
                m.tras_pausa(d);
            }
            crate::log(&format!("pausa: {:.1} s fuera del reloj del combate", d.as_secs_f32()));
        }
        let (Some((php, pmax)), Some((bhp, bmax))) = (crate::player_hp_max(), crate::chr_hp_max(ent)) else { return };
        let mut php = php;
        // Marca del cazador: una vez, en cuanto pasa el relleno de vida del principio.
        if !fx.boss_hp_cut && fx.since.elapsed() > Duration::from_secs(1) {
            fx.boss_hp_cut = true;
            if kind == EncounterKind::Boss && vida_jefe < 0.999 {
                crate::set_chr_hp(ent, (bmax as f64 * vida_jefe) as i32);
                fx.last_boss_hp = (bmax as f64 * vida_jefe) as i32;
                return;
            }
        }
        // Un golpe tuyo es una bajada de vida del rival mientras atacas. El resto (veneno,
        // podredumbre, sangrado, fuego del suelo...) le quita vida pero no es un golpe: si contara,
        // las bendiciones de «al golpear» se dispararían solas, una tras otra.
        let baja = if bhp < fx.last_boss_hp && fx.last_boss_hp != i32::MAX { fx.last_boss_hp - bhp } else { 0 };
        // Cooperativo: el daño de las bendiciones de un invitado que aplica el anfitrión no es un golpe de
        // nadie (en el juego del anfitrión ni, de vuelta, en el del invitado).
        let baja = if baja > 0 && crate::coop::hay_companeros() { crate::coop::sin_eco(ent, baja) } else { baja };
        let hit_boss = baja > 0 && crate::golpe_mio(ent);
        // Un golpe por animación de ataque (23-10: atacando mientras el rival perdía vida por otras
        // cosas, contaba varios golpes para las bendiciones de «cada X golpes»). Lo que baja después en
        // la misma animación (estados, proyectiles de tus bendiciones, que el juego apunta a tu nombre) no
        // es otro golpe, salvo armas de varios impactos por animación: pasado un momento y del tamaño de
        // un golpe.
        let hit_boss = hit_boss && {
            let anim = crate::player_anim().unwrap_or(0);
            let media = fx.motor.as_ref().map(|m| m.media_golpe()).unwrap_or(0.0);
            let otro = match fx.golpe_anim {
                Some((a, t)) if a == anim => t.elapsed() >= Duration::from_millis(150) && (media <= 0.0 || baja as f64 >= media * 0.5),
                _ => true,
            };
            if otro {
                fx.golpe_anim = Some((anim, Instant::now()));
            }
            otro
        };
        let hit_dmg = if hit_boss { baja } else { 0 };
        // Cooperativo: un golpe no vale más de 5 golpes medios (lo que pase de ahí es daño de otro que se
        // ha juntado con el tuyo).
        let hit_dmg = match fx.motor.as_ref().map(|m| m.media_golpe()) {
            Some(m) if m > 0.0 && crate::coop::hay_companeros() => hit_dmg.min((m * 5.0) as i32),
            _ => hit_dmg,
        };
        if hit_dmg > 0 {
            *fx.st_dmg.entry("arma".into()).or_insert(0.0) += hit_dmg as f64;
            fx.st_hits += 1;
        } else if baja > 0 {
            *fx.st_dmg.entry("estados".into()).or_insert(0.0) += baja as f64;
        }
        if php < fx.last_player_hp && fx.last_player_hp != i32::MAX {
            fx.st_taken += (fx.last_player_hp - php) as f64;
        }
        if pmax > 0 && fx.since.elapsed() > Duration::from_secs(2) {
            fx.st_min = fx.st_min.min(php.max(0) as f64 / pmax as f64);
        }
        let mut bhp = bhp;
        // Primer corte: se quita al primer golpe que baja la vida del boss.
        if fx.first_hit_pending && hit_boss {
            fx.first_hit_pending = false;
            crate::player_speffect(SP_FIRST_HIT, false);
        }
        // Daño recibido: Espinas y Juramento.
        if php < fx.last_player_hp && php > 0 {
            fx.took_damage = true;
            fx.last_hit_taken = Some(Instant::now());
            if espinas > 0.0 {
                let back = ((fx.last_player_hp - php) as f64 * espinas) as i32;
                bhp -= back;
                crate::set_chr_hp(ent, bhp);
                *fx.st_dmg.entry("espinas".into()).or_insert(0.0) += back as f64;
            }
        }
        // Motor de efectos (bendiciones con disparador y mutadores del boss).
        let mut motor_out = None;
        if let Some(m) = fx.motor.as_mut() {
            if !m.filled() && fx.since.elapsed() > Duration::from_millis(800) {
                // Vidas máximas nuevas (Goliat, mutadores...): llenar las dos una vez.
                m.llenar();
                php = crate::player_hp_max().map(|h| h.0).unwrap_or(php);
                bhp = crate::chr_hp(ent).unwrap_or(bhp);
                fx.last_player_hp = php;
            } else {
                let o = m.tick(&crate::motor::Entrada {
                    ent,
                    hit: hit_dmg,
                    otro: (baja - hit_dmg).max(0),
                    taken: fx.last_player_hp - php,
                    deflect: std::mem::take(&mut fx.ev_deflect),
                    php,
                    pmax,
                    bhp,
                    bmax,
                });
                bhp = o.bhp;
                php = o.php;
                motor_out = Some(o);
            }
        }
        // Lo que la DLL le quita al boss no cuenta como golpe en el fotograma siguiente.
        fx.last_boss_hp = bhp;
        fx.last_player_hp = php;
        // Monk: cada golpe al boss cura un 3 % de la vida máxima.
        fx.last_tick = Instant::now();
        if is_monje {
            if !crate::chr_speffects(0).contains(&SP_MONJE) {
                crate::player_speffect(SP_MONJE, true);
            }
            if hit_boss && php > 0 && php < pmax {
                let pacto_mult = if (php as f64) < pmax as f64 * 0.5 { pacto } else { 1.0 };
                let n = ((pmax as f64 * 0.03 * regen_mult * pacto_mult) as i32).min(pmax - php);
                crate::heal_player_by(n);
                *fx.st_heal.entry("pasiva".into()).or_insert(0.0) += n as f64;
                fx.last_player_hp = php + n;
            }
        }
        // Venganza: por debajo del 30 % de vida.
        if venganza {
            let low = (php as f64) < pmax as f64 * 0.3;
            if low != fx.vengeance_on {
                fx.vengeance_on = low;
                crate::player_speffect(SP_VENGEANCE, low);
            }
        }
        if is_tanque && !crate::chr_speffects(0).contains(&SP_IMPARABLE) {
            crate::player_speffect(SP_IMPARABLE, true);
        }
        // Samurai: desvío perfecto con cualquier arma. El desvío no gasta aguante (tools/ModBuilder:
        // guardStaminaMult 0 en la ventana 102001/11/13/15).
        if is_samurai {
            // Guardia más ágil: las animaciones de guardia (familia x035xxx) van un 50 % más rápidas,
            // para que cubrirse y desviar no deje clavado.
            let guarding = crate::player_anim().is_some_and(|a| (35_000..36_000).contains(&(a % 1_000_000)));
            let after_deflect = fx.deflect_fast_until.is_some_and(|t| Instant::now() < t);
            if after_deflect {
                crate::set_anim_speed(0, 1.8);
                fx.guard_fast = true;
            } else if guarding {
                crate::set_anim_speed(0, 1.5);
                fx.guard_fast = true;
            } else if fx.guard_fast {
                crate::set_anim_speed(0, 1.0);
                fx.guard_fast = false;
            }
            let efs = crate::chr_speffects(0);
            // El desvío (102000) y la marca se pierden al morir o con algunas cargas: se reponen si faltan.
            if !efs.contains(&SP_DEFLECT) {
                crate::player_speffect(SP_DEFLECT, true);
            }
            if !efs.contains(&SP_SAMURAI) {
                crate::player_speffect(SP_SAMURAI, true);
            }
            let now_deflect = efs.iter().any(|s| DEFLECT_MARKS.contains(s));
            if now_deflect && !fx.deflecting {
                // Como en Sekiro, cada desvío le quita postura al enemigo (la de la rotura con crítico).
                if let Some((antes, ahora, max)) = crate::quitar_postura(ent, DEFLECT_POISE) {
                    crate::log(&format!("samurái: postura del enemigo {antes:.0} -> {ahora:.0} / {max:.0}"));
                }
                // Tras desviar, recuperación ágil: 0,5 s a ×1,8.
                fx.deflect_fast_until = Some(Instant::now() + Duration::from_millis(500));
                fx.ev_deflect = true;
                crate::log("samurái: desvío");
            }
            fx.deflecting = now_deflect;
        }
        // Reloj de arena: cada 20 s, un 10 % de vida.
        if reloj > 0.0 && fx.last_clock.elapsed() >= Duration::from_secs(20) {
            fx.last_clock = Instant::now();
            crate::heal_player_by((pmax as f64 * reloj) as i32);
            *fx.st_heal.entry("reloj".into()).or_insert(0.0) += pmax as f64 * reloj;
            fx.last_player_hp = crate::player_hp_max().map(|h| h.0).unwrap_or(php);
        }
        if let Some(o) = motor_out {
            if o.runas > 0 {
                self.gain_runes(o.runas);
            }
            if let Some(run) = self.run.as_mut() {
                for k in o.run {
                    *run.contadores.entry(k).or_insert(0.0) += 1.0;
                }
            }
        }
        if hit_boss && let Some(run) = self.run.as_mut() {
            run.golpes += 1;
        }
    }

    /// Nombre legible de una fuente de daño o curación.
    pub(super) fn fuente_nombre(&self, k: &str) -> String {
        match k {
            "arma" => rogue_core::idioma::t("Your weapon").into(),
            "pasiva" => rogue_core::idioma::t("Passive").into(),
            "estados" => rogue_core::idioma::t("Ailments").into(),
            id => self.relics.get(id).map(|r| r.nombre.clone()).unwrap_or_else(|| id.to_string()),
        }
    }

    /// Historia de la run para el gráfico del final.
    pub(super) fn hist_view(&self, run: &RunState) -> Vec<HistView> {
        run.historia
            .iter()
            .map(|h| HistView {
                acto: h.acto,
                tipo: h.tipo.clone(),
                vida_min: h.vida_min,
                bendiciones: h.bendiciones.iter().filter_map(|id| self.relics.get(id)).map(|r| (r.icono, r.maldita)).collect(),
                objetos: h.objetos.clone(),
                detalle: h.combate.and_then(|i| run.combates.get(i)).map(|c| {
                    let tipo = match c.tipo.as_str() {
                        "Boss" => rogue_core::idioma::t("Lord of the act"),
                        "Elite" => rogue_core::idioma::t("Elite"),
                        _ => rogue_core::idioma::t("Fight"),
                    };
                    rogue_ui::grafico::Detalle {
                        enemigo: c.enemigo.clone(),
                        tipo: tipo.to_string(),
                        segundos: c.segundos,
                        recibido: c.recibido,
                        curado: c.curacion.iter().map(|x| x.1).sum(),
                        danio: c.danio.iter().map(|(k, v)| (self.fuente_nombre(k), *v)).collect(),
                    }
                }),
            })
            .collect()
    }

    /// Apunta en la run las estadísticas del combate (daño y curación por fuente) y escribe la run
    /// entera en runtime/runs/ para poder analizarla.
    pub(super) fn record_combat(&mut self, enc: &Encounter, won: bool) {
        let Some(fx) = self.fx.as_ref() else { return };
        let mut dmg = fx.st_dmg.clone();
        let mut heal = fx.st_heal.clone();
        if let Some(m) = fx.motor.as_ref() {
            for (k, v) in &m.stats_dmg {
                *dmg.entry(k.clone()).or_insert(0.0) += v;
            }
            for (k, v) in &m.stats_heal {
                *heal.entry(k.clone()).or_insert(0.0) += v;
            }
        }
        let sorted = |m: HashMap<String, f64>| {
            let mut v: Vec<(String, f64)> = m.into_iter().filter(|x| x.1 >= 1.0).map(|(k, v)| (k, v.round())).collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1));
            v
        };
        let vida = crate::player_hp_max().map(|(h, m)| h as f64 / m.max(1) as f64).unwrap_or(0.0);
        let st = rogue_core::run::CombatStats {
            enemigo: enc.name.clone(),
            tipo: format!("{:?}", enc.kind),
            acto: self.act() + 1,
            segundos: (fx.since.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            danio: sorted(dmg),
            curacion: sorted(heal),
            recibido: fx.st_taken.round(),
            vida_final: if won { (vida * 100.0).round() / 100.0 } else { 0.0 },
            golpes: fx.st_hits,
            ganado: won,
        };
        crate::log(&format!("combate: {} en {:.0} s; daño {:?}", st.enemigo, st.segundos, st.danio.iter().take(4).collect::<Vec<_>>()));
        if let Some(run) = self.run.as_mut() {
            run.combates.push(st);
            let idx = run.combates.len() - 1;
            if let Some(h) = run.historia.last_mut() {
                h.combate = Some(idx);
                h.vida_min = if won { (fx.st_min * 100.0).round() / 100.0 } else { 0.0 };
            }
            let dir = self.root.join("runtime").join("runs");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(format!("{}.json", run.seed)), run.to_json());
        }
        let vista = self
            .run
            .as_ref()
            .and_then(|r| r.combates.last())
            .map(|c| (c.segundos, c.danio.iter().map(|(k, v)| (self.fuente_nombre(k), *v)).collect::<Vec<_>>()));
        if let Ok(mut u) = LAST_COMBAT.lock() {
            *u = vista;
        }
    }

    /// Al ganar: efectos de victoria del motor (contadores de la run y runas).
    pub(super) fn motor_victoria(&mut self) {
        let Some(o) = self.fx.as_mut().and_then(|f| f.motor.as_mut()).map(|m| m.victoria()) else { return };
        if o.runas > 0 {
            self.gain_runes(o.runas);
        }
        if let Some(run) = self.run.as_mut() {
            for k in o.run {
                *run.contadores.entry(k).or_insert(0.0) += 1.0;
            }
        }
        // Misión: la espátula (800 golpes en la run): tres bendiciones raras, una vez.
        let mision =
            self.run.as_ref().is_some_and(|r| r.relics.iter().any(|o| o.id == "espatula") && r.golpes >= 800 && !r.contadores.contains_key("espatula_hecha"));
        if mision {
            if let Some(run) = self.run.as_mut() {
                run.contadores.insert("espatula_hecha".into(), 1.0);
            }
            let pj = self.run.as_ref().and_then(|r| r.personaje.clone());
            for _ in 0..3 {
                let pick = self.run.as_mut().and_then(|run| self.relics.roll_any(&run.relics, Some(Rarity::Rara), &mut run.rng_rewards, pj.as_deref()));
                if let Some(nr) = pick {
                    self.gain_relic(&nr);
                }
            }
            crate::log("misión de la espátula cumplida");
        }
    }

    /// Oleada recién encendida: la segunda fase, donde estaba la primera, y la animación de cada uno
    /// (la de transformación de su evento original).
    pub(super) fn entrada_de_fase(&mut self, enc: &Encounter) {
        let lista = enc.lista();
        let indices = enc.grupos().get(self.oleada).cloned().unwrap_or_default();
        let ents = self.de_oleada(enc);
        let segunda_fase = self.pos_fase.is_some();
        if let (Some(p), Some(&e)) = (self.pos_fase.take(), ents.first()) {
            crate::set_chr_pos(e, p);
        }
        // La segunda fase se forma invisible, congelada e intocable entre ráfagas doradas; aparece en
        // revelar_fase.
        if segunda_fase {
            for &e in &ents {
                crate::set_chr_invincible(e, true);
                crate::set_chr_render(e, false);
                crate::set_anim_speed(e, CONGELADO);
                crate::apply_speffect_to(e, SP_TRANSICION);
            }
            self.rafaga = Some(Instant::now());
            self.fase2_oculta = Some(Instant::now() + FASE2_OCULTA);
            return;
        }
        for (k, &e) in ents.iter().enumerate() {
            let x = indices.get(k).and_then(|&i| lista.get(i));
            if let Some(sp) = x.and_then(|x| x.efecto) {
                crate::apply_speffect_to(e, sp);
            }
            if let Some(a) = x.and_then(|x| x.anim) {
                crate::log(&format!("coliseo: animación de fase {a}: {}", crate::chr_request_anim(e, a)));
            }
        }
    }

    /// La segunda fase aparece: visible, a su velocidad, con su animación de transformación (la de su
    /// evento original) e intocable mientras la hace, sin ir ya a por el jugador.
    pub(super) fn revelar_fase(&mut self, enc: &Encounter) {
        self.fase2_oculta = None;
        let lista = enc.lista();
        let indices = enc.grupos().get(self.oleada).cloned().unwrap_or_default();
        for (k, &e) in self.de_oleada(enc).iter().enumerate() {
            crate::set_chr_render(e, true);
            crate::set_anim_speed(e, 1.0);
            crate::remove_speffect_from(e, SP_TRANSICION);
            crate::apply_speffect_to(e, SP_TRANSICION);
            let x = indices.get(k).and_then(|&i| lista.get(i));
            if let Some(sp) = x.and_then(|x| x.efecto) {
                crate::apply_speffect_to(e, sp);
                crate::log(&format!("coliseo: efecto de fase {sp}"));
            }
            if let Some(a) = x.and_then(|x| x.anim) {
                crate::log(&format!("coliseo: animación de fase {a}: {}", crate::chr_request_anim(e, a)));
            }
        }
        self.fase2_intocable = Some(Instant::now() + ENTRADA_FASE2);
        crate::log(&format!("coliseo: {} aparece en su segunda fase", enc.name));
    }

    /// Segunda fase de un jefe de un solo personaje (`renacer`, Malenia): al bajar del umbral se queda
    /// intocable, recibe los efectos de su evento original y hace su animación; al acabar, vida llena.
    pub(super) fn renacer_tick(&mut self, enc: &Encounter, ent: u32) {
        let Some(r) = enc.renacer.as_ref() else { return };
        match self.renacido {
            None => {
                let Some((h, m)) = crate::chr_hp_max(ent) else { return };
                if h <= 0 || (h as f64) > r.vida * m.max(1) as f64 {
                    return;
                }
                self.renacido = Some(Instant::now());
                crate::set_chr_invincible(ent, true);
                for &sp in &r.quitar {
                    crate::remove_speffect_from(ent, sp);
                }
                for &sp in &r.efectos {
                    crate::apply_speffect_to(ent, sp);
                }
                if let Some(a) = r.anim {
                    crate::chr_request_anim(ent, a);
                }
                crate::log(&format!("coliseo: {} pasa a su segunda fase", enc.name));
            }
            Some(t) if t.elapsed().as_secs_f64() >= r.segundos.max(1.0) && !self.renacer_cerrado => {
                self.renacer_cerrado = true;
                crate::fill_hp(ent);
                crate::set_chr_invincible(ent, false);
            }
            _ => {}
        }
    }

    pub(super) fn end_fx(&mut self) {
        self.parar_musica();
        if let Some(m) = self.fx.as_mut().and_then(|f| f.motor.as_mut()) {
            m.fin();
            let golpes = m.ultimos_golpes();
            if !golpes.is_empty()
                && let Some(seed) = self.run.as_ref().map(|r| r.seed.clone())
            {
                self.golpes_run = (seed, golpes);
            }
        }
        for sp in [SP_DMG_DYNAMIC, SP_FIRST_HIT, SP_VENGEANCE, SP_DEFENSE_DYNAMIC] {
            crate::player_speffect(sp, false);
        }
        self.acto_prueba = None;
    }
}

impl Director {
    /// Mutador de la ascensión para un tipo de combate: uno por run y tipo, sacado de la semilla, el mismo
    /// en todos los combates de ese tipo (la pantalla de la ascensión lo enseña; 23-10).
    pub(super) fn mutador_ascension(&self, run: &RunState, kind: EncounterKind, ultimo_acto: bool) -> Option<crate::motor::Mutador> {
        let asc = run.ascension;
        let (clave, nivel) = match kind {
            EncounterKind::Elite if asc >= 8 => ("elite", "oro"),
            EncounterKind::Elite if asc >= 4 => ("elite", "plata"),
            EncounterKind::Boss if asc >= 10 && ultimo_acto => ("final", "prismatico"),
            EncounterKind::Boss if asc >= 8 => ("jefe", "oro"),
            EncounterKind::Weak | EncounterKind::Normal if asc >= 10 => ("resto", "plata"),
            _ => return None,
        };
        let cands: Vec<&crate::motor::Mutador> = self.mutadores.iter().filter(|m| m.nivel == nivel && !run.mutadores.contains(&m.id)).collect();
        let h = format!("{}{clave}", run.seed).bytes().fold(2_166_136_261u32, |h, b| (h ^ b as u32).wrapping_mul(16_777_619));
        (!cands.is_empty()).then(|| cands[h as usize % cands.len()].clone())
    }

    /// Pantalla de la ascensión (pausa): cada nivel con lo que hace, los mutadores de esta run y el total.
    pub(super) fn texto_ascension(&self) -> String {
        use rogue_core::idioma::{t, tf};
        let Some(run) = &self.run else { return String::new() };
        let asc = run.ascension;
        let mut l: Vec<String> = (1..=asc as usize).map(|k| format!("{k}. {}", t(rogue_core::perfil::ASCENSIONES[k - 1]))).collect();
        let (vida, danio) = rogue_core::perfil::factores_ascension(asc);
        l.push(String::new());
        l.push(tf("In total, enemies have {}% more HP and deal {}% more damage.", &[&((vida - 1.0) * 100.0).round(), &((danio - 1.0) * 100.0).round()]));
        if run.coop {
            l.push(tf("Co-op: enemies have {}% more HP for each player after the first.", &[&(self.coop_extra * 100.0).round()]));
        }
        let tipos: [(&'static str, EncounterKind, bool); 4] = [
            ("Elites: {}. {}", EncounterKind::Elite, false),
            ("Act bosses: {}. {}", EncounterKind::Boss, false),
            ("Final boss: {}. {}", EncounterKind::Boss, true),
            ("Other fights: {}. {}", EncounterKind::Normal, false),
        ];
        let muts: Vec<String> = tipos.iter().filter_map(|(f, k, fin)| self.mutador_ascension(run, *k, *fin).map(|m| tf(f, &[&m.nombre, &m.texto]))).collect();
        if !muts.is_empty() {
            l.push(String::new());
            l.push(t("Mutators in this run").to_string());
            l.extend(muts);
        }
        // Párrafos (separados por una línea en blanco): los niveles, el total y los mutadores; dentro, una
        // línea por cosa (evento.rs las pinta como lista).
        l.split(|x| x.is_empty()).map(|p| p.join("\n")).collect::<Vec<_>>().join("\n\n")
    }
}

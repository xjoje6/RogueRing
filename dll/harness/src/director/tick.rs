//! Lo que el director hace en cada fotograma: viajes, combates, pantallas y su estado.

use super::*;

/// Llamado en cada fotograma desde la tarea del juego.
pub fn tick() {
    let Ok(mut guard) = DIRECTOR.try_lock() else { return };
    let Some(d) = guard.as_mut() else { return };
    crate::clear_bloodstain();
    d.coop_tick();
    d.caido_tick();
    if d.char_pendiente.is_some()
        && jugador_listo()
        && let Some(c) = d.char_pendiente.take().and_then(|i| d.chars.get(i).cloned())
    {
        let m = d.aplicar_personaje(&c);
        crate::log(&format!("personaje aplicado tras la carga: {m}"));
    }
    // Ganado el combate, nada te mata hasta volver (19-10: la nube del Avatar putrefacto le mató
    // tras ganar; el juego lo reapareció muerto y el siguiente viaje se quedó colgado). Y fuera los
    // estados, una vez por segundo, por si algo los vuelve a poner.
    if matches!(d.phase, Phase::Won { .. } | Phase::Returning { .. }) {
        crate::set_player_invincible(true);
        if d.curado_victoria.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
            d.curado_victoria = Some(Instant::now());
            curar_estados();
        }
    }
    match &d.phase {
        Phase::Arena { enc, since, llegada, ent, spawned_at, ready } => {
            let (enc, since, llegada, ent, spawned_at, ready) = (enc.clone(), *since, *llegada, *ent, *spawned_at, *ready);
            let alive = player_hp().unwrap_or(0) > 0;
            // Cooperativo, invitado: el anfitrión cierra el combate (protocolo.rs), también si este juego
            // sigue en la carga o en la llegada.
            if !crate::coop::soy_anfitrion()
                && let rogue_core::protocolo::Accion::Terminar(r) = d.protocolo(false, 0.0)
            {
                d.cerrar_combate(&enc, r);
                return;
            }
            if ent == 0 {
                // Colocados en el MSB: existen y actúan desde que carga el mapa. Terminada la carga se
                // apunta dónde están y se apagan hasta su oleada (apagarlos mientras carga la deja
                // colgada); al encenderlos vuelven a su sitio (grupo.rs). Apuntarlo antes no vale: en los
                // mapas grandes el juego mueve el origen de las coordenadas al acabar de colocar al
                // jugador, y lo apuntado durante la carga mandaba a los jefes a 1000 m de altura en la
                // arena de Placidusax (18-10: Heolstor y la Bestia con vida pero sin verse).
                if d.jefe_msb && llegada.is_some() {
                    for i in 0..HUECOS_MSB {
                        let e = JEFE_MSB + i;
                        if !d.origenes.contains_key(&e)
                            && let Some(p) = crate::chr_pos(e)
                        {
                            d.origenes.insert(e, p);
                            crate::log(&format!("coliseo: {e} apuntado en {p:?} (jugador en {:?})", crate::player_pos()));
                        }
                        crate::set_chr_disabled(e, true);
                    }
                }
                // Viaje que no llega (19-10: se quedó en la barra tras morir con el combate ya
                // ganado): a los 30 s se repite una vez. El invitado no viaja: lo lleva Seamless.
                if llegada.is_none() && since.elapsed() > Duration::from_secs(30) && !d.viaje_repetido && !crate::coop::invitado_en_sala() {
                    d.viaje_repetido = true;
                    crate::log(&format!("coliseo: {} sin llegada en 30 s; se repite el viaje", enc.name));
                    flag_set(d.flag_combate, true);
                }
                // Esperar a estar en el coliseo con la carga terminada y, un poco después, crear el boss.
                if llegada.is_none()
                    && alive
                    && since.elapsed() > Duration::from_secs(2)
                    && crate::player_area() == Some(d.area_combate)
                    && crate::screen_fade() < 0.05
                {
                    d.phase = Phase::Arena { enc, since, llegada: Some(Instant::now()), ent, spawned_at, ready };
                } else if alive && llegada.is_some_and(|t| t.elapsed() > ESPERA_RIVAL) && since.elapsed() > TELON_MINIMO {
                    // Los enemigos colocados en la variante del coliseo los enciende el evento del mapa
                    // (flag de la oleada); sin variante, se crean con spawn_chr (grupo.rs).
                    let e = d.preparar_grupo(&enc);
                    let como = if d.jefe_msb { "encendido" } else { "creado" };
                    crate::log(&format!("coliseo: {} {como} como {e} ({} enemigos)", enc.name, d.grupo.len()));
                    d.phase = Phase::Arena { enc, since, llegada, ent: e, spawned_at: Some(Instant::now()), ready: false };
                }
            } else if !ready {
                d.apagar_pendientes(&enc);
                // Quietos mientras carga (ARRANQUE).
                for e in d.de_oleada(&enc) {
                    crate::set_anim_speed(e, 0.0);
                }
                // Cuando la primera oleada existe: escalado del acto, vida llena y reliquias.
                if d.oleada_cargada(&enc) {
                    let tier = d.tramo(enc.kind);
                    // En cooperativo, todos en su copia del rival: el golpe que recibe cada uno se calcula
                    // en su juego con los datos de su copia, y su vida máxima es la que se ve en la barra
                    // (sin tramo, al invitado no le bajaba la barra: la vida que manda el anfitrión pasa
                    // de su máximo; 16-10). El mismo efecto dos veces no se suma. La vida que queda
                    // la manda el anfitrión.
                    crate::apply_speffect_to(ent, tier);
                    heal_player();
                    d.apply_relics();
                    // Y el maná lleno en cada combate, para todos (24-10: al invitado se lo recargaba el
                    // viaje de Seamless y al anfitrión no; «así puedes usar más las cenizas de guerra»).
                    crate::refill_player_stamina_fp();
                    d.start_fx(&enc);
                    if let Some(m) = d.fx.as_mut().and_then(|f| f.motor.as_mut()) {
                        m.inicio(ent);
                    }
                    d.escalar_oleada(&enc, ent);
                    d.ready_at = Some(Instant::now());
                    d.vigia = None;
                    d.commence_shown = false;
                    d.clear_since = None;
                    d.visible = false;
                    d.visible_desde = None;
                    crate::log(&format!("coliseo: {} listo (tramo {tier})", enc.name));
                    if crate::seamless_cargado() {
                        crate::log(&format!("cooperativo: jefe {ent} es {}, sesión {:?}", crate::chr_handle(ent).unwrap_or_default(), crate::sesion()));
                    }
                    d.empezar_musica(&enc);
                    d.phase = Phase::Arena { enc, since, llegada, ent, spawned_at, ready: true };
                } else if spawned_at.is_some_and(|t| t.elapsed() > Duration::from_secs(15)) {
                    crate::log(&format!("coliseo: {} no aparece; se da por ganado", enc.name));
                    d.phase = Phase::Won { enc, at: Instant::now() };
                }
            } else {
                use rogue_core::run::Formato;
                // Vida al máximo en los primeros segundos de cada oleada (el escalado cambia su máximo),
                // con el escalado comprobado antes: puesto justo al encenderlos, a veces no queda (el jugador,
                // 16-10: unos Exiliados con su vida base, 134 en vez de 806).
                if d.oleada_desde.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) {
                    d.asegurar_escalado(&enc);
                    for e in d.de_oleada(&enc) {
                        crate::fill_hp(e);
                    }
                }
                d.apagar_pendientes(&enc);
                d.mejorar_invocados();
                d.vigilar_invocacion();
                let vivos = d.vivos(&enc);
                let olas = enc.grupos().len();
                // Escolta: al caer el líder, cae su gente. Aguante: se gana al pasar el tiempo.
                let lider = (enc.formato == Formato::Escolta).then(|| d.grupo.get(enc.lider.unwrap_or(0)).copied()).flatten();
                let lider_caido = lider.is_some_and(|l| d.muertos.contains(&l));
                let segundos = Duration::from_secs(enc.segundos.unwrap_or(60) as u64);
                let aguantado = enc.formato == Formato::Aguante && d.ready_at.is_some_and(|t| t.elapsed() >= segundos);
                let vacia = d.oleada_lista && vivos.is_empty();
                // Cooperativo (protocolo.rs): el invitado no decide con sus copias de los rivales; las
                // oleadas las enciende cuando lo dice el anfitrión.
                let invitado = d.protocolo(false, 0.0) != rogue_core::protocolo::Accion::Solo && !crate::coop::soy_anfitrion();
                let (vacia, lider_caido, aguantado) = (vacia && !invitado, lider_caido && !invitado, aguantado && !invitado);
                if invitado && let rogue_core::protocolo::Accion::Oleada(n) = d.protocolo(false, 0.0) {
                    d.oleada = n;
                    d.oleada_lista = false;
                    d.vigia = None;
                    d.encender_oleada(&enc, n);
                    let e = d.de_oleada(&enc).first().copied().unwrap_or(ent);
                    crate::log(&format!("cooperativo: oleada {n} (la ha encendido el anfitrión)"));
                    d.phase = Phase::Arena { enc, since, llegada, ent: e, spawned_at, ready };
                    return;
                }
                // Jefe de dos fases con un personaje por fase: la segunda sale cuando la primera baja de
                // `fase_en` (como el evento de su mapa), no al morir.
                // También si la primera fase cae de un golpe por debajo del umbral (18-10: Godfrey de
                // un primer corte x27): la transición se hace igual, sin dar el combate por ganado.
                let fase_pendiente = enc.fase_en.is_some() && d.oleada_lista && d.oleada + 1 < olas;
                let cambio_fase = fase_pendiente
                    && (vacia
                        || enc.fase_en.is_some_and(|r| {
                            d.de_oleada(&enc).first().and_then(|&e| crate::chr_hp_max(e)).is_some_and(|(h, m)| h > 0 && (h as f64) <= r * m.max(1) as f64)
                        }));
                // Primero la transición: la primera fase se queda intocable, lenta y en llamas unos
                // segundos, con el aviso de que no ha caído; después se va y sale la segunda.
                let mut cambio_listo = false;
                if cambio_fase {
                    match d.transicion {
                        None => {
                            d.transicion = Some(Instant::now());
                            d.rafaga = Some(Instant::now());
                            for e in d.de_oleada(&enc) {
                                crate::set_chr_invincible(e, true);
                                crate::set_anim_speed(e, CONGELADO);
                                crate::apply_speffect_to(e, SP_TRANSICION);
                            }
                            crate::aviso_fase();
                            crate::log(&format!("coliseo: {} empieza a cambiar de fase", enc.name));
                        }
                        Some(t) if t.elapsed() >= TRANSICION_FASE => {
                            d.transicion = None;
                            for e in d.de_oleada(&enc) {
                                if d.pos_fase.is_none() {
                                    d.pos_fase = crate::chr_pos(e);
                                }
                                crate::set_chr_disabled(e, true);
                                d.muertos.insert(e);
                            }
                            cambio_listo = true;
                            crate::log(&format!("coliseo: {} cambia de fase", enc.name));
                        }
                        Some(_) => {
                            if let Some(v) = d.vigia.as_mut() {
                                v.desde = Instant::now();
                            }
                            // Otra ráfaga dorada: quitar y volver a poner el efecto lo hace salir de nuevo.
                            if d.rafaga.is_none_or(|t| t.elapsed() >= RAFAGA_FASE) {
                                d.rafaga = Some(Instant::now());
                                for e in d.de_oleada(&enc) {
                                    crate::remove_speffect_from(e, SP_TRANSICION);
                                    crate::apply_speffect_to(e, SP_TRANSICION);
                                }
                            }
                        }
                    }
                }
                let avanza = if fase_pendiente { cambio_listo } else { vacia };
                if avanza && d.oleada + 1 < olas && !lider_caido {
                    d.oleada += 1;
                    d.oleada_lista = false;
                    d.vigia = None;
                    let w = d.oleada;
                    d.encender_oleada(&enc, w);
                    let e = d.de_oleada(&enc).first().copied().unwrap_or(ent);
                    d.phase = Phase::Arena { enc, since, llegada, ent: e, spawned_at, ready };
                } else if let Some(r) = {
                    let gana = lider_caido || aguantado || (vacia && !fase_pendiente) || std::mem::take(&mut d.forzar_victoria);
                    let en_arena = llegada.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                    match d.protocolo(gana, en_arena) {
                        rogue_core::protocolo::Accion::Solo => gana.then_some(rogue_core::protocolo::Resultado::Victoria),
                        rogue_core::protocolo::Accion::Terminar(r) => Some(r),
                        _ => None,
                    }
                } && (r == rogue_core::protocolo::Resultado::Derrota || crate::coop::soy_anfitrion() || !d.coop_activo())
                {
                    if r == rogue_core::protocolo::Resultado::Derrota {
                        d.cerrar_combate(&enc, r);
                        return;
                    }
                    for e in d.grupo.clone() {
                        if crate::chr_hp(e).is_some_and(|h| h > 0) {
                            crate::set_chr_hp(e, 0);
                        }
                    }
                    // Racha: vale doble. Contrarreloj: una bendición más si cae a tiempo.
                    if enc.formato == Formato::Racha {
                        d.premio_extra += 1;
                    }
                    if enc.segundos.is_some() && enc.formato != Formato::Aguante && d.ready_at.is_some_and(|t| t.elapsed() <= segundos) {
                        d.premio_extra += 1;
                        crate::log(&format!("coliseo: {} a tiempo; una bendición más", enc.name));
                    }
                    // Letrero del juego: jefe final, jefe de acto o enemigo.
                    let last_act = d.run.as_ref().is_some_and(|r| r.act_index + 1 >= d.acts.len());
                    let msg = match enc.kind {
                        EncounterKind::Boss if last_act => eldenring::cs::STATUS_MESSAGE_DEMIGOD_FELLED,
                        EncounterKind::Boss => eldenring::cs::STATUS_MESSAGE_GREAT_ENEMY_FELLED,
                        _ => eldenring::cs::STATUS_MESSAGE_ENEMY_FELLED,
                    };
                    crate::status_message(msg);
                    d.motor_victoria();
                    d.record_combat(&enc, true);
                    d.end_fx();
                    crate::log(&format!("coliseo: {} derrotado", enc.name));
                    d.anotar_resultado(rogue_core::protocolo::Resultado::Victoria);
                    d.revivir(0);
                    let v = d.caido_cfg_vida();
                    d.levantarse(Some(v), "combate ganado");
                    d.phase = Phase::Won { enc, at: Instant::now() };
                } else if !alive && !d.coop_activo() {
                    d.end_fx();
                    d.record_combat(&enc, false);
                    d.fx = None;
                    crate::log(&format!("run terminada: muerte contra {}", enc.name));
                    d.phase = Phase::Over { won: false, new_run: false, shown: None };
                    d.sel = 0;
                    d.save();
                } else if !d.arranque_suelto && d.oleada == 0 && (d.visible_desde.is_none_or(|t| t.elapsed() < ARRANQUE) || d.esperando_llegada()) {
                    // Telón y ARRANQUE: la primera oleada quieta donde apareció, e intocable mientras (si no,
                    // se le pegaba gratis; 17-10). En cooperativo, el anfitrión además espera a que
                    // hayan llegado todos (coop.rs, esperando_llegada).
                    for e in &vivos {
                        crate::set_anim_speed(*e, 0.0);
                        crate::set_chr_invincible(*e, true);
                    }
                    d.rivales_intocables = true;
                    if let Some(v) = d.vigia.as_mut() {
                        v.desde = Instant::now();
                    }
                } else if d.pausa.is_some() && !d.coop_activo() {
                    // En cooperativo la pausa no congela nada, como cualquier juego en línea (17-10).
                    for e in &vivos {
                        crate::set_anim_speed(*e, 0.0);
                        crate::set_chr_invincible(*e, true);
                    }
                    d.rivales_intocables = true;
                    crate::set_player_invincible(true);
                    if let Some(fx) = d.fx.as_mut() {
                        fx.pausa_desde.get_or_insert_with(Instant::now);
                        if let Some(m) = fx.motor.as_mut() {
                            m.pausar();
                        }
                    }
                    // En pausa no cuenta como parado.
                    if let Some(v) = d.vigia.as_mut() {
                        v.desde = Instant::now();
                    }
                } else {
                    // Ya se mueven: se les puede pegar (los quietos del telón o de la pausa).
                    if d.rivales_intocables {
                        d.rivales_intocables = false;
                        for e in &vivos {
                            crate::set_chr_invincible(*e, false);
                        }
                    }
                    if !d.arranque_suelto {
                        d.arranque_suelto = true;
                        for e in &vivos {
                            crate::set_anim_speed(*e, 1.0);
                        }
                        crate::log(&format!("coliseo: {} empieza", enc.name));
                        if let Some(a) = enc.anim_inicio {
                            crate::log(&format!("coliseo: animación de entrada {a}: {}", crate::chr_request_anim(ent, a)));
                        }
                    }
                    // Oleada nueva: escalado cuando existe.
                    if !d.oleada_lista && d.oleada_cargada(&enc) {
                        d.escalar_oleada(&enc, 0);
                        d.entrada_de_fase(&enc);
                    }
                    // Segunda fase formándose: ráfagas doradas en su sitio y, al acabar, aparece.
                    if let Some(t) = d.fase2_oculta {
                        if Instant::now() >= t {
                            d.revelar_fase(&enc);
                        } else {
                            if let Some(v) = d.vigia.as_mut() {
                                v.desde = Instant::now();
                            }
                            if d.rafaga.is_none_or(|r| r.elapsed() >= RAFAGA_FASE) {
                                d.rafaga = Some(Instant::now());
                                for e in d.de_oleada(&enc) {
                                    crate::remove_speffect_from(e, SP_TRANSICION);
                                    crate::apply_speffect_to(e, SP_TRANSICION);
                                }
                            }
                        }
                    }
                    if d.fase2_intocable.is_some_and(|t| Instant::now() >= t) {
                        d.fase2_intocable = None;
                        for e in d.de_oleada(&enc) {
                            crate::set_chr_invincible(e, false);
                            crate::remove_speffect_from(e, SP_TRANSICION);
                        }
                    }
                    d.renacer_tick(&enc, ent);
                    d.ultimo_tick(&enc, &vivos);
                    if enc.formato == Formato::Aguante {
                        for e in &vivos {
                            crate::fill_hp(*e);
                        }
                    }
                    let obj = d.objetivo(ent, &vivos);
                    if obj != ent {
                        d.phase = Phase::Arena { enc: enc.clone(), since, llegada, ent: obj, spawned_at, ready };
                    }
                    d.vigilar(obj);
                    d.pisadas(obj);
                    // Cooperativo: vida del jefe cada 5 s en el registro, para comparar los de los dos juegos.
                    if crate::seamless_cargado() && d.diag_at.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
                        d.diag_at = Some(Instant::now());
                        if let Some((hp, max)) = crate::chr_hp_max(obj) {
                            crate::log(&format!("cooperativo: vida del jefe {hp}/{max}, sesión {:?}", crate::sesion()));
                        }
                    }
                    // Invitado: cuántas copias de la oleada no llevan el tramo (lo que hay que compensarle).
                    if d.fx.as_ref().and_then(|f| f.motor.as_ref()).is_some_and(|m| m.invitado.is_some()) {
                        let tier = d.tramo(enc.kind);
                        let vivos_ola: Vec<u32> = d.de_oleada(&enc).into_iter().filter(|e| crate::chr_hp(*e).is_some_and(|h| h > 0)).collect();
                        let sin = vivos_ola.iter().filter(|e| !crate::chr_speffects(**e).contains(&tier)).count();
                        let falta = if vivos_ola.is_empty() { 1.0 } else { sin as f64 / vivos_ola.len() as f64 };
                        if let Some(m) = d.fx.as_mut().and_then(|f| f.motor.as_mut()) {
                            if (m.invitado_falta - falta).abs() > 0.01 {
                                crate::log(&format!("cooperativo: invitado, {sin} de {} sin el tramo en este juego", vivos_ola.len()));
                            }
                            m.invitado_falta = falta;
                        }
                    }
                    d.tick_fx(obj, enc.kind);
                    if d.barra_grupo(&enc, obj).is_some_and(|(_, hp, max)| hp * 2 < max) {
                        d.subir_musica();
                    }
                }
            }
        }
        Phase::Combat { enc, since, healed, tries, last_try } => {
            let (enc, since, healed, tries, last_try) = (enc.clone(), *since, *healed, *tries, *last_try);
            if flag_get(enc.defeat_flag) {
                d.phase = Phase::Won { enc, at: Instant::now() };
            } else if player_hp() == Some(0) && since.elapsed() > Duration::from_secs(3) {
                crate::log(&format!("run terminada: muerte contra {}", enc.name));
                d.phase = Phase::Over { won: false, new_run: false, shown: None };
                d.sel = 0;
                d.save();
            } else if !healed && since.elapsed() > Duration::from_secs(3) && player_hp().unwrap_or(0) > 0 {
                // Esperar a que el boss exista (carga terminada) antes de preparar la llegada.
                let boss_ent: u32 = enc.id.parse().unwrap_or(0);
                if !crate::chr_loaded(boss_ent) {
                    return;
                }
                // Escalado del acto (una vez por combate).
                if tries == 0 && last_try.is_none() {
                    let tier = d.tramo(enc.kind);
                    let ok = crate::apply_speffect_to(boss_ent, tier);
                    crate::log(&format!("escalado de {}: tramo {tier} ({})", enc.name, if ok { "ok" } else { "sin boss" }));
                    d.phase = Phase::Combat { enc, since, healed, tries, last_try: Some(Instant::now() - Duration::from_millis(1500)) };
                    return;
                }
                // Vida del boss al máximo tras el escalado.
                crate::fill_hp(boss_ent);
                let Some(t) = enc.llegada else {
                    crate::log(&format!("llegada junto a {}: directa", enc.name));
                    heal_player();
                    d.apply_relics();
                    d.phase = Phase::Combat { enc, since, healed: true, tries, last_try };
                    return;
                };
                // Tras cada salto, comprobar a los 2 s que sigue cerca (la carga puede devolverlo).
                if last_try.is_some_and(|l| l.elapsed() < Duration::from_secs(2)) {
                    return;
                }
                // Cerca en planta y sin haberse caído (más de 6 m por debajo del punto = sin suelo).
                let near = crate::player_block_pos().is_some_and(|p| (p[0] - t[0]).hypot(p[2] - t[2]) < 12.0 && p[1] > t[1] - 6.0);
                if near || tries >= 4 {
                    crate::log(&format!("llegada junto a {}: {} tras {tries} saltos", enc.name, if near { "ok" } else { "FALLÓ" }));
                    heal_player();
                    d.apply_relics();
                    crate::set_player_invincible(false);
                    d.phase = Phase::Combat { enc, since, healed: true, tries, last_try };
                } else {
                    // Invencible mientras se coloca (si cae, no muere y se reintenta), un poco más alto.
                    crate::set_player_invincible(true);
                    let salto = crate::teleport_local([t[0], t[1] + 1.0 * tries as f32, t[2]]);
                    crate::log(&format!("salto hacia {}: {salto:?}", enc.name));
                    d.phase = Phase::Combat { enc, since, healed, tries: tries + 1, last_try: Some(Instant::now()) };
                }
            }
        }
        Phase::Over { won, new_run: false, shown: None } if player_hp().unwrap_or(0) > 0 => {
            d.phase = Phase::Over { won: *won, new_run: false, shown: Some(Instant::now()) };
        }
        Phase::Over { new_run: true, .. } if player_hp().unwrap_or(0) > 0 => {
            // Run nueva preparada y vuelta al menú cuando el jugador vuelve a estar vivo (tras la muerte el
            // juego lo reaparece).
            let seed = semilla_nueva();
            d.limpiar_personaje();
            d.char_pendiente = None;
            // La de cooperativo se hace al elegir Nueva run (nodos.rs: semilla del anfitrión).
            d.run = Some(RunState::nueva(&seed, &d.acts, d.perfil.rivales_recientes()));
            // Al menú principal: desde ahí, otra run, los personajes o salir.
            d.guardada = None;
            d.phase = Phase::MainMenu;
            d.sel = d.menu_opciones().iter().position(|o| o.3 == Menu::Nueva).unwrap_or(0);
            d.recorded = false;
            d.over_sel = usize::MAX;
            crate::set_runes(0);
            crate::clear_bloodstain();
            d.viajar(HUB_WARP_FLAG);
            // La protección de la run perdida se quita sola en el centro (caido.rs): el invitado no viaja
            // solo y aquí aún puede estar en la arena.
            if !d.intocable_derrota {
                d.fin_gracia();
                crate::set_player_invincible(false);
            }
            heal_player();
            crate::log(&format!("nueva run {seed}"));
            d.save();
        }
        Phase::Won { enc, at } if at.elapsed() >= RETURN_DELAY => {
            // Dejar el boss "vivo" para futuras runs y volver al hub.
            let enc = enc.clone();
            flag_set(enc.defeat_flag, false);
            // Sin estados de este combate al volver (la podredumbre seguía en el siguiente).
            curar_estados();
            if d.coop_activo() && !crate::coop::soy_anfitrion() {
                let acto = d.run.as_ref().map(|r| r.act_index).unwrap_or(0);
                d.vuelta_guardada = Some((equip::guardar_viales(), player_hp(), acto, Instant::now()));
                crate::log(&format!("cooperativo: al volver, viales {:?} y vida {:?} (se guardan)", equip::hp_flasks(), player_hp()));
            }
            d.viajar(HUB_WARP_FLAG);
            d.phase = Phase::Returning { enc, at: Instant::now() };
        }
        Phase::Returning { enc, at } if at.elapsed() >= HUB_SETTLE => {
            let enc = enc.clone();
            // Por averiguar (docs/COOP_PROTOCOLO.md): cómo vuelve quien murió si el combate se gana.
            if d.coop_activo() {
                crate::log(&format!("cooperativo: de vuelta tras {}, vida {:?}", enc.name, crate::player_hp_max()));
                d.revivir(1);
            }
            crate::set_player_invincible(false);
            d.curado_victoria = None;
            let fx_end = d.fx.take();
            let clean = fx_end.as_ref().is_some_and(|fx| !fx.took_damage);
            let mut extra = vec![];
            // Como en StS, las élites dan una bendición. Juramento: otra si no te tocaron.
            let owned = d.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
            let bonus = (enc.kind == EncounterKind::Elite) as usize
                + (clean && d.relics.has(&owned, "juramento")) as usize
                + std::mem::take(&mut d.premio_extra);
            if enc.kind == EncounterKind::Elite {
                let forja = d.relics.has(&owned, "forja");
                if let Some(r) = d.run.as_mut() {
                    r.elites_won += 1;
                    if forja {
                        r.forja += 1;
                    }
                }
            }
            for _ in 0..bonus {
                let owned = d.run.as_ref().map(|r| r.relics.clone()).unwrap_or_default();
                let rar = match d.run.as_mut().map(|r| r.rng_rewards.range(0, 100)).unwrap_or(0) {
                    x if x < 50 => Rarity::Comun,
                    x if x < 85 => Rarity::PocoComun,
                    _ => Rarity::Rara,
                };
                let got = d.run.as_mut().and_then(|r| d.relics.roll(&owned, rar, &mut r.rng_rewards, r.personaje.as_deref()));
                if let Some(r) = got {
                    extra.push(r.nombre.clone());
                    d.gain_relic(&r);
                }
            }
            let extra_choices = d.run.as_ref().map(|r| d.relics.dll_sum(&r.relics, "opciones") as usize).unwrap_or(0);
            let acts = if d.run.as_ref().is_some_and(|r| r.coop) { &d.acts_coop } else { &d.acts };
            let acto_antes = d.run.as_ref().map(|r| r.act_index);
            let finished = d.run.as_mut().map(|r| r.complete(acts)).unwrap_or(Ok(false));
            if finished == Ok(false) && d.run.as_ref().map(|r| r.act_index) != acto_antes {
                d.recargar_viales = true;
            }
            // Acto nuevo: el arma sube al nivel del acto (conserva su ceniza).
            if let Some((m, nivel)) = d.reforge() {
                crate::log(&format!("arma: {m}"));
                extra.push(rogue_core::idioma::tf("Weapon upgraded to +{}", &[&nivel]));
            }
            d.phase = if finished == Ok(true) {
                crate::status_message(eldenring::cs::STATUS_MESSAGE_VICTORY);
                Phase::Over { won: true, new_run: false, shown: None }
            } else {
                let pool = d.offer_pool();
                let dobles = !d.character().is_some_and(|c| c.sin_dobles);
                let fuera = d.no_ofrecer();
                let run = d.run.as_mut().unwrap();
                let mut offer = rewards::generate_n(&pool, enc.kind, run.act_index, &mut run.rng_rewards, &fuera, 3 + extra_choices);
                rewards::roll_doubles(&mut offer.choices, &mut run.rng_rewards, dobles);
                // Una bendición entre las opciones (común o poco común; poco común o rara en élites; rara en jefes).
                let rar = match (enc.kind, run.rng_rewards.range(0, 100)) {
                    (EncounterKind::Boss, _) => Rarity::Rara,
                    (EncounterKind::Elite, x) if x < 60 => Rarity::PocoComun,
                    (EncounterKind::Elite, _) => Rarity::Rara,
                    (_, x) if x < 65 => Rarity::Comun,
                    _ => Rarity::PocoComun,
                };
                let mut owned = run.relics.clone();
                // No ofrecer la misma que se acaba de ganar por la élite o Juramento.
                for name in &extra {
                    if let Some(r) = d.relics.reliquias.iter().find(|r| &r.nombre == name) {
                        owned.push(OwnedRelic { id: r.id.clone(), nivel: 0 });
                    }
                }
                let pj = run.personaje.clone();
                // Élites y jefes pueden ofrecer una mítica en lugar de la bendición (pocas veces).
                let pct = match enc.kind {
                    EncounterKind::Boss => MITICA_JEFE,
                    EncounterKind::Elite => MITICA_ELITE,
                    _ => 0,
                };
                let mitica = run.rng_rewards.range(0, 100) < pct;
                let blessing = mitica
                    .then(|| d.relics.roll(&owned, Rarity::Mitica, &mut run.rng_rewards, pj.as_deref()))
                    .flatten()
                    .or_else(|| d.relics.roll(&owned, rar, &mut run.rng_rewards, pj.as_deref()));
                let mut cartas: Vec<Carta> = offer.choices.into_iter().map(Carta::Objeto).collect();
                // Si todos los objetos son de la misma categoría (personajes sin cenizas que encajen),
                // uno de ellos pasa a ser otra bendición.
                let todos_iguales = cartas.len() >= 3
                    && cartas.windows(2).all(|w| matches!((&w[0], &w[1]), (Carta::Objeto(a), Carta::Objeto(b)) if a.categoria == b.categoria));
                if let Some(b) = &blessing {
                    owned.push(OwnedRelic { id: b.id.clone(), nivel: 0 });
                }
                let segunda = if todos_iguales { d.relics.roll(&owned, rar, &mut run.rng_rewards, pj.as_deref()) } else { None };
                if let Some(s) = segunda {
                    cartas.pop();
                    cartas.push(Carta::Bendicion(s));
                }
                cartas.extend(blessing.map(Carta::Bendicion));
                run.rng_rewards.shuffle(&mut cartas);
                let runas = offer.runes;
                let got = d.gain_runes(runas);
                Phase::Reward { enemy: enc.name.clone(), runes: got, extra, cartas }
            };
            d.sel = 0;
            if !d.pending.is_empty() {
                let eff = std::mem::take(&mut d.pending);
                let back = std::mem::replace(&mut d.phase, Phase::Map);
                let msg = d.apply_effects(eff, back);
                crate::log(&format!("premio del evento: {msg}"));
            }
            d.save();
        }
        _ => {}
    }
    if matches!(d.phase, Phase::Personajes { .. }) {
        crate::mirar_a_camara();
    }
    // Avisos de objeto nuevo del vestuario: Y (forzada, pasa aunque haya pantalla del mod) mientras se
    // vea el aviso; con el aviso cerrado una Y de más haría otra cosa.
    // Se acaba a los 8 s o, pasados 3, en cuanto lleva 1,5 s sin verse ninguno (el arranque espera a
    // esto para quitar su capa).
    if let Some(hasta) = d.cerrar_avisos {
        static ULTIMA: Mutex<Option<Instant>> = Mutex::new(None);
        let Ok(mut u) = ULTIMA.lock() else { return };
        let empezado = hasta - Duration::from_secs(8);
        let quieto = empezado.elapsed() > Duration::from_secs(3) && u.is_none_or(|t| t.elapsed() > Duration::from_millis(1500));
        if Instant::now() > hasta || quieto {
            d.cerrar_avisos = None;
            *u = None;
        } else if crate::ui_visible(19) && u.is_none_or(|t| t.elapsed() > Duration::from_millis(700)) {
            pad::press_forzado(pad::button("Y").unwrap_or(0), 120);
            *u = Some(Instant::now());
        }
    }
    // HUD del juego: oculto en el menú del mod y en Personajes (salvo mientras se usa un objeto de
    // Seamless por detrás del menú: el juego lo usa como si se jugara).
    let ocultar = matches!(d.phase, Phase::MainMenu | Phase::Personajes { .. }) && d.sala.is_none() && !d.libre;
    if ocultar || d.hud_oculto {
        crate::hud(!ocultar);
        d.hud_oculto = ocultar;
    }
    // Acto nuevo: viales y vida llenos, como al descansar en una gracia, en cuanto se está en el centro y
    // sin fundido (16-10).
    if d.recargar_viales
        && crate::player_area() == Some(HUB_AREA)
        && crate::screen_fade() < 0.05
        && matches!(d.phase, Phase::Reward { .. } | Phase::Replace { .. } | Phase::Map)
    {
        d.recargar_viales = false;
        let ok = crate::rest_at_grace();
        crate::log(&format!("acto nuevo: viales recargados ({})", if ok { "ok" } else { "error" }));
    }
    // Marcas del personaje en combate: se pierden al cambiar de mapa y hasta el empiece del combate el
    // Monje esquivaba con la voltereta normal (17-10). Se vuelven a poner si faltan.
    if matches!(d.phase, Phase::Arena { .. } | Phase::Combat { .. }) && player_hp().is_some_and(|h| h > 0) {
        let tiene = crate::chr_speffects(0);
        if !tiene.is_empty() {
            for sp in d.marcas_personaje() {
                if !tiene.contains(&sp) {
                    crate::player_speffect(sp, true);
                }
            }
        }
    }
    // Tamaño fijo de las bendiciones (Diminuto, Goliat): fuera del motor de combate también, para que
    // esté desde que se llega a la arena y siga tras matar al rival (16-10). En combate lo pone
    // el motor (con los cambios de Científico loco).
    if d.fx.as_ref().and_then(|f| f.motor.as_ref()).is_none() {
        let s = d.tamano_pasivo() as f32;
        if (s - 1.0).abs() > 0.001 || (d.tamano_puesto - 1.0).abs() > 0.001 {
            crate::set_chr_scale(0, s);
            d.tamano_puesto = s;
        }
    }
    // Compensación del arma de fuera de estilo, una vez por segundo (cambia al equipar otra).
    if d.run.as_ref().is_some_and(|r| r.personaje.is_some()) && d.compensacion_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
        d.compensacion_at = Some(Instant::now());
        d.compensar_arma();
        // Fuera de las ranuras rápidas lo de Seamless (la Efigie con la que se unió el invitado).
        if d.sala.is_none() && matches!(d.phase, Phase::Map | Phase::Neow { .. }) {
            let n = crate::equip::quitar_seamless_de_rapidas();
            if n > 0 {
                crate::log(&format!("ranuras rápidas: {n} con un objeto de Seamless; puesto el vial"));
            }
        }
    }
    // Invitado: lo que Seamless le recarga al traerlo de vuelta se le devuelve como estaba al ganar,
    // mientras vuelve, elige recompensa o mira el mapa, en el mismo acto y unos segundos (una gracia o el
    // cambio de acto sí recargan).
    if let Some((g, hp, acto, t)) = d.vuelta_guardada.as_ref() {
        let sigue = t.elapsed() < Duration::from_secs(25)
            && d.run.as_ref().is_some_and(|r| r.act_index == *acto)
            && matches!(d.phase, Phase::Won { .. } | Phase::Returning { .. } | Phase::Reward { .. } | Phase::Replace { .. } | Phase::Map);
        if !sigue {
            d.vuelta_guardada = None;
        } else {
            let n = equip::devolver_viales(g);
            if n > 0 {
                crate::log(&format!("cooperativo: el viaje de vuelta recargó viales; devueltos, quedan {:?}", equip::hp_flasks()));
            }
            if let (Some(h), Some(ahora)) = (*hp, player_hp())
                && ahora > h
                && h > 0
            {
                crate::set_player_hp(h);
                crate::log(&format!("cooperativo: el viaje de vuelta llenó la vida ({ahora}); vuelve a {h}"));
            }
        }
    }
    // Viales que suben sin beber ni cambiar de acto: al registro (23-10: a su compañero se le
    // recargaban entre combates).
    // También en combate (si no, se comparaba con los de antes de pelear y no se veía nada).
    if !matches!(d.phase, Phase::MainMenu) && d.viales_vistos_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
        d.viales_vistos_at = Some(Instant::now());
        let ahora = equip::hp_flasks();
        if let (Some(a), Some(b)) = (d.viales_vistos, ahora)
            && b.0 > a.0
        {
            crate::log(&format!("viales: suben de {} a {} ({})", a.0, b.0, format!("{:?}", d.phase).split([' ', '{', '(']).next().unwrap_or("")));
        }
        d.viales_vistos = ahora;
    }
    if matches!(d.phase, Phase::Map | Phase::Rest) {
        d.viales = equip::hp_flasks();
        d.arma = d.arma_nivel();
    }
    // Qué pantalla se ve y si el personaje recibe el mando.
    let screen = match d.phase {
        _ if d.pausa.is_some() || d.menu_pruebas.is_some() => overlay::SCREEN_EVENT,
        _ if d.show_compendio => overlay::SCREEN_LIST,
        Phase::Map if d.show_list => overlay::SCREEN_LIST,
        Phase::MainMenu if d.libre => overlay::SCREEN_NONE,
        Phase::MainMenu => overlay::SCREEN_EVENT,
        Phase::Personajes { .. } => overlay::SCREEN_PERSONAJES,
        Phase::Map => overlay::SCREEN_MAP,
        Phase::Reward { .. }
        | Phase::Treasure { .. }
        | Phase::Shop { .. }
        | Phase::Rest
        | Phase::Templar { .. }
        | Phase::Replace { .. }
        | Phase::CharSelect
        | Phase::Neow { .. } => overlay::SCREEN_CARDS,
        Phase::Event { .. } => overlay::SCREEN_EVENT,
        // Pantalla de fin: se muestra cuando el jugador ya ha reaparecido tras morir.
        Phase::Over { new_run: false, .. } if player_hp().unwrap_or(0) > 0 => overlay::SCREEN_OVER,
        _ => overlay::SCREEN_NONE,
    };
    let screen = if overlay::OCULTAR.load(Ordering::Relaxed) { overlay::SCREEN_NONE } else { screen };
    overlay::SCREEN.store(screen, Ordering::Relaxed);
    // Combate visible: listo hace 1 s y la pantalla del juego despejada 0,3 s seguidos (la carga
    // puede alargarse; un retraso fijo no basta).
    if matches!(d.phase, Phase::Arena { ready: true, .. }) && !d.visible {
        let clear = crate::screen_fade() < 0.05;
        d.clear_since = if clear { d.clear_since.or(Some(Instant::now())) } else { None };
        // Seguro: a los 8 s se da por visible aunque el fundido no se lea (el telón no debe quedarse).
        if d.ready_at.is_some_and(|t| t.elapsed() > Duration::from_secs(8))
            || (d.ready_at.is_some_and(|t| t.elapsed() > Duration::from_secs(1)) && d.clear_since.is_some_and(|t| t.elapsed() > Duration::from_millis(300)))
        {
            d.visible = true;
            d.visible_desde = Some(Instant::now());
            crate::log("coliseo: combate visible");
            d.invocar();
        }
    }
    // "EMPEZAD" del coliseo, una vez, justo cuando aparece la barra.
    if matches!(d.phase, Phase::Arena { ready: true, .. }) && d.visible && !d.commence_shown {
        d.commence_shown = true;
        crate::status_message(eldenring::cs::STATUS_MESSAGE_COMMENCE);
    }
    // Barra de vida del boss mientras se pelea en el coliseo.
    let bar = match &d.phase {
        // Tras la carga: la pantalla de carga del juego sigue un rato después de que el boss exista.
        Phase::Arena { enc, ent, ready: true, .. } if d.visible => d.barra_grupo(enc, *ent),
        _ => None,
    };
    // Barras de jefe del juego: con hasta 3 rivales vivos, las del juego (el principal primero); con
    // más, la nuestra del grupo (juego.rs, barras_jefe).
    let nativas = match &d.phase {
        Phase::Arena { enc, ent, ready: true, .. } if d.visible => {
            let vivos: Vec<u32> = d.de_oleada(enc).into_iter().filter(|e| !d.muertos.contains(e) && crate::chr_hp(*e).is_some_and(|h| h > 0)).collect();
            // Orden fijo: los que ya tenían barra, en su sitio; los nuevos, detrás (el principal primero).
            // Antes el principal iba siempre arriba y las barras se cambiaban de sitio cada vez que
            // golpeabas a otro (23-10, Monjes de Nox).
            let mut orden: Vec<u32> = d.barras_nativas.iter().copied().filter(|e| vivos.contains(e)).collect();
            if !orden.contains(ent) && vivos.contains(ent) {
                orden.push(*ent);
            }
            orden.extend(vivos.iter().copied().filter(|e| !d.barras_nativas.contains(e) && e != ent));
            if orden.len() <= 3 { orden } else { vec![] }
        }
        _ => vec![],
    };
    let nombre = match &d.phase {
        Phase::Arena { enc, .. } => d.nombres_jefe.get(&enc.id).copied().unwrap_or(-1),
        _ => -1,
    };
    if nativas != d.barras_nativas {
        crate::barras_jefe(&nativas, nombre, &d.nombres_npc);
        crate::log(&format!("barras del juego: {nativas:?} (nombre {nombre})"));
        d.barras_nativas = nativas.clone();
    } else if !nativas.is_empty() {
        crate::barras_jefe(&nativas, nombre, &d.nombres_npc);
    }
    let rival = d.fx.as_ref().and_then(|f| f.motor.as_ref()).map(|m| m.rival_ent());
    let hueco = rival.and_then(|r| nativas.iter().position(|&e| e == r)).unwrap_or(nativas.len().saturating_sub(1));
    super::BARRA_NATIVA.store(if nativas.is_empty() || bar.is_none() { 0 } else { nativas.len() as u32 + 256 * hueco as u32 }, Ordering::Relaxed);
    if let Ok(mut s) = super::SALA_UI.try_lock() {
        *s = d.vista_sala();
    }
    if let Ok(mut b) = BOSS_BAR.try_lock() {
        *b = bar;
    }
    // Tira de bendiciones mientras se pelea (en el coliseo, con el combate ya a la vista).
    let peleando = matches!(d.phase, Phase::Arena { ready: true, .. } if d.visible) || matches!(d.phase, Phase::Combat { .. });
    let tira = match d.fx.as_ref().and_then(|f| f.motor.as_ref()) {
        Some(m) if peleando => m.hud().into_iter().map(|h| (d.relics.reliquias.iter().find(|r| r.id == h.src).map(|r| r.icono).unwrap_or(0), h)).collect(),
        _ => vec![],
    };
    let mut rival = match d.fx.as_ref().and_then(|f| f.motor.as_ref()) {
        Some(m) if peleando => {
            let r = m.rival();
            let icono = |src: &str| d.relics.reliquias.iter().find(|x| x.id == src).map(|x| x.icono).unwrap_or(0);
            (r.estados.into_iter().map(|e| (icono(&e.src), e)).collect(), r.marcas)
        }
        _ => (vec![], vec![]),
    };
    d.rival_coop(&mut rival.0);
    if let Ok(mut r) = RIVAL.try_lock() {
        *r = rival;
    }
    if let Ok(mut t) = TIRA.try_lock() {
        *t = tira;
    }
    let aliados = d.aliados();
    if let Ok(mut a) = ALIADOS.try_lock() {
        *a = aliados;
    }
    // Pausa en combate: el detalle de lo que está en marcha.
    let pausa = match (d.pausa, d.fx.as_ref().and_then(|f| f.motor.as_ref()), d.run.as_ref()) {
        (Some(_), Some(m), Some(run)) => {
            let hud = m.hud();
            let filas = hud
                .into_iter()
                .filter_map(|h| {
                    let r = d.relics.get(&h.src)?;
                    let nivel = run.relics.iter().find(|o| o.id == h.src).map(|o| o.nivel).unwrap_or(0);
                    let texto = r.niveles.get(nivel).or(r.niveles.first()).map(|l| l.texto.clone()).unwrap_or_default();
                    Some(FilaPausa { icono: r.icono, nombre: nombre_nivel(r, nivel), texto, contra: r.contra.clone().unwrap_or_default(), estado: h })
                })
                .collect::<Vec<_>>();
            let resto = run
                .relics
                .iter()
                .filter_map(|o| d.relics.get(&o.id))
                .filter(|r| !filas.iter().any(|f| f.nombre == r.nombre))
                .map(|r| r.nombre.clone())
                .collect();
            let arma = d.arma.map(|(n, max)| rogue_core::idioma::tf("Weapon +{} / {}", &[&n, &max])).unwrap_or_default();
            Some(PausaCombate { arma, filas, resto })
        }
        _ => None,
    };
    if let Ok(mut p) = PAUSA_COMBATE.try_lock() {
        *p = pausa;
    }
    // Telón: desde que eliges un combate hasta llegar al coliseo con la carga terminada (como poco
    // `TELON_MINIMO`, para que se lea el nombre del rival), y en la vuelta al hub. El rival aparece
    // después, con el jugador ya en la arena.
    let cover = match &d.phase {
        Phase::Arena { since, llegada, .. } => llegada.is_none() || since.elapsed() < TELON_MINIMO,
        Phase::Returning { .. } => true,
        _ => false,
    };
    overlay::COVER.store(cover, Ordering::Relaxed);
    let rival = match &d.phase {
        Phase::Arena { enc, .. } if cover => Some((
            match enc.kind {
                EncounterKind::Boss => rogue_core::idioma::t("LORD OF THE ACT"),
                EncounterKind::Elite => rogue_core::idioma::t("ELITE"),
                _ => rogue_core::idioma::t("FIGHT"),
            }
            .to_string(),
            enc.name.clone(),
        )),
        _ => None,
    };
    // Fuera de la llegada a un combate no hay rival: si se quedaba el último, el telón de la vuelta
    // o el de otra run enseñaban el nombre de una pelea anterior.
    if let Ok(mut c) = COVER_TEXT.try_lock() {
        *c = rival;
    }
    crate::set_player_locked((screen != overlay::SCREEN_NONE && d.sala.is_none()) || cover);
    pad::BLOCK_GAME.store(screen != overlay::SCREEN_NONE, Ordering::Relaxed);
    pad::MENU_PROPIO.store(!d.libre, Ordering::Relaxed);
    pad::SELECT_PROPIO.store(d.libre, Ordering::Relaxed);
    d.nav(pad::UI_BUTTONS.load(Ordering::Relaxed));
}

/// Arranque: el menú principal del mod ya se ve y detrás no queda nada a medias (el viaje al centro y
/// los avisos de objeto nuevo del vestuario), así que la capa del arranque se puede quitar.
pub fn menu_listo() -> bool {
    let Ok(g) = DIRECTOR.try_lock() else { return false };
    let Some(d) = g.as_ref() else { return false };
    matches!(d.phase, Phase::MainMenu) && d.cerrar_avisos.is_none() && crate::player_area() == Some(HUB_AREA) && crate::screen_fade() < 0.05
}

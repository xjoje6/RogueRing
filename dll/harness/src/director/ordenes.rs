//! Órdenes `run ...` del banco de pruebas (se ejecutan en el hilo del juego).

use super::*;

pub fn command(root: &Path, args: &[&str]) -> String {
    let mut guard = DIRECTOR.lock().unwrap();
    match args {
        ["nueva", seed] => match build_director(root, seed) {
            Ok(mut d) => {
                // En una sesión (o con la sombra), la run del cooperativo: la semilla de la sesión y los dúos.
                if let Some(semilla) = crate::coop::semilla_sesion() {
                    let mut r = RunState::nueva(&semilla, &d.acts_coop, vec![]);
                    r.coop = true;
                    r.personal(crate::coop::yo());
                    d.run = Some(r);
                }
                d.limpiar_personaje();
                // Cada run empieza sin runas (ni las de una run ganada ni las de una perdida).
                crate::set_runes(0);
                crate::clear_bloodstain();
                d.save();
                let s = d.status();
                *guard = Some(d);
                s
            }
            Err(e) => e,
        },
        ["estado"] => guard.as_ref().map(|d| d.status()).unwrap_or_else(|| "sin run".into()),
        ["ir", c, r] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            if !matches!(d.phase, Phase::Map) {
                return format!("error: fase {:?}", d.phase);
            }
            let (Ok(c), Ok(r)) = (c.parse(), r.parse()) else { return "error coordenadas".into() };
            d.travel((c, r))
        }
        ["elegir", k] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            d.sel = k.parse().unwrap_or(0);
            d.confirm()
        }
        ["forzar", "rival", id] => {
            // Depuración: pelear contra un encuentro concreto (id de data/acts) en el nodo actual.
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            let Some(enc) = d.acts.iter().chain(d.acts_coop.iter()).flat_map(|a| a.encounters.iter().chain(&a.mezclas)).find(|e| e.id == *id).cloned() else {
                return format!("no hay encuentro {id}");
            };
            d.enter_action(NodeAction::Combat(enc))
        }
        ["forzar", que] => {
            // Depuración: entrar en un tipo de nodo sin pasar por el mapa (el nodo actual se reutiliza).
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            let action = match *que {
                "evento" => NodeAction::Event,
                "gracia" => NodeAction::Rest,
                "tesoro" => NodeAction::Treasure,
                "tienda" => NodeAction::Shop,
                "elite" => match d.run.as_mut().and_then(|r| r.next_encounter(EncounterKind::Elite)) {
                    Some(enc) => NodeAction::Combat(enc),
                    None => return "sin élites".into(),
                },
                "combate" => match d.run.as_mut().and_then(|r| r.next_encounter(EncounterKind::Normal)) {
                    Some(enc) => NodeAction::Combat(enc),
                    None => return "sin encuentros".into(),
                },
                _ => return "forzar <evento|gracia|tesoro|tienda|combate>".into(),
            };
            d.enter_action(action)
        }
        ["soltar"] => {
            // Sin director: el banco tiene el personaje para él solo (mediciones en el coliseo).
            *guard = None;
            overlay::SCREEN.store(overlay::SCREEN_NONE, Ordering::Relaxed);
            pad::BLOCK_GAME.store(false, Ordering::Relaxed);
            crate::set_player_locked(false);
            crate::set_player_invincible(false);
            "director fuera".into()
        }
        // Caídos del cooperativo: ganar el combate (como la pausa de pruebas) y que me levante un compañero.
        ["ganar"] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            d.forzar_victoria = true;
            "ok".into()
        }
        // La invocación de mi personaje, creada como si fuera de la sombra (un compañero): en el juego del
        // anfitrión, sin modo compañero (grupo.rs, sin_companero).
        ["invocar_sombra"] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            let Some(inv) = d.invocacion_actual() else { return "sin invocación".into() };
            let sp = d.sp_ceniza();
            d.invocar_para(1, (inv.chr, inv.npc, inv.think), &inv.posiciones, 1, sp, crate::player_pos().unwrap_or_default(), inv.nombre.clone());
            format!("{} (think {})", inv.nombre, super::grupo::sin_companero((inv.chr, inv.npc, inv.think)).2)
        }
        ["invocar_mia"] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            d.invocar();
            "ok".into()
        }
        ["levantame"] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            let v = d.caido_cfg_vida();
            d.levantarse(Some(v), "banco");
            "ok".into()
        }
        ["terminar"] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            d.end_fx();
            d.fx = None;
            d.phase = Phase::Over { won: false, new_run: false, shown: None };
            d.sel = 0;
            "run terminada (pantalla de fin)".into()
        }
        ["obtener", id] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            let Some(it) = d.pool.iter().find(|p| p.id.to_string() == *id).cloned() else { return "no está en el pool".into() };
            let back = std::mem::replace(&mut d.phase, Phase::Map);
            d.acquire(it, back)
        }
        ["reliquia", id] => {
            let Some(d) = guard.as_mut() else { return "sin run".into() };
            match d.relics.get(id).cloned() {
                Some(r) => {
                    d.gain_relic(&r);
                    d.save();
                    format!("reliquia {}", r.nombre)
                }
                None => "reliquia desconocida".into(),
            }
        }
        _ => "órdenes: run nueva <semilla> | run estado | run ir <col> <fila> | run elegir <n> | run reliquia <id>".into(),
    }
}

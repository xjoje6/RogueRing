//! Reglas de RogueRing que cambian el juego base (peso y requisitos del equipo, escalado de los bosses,
//! viales). Se aplican en memoria, una vez por sesión: los params en memoria nunca se guardan, así que
//! el juego sin el mod no las nota.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use eldenring::cs::{NpcParam, SoloParamRepository, SpEffectParam};
use eldenring::param::{EQUIP_PARAM_ACCESSORY_ST, EQUIP_PARAM_PROTECTOR_ST, EQUIP_PARAM_WEAPON_ST, NPC_PARAM_ST};
use fromsoftware_shared::FromStatic;

static ACTIVAS: AtomicBool = AtomicBool::new(false);

/// Tramo neutro de escalado (x1): el del acto lo pone la DLL al crear el boss.
const TRAMO_NEUTRO: i32 = 7000;

/// Efecto de curación del Frasco de lágrimas carmesí, +0 a +12 (EquipParamGoods 1001-1025, refId).
const VIALES: std::ops::RangeInclusive<u32> = 501_000..=501_012;
/// Curación de un vial en el modo: toda la vida (el juego la recorta a la máxima). Solo se rellenan
/// descansando en una gracia.
const VIAL_ENTERO: i32 = -99_999;

fn sp(row: &NPC_PARAM_ST, i: usize) -> i32 {
    match i {
        0 => row.sp_effect_id0(),
        1 => row.sp_effect_id1(),
        2 => row.sp_effect_id2(),
        3 => row.sp_effect_id3(),
        4 => row.sp_effect_id4(),
        5 => row.sp_effect_id5(),
        6 => row.sp_effect_id6(),
        7 => row.sp_effect_id7(),
        _ => row.sp_effect_id8(),
    }
}

fn set_sp(row: &mut NPC_PARAM_ST, i: usize, v: i32) {
    match i {
        0 => row.set_sp_effect_id0(v),
        1 => row.set_sp_effect_id1(v),
        2 => row.set_sp_effect_id2(v),
        3 => row.set_sp_effect_id3(v),
        4 => row.set_sp_effect_id4(v),
        5 => row.set_sp_effect_id5(v),
        6 => row.set_sp_effect_id6(v),
        7 => row.set_sp_effect_id7(v),
        _ => row.set_sp_effect_id8(v),
    }
}

/// Aplica las reglas. `npcs`: NpcParam de los bosses de los actos; `lastre`: pieza de armadura que
/// conserva peso y cuánto (data/peso.json).
pub fn activar(npcs: &[i32], lastre: Option<(u32, f32)>) {
    if ACTIVAS.load(Ordering::Relaxed) {
        return;
    }
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
        crate::log("reglas: sin SoloParamRepository");
        return;
    };
    let (mut pesos, mut tramos, mut viales) = (0, vec![], 0);
    // Esquiva media siempre: todo el equipo pesa 0 salvo el lastre.
    for rc in repo.params_mut() {
        let nombre = rc.struct_name().to_string();
        match nombre.as_str() {
            "EQUIP_PARAM_WEAPON_ST" => {
                // Sin requisitos de atributos: cualquier arma que salga de recompensa se empuña sin
                // castigo (escala con lo que tenga el personaje).
                for (_, r) in unsafe { rc.data.rows_mut::<EQUIP_PARAM_WEAPON_ST>() } {
                    pesos += 1;
                    r.set_weight(0.0);
                    r.set_proper_strength(0);
                    r.set_proper_agility(0);
                    r.set_proper_magic(0);
                    r.set_proper_faith(0);
                    r.set_proper_luck(0);
                }
            }
            "EQUIP_PARAM_ACCESSORY_ST" => {
                for (_, r) in unsafe { rc.data.rows_mut::<EQUIP_PARAM_ACCESSORY_ST>() } {
                    pesos += 1;
                    r.set_weight(0.0);
                }
            }
            "EQUIP_PARAM_PROTECTOR_ST" => {
                for (id, r) in unsafe { rc.data.rows_mut::<EQUIP_PARAM_PROTECTOR_ST>() } {
                    pesos += 1;
                    r.set_weight(match lastre {
                        Some((l, w)) if l == id => w,
                        _ => 0.0,
                    });
                }
            }
            _ => {}
        }
    }
    // Escalado normalizado: el tramo de zona del enemigo pasa a neutro (el del acto lo pone la DLL); si
    // no lleva, un hueco libre (nunca se pisa otro efecto). Tramos del juego base, 7000-7999, y del DLC,
    // 20007000-20007999: el del DLC se sumaba al del acto (hasta x8,6 de vida y x3,8 de ataque más) y
    // el León danzante o los Caballeros de la muerte mataban de un golpe (16-10).
    let es_tramo = |v: i32| (7000..8000).contains(&v) || (20_007_000..20_008_000).contains(&v);
    for &npc in npcs {
        if npc <= 0 || tramos.contains(&npc) {
            continue;
        }
        let Some(row) = repo.get_mut::<NpcParam>(npc as u32) else { continue };
        let hueco = (0..9).find(|&i| es_tramo(sp(row, i))).or_else(|| (0..9).find(|&i| sp(row, i) <= 0));
        if let Some(i) = hueco {
            tramos.push(npc);
            set_sp(row, i, TRAMO_NEUTRO);
        }
    }
    for id in VIALES {
        if let Some(row) = repo.get_mut::<SpEffectParam>(id) {
            viales += 1;
            row.set_change_hp_estus_flask_point(VIAL_ENTERO);
        }
    }
    crate::log(&format!("reglas activadas: {} pesos, {} bosses a tramo neutro, {} viales curan entero", pesos, tramos.len(), viales));
    ACTIVAS.store(true, Ordering::Relaxed);
}

/// NpcParam de todos los encuentros de los actos y el lastre de data/peso.json.
pub fn datos(root: &Path, acts: &[rogue_core::run::ActDef]) -> (Vec<i32>, Option<(u32, f32)>) {
    let npcs = acts.iter().flat_map(|a| a.encounters.iter()).flat_map(|e| e.lista()).map(|x| x.npc).filter(|&n| n > 0).collect();
    let lastre = std::fs::read_to_string(root.join("data").join("peso.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| Some((v.get("lastre")?.as_u64()? as u32, (v.get("carga_max")?.as_f64()? * v.get("proporcion")?.as_f64()?) as f32)));
    (npcs, lastre)
}

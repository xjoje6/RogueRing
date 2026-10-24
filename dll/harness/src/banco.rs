//! Banco de pruebas (solo con la característica `banco`; la versión de jugador no lo lleva). Lee
//! órdenes de `harness/cmd.txt` (junto a la DLL), las ejecuta en el hilo del juego y escribe la
//! respuesta en `harness/out.txt`. No usa ratón ni teclado del sistema.
//!
//! Órdenes: ping | estado | flag <id> [0|1] | chr <entidad> | matar <entidad> | ir <entidad> [dist] | irpos <x> <y> <z> (local al bloque)
//!          | dios <0|1> | pulsar <botón> [ms] | foco <0|1> | ui | run ... | pantalla <mapa|nada> | dar <cat> <id> [n] | ventana [x y w h]

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eldenring::cs::{CSEventFlagMan, CSMenuManImp, ItemLotParam_map, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::*;

/// Modo pruebas: vida del jugador al máximo en cada fotograma.
static GOD: AtomicBool = AtomicBool::new(false);
/// Grabación de las animaciones del jugador fotograma a fotograma (`grabar_anims`): (momento, animación)
/// en cada cambio. Las órdenes tardan 0,1-0,6 s en ir y volver: muestreando se pierden las cortas.
static GRABACION: std::sync::Mutex<Option<Vec<(Instant, i32)>>> = std::sync::Mutex::new(None);
/// Posición fijada con `ventana x y w h`: el juego a veces recoloca su ventana y hay que devolverla.
static WINDOW_TARGET: std::sync::Mutex<Option<(i32, i32, i32, i32)>> = std::sync::Mutex::new(None);

fn enforce_window() {
    let Some((x, y, w, h)) = *WINDOW_TARGET.lock().unwrap() else { return };
    let Some(hw) = game_window() else { return };
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(hw, &mut r);
        if r.left != x || r.top != y {
            let _ = SetWindowPos(hw, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER);
            log(&format!("ventana recolocada desde ({}, {})", r.left, r.top));
        }
    }
}

/// Velocidad de animación del jugador forzada en cada fotograma (`velo`).
static SPEED_TEST: AtomicU32 = AtomicU32::new(0);

/// Medición de balance (`medir <entidad> [atacar]`, `medida`, `medir 0`). Los dos aguantan: cada golpe
/// al jugador se apunta como fracción de su vida y se cura; cada bajada de vida del enemigo se apunta
/// y se rellena. Con `atacar`, el jugador se pega al enemigo y da un R1 cada poco.
struct Medida {
    ent: u32,
    atacar: bool,
    recibidos: Vec<f32>,
    dados: Vec<i32>,
    vida_jugador: i32,
    vida_enemigo: i32,
    ultimo_ataque: Instant,
}

static MEDIDA: std::sync::Mutex<Option<Medida>> = std::sync::Mutex::new(None);

fn medir_fotograma() {
    let Ok(mut g) = MEDIDA.lock() else { return };
    let Some(m) = g.as_mut() else { return };
    if let Some((hp, max)) = player_hp_max() {
        m.vida_jugador = max;
        if hp > 0 && hp < max {
            m.recibidos.push((max - hp) as f32 / max as f32);
            set_player_hp(max);
        }
    }
    if let Some((hp, max)) = chr_hp_max(m.ent) {
        m.vida_enemigo = max;
        if hp > 0 && hp < max {
            m.dados.push(max - hp);
            fill_hp(m.ent);
        }
    }
    if m.atacar && m.ultimo_ataque.elapsed() > Duration::from_millis(1300) {
        m.ultimo_ataque = Instant::now();
        if let (Some(e), Some(p)) = (chr_pos(m.ent), player_pos()) {
            let (dx, dz) = (p[0] - e[0], p[2] - e[2]);
            let d = dx.hypot(dz).max(0.001);
            if d > 3.0 {
                set_player_pos([e[0] + dx / d * 2.0, e[1] + 0.3, e[2] + dz / d * 2.0]);
            }
            mirar_hacia(e, 0.0);
            pad::press(0x0200, 120);
        }
    }
}

/// Coloca al jugador en un punto (coordenadas de `player_pos`).
fn set_player_pos(t: [f32; 3]) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let Some(p) = wcm.main_player.as_mut() else { return false };
    let phys = &mut p.chr_ins.modules.physics;
    (phys.position.0, phys.position.1, phys.position.2) = (t[0], t[1], t[2]);
    phys.chr_proxy_pos_update_requested = true;
    true
}

fn medida_json() -> String {
    let Ok(g) = MEDIDA.lock() else { return "error".into() };
    let Some(m) = g.as_ref() else { return "sin medida".into() };
    serde_json::json!({
        "vida_jugador": m.vida_jugador,
        "vida_enemigo": m.vida_enemigo,
        "recibidos": m.recibidos,
        "dados": m.dados,
    })
    .to_string()
}

/// Orden `red [entidad]`: qué grupos de personajes tienen búfer de sincronización de red
/// (`NetChrSync`) y, con una entidad, su identificador de red y quién la mueve (Local/Remote).
/// Sirve para saber si un jefe de `spawn_chr` puede sincronizarse en cooperativo (docs/COOP_SALAS.md).
fn red_info(ent: Option<u32>) -> String {
    use eldenring::cs::{ChrIns, ChrSet};
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return "error WorldChrMan".into() };
    let sync = &*wcm.net_chr_sync;
    let con_sync: Vec<usize> = sync.net_chr_set_sync.iter().enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect();
    let hueco = |set: *const ChrSet<ChrIns>| wcm.chr_sets.iter().position(|s| s.as_ref().is_some_and(|s| std::ptr::eq(&**s, set)));
    let grupo = |nombre: &str, set: &ChrSet<ChrIns>| {
        let i = set.index;
        let s = usize::try_from(i).ok().and_then(|i| sync.net_chr_set_sync.get(i)).and_then(|s| s.as_ref());
        serde_json::json!({
            "grupo": nombre,
            "index": i,
            "capacidad": set.capacity,
            "hueco_chr_sets": hueco(set),
            "sync": s.map(|s| serde_json::json!({ "capacidad": s.capacity, "mismo_set": std::ptr::eq(s.chr_set.as_ptr(), set) })),
        })
    };
    let grupos = vec![
        grupo("fantasmas", &wcm.ghost_chr_set),
        grupo("invocaciones", &wcm.summon_buddy_chr_set),
        grupo("depuracion", &wcm.debug_chr_set),
        grupo("mapa_abierto", &wcm.open_field_chr_set.base),
    ];
    let mut chr = serde_json::Value::Null;
    if let Some(ent) = ent {
        let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain([&wcm.debug_chr_set, &wcm.open_field_chr_set.base]);
        'fuera: for set in sets {
            for k in 0..set.capacity as usize {
                let e = unsafe { set.entries.add(k).as_ref() };
                let Some(c) = e.chr_ins else { continue };
                let c = unsafe { c.as_ref() };
                if c.event_entity_id != ent {
                    continue;
                }
                let h = &c.field_ins_handle;
                chr = serde_json::json!({
                    "set_index": set.index,
                    "contenedor": h.selector.container(),
                    "indice": h.selector.index(),
                    "handle": h.to_string(),
                    "npc": c.npc_param_id,
                    "entrada": k,
                    "carga": format!("{:?}", e.chr_load_status),
                    "control": format!("{:?}", e.chr_update_type),
                });
                break 'fuera;
            }
        }
    }
    serde_json::json!({ "chr_slot_count": sync.chr_slot_count, "con_sync": con_sync.len(), "huecos_sync": con_sync, "grupos": grupos, "chr": chr }).to_string()
}

/// Orden `red control <entidad> <local|remoto>`: cambia quién mueve al personaje en esta copia del
/// juego. Con `remoto` y sin compañero, debería quedarse quieto (nadie le manda nada).
fn red_control(ent: u32, remoto: bool) -> String {
    use eldenring::cs::ChrUpdateType;
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return "error WorldChrMan".into() };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain([&wcm.debug_chr_set, &wcm.open_field_chr_set.base]);
    for set in sets {
        for k in 0..set.capacity as usize {
            let e = unsafe { set.entries.add(k).as_mut() };
            let Some(c) = e.chr_ins else { continue };
            if unsafe { c.as_ref() }.event_entity_id == ent {
                let antes = format!("{:?}", e.chr_update_type);
                e.chr_update_type = if remoto { ChrUpdateType::Remote } else { ChrUpdateType::Local };
                return format!("ok {antes} -> {:?}", e.chr_update_type);
            }
        }
    }
    "no cargado".into()
}

fn respond(id: &str, body: &str) {
    if let Some(d) = DIR.get()
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(d.join("out.txt"))
    {
        let _ = writeln!(f, "#{id} {body}");
    }
}

/// Fila de ItemLot y flag del evento de entrega (ver tools/ModBuilder).
const REWARD_LOT: u32 = 79_600_000;
const REWARD_FLAG: u32 = 1049300190;

/// Entrega un objeto: reescribe la fila de ItemLot reservada y dispara el evento que la otorga.
fn give_item(category: i32, id: i32, n: u8) -> Result<(), String> {
    let repo = unsafe { SoloParamRepository::instance_mut() }.map_err(|_| "sin SoloParamRepository")?;
    let row = repo.get_mut::<ItemLotParam_map>(REWARD_LOT).ok_or("falta la fila de recompensa (¿regulation.bin del mod?)")?;
    row.set_lot_item_id01(id);
    row.set_lot_item_category01(category);
    row.set_lot_item_num01(n);
    row.set_lot_item_base_point01(1000);
    flag_set(REWARD_FLAG, true);
    Ok(())
}

/// Órdenes que no tocan estado del juego: se ejecutan en el hilo lector.
fn run_outside(cmd: &[&str]) -> Option<String> {
    match cmd {
        ["ventana"] => Some(match game_window() {
            Some(h) => {
                let mut r = RECT::default();
                unsafe {
                    let _ = GetWindowRect(h, &mut r);
                }
                format!("{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}", r.left, r.top, r.right - r.left, r.bottom - r.top)
            }
            None => "error sin ventana".into(),
        }),
        ["ventana", x, y, w, h] => Some(match (game_window(), x.parse(), y.parse(), w.parse(), h.parse()) {
            (Some(hw), Ok(x), Ok(y), Ok(w), Ok(h)) => {
                *WINDOW_TARGET.lock().unwrap() = Some((x, y, w, h));
                pad::PIN_X.store(x, Ordering::Relaxed);
                pad::PIN_Y.store(y, Ordering::Relaxed);
                pad::PIN_HWND.store(hw.0 as isize, Ordering::Relaxed);
                // SWP_NOACTIVATE: moverla sin quitar el foco a la ventana que se esté usando.
                let ok = unsafe { SetWindowPos(hw, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER) }.is_ok();
                if ok { "ok".into() } else { "error SetWindowPos".into() }
            }
            _ => "error argumentos o sin ventana".into(),
        }),
        ["diag"] => Some(format!(
            "getstate={} getstate_ex={} caps={} foreground={}",
            pad::CALLS[0].load(Ordering::Relaxed),
            pad::CALLS[1].load(Ordering::Relaxed),
            pad::CALLS[2].load(Ordering::Relaxed),
            pad::CALLS[3].load(Ordering::Relaxed)
        )),
        ["pantalla", which] => Some({
            let v = match *which {
                "mapa" => overlay::SCREEN_MAP,
                "cartas" => overlay::SCREEN_CARDS,
                _ => overlay::SCREEN_NONE,
            };
            overlay::SCREEN.store(v, Ordering::Relaxed);
            "ok".into()
        }),
        ["foco", v] => Some({
            let h = if *v == "1" { game_window().map(|h| h.0 as isize).unwrap_or(0) } else { 0 };
            pad::FAKE_FOCUS_HWND.store(h, std::sync::atomic::Ordering::Relaxed);
            if *v == "1" && h == 0 { "error sin ventana".into() } else { "ok".into() }
        }),
        ["forzar", b] => Some(match pad::button(b) {
            Some(bits) => {
                pad::press_forzado(bits, 150);
                "ok".into()
            }
            None => "error: botón".into(),
        }),
        // Ráfaga con el tiempo exacto (hilo propio): las órdenes de una en una llegan tarde.
        ["rafaga", b, n, cada] => Some(match (pad::button(b), n.parse::<u32>(), cada.parse::<u64>()) {
            (Some(bits), Ok(n), Ok(cada)) => {
                std::thread::spawn(move || {
                    for _ in 0..n {
                        pad::press(bits, 50);
                        std::thread::sleep(Duration::from_millis(cada));
                    }
                });
                "ok".into()
            }
            _ => "error: rafaga <botón> <veces> <cada ms>".into(),
        }),
        ["pulsar", b] | ["pulsar", b, _] => Some(match pad::button(b) {
            Some(bits) => {
                pad::press(bits, cmd.get(2).and_then(|m| m.parse().ok()).unwrap_or(150));
                "ok".into()
            }
            None => "error botón (A B X Y START BACK LB RB ARRIBA ABAJO IZQUIERDA DERECHA)".into(),
        }),
        _ => None,
    }
}

/// Órdenes que leen o escriben estado del juego: se ejecutan en el hilo del juego (tarea por frame).
fn run_in_game(cmd: &[&str], frame: u64) -> String {
    match cmd {
        ["ping"] => format!("pong frame={frame}"),
        ["estado"] => {
            let mut j = serde_json::json!({ "frame": frame, "en_partida": false });
            if let Ok(wcm) = unsafe { WorldChrMan::instance() }
                && let Some(p) = wcm.main_player.as_ref()
            {
                let data = &p.chr_ins.modules.data;
                let pos = &p.chr_ins.modules.physics.position;
                j["en_partida"] = true.into();
                j["hp"] = data.hp.into();
                j["hp_max"] = data.max_hp.into();
                j["pos"] = serde_json::json!([pos.0, pos.1, pos.2]);
                let g = &p.chr_ins.chunk_position;
                j["global"] = serde_json::json!([g.0, g.1, g.2]);
                let b = &p.block_position;
                j["bloque"] = serde_json::json!([b.x, b.y, b.z]);
                j["bloque_id"] = format!("{:?}", p.current_block_id).into();
                j["runas"] = unsafe { p.player_game_data.as_ref() }.rune_count.into();
            }
            j.to_string()
        }
        ["flag", id] => match (id.parse::<u32>(), unsafe { CSEventFlagMan::instance() }) {
            (Ok(id), Ok(m)) => format!("{}", m.virtual_memory_flag.get_flag(id) as u8),
            (Err(_), _) => "error id".into(),
            (_, Err(_)) => "error CSEventFlagMan no disponible".into(),
        },
        ["flag", id, v] => match (id.parse::<u32>(), unsafe { CSEventFlagMan::instance_mut() }) {
            (Ok(id), Ok(m)) => {
                m.virtual_memory_flag.set_flag(id, *v == "1");
                "ok".into()
            }
            (Err(_), _) => "error id".into(),
            (_, Err(_)) => "error CSEventFlagMan no disponible".into(),
        },
        ["efectos", ent] => match ent.parse::<u32>() {
            Ok(e) => format!("{:?}", chr_speffects(e)),
            Err(_) => "error: efectos <entidad|0>".into(),
        },
        ["resist", ent] => match ent.parse::<u32>().ok().and_then(chr_resist) {
            Some((c, m)) => format!("{c:?} / {m:?}"),
            None => "no cargado".into(),
        },
        ["estados", ent] => match ent.parse::<u32>() {
            Ok(e) => format!("{:?}", chr_state_infos(e)),
            Err(_) => "error: estados <entidad>".into(),
        },
        ["gatillo", ms] | ["gatillo", "izq", ms] => match ms.parse::<u64>() {
            Ok(ms) => {
                pad::gatillo(ms, cmd.len() == 3);
                "ok".into()
            }
            Err(_) => "error: gatillo [izq] <ms>".into(),
        },
        ["velo", v] => match v.parse::<f32>() {
            Ok(v) => {
                SPEED_TEST.store(v.to_bits(), Ordering::Relaxed);
                "ok".into()
            }
            Err(_) => "error: velo <v> (0 = desactivar)".into(),
        },
        ["palanca", x, y, ms] => match (x.parse::<i16>(), y.parse::<i16>(), ms.parse::<u64>()) {
            (Ok(x), Ok(y), Ok(ms)) => {
                pad::stick(x, y, ms);
                "ok".into()
            }
            _ => "error: palanca <x> <y> <ms>".into(),
        },
        ["mirar", g] => match g.parse::<f32>() {
            Ok(g) => {
                MIRAR_DESFASE.store(g.to_bits(), Ordering::Relaxed);
                format!("{}", mirar_a_camara())
            }
            Err(_) => "error: mirar <grados>".into(),
        },
        ["anim_actual"] => format!("{:?}", player_anim()),
        // Números de todos los objetos (EquipParamGoods) a runtime/harness/goods_ids.txt: comparándolos con
        // los del juego se ven los que añade Seamless (sus objetos de sesión).
        ["goods_ids"] => match unsafe { SoloParamRepository::instance_mut() } {
            Ok(repo) => {
                let mut ids = vec![];
                for rc in repo.params_mut() {
                    if rc.struct_name() == "EQUIP_PARAM_GOODS_ST" {
                        for (id, r) in unsafe { rc.data.rows_mut::<eldenring::param::EQUIP_PARAM_GOODS_ST>() } {
                            ids.push(format!("{id}	{}	{}	{}", r.ref_id_default(), r.goods_type(), r.icon_id()));
                        }
                    }
                }
                let f = crate::DIR.get().cloned().unwrap_or_default().join("goods_ids.txt");
                let _ = std::fs::write(&f, ids.join("
"));
                format!("{} objetos en {}", ids.len(), f.display())
            }
            Err(_) => "sin SoloParamRepository".into(),
        },
        // Prueba: pone el objeto en la ranura rápida 1 y lo usa (botón X del mando).
        ["vigilar_ini"] => crate::diag::vigilar_ini(),
        ["vigilar", texto] => crate::diag::vigilar(texto),
        ["buscar_clave"] => crate::diag::buscar_clave(),
        ["usar_objeto", id] => match id.parse::<u32>() {
            Ok(id) => match equip::poner_rapido(0, 0x4000_0000 | id) {
                Ok(antes) => {
                    pad::press(pad::button("X").unwrap_or(0x4000), 120);
                    format!("puesto y usado; antes {antes:?}")
                }
                Err(e) => format!("error: {e}"),
            },
            Err(_) => "error: usar_objeto <id>".into(),
        },
        ["coop", "sombra", modo] => crate::coop::poner_sombra(modo),
        ["coop", "estado"] => crate::coop::resumen(),
        ["grabar_anims", v] => {
            let mut g = GRABACION.lock().unwrap();
            if *v == "1" {
                *g = Some(vec![]);
                "ok".into()
            } else {
                let lista = g.take().unwrap_or_default();
                let Some(t0) = lista.first().map(|x| x.0) else { return "[]".into() };
                serde_json::to_string(&lista.iter().map(|(t, a)| ((t.duration_since(t0).as_secs_f64() * 100.0).round() / 100.0, *a)).collect::<Vec<_>>())
                    .unwrap_or_default()
            }
        }
        ["vel_actual"] => format!("{:?}", velocidad_y_agarre()),
        ["anims"] => match unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.main_player.as_ref()) {
            Some(p) => {
                format!("{:?}", p.chr_ins.modules.time_act.anim_queue.iter().map(|a| (a.anim_id, (a.play_time * 10.0).round() / 10.0)).collect::<Vec<_>>())
            }
            None => "sin jugador".into(),
        },
        ["fundido"] => format!("{:.2}", screen_fade()),
        ["agarre"] => format!("{:?}", equip::arm_style()),
        ["agarre", "dos"] => format!("{} {:?}", equip::set_two_handed(true), equip::arm_style()),
        ["agarre", "una"] => format!("{} {:?}", equip::set_two_handed(false), equip::arm_style()),
        ["agarre", "solo", v] => {
            // Solo el campo, sin reequipar (para ver si el juego lo aplica por sí mismo).
            if let Some(gd) = equip::player_game_data() {
                gd.equipment.chr_asm.equipment.arm_style =
                    if *v == "dos" { eldenring::cs::ChrAsmArmStyle::RightBothHands } else { eldenring::cs::ChrAsmArmStyle::OneHanded };
            }
            format!("{:?}", equip::arm_style())
        }
        ["se", k] => match k.parse::<u32>() {
            Ok(k) => {
                director::play_se_slot(k);
                "ok".into()
            }
            Err(_) => "error: se <k>".into(),
        },
        ["bala", id] | ["bala", id, _] => match id.parse::<i32>() {
            Ok(id) => match spawn_bullet(id, cmd.get(2).and_then(|e| e.parse().ok())) {
                Ok(()) => "ok".into(),
                Err(e) => format!("error: {e}"),
            },
            Err(_) => "error: bala <id> [entidad objetivo]".into(),
        },
        ["bala_en", id, ent] => match (id.parse::<i32>(), ent.parse::<u32>()) {
            // Proyectil soltado a los pies de una entidad, sin objetivo (efectos visuales en el suelo).
            (Ok(id), Ok(e)) => match if e == 0 { player_pos() } else { chr_pos(e) } {
                Some(p) => match spawn_bullet_at(id, None, Some([p[0], p[1] + 0.1, p[2]])) {
                    Ok(()) => "ok".into(),
                    Err(e) => format!("error: {e}"),
                },
                None => "sin entidad".into(),
            },
            _ => "error: bala_en <id> <entidad|0 = jugador>".into(),
        },
        // Efectos visuales de un SpEffect tal como los tiene el juego (jefes de Nightreign sin efectos,
        // 17-10): sus vfx y, de cada uno, el FXR que lleva puesto y en qué punto.
        ["sp_vfx", id] => match id.parse::<u32>() {
            Ok(i) => {
                let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance() }) else { return "sin parámetros".into() };
                match repo.get::<eldenring::cs::SpEffectParam>(i) {
                    Some(r) => {
                        let v = [r.vfx_id(), r.vfx_id1(), r.vfx_id2(), r.vfx_id3(), r.vfx_id4(), r.vfx_id5(), r.vfx_id6(), r.vfx_id7()];
                        let filas: Vec<String> = v
                            .iter()
                            .filter(|&&x| x > 0)
                            .map(|&x| match repo.get::<eldenring::cs::SpEffectVfxParam>(x as u32) {
                                Some(f) => format!("{x}: fxr {} en {} (inicio {}, fin {})", f.midst_sfx_id(), f.midst_dmy_id(), f.init_sfx_id(), f.finish_sfx_id()),
                                None => format!("{x}: SIN FILA"),
                            })
                            .collect();
                        format!("{i} ({} s): {}", r.effect_endurance(), filas.join("; "))
                    }
                    None => format!("{i}: SIN FILA"),
                }
            }
            _ => "error: sp_vfx <id>".into(),
        },
        // Cambia el FXR de una fila de efecto visual (para probar un FXR con un efecto que se ve).
        ["vfx_fxr", id, fxr] => match (id.parse::<u32>(), fxr.parse::<i32>()) {
            (Ok(i), Ok(f)) => {
                let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return "sin parámetros".into() };
                match repo.get_mut::<eldenring::cs::SpEffectVfxParam>(i) {
                    Some(r) => {
                        let antes = r.midst_sfx_id();
                        r.set_midst_sfx_id(f);
                        format!("{i}: fxr {antes} -> {f}")
                    }
                    None => format!("{i}: SIN FILA"),
                }
            }
            _ => "error: vfx_fxr <vfx> <fxr>".into(),
        },
        ["efecto_a", ent, id] => match (ent.parse::<u32>(), id.parse::<i32>()) {
            (Ok(e), Ok(i)) => format!("{}", apply_speffect_to(e, i)),
            _ => "error: efecto_a <entidad> <id>".into(),
        },
        ["efecto", id] | ["efecto", id, _] => match id.parse::<i32>() {
            Ok(id) => format!("{}", player_speffect(id, cmd.get(2).is_none_or(|v| *v != "0"))),
            Err(_) => "error: efecto <id> [0|1]".into(),
        },
        ["carga"] => format!("{:?}", max_equip_load()),
        ["mandos"] => match unsafe { eldenring::fd4::FD4PadManager::instance() } {
            Ok(m) => format!("libScePad {} segundo_plano {}", m.use_lib_sce_pad, m.is_back_ground_window),
            Err(_) => "sin FD4PadManager".into(),
        },
        ["dios", v] => {
            GOD.store(*v == "1", Ordering::Relaxed);
            if *v != "1" {
                set_player_invincible(false);
            }
            "ok".into()
        }
        ["ir", ent] | ["ir", ent, _] => {
            let Ok(ent) = ent.parse::<u32>() else { return "error id".into() };
            let dist: f32 = cmd.get(2).and_then(|d| d.parse().ok()).unwrap_or(4.0);
            let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return "error WorldChrMan".into() };
            let mut target = None;
            let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
            'outer: for set in sets {
                for c in set.characters() {
                    if c.event_entity_id == ent {
                        // Posición global (chunk): la física de cada personaje puede ir en otro origen.
                        let p = &c.chunk_position;
                        target = Some((p.0, p.1, p.2));
                        break 'outer;
                    }
                }
            }
            let Some((bx, by, bz)) = target else { return "no cargado".into() };
            let Some(player) = wcm.main_player.as_mut() else { return "sin jugador".into() };
            let pc = player.chr_ins.chunk_position;
            // Punto a `dist` metros del boss, del lado por el que viene el jugador.
            let (dx, dz) = (pc.0 - bx, pc.2 - bz);
            let len = (dx * dx + dz * dz).sqrt().max(0.001);
            let (tx, ty, tz) = (bx + dx / len * dist, by + 0.5, bz + dz / len * dist);
            // Mismo desplazamiento global aplicado a la posición física del jugador.
            let phys = &mut player.chr_ins.modules.physics;
            phys.position.0 += tx - pc.0;
            phys.position.1 += ty - pc.1;
            phys.position.2 += tz - pc.2;
            phys.chr_proxy_pos_update_requested = true;
            format!("ok salto {:.1} m", ((tx - pc.0).powi(2) + (ty - pc.1).powi(2) + (tz - pc.2).powi(2)).sqrt())
        }
        ["ui"] => match unsafe { CSMenuManImp::instance() } {
            Ok(m) => {
                let vis: Vec<usize> = m.ui_states.iter().enumerate().filter(|(_, u)| u.visible()).map(|(i, _)| i).collect();
                serde_json::json!({ "visibles": vis, "popup": m.popup_menu.is_some() }).to_string()
            }
            Err(_) => "error CSMenuMan".into(),
        },
        ["dar", cat, id] | ["dar", cat, id, _] => {
            let categoria = match *cat {
                "objeto" => 1,
                "arma" => 2,
                "armadura" => 3,
                "talisman" => 4,
                "ceniza" => 5,
                n => n.parse().unwrap_or(-1),
            };
            let (Ok(id), true) = (id.parse::<i32>(), categoria > 0) else { return "error: dar <objeto|arma|armadura|talisman|ceniza> <id> [n]".into() };
            let n: u8 = cmd.get(3).and_then(|x| x.parse().ok()).unwrap_or(1);
            match give_item(categoria, id, n) {
                Ok(()) => "ok".into(),
                Err(e) => format!("error: {e}"),
            }
        }
        ["dar_modo", id, modo] => match (u32::from_str_radix(id.trim_start_matches("0x"), 16), modo.parse::<u32>()) {
            (Ok(id), Ok(m)) => format!("{:?}", equip::give_modo(id, 1, -1, m)),
            _ => "error: dar_modo <id hex> <modo>".into(),
        },
        ["irpos", x, y, z] => {
            let (Ok(x), Ok(y), Ok(z)) = (x.parse::<f32>(), y.parse::<f32>(), z.parse::<f32>()) else { return "error coordenadas".into() };
            match teleport_local([x, y, z]) {
                Some(d) => format!("ok salto {d:.1} m"),
                None => "sin jugador".into(),
            }
        }
        ["atributos", rest @ ..] if rest.len() == 8 => {
            // vig men agu fue des int fe arc. El nivel se deriva como en el juego (suma − 79).
            let v: Vec<u32> = rest.iter().filter_map(|x| x.parse().ok()).collect();
            if v.len() != 8 {
                return "error: atributos <vig> <men> <agu> <fue> <des> <int> <fe> <arc>".into();
            }
            match unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
                Some(p) => {
                    let gd = unsafe { p.player_game_data.as_mut() };
                    (gd.vigor, gd.mind, gd.endurance, gd.strength, gd.dexterity, gd.intelligence, gd.faith, gd.arcane) =
                        (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
                    gd.level = v.iter().sum::<u32>().saturating_sub(79).max(1);
                    let lvl = gd.level;
                    // Guardar ya: al recargar la partida el juego recalcula vida, PC y aguante.
                    if let Ok(gm) = unsafe { eldenring::cs::GameMan::instance_mut() } {
                        gm.save_requested = true;
                    }
                    format!("ok nivel {lvl} (guardado pedido; se recalcula al recargar la partida)")
                }
                None => "sin jugador".into(),
            }
        }
        ["invocar", chr, npc, think, ent] | ["invocar", chr, npc, think, ent, _] => {
            // Crea un personaje (p. ej. un boss) a `dist` metros delante del jugador.
            let (Ok(chr), Ok(npc), Ok(think), Ok(ent)) = (chr.parse::<i32>(), npc.parse::<i32>(), think.parse::<i32>(), ent.parse::<i32>()) else {
                return "error: invocar <chr> <npc> <think> <entidad> [dist]".into();
            };
            let dist: f32 = cmd.get(5).and_then(|d| d.parse().ok()).unwrap_or(12.0);
            let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return "error WorldChrMan".into() };
            let Some(p) = wcm.main_player.as_ref() else { return "sin jugador".into() };
            let pos = p.chr_ins.modules.physics.position;
            let req = eldenring::cs::ChrDebugSpawnRequest {
                chr_id: chr,
                chara_init_param_id: -1,
                npc_param_id: npc,
                npc_think_param_id: think,
                event_entity_id: ent,
                talk_id: -1,
                pos_x: pos.0,
                pos_y: pos.1 + 0.5,
                pos_z: pos.2 + dist,
            };
            wcm.spawn_debug_character(&req);
            format!("ok c{chr:04} npc {npc} ia {think} entidad {ent}")
        }
        ["equipar", slot, id] => match (slot.parse::<u32>(), u32::from_str_radix(id.trim_start_matches("0x"), if id.starts_with("0x") { 16 } else { 10 })) {
            (Ok(slot), Ok(id)) => match equip::equip(slot, id) {
                Ok(()) => "ok".into(),
                Err(e) => format!("error: {e}"),
            },
            _ => "error: equipar <ranura> <id>".into(),
        },
        ["meter", id] | ["meter", id, _] => {
            let gem = cmd.get(2).and_then(|g| g.parse::<i32>().ok()).unwrap_or(-1);
            match u32::from_str_radix(id.trim_start_matches("0x"), if id.starts_with("0x") { 16 } else { 10 }) {
                Ok(id) => match equip::give(id, 1, gem) {
                    Ok(()) => "ok".into(),
                    Err(e) => format!("error: {e}"),
                },
                _ => "error: meter <id> [ceniza]".into(),
            }
        }
        // Apaga o enciende del todo a un personaje (como a los de una oleada que aún no toca).
        // Animación del propio personaje y su velocidad (caído del cooperativo: elegir la postura).
        ["anim_jugador", anim] => match anim.parse::<i32>() {
            Ok(a) => format!("{}", crate::player_request_anim(a)),
            _ => "error: anim_jugador <animación>".into(),
        },
        ["vel_chr", ent, v] => match (ent.parse::<u32>(), v.parse::<f32>()) {
            (Ok(e), Ok(v)) => {
                crate::set_anim_speed(e, v);
                "ok".into()
            }
            _ => "error: vel_chr <entidad> <velocidad>".into(),
        },
        ["atk_pc", id] => format!("{:?}", id.parse::<i32>().ok().and_then(atk_pc_fijo)),
        ["solo_suelo", v] => {
            SOLO_SUELO.store(*v == "1", Ordering::Relaxed);
            "ok".into()
        }
        ["ocultar", v] => {
            overlay::OCULTAR.store(*v == "1", Ordering::Relaxed);
            "ok".into()
        }
        ["vel_jugador", v] => match v.parse::<f32>() {
            Ok(v) => {
                crate::set_anim_speed(0, v);
                "ok".into()
            }
            _ => "error: vel_jugador <velocidad>".into(),
        },
        ["forzar_anim", ent, anim] => match (ent.parse::<u32>(), anim.parse::<i32>()) {
            (Ok(e), Ok(a)) => format!("{}", crate::chr_request_anim(e, a)),
            _ => "error: forzar_anim <entidad> <animación>".into(),
        },
        ["congelar", ent, v] => match ent.parse::<u32>() {
            Ok(e) => {
                crate::set_chr_disabled(e, *v == "1");
                "ok".into()
            }
            Err(_) => "error: congelar <entidad> <0|1>".into(),
        },
        ["desequipar", slot] => match slot.parse::<u32>().map(equip::unequip) {
            Ok(Ok(())) => "ok".into(),
            Ok(Err(e)) => format!("error: {e}"),
            Err(_) => "error: desequipar <ranura>".into(),
        },
        ["quitar", id] => match u32::from_str_radix(id.trim_start_matches("0x"), if id.starts_with("0x") { 16 } else { 10 }) {
            Ok(id) => match equip::remove(id, 1) {
                Ok(()) => "ok".into(),
                Err(e) => format!("error: {e}"),
            },
            _ => "error: quitar <id>".into(),
        },
        // Todo el inventario: id con su categoría en los 4 bits altos (4 = objeto) y cantidad.
        ["inventario_todo"] => equip::inventory_all().iter().map(|e| format!("{}:{:#x}:x{}", e.0, e.1, e.2)).collect::<Vec<_>>().join(" "),
        ["inventario"] => equip::inventory()
            .iter()
            .filter(|e| e.2 >> 28 == 0)
            .map(|e| format!("{}:{:#x}:{:#x}:g{}:x{}", e.0, e.1, e.2, equip::gem_of(e.1), e.3))
            .collect::<Vec<_>>()
            .join(" "),
        ["equipo"] => {
            equip::equipped()
                .iter()
                .enumerate()
                .map(|(s, &i)| format!("{s}:{}", equip::item_at(i).map(|x| format!("{x:#x}")).unwrap_or("-".into())))
                .collect::<Vec<_>>()
                .join(" ")
                + &format!(" | asm {:?}", equip::asm_ids())
        }
        ["buscar", id] => {
            format!("{:?}", u32::from_str_radix(id.trim_start_matches("0x"), if id.starts_with("0x") { 16 } else { 10 }).ok().and_then(equip::find_item))
        }
        ["nodead", v] => format!("{}", set_player_no_dead(*v == "1")),
        ["vida", n] => match n.parse::<i32>() {
            Ok(v) => {
                set_player_hp(v);
                format!("{:?}", player_hp_max())
            }
            Err(_) => "error: vida <n>".into(),
        },
        ["morir"] => match unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
            Some(p) => {
                p.chr_ins.modules.data.hp = 0;
                "ok".into()
            }
            None => "sin jugador".into(),
        },
        ["run", rest @ ..] => {
            let root = DIR.get().and_then(|d| d.parent()).and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_default();
            director::command(&root, rest)
        }
        // Postura (super armor) de un personaje: (actual, máxima, recuperación); con fracción, se la quita.
        ["postura", ent] | ["postura", ent, _] => match ent.parse::<u32>() {
            Ok(ent) => {
                if let Some(f) = cmd.get(2).and_then(|f| f.parse::<f32>().ok()) {
                    quitar_postura(ent, f);
                }
                let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return "error WorldChrMan".into() };
                let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
                sets.flat_map(|s| s.characters()).find(|c| c.event_entity_id == ent).map_or("no cargado".into(), |c| {
                    let m = &c.modules.super_armor;
                    let ta = &c.modules.time_act;
                    let anim = ta.anim_queue[(ta.write_idx as usize + ta.anim_queue.len() - 1) % ta.anim_queue.len()].anim_id;
                    format!("{:.1} / {:.1} recuperación {:.2} rota {} anim {anim}", m.sa_durability, m.sa_durability_max, m.recover_time, m.poise_broken_state)
                })
            }
            Err(_) => "error: postura <entidad> [fracción]".into(),
        },
        ["medir", "0"] => {
            *MEDIDA.lock().unwrap() = None;
            "ok".into()
        }
        ["medir", ent] | ["medir", ent, _] => match ent.parse::<u32>() {
            Ok(ent) => {
                fill_hp(ent);
                *MEDIDA.lock().unwrap() = Some(Medida {
                    ent,
                    atacar: cmd.get(2) == Some(&"atacar"),
                    recibidos: vec![],
                    dados: vec![],
                    vida_jugador: 0,
                    vida_enemigo: 0,
                    ultimo_ataque: Instant::now(),
                });
                "ok".into()
            }
            Err(_) => "error: medir <entidad> [atacar] | medir 0".into(),
        },
        ["medida"] => medida_json(),
        ["tramos"] => {
            // Multiplicadores de los tramos de escalado de zona (SpEffect 7000-7200).
            let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return "error SoloParamRepository".into() };
            let filas: Vec<serde_json::Value> = (7000..=7200)
                .step_by(10)
                .filter_map(|id| {
                    let r = repo.get::<eldenring::cs::SpEffectParam>(id)?;
                    Some(serde_json::json!({
                        "id": id,
                        "vida": r.max_hp_rate(),
                        "ataque": r.physics_attack_power_rate(),
                        "ataque_magia": r.magic_attack_power_rate(),
                        "defensa": r.physics_diffence_rate(),
                    }))
                })
                .collect();
            serde_json::Value::from(filas).to_string()
        }
        ["romper", ent] => match ent.parse::<u32>() {
            Ok(ent) => format!("{}", romper_postura(ent)),
            Err(_) => "error: romper <entidad>".into(),
        },
        ["red"] => red_info(None),
        ["red", ent] => match ent.parse::<u32>() {
            Ok(ent) => red_info(Some(ent)),
            _ => "error: red [entidad]".into(),
        },
        ["red", "control", ent, modo] => match ent.parse::<u32>() {
            Ok(ent) => red_control(ent, *modo == "remoto"),
            _ => "error: red control <entidad> <local|remoto>".into(),
        },
        // Vida de un enemigo (prueba de las fases de los jefes: tools/probar_fases.py).
        ["vida_chr", ent, hp] => match (ent.parse::<u32>(), hp.parse::<i32>()) {
            (Ok(e), Ok(h)) => {
                crate::set_chr_hp(e, h);
                "ok".into()
            }
            _ => "error: vida_chr <entidad> <vida>".into(),
        },
        ["chr", ent] | ["matar", ent] => {
            let Ok(ent) = ent.parse::<u32>() else { return "error id".into() };
            let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return "error WorldChrMan".into() };
            let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
            for set in sets {
                for c in set.characters() {
                    if c.event_entity_id != ent {
                        continue;
                    }
                    if cmd[0] == "matar" {
                        c.modules.data.hp = 0;
                        return "ok".into();
                    }
                    let (hp, hp_max) = (c.modules.data.hp, c.modules.data.max_hp);
                    let pos = &c.modules.physics.position;
                    let g = &c.chunk_position;
                    // Animación que reproduce (la última escrita en su cola), para ver si ataca.
                    let ta = &c.modules.time_act;
                    let anim = ta.anim_queue[(ta.write_idx as usize + ta.anim_queue.len() - 1) % ta.anim_queue.len()].anim_id;
                    return serde_json::json!({ "hp": hp, "hp_max": hp_max, "pos": [pos.0, pos.1, pos.2], "global": [g.0, g.1, g.2], "anim": anim }).to_string();
                }
            }
            "no cargado".into()
        }
        _ => format!("error orden desconocida: {}", cmd.join(" ")),
    }
}

/// Hilo lector: cada línea de cmd.txt es "<id> <orden...>"; cada id se procesa una sola vez. Las
/// órdenes que tocan el juego se pasan al hilo del juego por el canal que devuelve.
pub(crate) fn lector(dir: PathBuf) -> mpsc::Receiver<(String, Vec<String>)> {
    let (tx, rx) = mpsc::channel::<(String, Vec<String>)>();
    std::thread::spawn(move || {
        let mut done = std::collections::HashSet::new();
        let mut ticks = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            ticks += 1;
            if ticks.is_multiple_of(5) {
                enforce_window();
            }
            let Ok(text) = fs::read_to_string(dir.join("cmd.txt")) else { continue };
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                let Some(id) = parts.next() else { continue };
                if !done.insert(id.to_string()) {
                    continue;
                }
                let args: Vec<String> = parts.map(String::from).collect();
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                log(&format!("orden #{id}: {}", args.join(" ")));
                match run_outside(&refs) {
                    Some(r) => respond(id, &r),
                    None => {
                        let _ = tx.send((id.to_string(), args));
                    }
                }
            }
        }
    });
    rx
}

/// En cada fotograma, en el hilo del juego: modo dios, velocidad forzada y órdenes pendientes.
pub(crate) fn fotograma(ordenes: &mpsc::Receiver<(String, Vec<String>)>, frame: u64) {
    let v = f32::from_bits(SPEED_TEST.load(Ordering::Relaxed));
    if v > 0.0 {
        set_anim_speed(0, v);
    }
    medir_fotograma();
    if let Some(lista) = GRABACION.lock().unwrap().as_mut()
        && let Some(a) = player_anim()
        && lista.last().is_none_or(|x| x.1 != a)
    {
        lista.push((Instant::now(), a));
    }
    if GOD.load(Ordering::Relaxed)
        && let Ok(wcm) = unsafe { WorldChrMan::instance_mut() }
        && let Some(p) = wcm.main_player.as_mut()
    {
        // Invencible de verdad (el flag que usa el juego durante las cargas) y vida llena.
        p.chr_ins.chr_flags1c5.set_is_invincible(true);
        let d = &mut p.chr_ins.modules.data;
        if d.hp > 0 {
            d.hp = d.max_hp;
        }
    }
    while let Ok((id, args)) = ordenes.try_recv() {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_in_game(&refs, frame))).unwrap_or_else(|_| "error pánico".into());
        respond(&id, &r);
    }
}

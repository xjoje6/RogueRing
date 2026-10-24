//! Lo que el mod lee y escribe en la memoria del juego: personajes, efectos, equipo, proyectiles,
//! postura, cámara, interfaz y ventana.

use std::sync::atomic::Ordering;
use std::time::Duration;

use eldenring::cs::{CSEventFlagMan, CSMenuManImp, WorldChrMan};
use fromsoftware_shared::FromStatic;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

use crate::log;

pub(crate) fn flag_get(id: u32) -> bool {
    unsafe { CSEventFlagMan::instance() }.map(|m| m.virtual_memory_flag.get_flag(id)).unwrap_or(false)
}

pub(crate) fn flag_set(id: u32, v: bool) {
    if let Ok(m) = unsafe { CSEventFlagMan::instance_mut() } {
        m.virtual_memory_flag.set_flag(id, v);
    }
}

/// ¿Está visible el elemento `i` de la interfaz del juego? (19 = aviso de objeto nuevo).
pub(crate) fn ui_visible(i: usize) -> bool {
    unsafe { CSMenuManImp::instance() }.ok().and_then(|m| m.ui_states.get(i).map(|u| u.visible())).unwrap_or(false)
}

/// Barras de jefe del propio juego (CSFeManImp.boss_health_displays, 3 huecos): cada una sigue a un
/// rival con su nombre del juego (el nameId de su NpcParam) y el juego pone la vida y el número de
/// daño (23-10: barras reales en lugar de la nuestra). Sin rivales, las apaga. Devuelve cuántas.
pub(crate) fn barras_jefe(ents: &[u32], nombre_combate: i32, por_npc: &std::collections::HashMap<i32, i32>) -> usize {
    let Ok(fe) = (unsafe { eldenring::cs::CSFeManImp::instance_mut() }) else { return 0 };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return 0 };
    let repo = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok();
    let vacio: eldenring::cs::FieldInsHandle = unsafe { std::mem::transmute_copy(&[u32::MAX, u32::MAX]) };
    let sets: Vec<_> = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base)).collect();
    let mut n = 0;
    for (k, hueco) in fe.boss_health_displays.iter_mut().enumerate() {
        let chr = ents.get(k).and_then(|&e| sets.iter().find_map(|s| s.characters().find(|c| c.event_entity_id == e).map(|c| (c.field_ins_handle, c.npc_param_id))));
        match chr {
            Some((h, npc)) => {
                if hueco.field_ins_handle != h {
                    hueco.damage_taken = 0;
                }
                // Su nombre del juego o, si no tiene (los enemigos corrientes), el del combate; con varias
                // barras, el de su enemigo (data/nombres_jefe.json, «enemigos»).
                let propio = repo.and_then(|r| r.get::<eldenring::cs::NpcParam>(npc as u32)).map(|r| r.name_id()).filter(|&n| n > 0);
                let suyo = if ents.len() > 1 { por_npc.get(&npc).copied() } else { None };
                hueco.fmg_id = propio.or(suyo).unwrap_or(nombre_combate);
                hueco.field_ins_handle = h;
                n += 1;
            }
            None => {
                hueco.field_ins_handle = vacio;
                hueco.fmg_id = -1;
                hueco.damage_taken = 0;
            }
        }
    }
    n
}

/// HUD del juego (vida, PF, aguante y equipo): oculto en el menú del mod y en Personajes.
pub(crate) fn hud(visible: bool) {
    if let Ok(fe) = unsafe { eldenring::cs::CSFeManImp::instance_mut() } {
        fe.hud_state = if visible { eldenring::cs::CSFeManHudState::Default } else { eldenring::cs::CSFeManHudState::HideAll };
        fe.frontend_values.enable_equip_hud = visible;
    }
}

/// Desfase del giro hacia la cámara, en grados (orden `mirar` del banco, para ajustar la convención).
pub(crate) static MIRAR_DESFASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Gira al jugador para que mire a la cámara (pantalla de Personajes): se le ve de frente.
pub(crate) fn mirar_a_camara() -> bool {
    use eldenring::cs::CSCamExt;
    let Ok(cam) = (unsafe { eldenring::cs::CSCamera::instance() }) else { return false };
    let c = cam.pers_cam_1.position();
    let desfase = f32::from_bits(MIRAR_DESFASE.load(Ordering::Relaxed)).to_radians();
    mirar_hacia([c.0, c.1, c.2], desfase)
}

/// Gira al jugador hacia un punto (coordenadas de `player_pos`).
pub(crate) fn mirar_hacia(t: [f32; 3], desfase: f32) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let Some(p) = wcm.main_player.as_mut() else { return false };
    let phys = &mut p.chr_ins.modules.physics;
    let (dx, dz) = (t[0] - phys.position.0, t[2] - phys.position.2);
    // El personaje mira hacia -dirección del yaw (con atan2(dx, dz) queda de espaldas).
    let yaw = (-dx).atan2(-dz) + desfase;
    let q = eldenring::rotation::Quaternion(0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos());
    phys.orientation = q;
    phys.interpolated_orientation = q;
    true
}

/// Hacia dónde apunta el stick en el suelo, respecto a la cámara: un punto a 1 m del jugador. `lado`
/// cambia el sentido de izquierda y derecha (la convención de ejes del juego, por comprobar).
pub(crate) fn punto_del_stick(sx: f32, sy: f32, lado: f32) -> Option<[f32; 3]> {
    use eldenring::cs::CSCamExt;
    let cam = unsafe { eldenring::cs::CSCamera::instance() }.ok()?.pers_cam_1.position();
    let p = player_pos()?;
    let (fx, fz) = (p[0] - cam.0, p[2] - cam.2);
    let n = fx.hypot(fz);
    if n < 0.01 {
        return None;
    }
    let (fx, fz) = (fx / n, fz / n);
    let (rx, rz) = (fz * lado, -fx * lado);
    Some([p[0] + rx * sx + fx * sy, p[1], p[2] + rz * sx + fz * sy])
}

/// Gira al jugador sobre su eje hacia un punto, como mucho `max` radianes (convención de `mirar_hacia`).
pub(crate) fn girar_hacia(t: [f32; 3], max: f32) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let Some(p) = wcm.main_player.as_mut() else { return false };
    let phys = &mut p.chr_ins.modules.physics;
    let (dx, dz) = (t[0] - phys.position.0, t[2] - phys.position.2);
    let quiero = (-dx).atan2(-dz);
    let q = &phys.orientation;
    let ahora = 2.0 * q.1.atan2(q.3);
    let mut d = quiero - ahora;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    let yaw = ahora + d.clamp(-max, max);
    let q = eldenring::rotation::Quaternion(0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos());
    phys.orientation = q;
    phys.interpolated_orientation = q;
    true
}

/// Salir del juego desde el menú del mod: se pide un guardado y, un momento después, se cierra la
/// ventana como con Alt+F4.
pub(crate) fn salir_del_juego() {
    if let Ok(gm) = unsafe { eldenring::cs::GameMan::instance_mut() } {
        gm.save_requested = true;
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(1500));
        if let Some(h) = game_window() {
            unsafe {
                let _ = PostMessageW(Some(h), WM_CLOSE, windows::Win32::Foundation::WPARAM(0), LPARAM(0));
            }
        }
    });
}

/// Ventana principal de este proceso.
pub(crate) fn game_window() -> Option<HWND> {
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == GetCurrentProcessId() && IsWindowVisible(h).as_bool() && GetWindow(h, GW_OWNER).is_err() {
                *(l.0 as *mut isize) = h.0 as isize;
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    let mut found: isize = 0;
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut found as *mut isize as isize));
    }
    (found != 0).then_some(HWND(found as _))
}

/// Coloca al jugador en unas coordenadas locales del bloque actual (las del MSB). Devuelve el salto.
pub(crate) fn teleport_local(t: [f32; 3]) -> Option<f32> {
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let player = wcm.main_player.as_mut()?;
    let b = player.block_position;
    let (dx, dy, dz) = (t[0] - b.x, t[1] + 0.5 - b.y, t[2] - b.z);
    let phys = &mut player.chr_ins.modules.physics;
    phys.position.0 += dx;
    phys.position.1 += dy;
    phys.position.2 += dz;
    phys.chr_proxy_pos_update_requested = true;
    Some((dx * dx + dy * dy + dz * dz).sqrt())
}

/// ¿Está cargado en el mundo el personaje con esta entidad?
pub(crate) fn chr_loaded(ent: u32) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        if set.characters().any(|c| c.event_entity_id == ent) {
            return true;
        }
    }
    false
}

/// Aplica un SpEffect al personaje con esta entidad (p. ej. el tramo de escalado del acto).
pub(crate) fn apply_speffect_to(ent: u32, sp: i32) -> bool {
    use eldenring::cs::ChrInsExt;
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.apply_speffect(sp, false);
                return true;
            }
        }
    }
    false
}

/// Vida del personaje al máximo (tras cambiar su escalado).
pub(crate) fn fill_hp(ent: u32) -> Option<i32> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return None };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let d = &mut c.modules.data;
                d.hp = d.max_hp;
                return Some(d.max_hp);
            }
        }
    }
    None
}

/// Aplica o quita un SpEffect al jugador (reliquias).
pub(crate) fn player_speffect(sp: i32, apply: bool) -> bool {
    use eldenring::cs::ChrInsExt;
    let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else { return false };
    if apply {
        p.apply_speffect(sp, false);
    } else {
        p.remove_speffect(sp);
    }
    true
}

/// Runas del jugador (lectura y escritura).
pub(crate) fn runes() -> Option<u32> {
    let p = unsafe { WorldChrMan::instance() }.ok()?.main_player.as_ref()?;
    Some(unsafe { p.player_game_data.as_ref() }.rune_count)
}

pub(crate) fn set_runes(n: u32) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        unsafe { p.player_game_data.as_mut() }.rune_count = n;
    }
}

/// Borra la mancha de runas perdidas: al morir en una run no deben quedar en el coliseo para la
/// siguiente (cada run empieza sin runas).
pub(crate) fn clear_bloodstain() {
    if let Ok(g) = unsafe { eldenring::cs::GameDataMan::instance_mut() } {
        // El juego vuelve a poner `has_bloodstain` al cargar la zona: además de quitarla, la mancha
        // se queda sin runas y en un mapa que no existe, así que aunque vuelva no aparece donde se pelea.
        // Área 99: no existe (BlockId::none(), -1, el juego lo lee como «en todos los mapas»).
        let ninguna = eldenring::cs::BlockId::from_parts(99, 99, 0, 0);
        let b = &mut *g.bloodstain;
        if g.has_bloodstain || b.rune_count != 0 || b.block_id != ninguna {
            g.has_bloodstain = false;
            b.rune_count = 0;
            b.hero_points = 0;
            b.block_id = ninguna;
            log("runas perdidas borradas");
        }
    }
}

/// Descanso de gracia del propio juego: fundido, frascos recargados y vida llena (sin reiniciar el mundo).
pub(crate) fn rest_at_grace() -> bool {
    use eldenring::cs::{CSEventManImp, TimeTransitionParams};
    let Ok(em) = (unsafe { CSEventManImp::instance_mut() }) else { return false };
    let params = TimeTransitionParams { reset_main_character: true, reset_magic_charges: true, restore_estus: true, ..TimeTransitionParams::bonfire_rest() };
    em.world_area_time.fade_out_and_pass_time(TimeTransitionParams { reset_world: false, ..params });
    true
}

/// Invencibilidad del jugador (llegada a la arena; el modo dios la fuerza cada fotograma).
pub(crate) fn set_player_invincible(v: bool) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        p.chr_ins.chr_flags1c5.set_is_invincible(v);
    }
}

/// Pone la ventana del juego delante. Al arrancar a veces se queda detrás de la consola del lanzador y
/// entonces no hace caso al mando: el mod no pasa del título (16-10). Windows solo deja quitarle
/// el foco a otra ventana a quien comparte su entrada, así que se une un momento al hilo de la ventana
/// de delante.
pub(crate) fn traer_al_frente() -> bool {
    let Some(h) = game_window() else { return false };
    unsafe {
        let delante = GetForegroundWindow();
        let hilo = GetWindowThreadProcessId(delante, None);
        let mio = GetCurrentThreadId();
        let unido = hilo != 0 && hilo != mio && AttachThreadInput(mio, hilo, true).as_bool();
        let ok = SetForegroundWindow(h).as_bool();
        if unido {
            let _ = AttachThreadInput(mio, hilo, false);
        }
        ok
    }
}

/// El personaje de otro jugador de la sesión, por su SteamID (cada personaje de jugador apunta a su
/// entrada de la sesión).
fn con_jugador(steam: u64, f: impl FnOnce(&mut eldenring::cs::ChrIns)) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    for p in wcm.player_chr_set.characters() {
        let entrada = unsafe { std::ptr::addr_of!(p.session_manager_player_entry).cast::<*const eldenring::cs::SessionManagerPlayerEntryBase>().read() };
        if !entrada.is_null() && unsafe { (*entrada).steam_id } == steam {
            f(&mut p.chr_ins);
            return true;
        }
    }
    false
}

/// Posición del personaje de otro jugador de la sesión en este juego.
pub(crate) fn pos_jugador(steam: u64) -> Option<[f32; 3]> {
    let mut r = None;
    con_jugador(steam, |c| {
        let p = &c.modules.physics.position;
        r = Some([p.0, p.1, p.2]);
    });
    r
}

/// Pide una animación al propio personaje (ForceAnimationPlayback del EMEVD): las de caído del
/// cooperativo, en huecos de animación de evento (ModBuilder).
pub(crate) fn player_request_anim(anim: i32) -> bool {
    let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else { return false };
    p.chr_ins.modules.event.request_animation_id = anim;
    true
}

/// Tamaño del personaje de otro jugador (Diminuto, Goliat): su juego solo escala el suyo.
pub(crate) fn set_jugador_scale(steam: u64, s: f32) -> bool {
    con_jugador(steam, |c| {
        let ctrl = &mut c.chr_ctrl;
        ctrl.scale_size_x = s;
        ctrl.scale_size_y = s;
        ctrl.scale_size_z = s;
    })
}

/// Pone o quita un SpEffect al propio jugador.
pub(crate) fn speffect_propio(sp: i32, poner: bool) -> bool {
    use eldenring::cs::ChrInsExt;
    let Some(player) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else { return false };
    if poner {
        player.chr_ins.apply_speffect(sp, false);
    } else {
        player.chr_ins.remove_speffect(sp);
    }
    true
}

/// Críticos como en Nightreign (16-10): al enemigo que recibe un crítico o un agarre le pueden
/// pegar los demás (el compañero, las invocaciones) mientras dura. En ER no: las animaciones del que lo
/// recibe ponen cada fotograma la marca 67 de TAE (`invincible_excluding_throw_attacks_defender`). Se
/// quita en una tarea que va después de la animación y antes de la física y los golpes (lib.rs). Solo
/// a enemigos (tipo Npc): a un jugador agarrado no se le abre. No se mira el estado del agarre: cuando
/// el crítico lo hace un jugador remoto, en el juego del anfitrión el enemigo no está en «recibiendo» y
/// solo funcionaba en un sentido (17-10). La marca solo la ponen esas animaciones. Devuelve a
/// cuántos se les quitó.
pub(crate) fn abrir_criticos() -> usize {
    use eldenring::cs::ChrInsExt;
    use std::collections::HashMap;
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return 0 };
    let mut n = 0;
    let ahora = std::time::Instant::now();
    let Ok(mut abiertos) = ABIERTOS.lock() else { return 0 };
    let abiertos = abiertos.get_or_insert_with(HashMap::new);
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            // El tipo se lee como número (un valor que no esté en el enum no puede ser un enum de Rust).
            let tipo = unsafe { std::ptr::addr_of!(c.chr_type).cast::<i32>().read() };
            if tipo != 5 {
                continue;
            }
            // Y el resto de invencibilidades de sus animaciones (levantarse tras un crítico o una caída,
            // esquivas...): al enemigo se le puede pegar siempre (23-10: ni en los críticos ni al
            // levantarse se le podía).
            let clave = &*c as *const _ as usize;
            let f = &mut c.modules.action_flag.action_modifiers_flags;
            let antes = f.0;
            f.set_invincible_excluding_throw_attacks_defender(false);
            f.set_perfect_invincibility(false);
            f.set_dodging(false);
            f.set_jump_frames_lower_body_iframes(false);
            f.set_pve_only_iframes(false);
            if f.0 != antes {
                n += 1;
                // Supercoraza, como en sus ataques: el golpe le hace daño pero no le corta la animación
                // (24-10: pegarle al levantarse lo ponía de pie y atacando al instante).
                f.set_super_armor(true);
                // Lo que se le pega así no le baja la postura (como en Nightreign): si no, se rompía a
                // golpes mientras recibía un crítico o se levantaba.
                if abiertos.insert(clave, ahora).is_none() {
                    c.apply_speffect(SP_SIN_POSTURA, false);
                }
            } else if abiertos.get(&clave).is_some_and(|t| ahora.duration_since(*t) > std::time::Duration::from_millis(250)) {
                abiertos.remove(&clave);
                c.remove_speffect(SP_SIN_POSTURA);
            }
        }
    }
    n
}

/// Sin daño de postura (tools/ModBuilder): mientras se le quita al enemigo la invencibilidad de sus
/// animaciones (abrir_criticos).
const SP_SIN_POSTURA: i32 = 79_619_055;
/// Enemigos con la invencibilidad quitada y cuándo se vio por última vez (abrir_criticos).
static ABIERTOS: std::sync::Mutex<Option<std::collections::HashMap<usize, std::time::Instant>>> = std::sync::Mutex::new(None);

/// Dibujar o no al personaje con esta entidad (0 = jugador): el caído del cooperativo no se ve.
pub(crate) fn set_chr_render(ent: u32, v: bool) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    if ent == 0 {
        if let Some(p) = wcm.main_player.as_mut() {
            p.chr_ins.chr_flags1c5.set_enable_render(v);
        }
        return;
    }
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.chr_flags1c5.set_enable_render(v);
                return;
            }
        }
    }
}

/// Invencibilidad de un personaje por su entidad: los rivales mientras están quietos al empezar el
/// combate (si no, se les pegaba gratis; 17-10).
pub(crate) fn set_chr_invincible(ent: u32, v: bool) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                if c.chr_flags1c5.is_invincible() != v {
                    c.chr_flags1c5.set_is_invincible(v);
                }
                return;
            }
        }
    }
}

/// Personaje quieto (sin moverse ni actuar) aunque el mando llegue por otra vía que XInput
/// (con un mando de PS5 el juego usa libScePad y no pasa por nuestro desvío).
pub(crate) fn set_player_locked(v: bool) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let f = &mut p.chr_ins.debug_flags;
        if f.disabled_movement() != v || f.disabled_secondary_actions() != v {
            f.set_disabled_movement(v);
            f.set_disabled_secondary_actions(v);
        }
    }
}

/// Número de frascos carmesí (el reparto que se hace en las gracias). El descanso los rellena.
pub(crate) fn set_hp_flasks(n: u8) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        unsafe { p.player_game_data.as_mut() }.max_hp_flask = n;
    }
}

#[cfg(feature = "banco")]
/// Carga máxima que calcula el juego (la del menú de estado).
pub(crate) fn max_equip_load() -> Option<f32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    Some(unsafe { wcm.main_player.as_ref()?.player_game_data.as_ref() }.max_equip_load)
}

/// Área del bloque en que está el jugador (45 = coliseos).
pub(crate) fn player_area() -> Option<u8> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    Some(wcm.main_player.as_ref()?.current_block_id.area())
}

/// Vida del personaje con esta entidad.
pub(crate) fn chr_hp(ent: u32) -> Option<i32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return Some(c.modules.data.hp);
            }
        }
    }
    None
}

/// Pone la vida del personaje con esta entidad (sin pasar del máximo).
pub(crate) fn set_chr_hp(ent: u32, hp: i32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let d = &mut c.modules.data;
                d.hp = hp.clamp(0, d.max_hp);
                return;
            }
        }
    }
}

/// Vida y vida máxima del jugador.
pub(crate) fn player_hp_max() -> Option<(i32, i32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let d = &wcm.main_player.as_ref()?.chr_ins.modules.data;
    Some((d.hp, d.max_hp))
}

/// Suma vida al jugador (sin pasar del máximo ni revivirlo).
pub(crate) fn heal_player_by(n: i32) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let d = &mut p.chr_ins.modules.data;
        if d.hp > 0 {
            d.hp = (d.hp + n).min(d.max_hp);
        }
    }
}

/// Reescribe el multiplicador de daño recibido de un SpEffect (filas reservadas; < 1 = más defensa).
/// Ataque y defensa que da un SpEffect de escalado (tramo 7000-7999) a quien lo lleva.
pub(crate) fn tramo_rates(id: i32) -> Option<(f32, f32)> {
    let repo = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok()?;
    let row = repo.get::<eldenring::cs::SpEffectParam>(id as u32)?;
    Some((row.physics_attack_power_rate(), row.physics_diffence_rate()))
}

/// Aviso de cambio de fase de un jefe (la primera fase aún no ha caído).
pub(crate) fn aviso_fase() {
    crate::director::aviso(rogue_core::idioma::t("Not fallen yet"), rogue_core::idioma::t("Its true strength awakens."));
}

pub(crate) fn set_speffect_cut_rate(id: i32, rate: f32) -> bool {
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return false };
    let Some(row) = repo.get_mut::<eldenring::cs::SpEffectParam>(id as u32) else { return false };
    row.set_slash_damage_cut_rate(rate);
    row.set_blow_damage_cut_rate(rate);
    row.set_thrust_damage_cut_rate(rate);
    row.set_neutral_damage_cut_rate(rate);
    row.set_magic_damage_cut_rate(rate);
    row.set_fire_damage_cut_rate(rate);
    row.set_thunder_damage_cut_rate(rate);
    row.set_dark_damage_cut_rate(rate);
    true
}

/// Fija los atributos (vig men agu fue des int fe arc) y el nivel, y pide guardar. El juego
/// recalcula vida, PC y aguante al recargar la partida (INVESTIGAR forzarlo sin recargar).
pub(crate) fn set_attributes(v: [u32; 8]) -> Option<u32> {
    let p = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut())?;
    let gd = unsafe { p.player_game_data.as_mut() };
    (gd.vigor, gd.mind, gd.endurance, gd.strength, gd.dexterity, gd.intelligence, gd.faith, gd.arcane) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
    gd.level = v.iter().sum::<u32>().saturating_sub(79).max(1);
    if let Ok(gm) = unsafe { eldenring::cs::GameMan::instance_mut() } {
        gm.save_requested = true;
    }
    Some(gd.level)
}

/// Reescribe el multiplicador de ataque de un SpEffect (filas reservadas por tools/ModBuilder).
/// Daño fijo de cada tipo del ataque de un proyectil del jugador (AtkParam_Pc), para el banco.
pub(crate) fn atk_pc_fijo(id: i32) -> Option<[u16; 5]> {
    let repo = unsafe { eldenring::cs::SoloParamRepository::instance() }.ok()?;
    let row = repo.get::<eldenring::cs::AtkParam_Pc>(id as u32)?;
    Some([row.atk_phys(), row.atk_mag(), row.atk_fire(), row.atk_thun(), row.atk_dark()])
}

pub(crate) fn set_speffect_attack_rate(id: i32, rate: f32) -> bool {
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return false };
    let Some(row) = repo.get_mut::<eldenring::cs::SpEffectParam>(id as u32) else { return false };
    row.set_physics_attack_power_rate(rate);
    row.set_magic_attack_power_rate(rate);
    row.set_fire_attack_power_rate(rate);
    row.set_thunder_attack_power_rate(rate);
    row.set_dark_attack_power_rate(rate);
    true
}

/// SpEffects activos en el personaje con esta entidad (0 = jugador).
pub(crate) fn chr_speffects(ent: u32) -> Vec<i32> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return vec![] };
    if ent == 0 {
        return wcm.main_player.as_ref().map(|p| p.chr_ins.special_effect.entries().map(|e| e.param_id).collect()).unwrap_or_default();
    }
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return c.special_effect.entries().map(|e| e.param_id).collect();
            }
        }
    }
    vec![]
}

#[cfg(feature = "banco")]
/// `stateInfo` de los SpEffects activos del personaje (tipo de estado: podredumbre 275, congelación
/// 120, sueño 434/267; medido en el juego con las ratas del Nido).
pub(crate) fn chr_state_infos(ent: u32) -> Vec<u16> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return vec![] };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return c.special_effect.entries().filter_map(|e| e.param_data.map(|p| unsafe { p.as_ref() }.state_info())).collect();
            }
        }
    }
    vec![]
}

/// Contadores de estado del personaje (módulo de resistencias, offsets de la tabla de TGA):
/// [veneno, podredumbre, sangrado, plaga, congelación, sueño, locura] y sus máximos.
pub(crate) fn chr_resist(ent: u32) -> Option<([i32; 7], [i32; 7])> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                unsafe {
                    let modules = &*c.modules as *const _ as *const u8;
                    let resist = *(modules.add(0x20) as *const *const u8);
                    if resist.is_null() {
                        return None;
                    }
                    let rd = |off: usize| *(resist.add(off) as *const i32);
                    let cur = std::array::from_fn(|i| rd(0x10 + 4 * i));
                    let max = std::array::from_fn(|i| rd(0x2C + 4 * i));
                    return Some((cur, max));
                }
            }
        }
    }
    None
}

/// Animación que está reproduciendo el jugador (id completo, p. ej. 42035000).
pub(crate) fn player_anim() -> Option<i32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let ta = &wcm.main_player.as_ref()?.chr_ins.modules.time_act;
    // Cola circular: la última escrita es la actual.
    let i = (ta.write_idx as usize + ta.anim_queue.len() - 1) % ta.anim_queue.len();
    Some(ta.anim_queue[i].anim_id)
}

/// Apaga o enciende del todo al personaje con esta entidad: sin actualizarse, dibujarse ni física
/// (`ChrDebugFlags::character_disabled`). Los enemigos colocados en el MSB del coliseo existen desde
/// que carga el mapa; así esperan a su oleada (el evento del mapa no llegaba a apagarlos).
pub(crate) fn set_chr_disabled(ent: u32, v: bool) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.debug_flags.set_character_disabled(v);
                return;
            }
        }
    }
}

/// Velocidad de animación (1 = normal) del personaje con esta entidad; 0 = jugador. El juego la
/// recalcula, así que se reescribe cada fotograma mientras dure el efecto.
pub(crate) fn set_anim_speed(ent: u32, v: f32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    if ent == 0 {
        if let Some(p) = wcm.main_player.as_mut() {
            p.chr_ins.modules.behavior.animation_speed = if en_agarre(&p.chr_ins) { 1.0 } else { v };
        }
        return;
    }
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.modules.behavior.animation_speed = if en_agarre(c) { 1.0 } else { v };
                return;
            }
        }
    }
}

/// Velocidad de animación del jugador y estado crudo de su nodo de agarres (banco de pruebas).
#[cfg(feature = "banco")]
pub(crate) fn velocidad_y_agarre() -> Option<(f32, u32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let p = wcm.main_player.as_ref()?;
    let nodo = &*p.chr_ins.modules.throw.throw_node;
    let estado = unsafe { std::ptr::addr_of!(nodo.throw_state).cast::<u32>().read() };
    Some((p.chr_ins.modules.behavior.animation_speed, estado))
}

/// ¿El jugador está atacando? Ataques (familia x03xxxx, sin la guardia x035xxx), cenizas (x04xxxx),
/// o un crítico o agarre. Medido en el juego con los tres personajes (ligero, fuerte, cargado,
/// ceniza, esquiva y guardia).
pub(crate) fn jugador_atacando() -> bool {
    let anim = player_anim().is_some_and(|a| {
        let f = a.rem_euclid(1_000_000);
        (30_000..50_000).contains(&f) && !(35_000..36_000).contains(&f)
    });
    anim || unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.main_player.as_ref()).is_some_and(|p| en_agarre(&p.chr_ins))
}

/// ¿El último golpe que recibió `ent` fue del jugador? (`ChrIns::last_hit_by`.) None si no se
/// encuentra o nadie le ha pegado. Atacar mientras otro pega no basta: en cooperativo los golpes del
/// compañero contaban para las bendiciones de quien estuviera atacando (16-10), y también los
/// de las invocaciones.
pub(crate) fn golpeado_por_mi(ent: u32) -> Option<bool> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let yo = wcm.main_player.as_ref()?.chr_ins.field_ins_handle;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return (!c.last_hit_by.is_empty()).then(|| c.last_hit_by == yo);
            }
        }
    }
    None
}

/// Marca que el ataque de cada proyectil de las bendiciones pone al rival que toca (tools/ModBuilder,
/// 0,15 s): el juego apunta ese daño a tu nombre, pero no es un golpe de tu arma. (El campo del juego
/// «último ataque recibido» no sirve: en el banco, 23-10, vale siempre 99999.)
const SP_IMPACTO_BENDICION: i32 = 79_619_053;

/// ¿La bajada de vida de `ent` es un golpe del jugador? Atacando, si el juego sabe quién le pegó, que sea
/// él, y sin la marca de un proyectil de bendición (23-10: contar golpes con la lógica del juego).
pub(crate) fn golpe_mio(ent: u32) -> bool {
    jugador_atacando() && golpeado_por_mi(ent) != Some(false) && !chr_speffects(ent).contains(&SP_IMPACTO_BENDICION)
}

/// ¿Está en un crítico o un agarre, como atacante o como víctima? Ahí las animaciones de los dos van
/// emparejadas: con otra velocidad en una de ellas se desincronizan, así que va a velocidad normal.
fn en_agarre(c: &eldenring::cs::ChrIns) -> bool {
    let nodo = &*c.modules.throw.throw_node;
    // El estado se lee como número: en reposo vale algo que no es ninguna variante del enum.
    let estado = unsafe { std::ptr::addr_of!(nodo.throw_state).cast::<u32>().read() };
    (3..=6).contains(&estado)
}

/// Réplica de `BulletSpawnData` de fromsoftware-rs (sus campos son privados): misma disposición y
/// alineada a 16 como el original (sus vectores son F32Vector4). El juego los lee con instrucciones
/// que piden esa alineación: sin ella, que funcione depende de dónde caiga en la pila, y cuando cae
/// mal cualquier proyectil cierra el juego.
#[repr(C, align(16))]
struct SpawnData {
    owner: [u32; 2],
    behavior_id: i32,
    magic_id: i32,
    unk10: u32,
    bullet_id: i32,
    goods_id: i32,
    dummy_poly_id: i32,
    target: [u32; 2],
    unk28: u32,
    unk2c: u32,
    unk30: [f32; 4],
    unk40: u32,
    unk44: u32,
    pad48: [u8; 8],
    acceleration_angle: [f32; 4],
    unk60: [f32; 4],
    angle: [f32; 4],
    position: [f32; 4],
    unk90: [u64; 3],
    pada8: [u8; 8],
    unkb0: [u8; 0x50],
    unk100: u8,
    pad101: [u8; 15],
}

const _: () = {
    use std::mem::{align_of, size_of};
    assert!(size_of::<SpawnData>() == 0x110 && size_of::<SpawnData>() == size_of::<eldenring::cs::BulletSpawnData>());
    assert!(align_of::<SpawnData>() == 16 && align_of::<SpawnData>() >= align_of::<eldenring::cs::BulletSpawnData>());
};

/// Lanza un proyectil del juego (Bullet) desdel jugador, hacia delante o hacia `target` (entidad).
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub(crate) fn spawn_bullet(bullet_id: i32, target: Option<u32>) -> Result<(), String> {
    spawn_bullet_at(bullet_id, target, None)
}

/// Origen por defecto de los proyectiles (comportamiento, hechizo, objeto; -1 = ninguno). Banco:
/// `bala_origen`.
pub(crate) static BALA_ORIGEN: std::sync::Mutex<[i32; 3]> = std::sync::Mutex::new([-1, -1, -1]);

/// Como `spawn_bullet`, pero saliendo de `desde` (coordenadas de `chr_pos`) si se da.
pub(crate) fn spawn_bullet_at(bullet_id: i32, target: Option<u32>, desde: Option<[f32; 3]>) -> Result<(), String> {
    let origen = BALA_ORIGEN.lock().map(|o| *o).unwrap_or([-1, -1, -1]);
    spawn_bullet_origen(bullet_id, target, desde, origen)
}

/// Como `spawn_bullet_at`, con origen (comportamiento, hechizo, objeto). Un proyectil del jugador solo
/// hace el daño de su ataque si sale de un hechizo (banco, 23-10): los de daño de verdad de las
/// bendiciones salen así (motor.rs).
pub(crate) fn spawn_bullet_origen(bullet_id: i32, target: Option<u32>, desde: Option<[f32; 3]>, origen: [i32; 3]) -> Result<(), String> {
    let wcm = unsafe { WorldChrMan::instance() }.map_err(|_| "sin WorldChrMan")?;
    let p = wcm.main_player.as_ref().ok_or("sin jugador")?;
    let owner: [u32; 2] = unsafe { std::mem::transmute_copy(&p.chr_ins.field_ins_handle) };
    let pos = p.chr_ins.modules.physics.position;
    let mut tgt = [u32::MAX, u32::MAX];
    let mut tgt_pos = None;
    if let Some(ent) = target {
        let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
        'b: for set in sets {
            for c in set.characters() {
                if c.event_entity_id == ent {
                    tgt = unsafe { std::mem::transmute_copy(&c.field_ins_handle) };
                    let q = &c.modules.physics.position;
                    tgt_pos = Some([q.0, q.1, q.2]);
                    break 'b;
                }
            }
        }
    }
    let salida = match desde {
        Some(p) => p,
        None => [pos.0, pos.1 + 1.2, pos.2],
    };
    // Sale mirando al objetivo (al pecho, no a los pies). Con el avance fijo, los proyectiles que giran
    // poco (las glintblades) se iban hacia un lado del coliseo y no llegaban nunca.
    let avance = tgt_pos
        .map(|t| [t[0] - salida[0], t[1] + 1.0 - salida[1], t[2] - salida[2]])
        .and_then(|v| {
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            (n > 0.1).then(|| [v[0] / n, v[1] / n, v[2] / n, 0.0])
        })
        .unwrap_or([0.0, 0.0, 1.0, 0.0]);
    let d = SpawnData {
        owner,
        behavior_id: origen[0],
        magic_id: origen[1],
        unk10: 0,
        bullet_id,
        goods_id: origen[2],
        dummy_poly_id: -1,
        target: tgt,
        unk28: 0,
        unk2c: 0,
        unk30: [0.0; 4],
        unk40: 0,
        unk44: 0,
        pad48: [0; 8],
        acceleration_angle: [0.0; 4],
        unk60: [0.0; 4],
        angle: avance,
        position: [salida[0], salida[1], salida[2], 1.0],
        unk90: [0; 3],
        pada8: [0; 8],
        unkb0: [0; 0x50],
        unk100: 0,
        pad101: [0; 15],
    };
    let bm = unsafe { eldenring::cs::CSBulletManager::instance_mut() }.map_err(|_| "sin CSBulletManager")?;
    let data = unsafe { &*(&d as *const SpawnData as *const eldenring::cs::BulletSpawnData) };
    bm.spawn_bullet(data).map_err(|e| format!("error del juego {e}"))
}

/// Posición del jugador (mismas coordenadas que `chr_pos`).
pub(crate) fn player_pos() -> Option<[f32; 3]> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let p = &wcm.main_player.as_ref()?.chr_ins.modules.physics.position;
    Some([p.0, p.1, p.2])
}

/// Postura del personaje (super armor). La rompe un golpe que la hace cruzar de positiva a 0: si se
/// deja en 0 justo, no queda nada que cruzar y no se rompe nunca. Por eso se deja como poco en
/// `POSTURA_MINIMA` y el siguiente golpe la rompe (en un jefe, caída con crítico). El juego la rellena entera en cuanto pasa `recover_time` sin daño de
/// postura, así que al bajarla también se retiene esa recuperación.
pub(crate) fn set_poise(ent: u32, v: f32) {
    let v = v.max(POSTURA_MINIMA);
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let m = &mut c.modules.super_armor;
                m.sa_durability = v;
                m.recover_time = m.recover_time.max(POSTURA_RETENIDA);
                return;
            }
        }
    }
}

/// Le quita a la postura esa fracción de la máxima (desvío del samurái). Si la deja a 0, la rompe en
/// el acto. Devuelve (antes, después, máxima).
pub(crate) fn quitar_postura(ent: u32, fraccion: f32) -> Option<(f32, f32, f32)> {
    let (p, max) = chr_poise(ent)?;
    let v = p - max * fraccion;
    if v <= POSTURA_MINIMA {
        romper_postura(ent);
        return Some((p, 0.0, max));
    }
    set_poise(ent, v);
    Some((p, v, max))
}

/// Rompe la postura ya: la deja en lo mínimo y le suelta encima el golpe de rotura (Bullet 79600010,
/// tools/ModBuilder), que la cruza. Sin ese golpe, en 0 el juego no la rompe nunca.
pub(crate) fn romper_postura(ent: u32) -> bool {
    let Some(p) = chr_pos(ent) else { return false };
    set_poise(ent, POSTURA_MINIMA);
    let ok = spawn_bullet_at(BALA_ROTURA, Some(ent), Some([p[0], p[1] + 1.0, p[2]])).is_ok();
    log(&format!("postura: rotura ({ok})"));
    ok
}

/// Proyectil de la rotura de postura (onda invisible del Rompetierra con mucho daño de postura).
const BALA_ROTURA: i32 = 79_600_010;

/// Segundos que la postura del enemigo tarda en recuperarse tras perderla (como en Sekiro, no al momento).
const POSTURA_RETENIDA: f32 = 4.0;
/// Lo que queda como poco al bajar la postura: cualquier golpe la cruza.
const POSTURA_MINIMA: f32 = 0.5;

/// Postura (actual, máxima) del personaje con esta entidad.
pub(crate) fn chr_poise(ent: u32) -> Option<(f32, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let m = &c.modules.super_armor;
                return Some((m.sa_durability, m.sa_durability_max));
            }
        }
    }
    None
}

/// Quita un SpEffect al personaje con esta entidad.
pub(crate) fn remove_speffect_from(ent: u32, sp: i32) -> bool {
    use eldenring::cs::ChrInsExt;
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.remove_speffect(sp);
                return true;
            }
        }
    }
    false
}

/// Reescribe el multiplicador de vida máxima de una fila reservada de SpEffect.
pub(crate) fn set_speffect_max_hp_rate(id: i32, rate: f32) -> bool {
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return false };
    let Some(row) = repo.get_mut::<eldenring::cs::SpEffectParam>(id as u32) else { return false };
    row.set_max_hp_rate(rate);
    true
}

/// El jugador no muere (se queda a 1 de vida): opción de depuración del propio juego.
pub(crate) fn set_player_no_dead(v: bool) -> bool {
    match unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
        Ok(d) => {
            d.player_no_dead = v;
            true
        }
        Err(_) => false,
    }
}

/// Duración original de la ventana de desvío (102001, 102011, 102013, 102015).
static DEFLECT_WINDOW: std::sync::Mutex<Option<[f32; 4]>> = std::sync::Mutex::new(None);

/// Multiplica la duración de la ventana de desvío (1.0 = la del juego).
pub(crate) fn scale_deflect_window(mult: f32) {
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return };
    let ids = [102_001u32, 102_011, 102_013, 102_015];
    let Ok(mut orig) = DEFLECT_WINDOW.lock() else { return };
    if orig.is_none() {
        let mut v = [0.0f32; 4];
        for (i, id) in ids.iter().enumerate() {
            v[i] = repo.get_mut::<eldenring::cs::SpEffectParam>(*id).map(|r| r.effect_endurance()).unwrap_or(0.0);
        }
        *orig = Some(v);
    }
    let base = orig.unwrap();
    for (i, id) in ids.iter().enumerate() {
        if let Some(r) = repo.get_mut::<eldenring::cs::SpEffectParam>(*id) {
            r.set_effect_endurance(base[i] * mult);
        }
    }
}

/// Letrero grande del propio juego ("ENEMIGO ABATIDO", "COMIENZA", "VICTORIA"...).
pub(crate) fn status_message(id: i32) -> bool {
    match unsafe { eldenring::cs::CSMenuManImp::instance_mut() } {
        Ok(m) => m.display_status_message(id),
        Err(_) => false,
    }
}

/// Tamaño del modelo del personaje con esta entidad (0 = jugador); 1 = normal.
pub(crate) fn set_chr_scale(ent: u32, s: f32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let apply = |c: &mut eldenring::cs::ChrIns| {
        let ctrl = &mut c.chr_ctrl;
        ctrl.scale_size_x = s;
        ctrl.scale_size_y = s;
        ctrl.scale_size_z = s;
    };
    if ent == 0 {
        if let Some(p) = wcm.main_player.as_mut() {
            apply(&mut p.chr_ins);
        }
        return;
    }
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                apply(c);
                return;
            }
        }
    }
}

/// Opacidad del fundido a negro del juego (0 = pantalla despejada; 1 = negro: cargas y transiciones).
pub(crate) fn screen_fade() -> f32 {
    let Ok(f) = (unsafe { eldenring::cs::CSFade::instance() }) else { return 0.0 };
    f.fade_plates.iter().map(|p| p.current_color.a).fold(0.0, f32::max)
}

/// PF actuales del jugador.
pub(crate) fn player_fp() -> Option<i32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    Some(wcm.main_player.as_ref()?.chr_ins.modules.data.fp)
}

/// Suma (o resta) PF al jugador, sin pasar del máximo ni bajar de 0.
pub(crate) fn add_player_fp(n: i32) {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let d = &mut p.chr_ins.modules.data;
        d.fp = (d.fp + n).clamp(0, d.max_fp);
    }
}

/// Aguante y PF del jugador al máximo.
pub(crate) fn refill_player_stamina_fp() {
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let d = &mut p.chr_ins.modules.data;
        d.stamina = d.max_stamina;
        d.fp = d.max_fp;
    }
}

/// Fija la vida del jugador.
/// Cooperativo, caídos (director/caido.rs): nadie muere en el combate. El efecto noDead solo frena el
/// daño del juego; lo que escribe el mod (el motor: maldiciones, daño aplazado) tampoco baja de 1
/// (23-10: dos muertes de verdad así).
pub(crate) static VIDA_MINIMA_1: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Veces que `suelo_de_vida` ha parado un golpe mortal (el director lo apunta en el registro).
pub(crate) static GOLPES_PARADOS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Banco: sin el efecto noDead ni la opción de depuración, para probar solo `suelo_de_vida`.
pub(crate) static SOLO_SUELO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Justo después del daño del fotograma (tarea en DmgMan_Post, lib.rs) y antes de que el juego procese
/// la muerte: con `VIDA_MINIMA_1`, una vida en 0 vuelve a 1. No depende de efectos del juego ni de
/// Seamless (23-10: en partida en línea el efecto noDead no paró dos golpes mortales).
pub(crate) fn suelo_de_vida() {
    if !VIDA_MINIMA_1.load(Ordering::Relaxed) {
        return;
    }
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let d = &mut p.chr_ins.modules.data;
        if d.hp <= 0 && d.max_hp > 0 {
            d.hp = 1;
            GOLPES_PARADOS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) fn set_player_hp(v: i32) {
    let suelo = if VIDA_MINIMA_1.load(Ordering::Relaxed) { 1 } else { 0 };
    if let Some(p) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.main_player.as_mut()) {
        let d = &mut p.chr_ins.modules.data;
        d.hp = v.clamp(suelo, d.max_hp.max(suelo));
    }
}

/// Posición (coordenadas del bloque) del personaje con esta entidad.
pub(crate) fn chr_pos(ent: u32) -> Option<[f32; 3]> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let p = &c.modules.physics.position;
                return Some([p.0, p.1, p.2]);
            }
        }
    }
    None
}

/// Animación actual del personaje con esta entidad.
pub(crate) fn chr_anim(ent: u32) -> Option<i32> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    let c = sets.flat_map(|s| s.characters()).find(|c| c.event_entity_id == ent)?;
    let ta = &c.modules.time_act;
    let i = (ta.write_idx as usize + ta.anim_queue.len() - 1) % ta.anim_queue.len();
    Some(ta.anim_queue[i].anim_id)
}

/// Coloca al personaje con esta entidad en unas coordenadas (las de `chr_pos`).
pub(crate) fn set_chr_pos(ent: u32, p: [f32; 3]) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                let phys = &mut c.modules.physics;
                phys.position.0 = p[0];
                phys.position.1 = p[1];
                phys.position.2 = p[2];
                phys.chr_proxy_pos_update_requested = true;
                return true;
            }
        }
    }
    false
}

/// Pide al personaje con esta entidad una animación, como `ForceAnimationPlayback` del EMEVD (el juego
/// la pone en el frame siguiente: `CSChrEventModule::request_animation_id`). Sirve para sacar de su
/// espera a los jefes de Nightreign que en su juego arranca el mapa.
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub(crate) fn chr_request_anim(ent: u32, anim: i32) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                c.modules.event.request_animation_id = anim;
                return true;
            }
        }
    }
    false
}

/// Vida y vida máxima del personaje con esta entidad.
pub(crate) fn chr_hp_max(ent: u32) -> Option<(i32, i32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return Some((c.modules.data.hp, c.modules.data.max_hp));
            }
        }
    }
    None
}

/// Identificador de red (`FieldInsHandle`) del personaje con esta entidad, para el registro del
/// cooperativo: el jefe colocado en el coliseo sale como `FieldIns(m45_00_00_00, 107, 1)`.
pub(crate) fn chr_handle(ent: u32) -> Option<String> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let sets = wcm.chr_sets.iter().flatten().map(|s| &**s).chain(std::iter::once(&wcm.open_field_chr_set.base));
    for set in sets {
        for c in set.characters() {
            if c.event_entity_id == ent {
                return Some(c.field_ins_handle.to_string());
            }
        }
    }
    None
}

/// ¿Va cargado Seamless Co-op (ersc.dll)?
pub(crate) fn seamless_cargado() -> bool {
    unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleA(windows::core::s!("ersc.dll")) }.is_ok()
}

/// ersc_settings.ini que va junto a ersc.dll (el Seamless cargado).
pub(crate) fn seamless_ini() -> Option<std::path::PathBuf> {
    use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleA};
    let m = unsafe { GetModuleHandleA(windows::core::s!("ersc.dll")) }.ok()?;
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(m), &mut buf) } as usize;
    Some(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n])).parent()?.join("ersc_settings.ini"))
}

/// Pone la contraseña de la sesión en el INI de Seamless (la elegida dentro del juego). Nunca se
/// escribe en el registro.
pub(crate) fn poner_clave_seamless(clave: &str) -> bool {
    let Some(ini) = seamless_ini() else { return false };
    let Ok(texto) = std::fs::read_to_string(&ini) else { return false };
    let mut puesta = false;
    let mut nuevo: Vec<String> = texto
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("cooppassword") {
                puesta = true;
                format!("cooppassword = {clave}")
            } else {
                l.to_string()
            }
        })
        .collect();
    if !puesta {
        nuevo.push(format!("cooppassword = {clave}"));
    }
    std::fs::write(&ini, nuevo.join("\r\n") + "\r\n").is_ok()
}

/// Contraseña de la sesión que hay en el INI de Seamless (para empezar el selector con ella).
pub(crate) fn clave_seamless() -> Option<String> {
    let texto = std::fs::read_to_string(seamless_ini()?).ok()?;
    texto.lines().find_map(|l| l.trim().strip_prefix("cooppassword")?.trim().strip_prefix('=').map(|v| v.trim().to_string()))
}

/// `save_file_extension` del ersc_settings.ini que va junto a ersc.dll (co2 si no está). Solo se
/// lee esa clave: el mismo archivo lleva la contraseña de la sesión.
pub(crate) fn seamless_extension() -> Option<String> {
    use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleA};
    let m = unsafe { GetModuleHandleA(windows::core::s!("ersc.dll")) }.ok()?;
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(m), &mut buf) } as usize;
    let dll = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    let ini = std::fs::read_to_string(dll.parent()?.join("ersc_settings.ini")).ok()?;
    let ext = ini.lines().find_map(|l| l.trim().strip_prefix("save_file_extension")?.trim().strip_prefix('=').map(|v| v.trim().to_string()));
    Some(ext.filter(|e| !e.is_empty()).unwrap_or_else(|| "co2".into()))
}

/// Partida que lee el juego con Seamless: ER0000.<extensión>, se ponga lo que se ponga en me3. Solo
/// es del mod con una extensión propia; con co2 o sl2 (Err) sería la de Seamless o la normal.
pub(crate) fn partida_seamless() -> Result<String, String> {
    let ext = seamless_extension().unwrap_or_else(|| "co2".into());
    if ext.eq_ignore_ascii_case("co2") || ext.eq_ignore_ascii_case("sl2") || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ext);
    }
    Ok(format!("ER0000.{ext}"))
}

/// Sesión del juego (la monta Seamless): jugadores, si este juego es el anfitrión y el SteamID del
/// anfitrión. Sin sesión, None.
pub(crate) fn sesion() -> Option<(usize, bool, u64)> {
    let sm = unsafe { eldenring::cs::CSSessionManager::instance() }.ok()?;
    let (mut n, mut yo_anfitrion, mut anfitrion) = (0, false, 0);
    for p in sm.players.iter() {
        if p.base.steam_id == 0 {
            continue;
        }
        n += 1;
        if p.is_host {
            anfitrion = p.base.steam_id;
        }
        if p.is_local_player {
            yo_anfitrion = p.is_host;
        }
    }
    (n > 0).then_some((n, yo_anfitrion, anfitrion))
}

/// Semilla común de una sesión con compañeros: sale del SteamID del anfitrión, así que todos los
/// juegos de la sesión generan la misma run (mismos mapas y rivales si eligen los mismos nodos).
pub(crate) fn semilla_sesion() -> Option<String> {
    sesion().filter(|s| s.0 >= 2 && s.2 != 0).map(|(_, _, h)| format!("CO{:06X}", h & 0xFF_FFFF))
}

/// Crea un personaje a `dist` metros delante del jugador (boss del coliseo).
pub(crate) fn spawn_chr(chr: i32, npc: i32, think: i32, ent: u32, dist: f32) -> bool {
    spawn_chr_en(chr, npc, think, ent, 0.0, dist)
}

/// Como `spawn_chr`, desplazado `dx` metros a un lado (los de un grupo, uno junto a otro).
pub(crate) fn spawn_chr_en(chr: i32, npc: i32, think: i32, ent: u32, dx: f32, dist: f32) -> bool {
    spawn_chr_junto(chr, npc, think, ent, None, dx, dist)
}

/// Como `spawn_chr_en`, junto a `centro` (coordenadas de `player_pos`) en lugar del jugador: las
/// invocaciones que el anfitrión crea para un invitado.
pub(crate) fn spawn_chr_junto(chr: i32, npc: i32, think: i32, ent: u32, centro: Option<[f32; 3]>, dx: f32, dist: f32) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let Some(p) = wcm.main_player.as_ref() else { return false };
    let q = p.chr_ins.modules.physics.position;
    let pos = centro.map(|c| (c[0], c[1], c[2])).unwrap_or((q.0, q.1, q.2));
    let req = eldenring::cs::ChrDebugSpawnRequest {
        chr_id: chr,
        chara_init_param_id: -1,
        npc_param_id: npc,
        npc_think_param_id: think,
        event_entity_id: ent as i32,
        talk_id: -1,
        pos_x: pos.0 + dx,
        pos_y: pos.1 + 0.5,
        pos_z: pos.2 + dist,
    };
    wcm.spawn_debug_character(&req);
    true
}

/// Posición del jugador en coordenadas locales del bloque (las del MSB).
pub(crate) fn player_block_pos() -> Option<[f32; 3]> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let b = wcm.main_player.as_ref()?.block_position;
    Some([b.x, b.y, b.z])
}

/// Alto del área de dibujo de la ventana del juego.
pub(crate) fn game_client_height() -> Option<i32> {
    let h = game_window()?;
    let mut r = RECT::default();
    unsafe { GetClientRect(h, &mut r).ok()? };
    Some(r.bottom - r.top).filter(|&v| v > 0)
}

//! Equipar objetos desde la DLL usando la función del propio juego (la que usa el menú de equipo).
//! Receta de la tabla de Cheat Engine de The Grand Archives (`equipItem_code.cea`): la función se
//! localiza por patrón y recibe (EquipGameData, ranura, &handle, índice global de inventario, 1, 1, 0).
//! Ranuras: 0-5 armas (izq/dcha × 3), 12-15 armadura, 17-20 talismanes.

use std::sync::OnceLock;

use eldenring::cs::{PlayerGameData, WorldChrMan};
use fromsoftware_shared::FromStatic;
use pelite::pe64::{Pe, PeView};

type EquipGearFn = unsafe extern "C" fn(*mut u8, u32, *const u32, u32, u32, u32, u32) -> u64;

type ItemGiveFn = unsafe extern "C" fn(*mut u8, *const u8, *mut u8, u32) -> u64;
type RemoveItemFn = unsafe extern "C" fn(*mut u8, u32, u32) -> u64;
type AddSoulFn = unsafe extern "C" fn(*mut u8, i32) -> u64;

static EQUIP_GEAR: OnceLock<Option<usize>> = OnceLock::new();
static REMOVE_ITEM: OnceLock<Option<usize>> = OnceLock::new();
static ITEM_GIVE: OnceLock<Option<usize>> = OnceLock::new();
static ADD_SOUL: OnceLock<Option<usize>> = OnceLock::new();

/// Singleton del juego que reparte objetos (lo encontramos por su nombre FD4, sin campos).
pub struct MapItemMan;
impl fromsoftware_shared::FromSingleton for MapItemMan {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("MapItemMan")
    }
}

/// Busca `patron` (bytes; None = comodín) en la sección de código del ejecutable.
fn scan(pattern: &[Option<u8>]) -> Option<usize> {
    unsafe {
        let base = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).ok()?.0 as *const u8;
        let view = PeView::module(base);
        for s in view.section_headers() {
            if &s.Name[..5] != b".text" {
                continue;
            }
            let start = base.add(s.VirtualAddress as usize);
            let bytes = std::slice::from_raw_parts(start, s.VirtualSize as usize);
            'outer: for i in 0..bytes.len().saturating_sub(pattern.len()) {
                for (j, p) in pattern.iter().enumerate() {
                    if let Some(b) = p
                        && bytes[i + j] != *b
                    {
                        continue 'outer;
                    }
                }
                return Some(start as usize + i);
            }
        }
        None
    }
}

fn equip_gear_fn() -> Option<usize> {
    *EQUIP_GEAR.get_or_init(|| {
        // "?? 8b f1 ?? 8b d8 ?? 63 ea ?? 8b f9" − 0x17
        let pat = [None, Some(0x8b), Some(0xf1), None, Some(0x8b), Some(0xd8), None, Some(0x63), Some(0xea), None, Some(0x8b), Some(0xf9)];
        let r = scan(&pat).map(|a| a - 0x17);
        crate::log(&format!("equipar: función {}", r.map(|a| format!("{a:#x}")).unwrap_or("NO ENCONTRADA".into())));
        r
    })
}

fn remove_item_fn() -> Option<usize> {
    *REMOVE_ITEM.get_or_init(|| {
        // "?? 83 ec ?? 8b f2 ?? 8b e9 ?? 85 c0 74" − 16 (TGA, RemoveItem_code.cea)
        let pat = [None, Some(0x83), Some(0xec), None, Some(0x8b), Some(0xf2), None, Some(0x8b), Some(0xe9), None, Some(0x85), Some(0xc0), Some(0x74)];
        let r = scan(&pat).map(|a| a - 16);
        crate::log(&format!("quitar: función {}", r.map(|a| format!("{a:#x}")).unwrap_or("NO ENCONTRADA".into())));
        r
    })
}

fn item_give_fn() -> Option<usize> {
    *ITEM_GIVE.get_or_init(|| {
        // "8B 02 83 F8 0A" − 0x52 (TGA, ItemGive_code.cea)
        let pat = [Some(0x8b), Some(0x02), Some(0x83), Some(0xf8), Some(0x0a)];
        let r = scan(&pat).map(|a| a - 0x52);
        crate::log(&format!("dar: función {}", r.map(|a| format!("{a:#x}")).unwrap_or("NO ENCONTRADA".into())));
        r
    })
}

fn add_soul_fn() -> Option<usize> {
    *ADD_SOUL.get_or_init(|| {
        // "44 8B ?? ?? 45 33 ?? 44 89 5C 24" (TGA, AddSoul_code.cea)
        let pat = [Some(0x44), Some(0x8b), None, None, Some(0x45), Some(0x33), None, Some(0x44), Some(0x89), Some(0x5c), Some(0x24)];
        let r = scan(&pat);
        crate::log(&format!("runas: función {}", r.map(|a| format!("{a:#x}")).unwrap_or("NO ENCONTRADA".into())));
        r
    })
}

/// Suma runas con la función del juego (la de matar a un enemigo): el contador del HUD las sube al
/// momento. Escribir el número a mano no lo avisa y no se ve hasta que algo lo refresca.
pub fn add_runes(n: u32) -> bool {
    let Some(f) = add_soul_fn() else { return false };
    let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else { return false };
    let pgd = p.player_game_data.as_ptr() as *mut u8;
    unsafe {
        let call: AddSoulFn = std::mem::transmute(f);
        call(pgd, n.min(i32::MAX as u32) as i32);
    }
    true
}

/// Mete el objeto directamente en el inventario (sin cartel). `item_id` con bits de categoría; en armas
/// el id ya lleva afinidad y mejora (id + afinidad×100 + nivel). `gem` = ceniza montada (EquipParamGem) o -1.
pub fn give(item_id: u32, qty: u32, gem: i32) -> Result<(), String> {
    give_modo(item_id, qty, gem, 0)
}

/// Como `give`, con el último argumento de la función del juego (prueba: ¿quita el aviso?).
pub fn give_modo(item_id: u32, qty: u32, gem: i32, modo: u32) -> Result<(), String> {
    let f = item_give_fn().ok_or("función de dar no encontrada")?;
    let man = unsafe { MapItemMan::instance_mut() }.map_err(|_| "sin MapItemMan")?;
    // Lista: [n][id, cantidad, -1, gema]...
    let mut list = [0u32; 8];
    list[0] = 1;
    list[1] = item_id;
    list[2] = qty;
    list[3] = u32::MAX;
    list[4] = gem as u32;
    let mut out = [0u8; 128];
    unsafe {
        let call: ItemGiveFn = std::mem::transmute(f);
        call(man as *mut _ as *mut u8, list.as_ptr() as *const u8, out.as_mut_ptr(), modo);
    }
    Ok(())
}

/// Da el objeto y devuelve el índice de la entrada nueva (la que no estaba antes), nunca una copia que
/// ya tuviera el jugador.
pub fn give_new(item_id: u32, gem: i32) -> Result<u32, String> {
    let antes: Vec<u32> = inventory().iter().filter(|e| e.2 == item_id).map(|e| e.0).collect();
    give(item_id, 1, gem)?;
    inventory().iter().find(|e| e.2 == item_id && !antes.contains(&e.0)).map(|e| e.0).ok_or_else(|| "no llegó al inventario".to_string())
}

/// Viales carmesí que quedan y máximo. El vial lleno de cada nivel es el objeto impar (1001-1025) y su
/// cantidad es lo que queda; el vacío (par) no cuenta.
pub fn hp_flasks() -> Option<(u32, u32)> {
    let gd = player_game_data()?;
    let max = gd.max_hp_flask as u32;
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let mut quedan = 0;
    for (head, cap) in
        [(inv.key_items_head.as_ptr() as *const u8, inv.key_items_capacity), (inv.normal_items_head.as_ptr() as *const u8, inv.normal_items_capacity)]
    {
        for i in 0..cap {
            let e = unsafe { head.add(i as usize * 0x18) };
            let (handle, id, qty) = unsafe { (*(e as *const u32), *(e.add(4) as *const u32), *(e.add(8) as *const u32)) };
            let goods = id & 0x0FFF_FFFF;
            if handle != 0 && handle != u32::MAX && id >> 28 == 4 && (1001..=1025).contains(&goods) && goods % 2 == 1 {
                quedan += qty;
            }
        }
    }
    Some((quedan.min(max), max))
}

/// Viales (carmesí y cerúleo) tal como están: su entrada del inventario, id y cantidad. Para
/// devolverlos si algo los recarga entre combates (al invitado, el viaje de vuelta con Seamless; 23-10).
pub fn guardar_viales() -> Vec<(usize, u32, u32)> {
    let Some(gd) = player_game_data() else { return Vec::new() };
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let mut v = Vec::new();
    for (head, cap) in
        [(inv.key_items_head.as_ptr() as *const u8, inv.key_items_capacity), (inv.normal_items_head.as_ptr() as *const u8, inv.normal_items_capacity)]
    {
        for i in 0..cap {
            let e = unsafe { head.add(i as usize * 0x18) };
            let (handle, id, qty) = unsafe { (*(e as *const u32), *(e.add(4) as *const u32), *(e.add(8) as *const u32)) };
            if handle != 0 && handle != u32::MAX && id >> 28 == 4 && (1000..=1025).contains(&(id & 0x0FFF_FFFF)) {
                v.push((e as usize, id, qty));
            }
        }
    }
    v
}

/// Vuelve a poner las cantidades guardadas (`guardar_viales`) en las entradas que siguen siendo el
/// mismo vial. Devuelve cuántas había que cambiar.
pub fn devolver_viales(g: &[(usize, u32, u32)]) -> usize {
    if player_game_data().is_none() {
        return 0;
    }
    let mut n = 0;
    for &(e, id, qty) in g {
        let e = e as *mut u8;
        unsafe {
            if *(e.add(4) as *const u32) == id && *(e.add(8) as *const u32) != qty {
                *(e.add(8) as *mut u32) = qty;
                n += 1;
            }
        }
    }
    n
}


pub(crate) fn player_game_data() -> Option<&'static mut PlayerGameData> {
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let p = wcm.main_player.as_mut()?;
    Some(unsafe { p.player_game_data.as_mut() })
}

/// Pone un objeto del inventario en la ranura rápida `q` (0-9) y la deja elegida: lo que se usa con
/// el botón de usar objeto. Devuelve lo que había (handle, índice, id y ranura elegida) para volver a
/// ponerlo con `restaurar_rapido`. Se escribe en los datos de equipo del jugador, como el menú.
pub fn poner_rapido(q: usize, item_id: u32) -> Result<(u32, i32, u32, i32), String> {
    let (idx, handle) = find_item(item_id).ok_or(format!("{item_id:#x} no está en el inventario"))?;
    let gd = player_game_data().ok_or("sin jugador")?;
    let eq = &mut gd.equipment;
    let ranura = (&mut eq.equip_item_data.quick_slots[q]) as *mut _ as *mut u32;
    let ids = (&mut eq.equipment_entries.quick_tems[q]) as *mut _ as *mut u32;
    unsafe {
        let antes = (*ranura, *(ranura.add(1)) as i32, *ids, eq.equip_item_data.selected_quick_slot);
        *ranura = handle;
        *(ranura.add(1)) = idx;
        *ids = item_id;
        eq.equip_item_data.selected_quick_slot = q as i32;
        Ok(antes)
    }
}

/// Diagnóstico: ranura rápida elegida y lo que hay en la `q` (handle, índice, id en el ChrAsm).
pub fn estado_rapido(q: usize) -> Option<(i32, u32, i32, u32)> {
    let gd = player_game_data()?;
    let eq = &mut gd.equipment;
    let ranura = (&mut eq.equip_item_data.quick_slots[q]) as *mut _ as *mut u32;
    let ids = (&mut eq.equipment_entries.quick_tems[q]) as *mut _ as *mut u32;
    unsafe { Some((eq.equip_item_data.selected_quick_slot, *ranura, *(ranura.add(1)) as i32, *ids)) }
}

/// Objetos de Seamless que el menú cooperativo pone en la ranura rápida (vasija, efigie, niebla...).
const SEAMLESS_OBJETOS: std::ops::RangeInclusive<u32> = 8_380_000..=8_380_010;

/// Si en una ranura rápida se ha quedado un objeto de Seamless, pone ahí el vial carmesí (24-10: el
/// invitado jugó toda la run con la Efigie de Malenia en vez de los viales; al unirse, el juego recarga su
/// equipo de la partida guardada y lo que devolvía `restaurar_rapido` se perdía). Devuelve cuántas cambió.
pub fn quitar_seamless_de_rapidas() -> usize {
    // El vial carmesí del jugador: el lleno (impar 1001-1025) o, sin ninguno lleno, el vacío.
    let vial = (1001..=1025u32).rev().step_by(2).chain((1000..=1024u32).rev().step_by(2)).find_map(|g| find_item(0x4000_0000 | g).map(|(i, h)| (i, h, 0x4000_0000 | g)));
    let Some(gd) = player_game_data() else { return 0 };
    let eq = &mut gd.equipment;
    let mut n = 0;
    for q in 0..eq.equip_item_data.quick_slots.len() {
        let ranura = (&mut eq.equip_item_data.quick_slots[q]) as *mut _ as *mut u32;
        let ids = (&mut eq.equipment_entries.quick_tems[q]) as *mut _ as *mut u32;
        unsafe {
            let id = *ids;
            if id >> 28 != 4 || !SEAMLESS_OBJETOS.contains(&(id & 0x0FFF_FFFF)) {
                continue;
            }
            match vial {
                Some((idx, handle, vid)) if !(0..q).any(|k| *((&eq.equipment_entries.quick_tems[k]) as *const _ as *const u32) == vid) => {
                    *ranura = handle;
                    *(ranura.add(1)) = idx;
                    *ids = vid;
                }
                // Sin vial, o ya puesto en otra ranura: la ranura vacía.
                _ => {
                    *ranura = 0;
                    *(ranura.add(1)) = u32::MAX;
                    *ids = u32::MAX;
                }
            }
            n += 1;
        }
    }
    n
}

/// Deja la ranura rápida `q` como estaba (lo que devolvió `poner_rapido`).
pub fn restaurar_rapido(q: usize, antes: (u32, i32, u32, i32)) {
    let Some(gd) = player_game_data() else { return };
    let eq = &mut gd.equipment;
    let ranura = (&mut eq.equip_item_data.quick_slots[q]) as *mut _ as *mut u32;
    let ids = (&mut eq.equipment_entries.quick_tems[q]) as *mut _ as *mut u32;
    unsafe {
        *ranura = antes.0;
        *(ranura.add(1)) = antes.1 as u32;
        *ids = antes.2;
    }
    eq.equip_item_data.selected_quick_slot = antes.3;
}

/// Índice global de inventario y handle del primer objeto con este id (id con bits de categoría:
/// 0x0 arma, 0x10000000 armadura, 0x20000000 talismán, 0x40000000 objeto, 0x80000000 ceniza).
pub fn find_item(item_id: u32) -> Option<(u32, u32)> {
    find_item_except(item_id, u32::MAX)
}

/// Como `find_item`, pero saltándose un índice (para distinguir una copia nueva de la equipada).
pub fn find_item_except(item_id: u32, except: u32) -> Option<(u32, u32)> {
    let gd = player_game_data()?;
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let key_cap = inv.key_items_capacity;
    let head = inv.normal_items_head.as_ptr() as *const u8;
    for i in 0..inv.normal_items_capacity {
        let e = unsafe { head.add(i as usize * 0x18) };
        let handle = unsafe { *(e as *const u32) };
        let id = unsafe { *(e.add(4) as *const u32) };
        if id == item_id && handle != 0 && handle != u32::MAX && i + key_cap != except {
            return Some((i + key_cap, handle));
        }
    }
    None
}

#[cfg(feature = "banco")]
/// Equipa en `slot` el objeto `item_id` (debe estar en el inventario).
pub fn equip(slot: u32, item_id: u32) -> Result<(), String> {
    let (idx, _) = find_item(item_id).ok_or(format!("{item_id:#x} no está en el inventario"))?;
    equip_idx(slot, idx)
}

/// Handle de la entrada de inventario `idx` (índice global, el de las ranuras).
fn entry(idx: u32) -> Option<(u32, u32)> {
    let gd = player_game_data()?;
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let i = idx.checked_sub(inv.key_items_capacity)?;
    if i >= inv.normal_items_capacity {
        return None;
    }
    let e = unsafe { (inv.normal_items_head.as_ptr() as *const u8).add(i as usize * 0x18) };
    Some(unsafe { (*(e as *const u32), *(e.add(4) as *const u32)) })
}

/// Equipa en `slot` la entrada de inventario `idx`.
pub fn equip_idx(slot: u32, idx: u32) -> Result<(), String> {
    let f = equip_gear_fn().ok_or("función de equipar no encontrada")?;
    let (handle, _) = entry(idx).ok_or(format!("índice {idx} fuera del inventario"))?;
    let gd = player_game_data().ok_or("sin jugador")?;
    let egd = &mut gd.equipment as *mut _ as *mut u8;
    let data = Box::new(handle);
    unsafe {
        let call: EquipGearFn = std::mem::transmute(f);
        call(egd, slot, &*data as *const u32, idx, 1, 1, 0);
    }
    Ok(())
}

/// Índices de inventario equipados en cada una de las 22 ranuras.
pub fn equipped() -> Vec<u32> {
    player_game_data().map(|gd| gd.equipment.equipment_item_idx_list.to_vec()).unwrap_or_default()
}

/// Id del objeto en un índice global de inventario (el que guardan las ranuras).
pub fn item_at(idx: u32) -> Option<u32> {
    entry(idx).map(|e| e.1)
}

/// Param ids que el ChrAsm tiene en cada ranura (lo que se dibuja y se usa de verdad).
pub fn asm_ids() -> Vec<i32> {
    player_game_data().map(|gd| gd.equipment.chr_asm.equipment_param_ids.to_vec()).unwrap_or_default()
}

#[cfg(feature = "banco")]
/// Quita `n` unidades del objeto del inventario (con la función del juego, que también limpia la ranura).
pub fn remove(item_id: u32, n: u32) -> Result<(), String> {
    let (idx, _) = find_item(item_id).ok_or(format!("{item_id:#x} no está en el inventario"))?;
    remove_idx(idx, n)
}

pub fn remove_idx(idx: u32, n: u32) -> Result<(), String> {
    let f = remove_item_fn().ok_or("función de quitar no encontrada")?;
    let gd = player_game_data().ok_or("sin jugador")?;
    let inv = &mut gd.equipment.equip_inventory_data as *mut _ as *mut u8;
    unsafe {
        let call: RemoveItemFn = std::mem::transmute(f);
        call(inv, idx, n);
    }
    Ok(())
}

/// Lo que hay en una ranura: (índice de inventario, id con bits de categoría). None si está vacía
/// (sin nada o con los puños, 110000).
pub fn slot_item(slot: u32) -> Option<(u32, u32)> {
    let idx = *equipped().get(slot as usize)?;
    let id = item_at(idx)?;
    let param = asm_ids().get(slot as usize).copied().unwrap_or(-1);
    if param == -1 || param == 110000 || id == u32::MAX {
        return None;
    }
    Some((idx, id))
}

/// Mete una copia del arma de `slot` con la ceniza `gem`, la equipa en su sitio y quita la vieja.
pub fn mount_gem(slot: u32, gem: i32) -> Result<(), String> {
    let (old_idx, id) = slot_item(slot).ok_or("ranura vacía")?;
    let new_idx = give_new(id, gem)?;
    equip_idx(slot, new_idx)?;
    remove_idx(old_idx, 1)
}

/// Deja la ranura vacía: armas con los puños (110000), talismanes sin nada (como `unequipItem` de TGA).
pub fn unequip(slot: u32) -> Result<(), String> {
    if slot <= 5 {
        let (idx, _) = find_item(110000).ok_or("no encuentro los puños en el inventario")?;
        return equip_idx(slot, idx);
    }
    let f = equip_gear_fn().ok_or("función de equipar no encontrada")?;
    let gd = player_game_data().ok_or("sin jugador")?;
    let egd = &mut gd.equipment as *mut _ as *mut u8;
    let none = Box::new(u32::MAX);
    unsafe {
        let call: EquipGearFn = std::mem::transmute(f);
        call(egd, slot, &*none as *const u32, u32::MAX, 1, 1, 0);
    }
    Ok(())
}

/// Desequipa y borra todos los talismanes (no hay talismanes en RogueRing: los sustituyen las bendiciones).
pub fn clear_talismans() -> usize {
    let mut n = 0;
    let asm = asm_ids();
    for slot in 17..=20u32 {
        // Lo que hay en el inventario se borra; lo equipado se quita aunque ya no esté en él.
        let inv = slot_item(slot);
        if asm.get(slot as usize).is_some_and(|&p| p != -1) && unequip(slot).is_ok() {
            n += 1;
        }
        if let Some((idx, _)) = inv {
            let _ = remove_idx(idx, 1);
        }
    }
    n
}

/// Agarra el arma de la mano derecha a dos manos (o vuelve a una mano) y la reequipa para que el
/// modelo y las animaciones lo reflejen.
pub fn set_two_handed(on: bool) -> bool {
    use eldenring::cs::ChrAsmArmStyle;
    let Some(gd) = player_game_data() else { return false };
    let want = if on { ChrAsmArmStyle::RightBothHands } else { ChrAsmArmStyle::OneHanded };
    if gd.equipment.chr_asm.equipment.arm_style == want {
        return true;
    }
    // Reequipar primero (equipar deja el arma a una mano) y fijar el agarre después.
    if let Some((idx, _)) = slot_item(1) {
        let _ = equip_idx(1, idx);
    }
    if let Some(gd) = player_game_data() {
        gd.equipment.chr_asm.equipment.arm_style = want;
    }
    true
}

#[cfg(feature = "banco")]
/// Estilo de agarre actual (una mano, dos manos...).
pub fn arm_style() -> Option<eldenring::cs::ChrAsmArmStyle> {
    player_game_data().map(|gd| gd.equipment.chr_asm.equipment.arm_style)
}

/// ¿Hay arma en la mano izquierda 1 (arma doble)?
pub fn left_hand_armed() -> bool {
    slot_item(0).is_some()
}

/// Deja activas la mano derecha 1 y la izquierda 1 (tras cambiar de arma el juego conserva la
/// ranura activa anterior, que puede haberse quedado con los puños).
pub fn select_first_weapons() {
    if let Some(gd) = player_game_data() {
        let sel = &mut gd.equipment.chr_asm.equipment.selected_slots;
        sel.right_weapon_slot = 0;
        sel.left_weapon_slot = 0;
    }
}

/// Todo el inventario normal: (índice global, handle, id con categoría, cantidad).
pub fn inventory() -> Vec<(u32, u32, u32, u32)> {
    let Some(gd) = player_game_data() else { return vec![] };
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let key_cap = inv.key_items_capacity;
    let head = inv.normal_items_head.as_ptr() as *const u8;
    let mut v = vec![];
    for i in 0..inv.normal_items_capacity {
        let e = unsafe { head.add(i as usize * 0x18) };
        let (handle, id, qty) = unsafe { (*(e as *const u32), *(e.add(4) as *const u32), *(e.add(8) as *const u32)) };
        if handle != 0 && handle != u32::MAX && id != u32::MAX {
            v.push((i + key_cap, handle, id, qty));
        }
    }
    v
}

/// Todo lo que lleva el jugador, en sus tres listas (0 normal, 1 objetos clave, 2 objetos clave del
/// multijugador): (lista, id con su categoría en los 4 bits altos, cantidad).
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub fn inventory_all() -> Vec<(u8, u32, u32)> {
    let Some(gd) = player_game_data() else { return vec![] };
    let inv = &gd.equipment.equip_inventory_data.items_data;
    let listas = [
        (0u8, inv.normal_items_head.as_ptr() as *const u8, inv.normal_items_capacity),
        (1, inv.key_items_head.as_ptr() as *const u8, inv.key_items_capacity),
        (2, inv.multiplay_key_items_head.as_ptr() as *const u8, inv.multiplay_key_items_capacity),
    ];
    let mut v = vec![];
    for (lista, head, cap) in listas {
        for i in 0..cap {
            let e = unsafe { head.add(i as usize * 0x18) };
            let (handle, id, qty) = unsafe { (*(e as *const u32), *(e.add(4) as *const u32), *(e.add(8) as *const u32)) };
            if handle != 0 && handle != u32::MAX && id != u32::MAX {
                v.push((lista, id, qty));
            }
        }
    }
    v
}

#[cfg(feature = "banco")]
/// Ceniza montada en el arma de este handle (id de EquipParamGem), o -1.
pub fn gem_of(handle: u32) -> i32 {
    use eldenring::cs::{CSGaitemImp, CSWepGaitemIns, GaitemHandle};
    use fromsoftware_shared::Superclass;
    let Ok(imp) = (unsafe { CSGaitemImp::instance() }) else { return -1 };
    let h: GaitemHandle = unsafe { std::mem::transmute(handle) };
    let Some(ins) = imp.gaitem_ins_by_handle(&h) else { return -1 };
    let Some(w) = ins.as_subclass::<CSWepGaitemIns>() else { return -1 };
    let gh = w.gem_slot_table.gem_slots[0].gaitem_handle;
    imp.gaitem_ins_by_handle(&gh).and_then(|g| g.item_id.param_id()).map(|p| p as i32).unwrap_or(-1)
}

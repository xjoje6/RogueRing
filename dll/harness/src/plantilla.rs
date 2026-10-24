//! Partida plantilla (tools/plantilla_partida.py): quien no tenga partida del mod empieza con un
//! personaje ya hecho, sin pasar por la creación de personaje. Al arrancar, antes de que el juego lea
//! los guardados, si `RogueRing.sl2` no existe o es la copia que me3 acaba de hacer de ER0000.sl2
//! (idéntica: todavía no se ha jugado), se escribe la plantilla con el SteamID del jugador y los MD5
//! recalculados. Una partida del mod ya jugada no se toca nunca, y ER0000.sl2 tampoco.

use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

/// SteamID de la plantilla (cuenta 0), el que se cambia por el del jugador.
const MARCADOR: u64 = 76_561_197_960_265_728;

/// SteamID64 de la cuenta con la que está Steam abierto (registro de Steam: ActiveUser).
fn steam_id() -> Option<u64> {
    let mut v: u32 = 0;
    let mut n = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Valve\\Steam\\ActiveProcess"),
            w!("ActiveUser"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as *mut _),
            Some(&mut n),
        )
    };
    (r.is_ok() && v != 0).then_some(MARCADOR + v as u64)
}

/// Cambia el marcador por `id` en los 12 bloques y recalcula el MD5 con el que empieza cada uno.
fn adaptar(b: &mut [u8], id: u64) -> Result<usize, String> {
    if b.len() < 0x40 || &b[..4] != b"BND4" {
        return Err("la plantilla no es un guardado".into());
    }
    let n = u32::from_le_bytes(b[0x0C..0x10].try_into().unwrap()) as usize;
    let (de, a) = (MARCADOR.to_le_bytes(), id.to_le_bytes());
    let mut cambios = 0;
    for i in 0..n {
        let e = 0x40 + i * 0x20;
        let size = u64::from_le_bytes(b[e + 8..e + 16].try_into().unwrap()) as usize;
        let off = u32::from_le_bytes(b[e + 0x10..e + 0x14].try_into().unwrap()) as usize;
        if off + size > b.len() || size < 16 {
            return Err(format!("bloque {i} fuera del archivo"));
        }
        let datos = &mut b[off + 16..off + size];
        let mut p = 0;
        while p + 8 <= datos.len() {
            if datos[p..p + 8] == de {
                datos[p..p + 8].copy_from_slice(&a);
                cambios += 1;
                p += 8;
            } else {
                p += 1;
            }
        }
        let md5 = Md5::digest(&b[off + 16..off + size]);
        b[off..off + 16].copy_from_slice(&md5);
    }
    Ok(cambios)
}

fn misma(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Instala la plantilla si hace falta. Devuelve qué ha hecho (para el log).
pub fn instalar(root: &Path) -> String {
    let plantilla = root.join("data").join("partida").join("plantilla.sl2");
    if !plantilla.is_file() {
        return "partida plantilla: no viene con el mod".into();
    }
    let Some(id) = steam_id() else { return "partida plantilla: no se sabe la cuenta de Steam".into() };
    let Ok(appdata) = std::env::var("APPDATA") else { return "partida plantilla: sin APPDATA".into() };
    let dir = PathBuf::from(appdata).join("EldenRing").join(id.to_string());
    let msg = instalar_en(&dir, &plantilla, id);
    if crate::seamless_cargado() {
        return format!("{msg}; {}", instalar_seamless(&dir, &plantilla, id));
    }
    msg
}

/// Con Seamless Co-op el juego lee ER0000.<extensión de su ersc_settings.ini> y no la partida de
/// me3. Con una extensión propia (`rogue`) es un archivo aparte: si no existe, se hace con la partida
/// del mod o, si no la hay, con la plantilla. Con co2 o sl2 no se instala nada (y el mod no se
/// activa): esas son la partida de Seamless y la normal del jugador.
fn instalar_seamless(dir: &Path, plantilla: &Path, id: u64) -> String {
    let destino = match crate::partida_seamless() {
        Ok(n) => dir.join(n),
        Err(ext) => return format!("partida de Seamless: con la extensión {ext} sería una partida del jugador; no se toca"),
    };
    #[cfg(feature = "banco")]
    let nombre = std::env::var("ROGUE_SAVEFILE").unwrap_or_else(|_| "RogueRing.sl2".into());
    #[cfg(not(feature = "banco"))]
    let nombre = String::from("RogueRing.sl2");
    let sl2 = dir.join(&nombre);
    let co2 = destino;
    if co2.exists() {
        return format!("partida de Seamless: {} ya existe (no se toca)", co2.display());
    }
    if sl2.exists() && !misma(&sl2, &dir.join("ER0000.sl2")) {
        return match std::fs::copy(&sl2, &co2) {
            Ok(_) => format!("partida de Seamless: {} hecha con {nombre}", co2.display()),
            Err(e) => format!("partida de Seamless: no se pudo copiar ({e})"),
        };
    }
    let mut b = match std::fs::read(plantilla) {
        Ok(b) => b,
        Err(e) => return format!("partida de Seamless: no se pudo leer la plantilla ({e})"),
    };
    match adaptar(&mut b, id).map(|_| std::fs::write(&co2, &b)) {
        Ok(Ok(())) => format!("partida de Seamless: plantilla instalada en {}", co2.display()),
        Ok(Err(e)) => format!("partida de Seamless: no se pudo escribir ({e})"),
        Err(e) => format!("partida de Seamless: {e}"),
    }
}

fn instalar_en(dir: &Path, plantilla: &Path, id: u64) -> String {
    // ROGUE_SAVEFILE: otro nombre de partida para las pruebas (con el `savefile` del perfil de me3 a
    // juego). La versión de jugador usa siempre RogueRing.sl2.
    #[cfg(feature = "banco")]
    let nombre = std::env::var("ROGUE_SAVEFILE").unwrap_or_else(|_| "RogueRing.sl2".into());
    #[cfg(not(feature = "banco"))]
    let nombre = String::from("RogueRing.sl2");
    // La partida del juego no se toca nunca, se llame como se llame la del mod.
    if nombre.eq_ignore_ascii_case("ER0000.sl2") || nombre.contains(['/', '\\']) {
        return format!("partida plantilla: nombre no válido ({nombre})");
    }
    let destino = dir.join(&nombre);
    if destino.exists() && !misma(&destino, &dir.join("ER0000.sl2")) {
        return format!("partida plantilla: {nombre} ya existe (no se toca)");
    }
    let mut b = match std::fs::read(plantilla) {
        Ok(b) => b,
        Err(e) => return format!("partida plantilla: no se pudo leer ({e})"),
    };
    match adaptar(&mut b, id) {
        Ok(n) => {
            let _ = std::fs::create_dir_all(dir);
            match std::fs::write(&destino, &b) {
                Ok(()) => format!("partida plantilla instalada en {} ({n} SteamID)", destino.display()),
                Err(e) => format!("partida plantilla: no se pudo escribir ({e})"),
            }
        }
        Err(e) => format!("partida plantilla: {e}"),
    }
}

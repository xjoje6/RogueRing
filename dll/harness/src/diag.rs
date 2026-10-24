//! Diagnóstico de Seamless Co-op para el banco de pruebas (solo con la feature `banco`). Su DLL va
//! ofuscada (ni textos ni importaciones a la vista), así que lo que hace se mira con el juego en marcha:
//!
//! - `vigilar_ini`: apunta cada vez que el juego abre su ersc_settings.ini (desvío de CreateFileW). Si
//!   lo vuelve a abrir al crear o unirse a una sala, cambiar la contraseña dentro del juego es escribir
//!   el INI.
//! - `vigilar <texto>` (y runtime/harness/vigilar.txt, una ruta por línea, desde el arranque): apunta cada
//!   apertura de un archivo cuya ruta lo contenga. Para saber qué bancos de efectos y sonido carga el
//!   juego de verdad (jefes de Nightreign sin efectos ni sonido, 17-10).
//! - `buscar_clave`: busca la contraseña del INI en la memoria del juego y apunta dónde está (nunca la
//!   contraseña): región, si es de ersc.dll y, si parece un std::string de MSVC, su tamaño y capacidad.

use std::sync::OnceLock;

use retour::RawDetour;
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress, LoadLibraryA};
use windows::Win32::System::Memory::{MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::core::s;

type CreateFileWFn = unsafe extern "system" fn(*const u16, u32, u32, *const core::ffi::c_void, u32, u32, isize) -> isize;

static CREATE_FILE: OnceLock<(usize, RawDetour)> = OnceLock::new();
/// Trozos de ruta que se apuntan al abrirse (en minúsculas).
static FILTROS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

unsafe extern "system" fn create_file_hook(nombre: *const u16, acceso: u32, compartir: u32, sa: *const core::ffi::c_void, disp: u32, flags: u32, plantilla: isize) -> isize {
    if !nombre.is_null() {
        let mut n = 0;
        while n < 1024 && unsafe { *nombre.add(n) } != 0 {
            n += 1;
        }
        let ruta = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(nombre, n) });
        let baja = ruta.to_lowercase();
        if FILTROS.lock().is_ok_and(|f| f.iter().any(|x| baja.contains(x.as_str()))) {
            crate::log(&format!("diag: se abre {ruta} (acceso {acceso:#x}, disposición {disp})"));
        }
    }
    let orig: CreateFileWFn = unsafe { std::mem::transmute(CREATE_FILE.get().unwrap().0) };
    unsafe { orig(nombre, acceso, compartir, sa, disp, flags, plantilla) }
}

/// Desvío de CreateFileW (kernelbase) que apunta las aperturas del INI de Seamless.
pub fn vigilar_ini() -> String {
    vigilar("ersc_settings")
}

/// Rutas de runtime/harness/vigilar.txt, desde el arranque (los bancos comunes se cargan pronto).
pub fn vigilar_al_arrancar(runtime: &std::path::Path) {
    if let Ok(s) = std::fs::read_to_string(runtime.join("vigilar.txt")) {
        for l in s.lines().map(str::trim).filter(|l| !l.is_empty()) {
            crate::log(&format!("diag: {}", vigilar(l)));
        }
    }
}

/// Apunta las aperturas de archivos cuya ruta contenga `texto`.
pub fn vigilar(texto: &str) -> String {
    if let Ok(mut f) = FILTROS.lock() {
        f.push(texto.to_lowercase());
    }
    if CREATE_FILE.get().is_some() {
        return format!("vigilando también «{texto}»");
    }
    unsafe {
        let Ok(m) = LoadLibraryA(s!("kernelbase.dll")) else { return "sin kernelbase".into() };
        let Some(f) = GetProcAddress(m, s!("CreateFileW")) else { return "sin CreateFileW".into() };
        let Ok(d) = RawDetour::new(f as *const (), create_file_hook as *const ()) else { return "no se pudo desviar".into() };
        let tramp = d.trampoline() as *const () as usize;
        let _ = CREATE_FILE.set((tramp, d));
        if CREATE_FILE.get().unwrap().1.enable().is_err() {
            return "no se pudo activar".into();
        }
    }
    format!("vigilando «{texto}»")
}

/// Busca la contraseña del INI en la memoria del juego. Devuelve dónde (sin la contraseña).
pub fn buscar_clave() -> String {
    let Some(ini) = crate::seamless_ini() else { return "sin Seamless".into() };
    let Ok(texto) = std::fs::read_to_string(&ini) else { return "sin INI".into() };
    let Some(clave) = texto.lines().find_map(|l| l.trim().strip_prefix("cooppassword")?.trim().strip_prefix('=').map(|v| v.trim().to_string())) else {
        return "INI sin contraseña".into();
    };
    if clave.len() < 3 {
        return "contraseña demasiado corta para buscarla".into();
    }
    let ersc = unsafe { GetModuleHandleA(s!("ersc.dll")) }.map(|h| h.0 as usize).unwrap_or(0);
    let utf8 = clave.as_bytes().to_vec();
    let utf16: Vec<u8> = clave.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut hallados = vec![];
    let mut dir = 0usize;
    let proceso = unsafe { GetCurrentProcess() };
    loop {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        if unsafe { VirtualQuery(Some(dir as *const _), &mut mbi, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let base = mbi.BaseAddress as usize;
        let tam = mbi.RegionSize;
        let legible = mbi.State == MEM_COMMIT && (mbi.Protect.0 & (PAGE_GUARD.0 | PAGE_NOACCESS.0)) == 0 && mbi.Protect.0 != 0;
        if legible && tam <= 512 << 20 {
            let mut buf = vec![0u8; tam];
            let mut leido = 0usize;
            let ok = unsafe { ReadProcessMemory(proceso, base as *const _, buf.as_mut_ptr() as *mut _, tam, Some(&mut leido)) }.is_ok();
            if ok {
                for (cod, aguja) in [("utf8", &utf8), ("utf16", &utf16)] {
                    let mut i = 0;
                    while let Some(p) = buf[i..leido].windows(aguja.len()).position(|w| w == aguja.as_slice()) {
                        let a = i + p;
                        // Tras el texto: ¿fin de cadena? Y, si fuera un std::string corto de MSVC, a +16 su
                        // tamaño y a +24 su capacidad (15).
                        let fin = buf.get(a + aguja.len()).copied().unwrap_or(1);
                        let u64_en = |o: usize| buf.get(a + o..a + o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
                        let de_ersc = ersc != 0 && mbi.AllocationBase as usize == ersc;
                        hallados.push(format!(
                            "{cod} en {:#x} (región {base:#x}+{:#x}, protección {:#x}, tipo {:#x}{}), sigue {fin:#x}, +16={:?} +24={:?}",
                            base + a,
                            a,
                            mbi.Protect.0,
                            mbi.Type.0,
                            if de_ersc { ", ersc.dll" } else { "" },
                            u64_en(16),
                            u64_en(24)
                        ));
                        i = a + 1;
                        if hallados.len() >= 40 {
                            break;
                        }
                    }
                }
            }
        }
        dir = base + tam;
        if dir == 0 || hallados.len() >= 40 {
            break;
        }
    }
    for h in &hallados {
        crate::log(&format!("diag: clave {h}"));
    }
    format!("{} sitios (en el registro)", hallados.len())
}

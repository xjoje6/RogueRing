//! RogueRing: boss rush roguelite sobre Elden Ring. La DLL la carga me3; arranca el overlay, el
//! director de la run y, en el banco de pruebas, el lector de órdenes.

mod arranque;
mod coop;
#[cfg(feature = "banco")]
mod banco;
#[cfg(feature = "banco")]
mod diag;
mod director;
mod equip;
mod globales;
mod idioma;
mod juego;
mod motor;
mod overlay;
mod pad;
mod plantilla;
mod sonido;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::SharedTaskImpExt;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

pub(crate) use juego::*;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Carpeta del registro (harness/).
pub(crate) fn dir_registro() -> Option<&'static PathBuf> {
    DIR.get()
}
/// Con Seamless, la partida que cargaría el juego no es del mod (ver `partida_seamless`): el mod no
/// hace nada.
pub(crate) static PARTIDA_AJENA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) fn log(msg: &str) {
    if let Some(d) = DIR.get()
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(d.join("harness.log"))
    {
        let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[{t}] {msg}");
        coop::registro_linea(&format!("[{t}] {msg}"));
    }
}

fn start(module: HMODULE) {
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(Some(module), &mut buf) } as usize;
    let dll = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    let dir = dll.parent().map(|p| p.join("harness")).unwrap_or_else(|| PathBuf::from("harness"));
    let _ = fs::create_dir_all(&dir);
    // El registro se abre en cada sesión; pasado un tamaño, el anterior queda como harness.old.log.
    let log_path = dir.join("harness.log");
    if fs::metadata(&log_path).is_ok_and(|m| m.len() > 4 << 20) {
        let _ = fs::rename(&log_path, dir.join("harness.old.log"));
    }
    let _ = DIR.set(dir.clone());
    log(&format!("cargado desde {}", dll.display()));
    // Un pánico dentro del juego lo cierra sin más (0xc0000409): que al menos quede en el registro.
    std::panic::set_hook(Box::new(|info| log(&format!("PÁNICO: {info}"))));
    // Fuera de DllMain: desviar funciones con el loader lock tomado es peligroso.
    std::thread::spawn(|| {
        log(&pad::install());
        // Teclado (DirectInput): en la versión de jugador, para aceptar en el título con E e Intro.
        log(&pad::install_dinput());
        // Solo para las pruebas en segundo plano (la versión de jugador no los lleva).
        if cfg!(feature = "banco") {
            log(&pad::install_focus());
            log(&pad::install_window_pin());
            log(&pad::install_cursor());
        }
    });
    // Overlay (DirectX 12). La raíz del proyecto es la carpeta padre de runtime/.
    let root = dll.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_default();
    let _ = director::ROOT.set(root.clone());
    // Con Seamless, el juego lee ER0000.<extensión de Seamless>, no la partida de me3. Con co2 o sl2
    // sería la partida de Seamless o la normal del jugador: el mod no se activa para no tocarla.
    if seamless_cargado()
        && let Err(ext) = partida_seamless()
    {
        PARTIDA_AJENA.store(true, std::sync::atomic::Ordering::Relaxed);
        log(&format!(
            "SEAMLESS con save_file_extension = {ext}: el juego cargaría ER0000.{ext}, que no es del mod. RogueRing no se activa \
             (pon save_file_extension = rogue en el ersc_settings.ini de su carpeta SeamlessCoop)."
        ));
    }
    // Antes de que el juego lea los guardados: partida plantilla para quien no tenga partida del mod.
    log(&plantilla::instalar(&root));
    let hmod = module.0 as isize;
    sonido::iniciar(root.clone());
    std::thread::spawn(move || {
        use hudhook::hooks::dx12::ImguiDx12Hooks;
        let r = hudhook::Hudhook::builder()
            .with::<ImguiDx12Hooks>(overlay::Overlay::new(root))
            .with_hmodule(hudhook::windows::Win32::Foundation::HINSTANCE(hmod as _))
            .build()
            .apply();
        log(&format!("hudhook: {r:?}"));
    });

    #[cfg(feature = "banco")]
    diag::vigilar_al_arrancar(&dir);
    #[cfg(feature = "banco")]
    let ordenes = banco::lector(dir);

    std::thread::spawn(move || {
        // En el primer arranque tras descomprimir (el antivirus revisa la DLL nueva y el juego va por
        // detrás) la búsqueda puede fallar a la primera y sin esto el mod no arrancaba hasta abrir el
        // juego otra vez (17-10): se reintenta cada segundo, dos minutos.
        let mut intentos = 0;
        let task = loop {
            match CSTaskImp::wait_for_instance(Duration::from_secs(60)) {
                Ok(t) => break t,
                Err(e) => {
                    intentos += 1;
                    log(&format!("sin CSTaskImp ({e}); intento {intentos}"));
                    if intentos >= 120 {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        };
        log("tarea del juego registrada");
        #[cfg(feature = "banco")]
        let mut frame: u64 = 0;
        task.run_recurring(
            move |_: &FD4TaskData| {
                if !PARTIDA_AJENA.load(std::sync::atomic::Ordering::Relaxed) {
                    arranque::tick();
                    director::autostart();
                    coop::tick();
                    director::tick();
                }
                #[cfg(feature = "banco")]
                {
                    frame += 1;
                    banco::fotograma(&ordenes, frame);
                }
            },
            CSTaskGroupIndex::FrameBegin,
        );
        // Cooperativo, caídos: nadie muere en el combate. Justo después del daño, antes de que el juego
        // procese la muerte (juego.rs, suelo_de_vida).
        task.run_recurring(
            move |_: &FD4TaskData| {
                if !PARTIDA_AJENA.load(std::sync::atomic::Ordering::Relaxed) {
                    suelo_de_vida();
                }
            },
            CSTaskGroupIndex::DmgMan_Post,
        );
        // Críticos como en Nightreign: después de la animación (que pone la marca) y antes de los golpes.
        task.run_recurring(
            move |_: &FD4TaskData| {
                if PARTIDA_AJENA.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if abrir_criticos() > 0 && !CRITICO_VISTO.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log("críticos: a un enemigo se le quitó la invencibilidad de su animación (primera vez en esta sesión)");
                }
            },
            CSTaskGroupIndex::ChrIns_PrePhysicsSafe,
        );
        // Y otra vez justo antes de que el juego mire los golpes: si algo la vuelve a poner entre medias
        // (23-10: ni durante los críticos se le podía pegar), aquí se quita también.
        task.run_recurring(
            move |_: &FD4TaskData| {
                if PARTIDA_AJENA.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if abrir_criticos() > 0 && !CRITICO_VUELTO.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log("críticos: la invencibilidad había vuelto antes de los golpes; quitada también ahí (primera vez en esta sesión)");
                }
            },
            CSTaskGroupIndex::DmgMan_Pre,
        );
    });
}

/// Ya se apuntó que la invencibilidad volvía entre la física y los golpes.
static CRITICO_VUELTO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ya se apuntó en el registro el primer crítico abierto.
static CRITICO_VISTO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// # Safety
/// Punto de entrada llamado por el cargador de Windows.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(module: HMODULE, reason: u32, _: *mut ()) -> bool {
    if reason == 1 {
        start(module);
    }
    true
}

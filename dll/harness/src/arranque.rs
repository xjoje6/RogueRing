//! Arranque directo: quien abre el mod no ve el título de Elden Ring. Mientras no hay personaje en el
//! mundo, la capa del mod tapa la pantalla (overlay) y aquí se pulsa A con el mando virtual, aunque no
//! haya mando real, hasta que empieza la carga: «Pulsa cualquier botón» y luego «Continuar» (la
//! partida del mod, RogueRing.sl2, es aparte de la del juego). La capa se quita cuando el menú del mod
//! está listo y quieto detrás (en el centro, sin avisos del juego pendientes). Si en 45 s no empieza a
//! cargar, se aparta y se ve el título.
//!
//! No se hace sin partida del mod (sin personaje, «Continuar» no existe y A empezaría una partida
//! nueva) ni con `runtime/sin_arranque` o `runtime/sin_autoinicio`.
//!
//! Sin el foco el juego no hace caso al mando: mientras otra ventana esté delante (la primera vez, la
//! consola que pide la contraseña de Seamless; 16-10) no se pulsa. La paciencia sigue contando:
//! esperar al foco sin límite dejó la capa puesta para siempre (16-10).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eldenring::cs::{CSNowLoadingHelper, WorldChrMan};
use fromsoftware_shared::FromStatic;

/// La capa del arranque está tapando la pantalla.
pub static TAPANDO: AtomicBool = AtomicBool::new(false);

/// Ya se apuntó en el registro que se espera al foco, y cuándo se intentó traer el juego delante.
static SIN_FOCO: AtomicBool = AtomicBool::new(false);
static AL_FRENTE: Mutex<Option<Instant>> = Mutex::new(None);
/// El juego ya ha tenido el foco en este arranque: si lo pierde, es que el jugador se ha ido a otra
/// cosa y no se le quita (18-10: con la tecla Windows y otro programa, el juego volvía delante).
static TUVO_FOCO: AtomicBool = AtomicBool::new(false);

enum Estado {
    Titulo { desde: Instant, pulsado: Instant, cargas: i32 },
    Dentro { desde: Instant },
    Hecho,
}

static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);

/// Tiempo máximo en el título antes de rendirse y dejar ver el menú del juego. Un arranque lento
/// (el primero tras reconstruir el mod) tarda unos 30 s en entrar.
const PACIENCIA: Duration = Duration::from_secs(45);
/// Cada cuánto se pulsa A en el título.
const CADA: Duration = Duration::from_millis(900);
/// Tras aparecer en el mundo, máximo que se tapa esperando la primera pantalla del mod.
const ESPERA_MOD: Duration = Duration::from_secs(20);

fn cargas() -> i32 {
    unsafe { CSNowLoadingHelper::instance() }.map(|n| n.menu_load_counter).unwrap_or(0)
}

/// ¿Está la ventana del juego delante? (Con el foco simulado del banco de pruebas, siempre.)
fn con_foco() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
    }
    let si = pid == std::process::id();
    if si {
        TUVO_FOCO.store(true, Ordering::Relaxed);
    }
    si
}

/// ¿Lo que está delante es una consola (la del lanzador, que tapaba el juego del amigo del jugador el
/// 17-10)? Solo entonces se trae el juego delante; cualquier otro programa es elección del jugador.
fn consola_delante() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(GetForegroundWindow(), &mut buf) };
    let clase = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    matches!(clase.as_str(), "ConsoleWindowClass" | "CASCADIA_HOSTING_WINDOW_CLASS" | "PseudoConsoleWindow")
}

fn en_mundo() -> bool {
    let vivo = unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.main_player.as_ref()).is_some_and(|p| p.chr_ins.modules.data.hp > 0);
    vivo && crate::player_area().is_some_and(|a| a != 255)
}

/// ¿Hay partida del mod? La copia me3 de ER0000.sl2 la primera vez (crates/mod-host/src/savefile.rs).
fn hay_partida() -> bool {
    let Ok(appdata) = std::env::var("APPDATA") else { return false };
    let Ok(dirs) = std::fs::read_dir(std::path::Path::new(&appdata).join("EldenRing")) else { return false };
    let nombre = std::env::var("ROGUE_SAVEFILE").unwrap_or_else(|_| "RogueRing.sl2".into());
    dirs.flatten().any(|d| d.path().join(&nombre).is_file())
}

pub fn tick() {
    let Ok(mut guard) = ESTADO.try_lock() else { return };
    let estado = guard.get_or_insert_with(|| {
        let root = crate::director::ROOT.get();
        let apagado = root.is_some_and(|r| r.join("runtime").join("sin_arranque").exists() || r.join("runtime").join("sin_autoinicio").exists());
        if apagado || !hay_partida() || en_mundo() {
            crate::log(&format!("arranque directo: no (apagado {apagado}, partida {})", hay_partida()));
            Estado::Hecho
        } else {
            TAPANDO.store(true, Ordering::Relaxed);
            crate::log("arranque directo: tapando el título");
            Estado::Titulo { desde: Instant::now(), pulsado: Instant::now() - CADA, cargas: cargas() }
        }
    });
    match estado {
        Estado::Titulo { desde, pulsado, cargas: antes } => {
            if en_mundo() {
                *estado = Estado::Dentro { desde: Instant::now() };
            } else if cargas() != *antes || crate::screen_fade() > 0.05 {
                // Cargando o fundiendo: no se pulsa (una A de más dentro de la partida sería «usar») y
                // la paciencia vuelve a contar desde aquí.
                *pulsado = Instant::now();
                *desde = Instant::now();
            } else if desde.elapsed() > PACIENCIA {
                crate::log("arranque directo: el título no respondió, se deja a la vista");
                TAPANDO.store(false, Ordering::Relaxed);
                *estado = Estado::Hecho;
            } else if !con_foco() {
                if !SIN_FOCO.swap(true, Ordering::Relaxed) {
                    crate::log("arranque directo: el juego no tiene el foco");
                }
                // Cada 3 s, solo durante el arranque y solo si lo tapa la consola del lanzador y el juego
                // aún no ha tenido el foco (si el jugador se va a otro programa, se espera a que vuelva).
                if !TUVO_FOCO.load(Ordering::Relaxed)
                    && consola_delante()
                    && let Ok(mut t) = AL_FRENTE.try_lock()
                    && t.is_none_or(|t| t.elapsed() >= Duration::from_secs(3))
                {
                    *t = Some(Instant::now());
                    let ok = crate::traer_al_frente();
                    crate::log(&format!("arranque directo: al frente ({})", if ok { "ok" } else { "no" }));
                }
            } else if pulsado.elapsed() >= CADA {
                crate::pad::press_aceptar_titulo(150);
                *pulsado = Instant::now();
            }
        }
        Estado::Dentro { desde } => {
            if crate::director::menu_listo() || desde.elapsed() > ESPERA_MOD {
                crate::log("arranque directo: hecho");
                TAPANDO.store(false, Ordering::Relaxed);
                *estado = Estado::Hecho;
            }
        }
        Estado::Hecho => {}
    }
}

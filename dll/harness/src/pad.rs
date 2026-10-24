//! Mando virtual dentro del proceso: intercepta XInput para añadir pulsaciones pedidas por el
//! banco de pruebas (por ejemplo, "Continuar" en el título). No toca teclado ni ratón del sistema.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use retour::RawDetour;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::core::{PCSTR, s};

#[repr(C)]
pub struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
pub struct XInputState {
    packet: u32,
    pad: XInputGamepad,
}

#[repr(C)]
pub struct XInputCapabilities {
    kind: u8,
    sub_type: u8,
    flags: u16,
    pad: XInputGamepad,
    vibration: [u16; 2],
}

// Mismos tamaños que en xinput.h.
const _: () =
    assert!(std::mem::size_of::<XInputGamepad>() == 12 && std::mem::size_of::<XInputState>() == 16 && std::mem::size_of::<XInputCapabilities>() == 20);

type GetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;
type GetCapsFn = unsafe extern "system" fn(u32, u32, *mut XInputCapabilities) -> u32;

struct Hooks {
    _d: Vec<RawDetour>,
    get_state: GetStateFn,
    get_state_ex: GetStateFn,
    get_caps: GetCapsFn,
}
unsafe impl Send for Hooks {}
unsafe impl Sync for Hooks {}

static HOOKS: OnceLock<Hooks> = OnceLock::new();
/// GetForegroundWindow original y ventana del juego (para fingir foco dentro del proceso).
static FG: OnceLock<(usize, RawDetour)> = OnceLock::new();
pub static FAKE_FOCUS_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
/// (botones, hasta cuándo pulsados, hasta cuándo fingir mando conectado)
static PRESS: Mutex<Option<(u16, Instant, Instant)>> = Mutex::new(None);
static PACKET: AtomicU32 = AtomicU32::new(1);
/// Diagnóstico: llamadas a GetState, GetStateEx, GetCapabilities y GetForegroundWindow.
pub static CALLS: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

pub fn button(name: &str) -> Option<u16> {
    Some(match name.to_ascii_uppercase().as_str() {
        "ARRIBA" | "UP" => 0x0001,
        "ABAJO" | "DOWN" => 0x0002,
        "IZQUIERDA" | "LEFT" => 0x0004,
        "DERECHA" | "RIGHT" => 0x0008,
        "START" => 0x0010,
        "BACK" | "SELECT" => 0x0020,
        "LB" => 0x0100,
        "RB" => 0x0200,
        "A" => 0x1000,
        "B" => 0x2000,
        "X" => 0x4000,
        "Y" => 0x8000,
        _ => return None,
    })
}

/// Palanca izquierda del banco de pruebas: (x, y, hasta cuándo).
static STICK: Mutex<Option<(i16, i16, Instant)>> = Mutex::new(None);

#[cfg(feature = "banco")]
/// Inclina la palanca izquierda durante `ms` milisegundos (solo banco de pruebas).
pub fn stick(x: i16, y: i16, ms: u64) {
    *STICK.lock().unwrap() = Some((x, y, Instant::now() + Duration::from_millis(ms)));
    // Que el mando virtual siga «conectado» mientras dura.
    let mut p = PRESS.lock().unwrap();
    let hasta = Instant::now() + Duration::from_millis(ms + 3000);
    match p.as_mut() {
        Some(e) => e.2 = e.2.max(hasta),
        None => *p = Some((0, Instant::now(), hasta)),
    }
}

/// Gatillos del banco de pruebas (RT, LT): apretados hasta este momento.
static GATILLO: Mutex<[Option<Instant>; 2]> = Mutex::new([None, None]);

#[cfg(feature = "banco")]
/// Mantiene RT (o LT, `izquierdo`) apretado `ms` milisegundos (solo banco de pruebas).
pub fn gatillo(ms: u64, izquierdo: bool) {
    GATILLO.lock().unwrap()[izquierdo as usize] = Some(Instant::now() + Duration::from_millis(ms));
    let mut p = PRESS.lock().unwrap();
    let hasta = Instant::now() + Duration::from_millis(ms + 3000);
    match p.as_mut() {
        Some(e) => e.2 = e.2.max(hasta),
        None => *p = Some((0, Instant::now(), hasta)),
    }
}

/// Hasta cuándo el mando virtual cuenta también en la versión de jugador, aunque no haya mando real
/// (arranque directo: pulsar «Continuar» en el título también a quien juega con teclado).
static FORZADO: Mutex<Option<Instant>> = Mutex::new(None);

/// Hasta cuándo se pulsan también E e Intro del teclado (arranque directo): con un mando que no es
/// de Xbox (los de PlayStation van por libScePad) o con teclado, el juego no ve el mando virtual y el
/// mod no pasaba del título (el amigo de 17-10). Solo dentro del juego (DirectInput).
static TECLADO: Mutex<Option<Instant>> = Mutex::new(None);

/// Aceptar en el título: A en el mando virtual y E e Intro en el teclado.
pub fn press_aceptar_titulo(ms: u64) {
    *TECLADO.lock().unwrap() = Some(Instant::now() + Duration::from_millis(ms));
    press_forzado(button("A").unwrap_or(0x1000), ms);
}

/// Como `press`, pero también en la versión de jugador y sin mando real.
pub fn press_forzado(buttons: u16, ms: u64) {
    *FORZADO.lock().unwrap() = Some(Instant::now() + Duration::from_millis(ms + 300));
    press(buttons, ms);
}

fn forzado() -> bool {
    FORZADO.lock().unwrap().is_some_and(|t| Instant::now() < t)
}

/// Pulsa `buttons` durante `ms` milisegundos (y luego los suelta, manteniendo el mando "conectado").
pub fn press(buttons: u16, ms: u64) {
    let now = Instant::now();
    *PRESS.lock().unwrap() = Some((buttons, now + Duration::from_millis(ms), now + Duration::from_millis(ms + 3000)));
}

/// Botones inyectados ahora mismo. El mando virtual está siempre "conectado" en el puerto 0:
/// si no, el juego deja de consultar XInput al ver que no hay mando y nunca ve las pulsaciones.
fn injected() -> Option<u16> {
    let now = Instant::now();
    match *PRESS.lock().unwrap() {
        Some((b, until, _)) if now < until => Some(b),
        _ => Some(0),
    }
}

/// Botones del mando tal como llegan (real + virtual), para la navegación de las pantallas del mod.
pub static UI_BUTTONS: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
/// Ataque cargado: RT mantenido al menos 0,6 s y soltado. Lo consume el motor de efectos.
static CHARGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RT_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn take_charged() -> bool {
    CHARGED.swap(false, Ordering::Relaxed)
}

/// Con el mod en marcha, START y SELECT son suyos (pausa propia): el juego no abre su menú ni su mapa.
pub static MENU_PROPIO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Manos libres del cooperativo: START abre el menú del juego (los objetos de Seamless) y SELECT,
/// que vuelve al menú del mod, sigue siendo suyo.
pub static SELECT_PROPIO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Cooperativo, caído (director/caido.rs): al juego solo le llegan la cámara (stick derecho) y la pausa.
pub static SOLO_CAMARA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Stick izquierdo del mando tal como llega (x en los 16 bits altos, y en los bajos): el caído se
/// arrastra con él aunque al juego no le llegue.
pub static STICK_IZQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Stick izquierdo de -1 a 1 (y hacia arriba).
pub fn stick_izquierdo() -> (f32, f32) {
    let v = STICK_IZQ.load(Ordering::Relaxed);
    ((v >> 16) as u16 as i16 as f32 / 32767.0, (v & 0xFFFF) as u16 as i16 as f32 / 32767.0)
}

/// Con una pantalla del mod abierta, el juego no recibe el mando (el personaje no se mueve).
pub static BLOCK_GAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Botones que seguían pulsados al cerrarse una pantalla del mod: el juego no los recibe hasta que se
/// sueltan (la A o la B que cierra la pausa hacía saltar o rodar al volver).
static RETENIDOS: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

unsafe fn apply(user: u32, st: *mut XInputState, r: u32) -> u32 {
    if user != 0 {
        return r;
    }
    let Some(b) = injected() else { return r };
    // Sin mando real: el mando virtual del banco de pruebas sigue «conectado»; en la versión de
    // jugador no hay nada que hacer (los menús del mod se manejan con el mando).
    if r != 0 && !cfg!(feature = "banco") && !forzado() {
        return r;
    }
    unsafe {
        // En segundo plano (foco simulado) el mando real se está usando en otra cosa, y detrás de una
        // pantalla del mod solo pasan las pulsaciones forzadas: solo cuentan las del mod.
        if r != 0 || background() || (BLOCK_GAME.load(Ordering::Relaxed) && forzado()) {
            std::ptr::write_bytes(st, 0, 1);
        }
        (*st).pad.buttons |= b;
        if let Some((x, y, hasta)) = *STICK.lock().unwrap()
            && Instant::now() < hasta
        {
            (*st).pad.thumb_lx = x;
            (*st).pad.thumb_ly = y;
        }
        let g = *GATILLO.lock().unwrap();
        if g[0].is_some_and(|t| Instant::now() < t) {
            (*st).pad.right_trigger = 255;
        }
        if g[1].is_some_and(|t| Instant::now() < t) {
            (*st).pad.left_trigger = 255;
        }
        UI_BUTTONS.store((*st).pad.buttons, Ordering::Relaxed);
        if MENU_PROPIO.load(Ordering::Relaxed) {
            (*st).pad.buttons &= !0x0030;
        } else if SELECT_PROPIO.load(Ordering::Relaxed) {
            (*st).pad.buttons &= !0x0020;
        }
        let rt = (*st).pad.right_trigger > 60;
        if let Ok(mut since) = RT_SINCE.try_lock() {
            match (rt, *since) {
                (true, None) => *since = Some(Instant::now()),
                (false, Some(t)) => {
                    if t.elapsed() >= Duration::from_millis(600) {
                        CHARGED.store(true, Ordering::Relaxed);
                    }
                    *since = None;
                }
                _ => {}
            }
        }
        // Lo que se pulsó para cerrar una pantalla del mod no le llega al juego hasta soltarlo.
        if BLOCK_GAME.load(Ordering::Relaxed) {
            RETENIDOS.store(u16::MAX, Ordering::Relaxed);
        } else {
            let r = RETENIDOS.load(Ordering::Relaxed) & (*st).pad.buttons;
            RETENIDOS.store(r, Ordering::Relaxed);
            if !forzado() {
                (*st).pad.buttons &= !r;
            }
        }
        STICK_IZQ.store(((((*st).pad.thumb_lx as u16) as u32) << 16) | ((*st).pad.thumb_ly as u16) as u32, Ordering::Relaxed);
        if SOLO_CAMARA.load(Ordering::Relaxed) && !forzado() {
            (*st).pad.buttons &= 0x0030;
            (*st).pad.thumb_lx = 0;
            (*st).pad.thumb_ly = 0;
            (*st).pad.left_trigger = 0;
            (*st).pad.right_trigger = 0;
        }
        // Con una pantalla del mod el juego no recibe el mando, salvo las pulsaciones forzadas del mod
        // (cerrar un aviso del juego por detrás de un menú propio).
        if BLOCK_GAME.load(Ordering::Relaxed) && !forzado() {
            let packet = (*st).packet;
            std::ptr::write_bytes(st, 0, 1);
            (*st).packet = packet;
        }
        (*st).packet = PACKET.fetch_add(1, Ordering::Relaxed);
    }
    0
}

unsafe extern "system" fn get_state_hook(user: u32, st: *mut XInputState) -> u32 {
    CALLS[0].fetch_add(1, Ordering::Relaxed);
    unsafe {
        let r = (HOOKS.get().unwrap().get_state)(user, st);
        apply(user, st, r)
    }
}

unsafe extern "system" fn get_state_ex_hook(user: u32, st: *mut XInputState) -> u32 {
    CALLS[1].fetch_add(1, Ordering::Relaxed);
    unsafe {
        let r = (HOOKS.get().unwrap().get_state_ex)(user, st);
        apply(user, st, r)
    }
}

unsafe extern "system" fn get_caps_hook(user: u32, flags: u32, caps: *mut XInputCapabilities) -> u32 {
    CALLS[2].fetch_add(1, Ordering::Relaxed);
    unsafe {
        let r = (HOOKS.get().unwrap().get_caps)(user, flags, caps);
        if r != 0 && user == 0 && cfg!(feature = "banco") && injected().is_some() {
            std::ptr::write_bytes(caps, 0, 1);
            (*caps).kind = 1; // XINPUT_DEVTYPE_GAMEPAD
            (*caps).sub_type = 1; // XINPUT_DEVSUBTYPE_GAMEPAD
            return 0;
        }
        r
    }
}

unsafe extern "system" fn get_foreground_hook() -> isize {
    CALLS[3].fetch_add(1, Ordering::Relaxed);
    let h = FAKE_FOCUS_HWND.load(Ordering::Relaxed);
    if h != 0 {
        return h;
    }
    let orig: unsafe extern "system" fn() -> isize = unsafe { std::mem::transmute(FG.get().unwrap().0) };
    unsafe { orig() }
}

/// Hace creer al juego (y solo al juego) que su ventana está en primer plano: sin esto ignora el
/// mando mientras se usa otra ventana. Devuelve texto para el log.
pub fn install_focus() -> String {
    unsafe {
        let Ok(m) = LoadLibraryA(s!("user32.dll")) else { return "sin user32".into() };
        let Some(f) = GetProcAddress(m, s!("GetForegroundWindow")) else { return "sin GetForegroundWindow".into() };
        let Ok(d) = RawDetour::new(f as *const (), get_foreground_hook as *const ()) else { return "no se pudo desviar GetForegroundWindow".into() };
        let tramp = d.trampoline() as *const () as usize;
        let _ = FG.set((tramp, d));
        if FG.get().unwrap().1.enable().is_err() {
            return "no se pudo activar el desvío de foco".into();
        }
        "foco simulado disponible".into()
    }
}

/// Instala los hooks. Devuelve un texto para el log.
pub fn install() -> String {
    unsafe {
        let Ok(m) = LoadLibraryA(s!("xinput1_4.dll")) else { return "sin xinput1_4.dll".into() };
        let by_ord = |n: usize| GetProcAddress(m, PCSTR(n as *const u8));
        let (Some(gs), Some(gse), Some(gc)) = (by_ord(2), by_ord(100), GetProcAddress(m, s!("XInputGetCapabilities"))) else {
            return "no encuentro las funciones de XInput".into();
        };
        // Primero se crean (sin activar) para guardar los trampolines; se activan al final.
        let mk = |t: *const (), h: *const ()| RawDetour::new(t, h).ok();
        let (Some(d1), Some(d2), Some(d3)) = (
            mk(gs as *const (), get_state_hook as *const ()),
            mk(gse as *const (), get_state_ex_hook as *const ()),
            mk(gc as *const (), get_caps_hook as *const ()),
        ) else {
            return "no se pudo desviar XInput".into();
        };
        let hooks = Hooks {
            get_state: std::mem::transmute::<*const (), GetStateFn>(d1.trampoline() as *const ()),
            get_state_ex: std::mem::transmute::<*const (), GetStateFn>(d2.trampoline() as *const ()),
            get_caps: std::mem::transmute::<*const (), GetCapsFn>(d3.trampoline() as *const ()),
            _d: vec![d1, d2, d3],
        };
        let _ = HOOKS.set(hooks);
        for d in &HOOKS.get().unwrap()._d {
            if d.enable().is_err() {
                return "no se pudo activar el desvío de XInput".into();
            }
        }
        "mando virtual listo".into()
    }
}

// --- Ventana fija en el monitor virtual -------------------------------------------------------
/// (hwnd, x, y) donde debe quedarse la ventana del juego; 0 = sin fijar.
pub static PIN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
pub static PIN_X: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
pub static PIN_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static SWP: OnceLock<(usize, RawDetour)> = OnceLock::new();

type SetWindowPosFn = unsafe extern "system" fn(isize, isize, i32, i32, i32, i32, u32) -> i32;

unsafe extern "system" fn set_window_pos_hook(h: isize, after: isize, mut x: i32, mut y: i32, cx: i32, cy: i32, flags: u32) -> i32 {
    const SWP_NOMOVE: u32 = 0x0002;
    let pin = PIN_HWND.load(Ordering::Relaxed);
    if pin != 0 && h == pin && flags & SWP_NOMOVE == 0 {
        // El juego intenta recolocar su ventana (p. ej. al cargar): la dejamos donde está fijada.
        x = PIN_X.load(Ordering::Relaxed);
        y = PIN_Y.load(Ordering::Relaxed);
    }
    let orig: SetWindowPosFn = unsafe { std::mem::transmute(SWP.get().unwrap().0) };
    unsafe { orig(h, after, x, y, cx, cy, flags) }
}

pub fn install_window_pin() -> String {
    unsafe {
        let Ok(m) = LoadLibraryA(s!("user32.dll")) else { return "sin user32".into() };
        let Some(f) = GetProcAddress(m, s!("SetWindowPos")) else { return "sin SetWindowPos".into() };
        let Ok(d) = RawDetour::new(f as *const (), set_window_pos_hook as *const ()) else { return "no se pudo desviar SetWindowPos".into() };
        let tramp = d.trampoline() as *const () as usize;
        let _ = SWP.set((tramp, d));
        if SWP.get().unwrap().1.enable().is_err() {
            return "no se pudo activar el desvío de SetWindowPos".into();
        }
        "ventana fijable".into()
    }
}

// --- Segundo plano: el juego no lee teclado ni ratón reales -----------------------------------
/// Modo de pruebas: el foco está simulado, así que el juego leería el teclado y el ratón reales
/// aunque se estén usando en otra ventana.
pub fn background() -> bool {
    FAKE_FOCUS_HWND.load(Ordering::Relaxed) != 0
}

// --- Segundo plano: el juego no mueve ni encierra el ratón real ------------------------------
// Con el foco simulado el juego se cree la ventana activa y en cada fotograma devuelve el ratón al
// centro de su ventana (SetCursorPos) y lo encierra en ella (ClipCursor): con las pruebas corriendo en
// el monitor virtual, el ratón del usuario se quedaba clavado en un punto. En segundo plano no hacen
// nada (ClipCursor suelta el ratón).
static SET_CURSOR: OnceLock<(usize, RawDetour)> = OnceLock::new();
static CLIP_CURSOR: OnceLock<(usize, RawDetour)> = OnceLock::new();

unsafe extern "system" fn set_cursor_pos_hook(x: i32, y: i32) -> i32 {
    if background() {
        return 1;
    }
    let orig: unsafe extern "system" fn(i32, i32) -> i32 = unsafe { std::mem::transmute(SET_CURSOR.get().unwrap().0) };
    unsafe { orig(x, y) }
}

unsafe extern "system" fn clip_cursor_hook(rect: *const std::ffi::c_void) -> i32 {
    let orig: unsafe extern "system" fn(*const std::ffi::c_void) -> i32 = unsafe { std::mem::transmute(CLIP_CURSOR.get().unwrap().0) };
    unsafe { orig(if background() { std::ptr::null() } else { rect }) }
}

/// Desvía SetCursorPos y ClipCursor (ver arriba). Devuelve texto para el log.
pub fn install_cursor() -> String {
    unsafe {
        let Ok(m) = LoadLibraryA(s!("user32.dll")) else { return "sin user32".into() };
        let (Some(scp), Some(clip)) = (GetProcAddress(m, s!("SetCursorPos")), GetProcAddress(m, s!("ClipCursor"))) else {
            return "sin SetCursorPos o ClipCursor".into();
        };
        let (Ok(d1), Ok(d2)) = (
            RawDetour::new(scp as *const (), set_cursor_pos_hook as *const ()),
            RawDetour::new(clip as *const (), clip_cursor_hook as *const ()),
        ) else {
            return "no se pudo desviar el ratón".into();
        };
        let _ = SET_CURSOR.set((d1.trampoline() as *const () as usize, d1));
        let _ = CLIP_CURSOR.set((d2.trampoline() as *const () as usize, d2));
        if SET_CURSOR.get().unwrap().1.enable().is_err() || CLIP_CURSOR.get().unwrap().1.enable().is_err() {
            return "no se pudo activar el desvío del ratón".into();
        }
        "ratón libre en segundo plano (SetCursorPos y ClipCursor)".into()
    }
}

static DINPUT: Mutex<Vec<(usize, RawDetour)>> = Mutex::new(Vec::new());
type GetDeviceStateFn = unsafe extern "system" fn(usize, u32, *mut u8) -> i32;

unsafe extern "system" fn dinput_state_hook_0(this: usize, size: u32, data: *mut u8) -> i32 {
    unsafe { dinput_state(0, this, size, data) }
}
unsafe extern "system" fn dinput_state_hook_1(this: usize, size: u32, data: *mut u8) -> i32 {
    unsafe { dinput_state(1, this, size, data) }
}

unsafe fn dinput_state(i: usize, this: usize, size: u32, data: *mut u8) -> i32 {
    let orig = DINPUT.lock().unwrap().get(i).map(|d| d.0).unwrap_or(0);
    let f: GetDeviceStateFn = unsafe { std::mem::transmute(orig) };
    let hr = unsafe { f(this, size, data) };
    if hr == 0 && background() && !data.is_null() {
        unsafe { std::ptr::write_bytes(data, 0, size as usize) };
    }
    // Teclado (256 teclas): E (0x12) e Intro (0x1C) mientras el arranque acepta en el título.
    if hr == 0 && size == 256 && !data.is_null() && TECLADO.lock().unwrap().is_some_and(|t| Instant::now() < t) {
        unsafe {
            *data.add(0x12) = 0x80;
            *data.add(0x1C) = 0x80;
        }
    }
    hr
}

/// Desvía IDirectInputDevice8::GetDeviceState (teclado y ratón). Se localiza creando un
/// dispositivo de prueba y leyendo su vtable, como el bloqueador de fromsoftware-rs.
pub fn install_dinput() -> String {
    type Create = unsafe extern "system" fn(isize, u32, *const windows::core::GUID, *mut *mut *const usize, usize) -> i32;
    type CreateDevice = unsafe extern "system" fn(*mut *const usize, *const windows::core::GUID, *mut *mut *const usize, usize) -> i32;
    type Release = unsafe extern "system" fn(*mut *const usize) -> u32;
    const IID_DI8W: windows::core::GUID = windows::core::GUID::from_values(0xbf798031, 0x483a, 0x4da2, [0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00]);
    const KEYBOARD: windows::core::GUID = windows::core::GUID::from_values(0x6F1D2B61, 0xD5A0, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00]);
    const MOUSE: windows::core::GUID = windows::core::GUID::from_values(0x6F1D2B60, 0xD5A0, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00]);
    unsafe {
        let Ok(m) = LoadLibraryA(s!("dinput8.dll")) else { return "sin dinput8.dll".into() };
        let Some(c) = GetProcAddress(m, s!("DirectInput8Create")) else { return "sin DirectInput8Create".into() };
        let create: Create = std::mem::transmute(c);
        let hinst = windows::Win32::System::LibraryLoader::GetModuleHandleA(None).map(|h| h.0 as isize).unwrap_or(0);
        let mut di: *mut *const usize = std::ptr::null_mut();
        if create(hinst, 0x0800, &IID_DI8W, &mut di, 0) != 0 || di.is_null() {
            return "DirectInput8Create falló".into();
        }
        let create_device: CreateDevice = std::mem::transmute(*(*di).add(3));
        let mut addrs = vec![];
        for g in [KEYBOARD, MOUSE] {
            let mut dev: *mut *const usize = std::ptr::null_mut();
            if create_device(di, &g, &mut dev, 0) == 0 && !dev.is_null() {
                addrs.push(*(*dev).add(9));
                let rel: Release = std::mem::transmute(*(*dev).add(2));
                rel(dev);
            }
        }
        let rel: Release = std::mem::transmute(*(*di).add(2));
        rel(di);
        addrs.dedup();
        let hooks: [*const (); 2] = [dinput_state_hook_0 as *const (), dinput_state_hook_1 as *const ()];
        let mut list = DINPUT.lock().unwrap();
        for (i, a) in addrs.iter().enumerate().take(2) {
            let Ok(d) = RawDetour::new(*a as *const (), hooks[i]) else { return "no se pudo desviar GetDeviceState".into() };
            list.push((d.trampoline() as *const () as usize, d));
        }
        for (_, d) in list.iter() {
            if d.enable().is_err() {
                return "no se pudo activar el desvío de DirectInput".into();
            }
        }
        format!("teclado y ratón bloqueables en segundo plano ({} desvíos)", list.len())
    }
}

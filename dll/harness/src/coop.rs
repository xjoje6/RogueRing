//! Cooperativo: canal entre los juegos de una sesión y quién es quién (rogue_core::coop para lo que
//! se dicen y cómo se decide).
//!
//! - Sesión: la monta Seamless con las del propio juego (CSSessionManager guarda cada jugador con su
//!   SteamID, su nombre y quién es el anfitrión).
//! - Canal: mensajes P2P de Steam (ISteamNetworkingMessages, por la API plana de steam_api64.dll,
//!   que el juego ya carga). Sin puertos ni servidores; canal propio (RR) para no mezclarse.
//! - Sombra: un compañero de mentira dentro del mismo juego para probar los votos sin otro PC (orden
//!   `coop sombra anfitrion|invitado|no` del banco de pruebas). Vota un poco después que tú y, una vez
//!   de cada dos, otro nodo (para ver el sorteo).
//!
//! El director publica su `Estado` con `publicar` y lee los de los demás con `companeros`.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::CSSessionManager;
use fromsoftware_shared::FromStatic;
use rogue_core::coop::{self, Estado, Suceso};

/// Canal de Steam propio de RogueRing ("RR").
const CANAL: i32 = 0x5252;
/// Canal del registro (versión de pruebas): cada juego manda sus líneas nuevas a los demás, que las
/// guardan junto al suyo en harness/companero_<SteamID>.log. Así no hay que pasarse los registros a mano
/// (22-10: probar con otra persona ralentiza todo).
const CANAL_REGISTRO: i32 = 0x524C;
/// Cada cuánto se mandan las líneas nuevas, cuánto como mucho por mensaje y cuánto se guarda sin enviar.
const CADA_REGISTRO: f64 = 2.0;
const TROZO_REGISTRO: usize = 48 * 1024;
const TOPE_REGISTRO: usize = 512 * 1024;

/// Registro compartido: encendido en la versión de pruebas (el director lo pone al cargar).
pub static REGISTRO_COMPARTIDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Líneas del registro aún sin mandar y cuándo se mandó lo último.
static REGISTRO: Mutex<(String, Option<Instant>)> = Mutex::new((String::new(), None));

/// Una línea más del registro para los compañeros (lib.rs `log`).
pub fn registro_linea(l: &str) {
    if !REGISTRO_COMPARTIDO.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Ok(mut r) = REGISTRO.lock() {
        r.0.push_str(l);
        r.0.push('\n');
        // Sin sesión se guarda lo último (también el arranque, que se manda al conectar).
        if r.0.len() > TOPE_REGISTRO {
            let corte = r.0.len() - TOPE_REGISTRO / 2;
            let corte = r.0[corte..].find('\n').map_or(corte, |i| corte + i + 1);
            r.0.drain(..corte);
        }
    }
}

/// Manda las líneas nuevas a los demás y guarda las que llegan.
fn tick_registro(otros: &[Jugador]) {
    let pendiente = match REGISTRO.lock() {
        Ok(mut r) if !r.0.is_empty() && r.1.is_none_or(|t| t.elapsed().as_secs_f64() >= CADA_REGISTRO) => {
            r.1 = Some(Instant::now());
            std::mem::take(&mut r.0)
        }
        _ => String::new(),
    };
    let mut resto = pendiente.as_str();
    while !resto.is_empty() {
        let mut n = resto.len().min(TROZO_REGISTRO);
        while !resto.is_char_boundary(n) {
            n -= 1;
        }
        for j in otros {
            steam_enviar_por(j.steam, resto[..n].as_bytes(), CANAL_REGISTRO);
        }
        resto = &resto[n..];
    }
    for (de, b) in steam_recibir_de(CANAL_REGISTRO) {
        let Some(dir) = crate::dir_registro() else { continue };
        let ruta = dir.join(format!("companero_{de}.log"));
        if std::fs::metadata(&ruta).is_ok_and(|m| m.len() > 8 << 20) {
            let _ = std::fs::rename(&ruta, dir.join(format!("companero_{de}.old.log")));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&ruta) {
            use std::io::Write;
            let _ = f.write_all(&b);
        }
    }
}

/// Cada cuánto se reenvía el estado (segundos); si cambia, se envía ya.
const CADA: f64 = 0.3;
/// SteamID de la sombra (no existe en Steam).
const SOMBRA_ID: u64 = 1;

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub enum Sombra {
    No,
    /// La sombra es el invitado (tú eres el anfitrión).
    Invitado,
    /// La sombra es el anfitrión (tú eres el invitado).
    Anfitrion,
}

/// Un jugador de la sesión.
#[derive(Clone, Debug)]
pub struct Jugador {
    pub steam: u64,
    pub nombre: String,
    pub anfitrion: bool,
}

/// Un compañero tal como lo ve el director.
#[derive(Clone, Debug)]
pub struct Companero {
    pub jugador: Jugador,
    pub estado: Option<Estado>,
    /// ¿Hay noticias recientes? (si no, no se espera su voto)
    pub conectado: bool,
}

struct Coop {
    sombra: Sombra,
    /// Yo (SteamID) y si soy el anfitrión.
    yo: u64,
    anfitrion: bool,
    /// Los demás jugadores de la sesión.
    otros: Vec<Jugador>,
    /// Último estado propio (se reenvía cada `CADA`).
    mio: Option<Estado>,
    enviado: Option<Instant>,
    /// Estado de cada compañero y cuándo llegó.
    suyos: BTreeMap<u64, (Estado, Instant)>,
    /// Nodos a los que se puede ir (para que la sombra vote algo válido).
    disponibles: Vec<(i32, i32)>,
    /// Sombra: desde cuándo ve tu voto y si esta vez vota otra cosa.
    sombra_t: Option<(String, Instant)>,
    sombra_otro: bool,
    /// Sucesos propios (número, suceso, cuándo) que van en el estado unos segundos.
    mis_sucesos: Vec<(u64, Suceso, Instant)>,
    siguiente: u64,
    /// Último suceso hecho de cada compañero, y los que esperan a que el director los haga.
    vistos: BTreeMap<u64, u64>,
    recibidos: Vec<(u64, Suceso)>,
    /// Daño de bendiciones mandado al anfitrión (rival, cantidad, cuándo): cuando él lo aplica, la vida
    /// del rival baja en mi copia y no es un golpe mío.
    eco: Vec<(u32, i32, Instant)>,
}

/// Segundos que un suceso viaja en el estado (se reenvía con él cada `CADA`).
const VIDA_SUCESO: f64 = 3.0;

static COOP: Mutex<Coop> = Mutex::new(Coop {
    sombra: Sombra::No,
    yo: 0,
    anfitrion: true,
    otros: vec![],
    mio: None,
    enviado: None,
    suyos: BTreeMap::new(),
    disponibles: vec![],
    sombra_t: None,
    sombra_otro: false,
    mis_sucesos: vec![],
    siguiente: 0,
    vistos: BTreeMap::new(),
    recibidos: vec![],
    eco: vec![],
});

// --- Steam (API plana) -----------------------------------------------------------------------

#[repr(C)]
struct Identidad {
    tipo: i32,
    tam: i32,
    datos: [u8; 128],
}

impl Identidad {
    fn steam(id: u64) -> Self {
        let mut datos = [0u8; 128];
        datos[..8].copy_from_slice(&id.to_le_bytes());
        // k_ESteamNetworkingIdentityType_SteamID = 16
        Identidad { tipo: 16, tam: 8, datos }
    }
}

type FnInterfaz = unsafe extern "C" fn() -> *mut u8;
type FnEnviar = unsafe extern "C" fn(*mut u8, *const Identidad, *const u8, u32, i32, i32) -> i32;
type FnRecibir = unsafe extern "C" fn(*mut u8, i32, *mut *mut u8, i32) -> i32;
type FnLiberar = unsafe extern "C" fn(*mut u8);

struct Steam {
    mensajes: *mut u8,
    enviar: FnEnviar,
    recibir: FnRecibir,
    liberar: FnLiberar,
}
unsafe impl Send for Steam {}
unsafe impl Sync for Steam {}

fn steam() -> Option<&'static Steam> {
    static S: std::sync::OnceLock<Option<Steam>> = std::sync::OnceLock::new();
    S.get_or_init(|| unsafe {
        use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        use windows::core::s;
        let m = GetModuleHandleA(s!("steam_api64.dll")).ok()?;
        let f = |n: windows::core::PCSTR| GetProcAddress(m, n);
        let interfaz: FnInterfaz = std::mem::transmute(f(s!("SteamAPI_SteamNetworkingMessages_SteamAPI_v002"))?);
        let mensajes = interfaz();
        if mensajes.is_null() {
            crate::log("coop: Steam sin interfaz de mensajes");
            return None;
        }
        let enviar: FnEnviar = std::mem::transmute(f(s!("SteamAPI_ISteamNetworkingMessages_SendMessageToUser"))?);
        let recibir: FnRecibir = std::mem::transmute(f(s!("SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel"))?);
        let liberar: FnLiberar = std::mem::transmute(f(s!("SteamAPI_SteamNetworkingMessage_t_Release"))?);
        let s = Steam { mensajes, enviar, recibir, liberar };
        crate::log("coop: Steam listo");
        Some(s)
    })
    .as_ref()
}

fn steam_enviar(a: u64, datos: &[u8]) -> bool {
    steam_enviar_por(a, datos, CANAL)
}

fn steam_enviar_por(a: u64, datos: &[u8], canal: i32) -> bool {
    let Some(s) = steam() else { return false };
    let id = Identidad::steam(a);
    // Fiable (8) + reabrir la sesión si se rompe (32).
    unsafe { (s.enviar)(s.mensajes, &id, datos.as_ptr(), datos.len() as u32, 8 | 32, canal) == 1 }
}

/// Mensajes recibidos: (SteamID de quien lo envía, bytes).
fn steam_recibir() -> Vec<(u64, Vec<u8>)> {
    steam_recibir_de(CANAL)
}

fn steam_recibir_de(canal: i32) -> Vec<(u64, Vec<u8>)> {
    let Some(s) = steam() else { return vec![] };
    let mut v = vec![];
    let mut buf = [std::ptr::null_mut::<u8>(); 16];
    let n = unsafe { (s.recibir)(s.mensajes, canal, buf.as_mut_ptr(), buf.len() as i32) };
    for &m in buf.iter().take(n.max(0) as usize) {
        unsafe {
            // SteamNetworkingMessage_t: m_pData (0), m_cbSize (8), m_conn (12), m_identityPeer (16:
            // tipo, tamaño y el SteamID en +8).
            let datos = *(m as *const *const u8);
            let tam = *(m.add(8) as *const i32);
            let de = *(m.add(24) as *const u64);
            if !datos.is_null() && tam > 0 {
                v.push((de, std::slice::from_raw_parts(datos, tam as usize).to_vec()));
            }
            (s.liberar)(m);
        }
    }
    v
}

// --- Sesión del juego ------------------------------------------------------------------------

/// Jugadores de la sesión: (yo, los demás). Sin sesión, None.
fn sesion() -> Option<(Jugador, Vec<Jugador>)> {
    let sm = unsafe { CSSessionManager::instance() }.ok()?;
    let mut yo = None;
    let mut otros = vec![];
    for p in sm.players.iter() {
        if p.base.steam_id == 0 {
            continue;
        }
        let j = Jugador {
            steam: p.base.steam_id,
            nombre: p.base.steam_name.to_string().unwrap_or_default(),
            anfitrion: p.is_host,
        };
        if p.is_local_player { yo = Some(j) } else { otros.push(j) }
    }
    Some((yo?, otros))
}

// --- API para el director --------------------------------------------------------------------

/// ¿Hay compañeros (sesión con más jugadores o sombra)?
pub fn hay_companeros() -> bool {
    COOP.lock().is_ok_and(|c| c.sombra != Sombra::No || !c.otros.is_empty())
}

/// ¿Soy el anfitrión? (sin compañeros, sí)
pub fn soy_anfitrion() -> bool {
    COOP.lock().map(|c| c.anfitrion).unwrap_or(true)
}

/// En una sesión y sin ser el anfitrión: los enemigos son del anfitrión (su vida y su escalado los
/// pone él; si los pusiera también el invitado, se doblarían o no cuadrarían).
pub fn soy_invitado() -> bool {
    hay_companeros() && !soy_anfitrion()
}

/// Jugadores de la sesión, yo incluido (1 sin sesión).
pub fn jugadores() -> usize {
    COOP.lock().map(|c| c.otros.len() + 1).unwrap_or(1)
}

/// Semilla de la run de la sesión: la del anfitrión (juego::semilla_sesion) o, con la sombra del banco
/// de pruebas, una fija.
pub fn semilla_sesion() -> Option<String> {
    if COOP.lock().is_ok_and(|c| c.sombra != Sombra::No) {
        return Some("COSOMBRA".into());
    }
    crate::semilla_sesion()
}

/// ¿Soy invitado en una sesión ahora mismo? Se mira la sesión del juego en el momento: mientras uno se
/// une puede faltar un instante, y entonces `soy_anfitrion` dice que sí (17-10: el invitado empezó
/// su propia run con su propia semilla). Con la sombra de anfitrión, también.
pub fn invitado_en_sala() -> bool {
    if COOP.lock().is_ok_and(|c| c.sombra == Sombra::Anfitrion) {
        return true;
    }
    crate::sesion().is_some_and(|s| s.0 >= 2 && !s.1)
}

/// Semilla de la run que lleva el anfitrión ahora (en cualquier pantalla), para retomarla juntos.
pub fn semilla_del_anfitrion() -> Option<String> {
    let c = COOP.lock().ok()?;
    let host = c.otros.iter().find(|j| j.anfitrion)?;
    c.suyos.get(&host.steam).and_then(|(e, _)| e.semilla.clone())
}

/// Semilla de la run que acaba de empezar el anfitrión (está eligiendo personaje o bendición, fase
/// «inicio»): con ella entran los demás. En el menú principal no: ahí lleva la de la run anterior. La
/// sombra no cuenta (su semilla es la mía).
pub fn semilla_anfitrion() -> Option<String> {
    let c = COOP.lock().ok()?;
    if c.sombra != Sombra::No {
        return None;
    }
    let host = c.otros.iter().find(|j| j.anfitrion)?;
    let (e, t) = c.suyos.get(&host.steam)?;
    (t.elapsed().as_secs_f64() < coop::SIN_NOTICIAS && (e.fase == "personaje" || e.fase == "elegido" || e.fase == "inicio")).then(|| e.semilla.clone()).flatten()
}

/// Semilla y ascensión de la run del anfitrión (lo que manda en su estado).
pub fn ascension_anfitrion() -> Option<(String, u32)> {
    let c = COOP.lock().ok()?;
    if c.sombra != Sombra::No {
        return None;
    }
    let host = c.otros.iter().find(|j| j.anfitrion)?;
    let (e, t) = c.suyos.get(&host.steam)?;
    (t.elapsed().as_secs_f64() < coop::SIN_NOTICIAS).then(|| Some((e.semilla.clone()?, e.ascension?))).flatten()
}

/// Mi SteamID (0 sin sesión; la sombra usa 2).
pub fn yo() -> u64 {
    COOP.lock().map(|c| c.yo).unwrap_or(0)
}

/// Suceso propio para los demás: va en el estado unos segundos y se manda ya. Los números siguen la
/// hora, así que tras reiniciar el juego siguen creciendo.
pub fn suceso(s: Suceso) {
    let Ok(mut c) = COOP.lock() else { return };
    if c.otros.is_empty() {
        return;
    }
    let ahora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64 * 1000).unwrap_or(0);
    let n = c.siguiente.max(ahora) + 1;
    c.siguiente = n;
    if let Suceso::Danio { ent, n: d } = &s {
        c.eco.push((*ent, *d, Instant::now()));
    }
    c.mis_sucesos.push((n, s, Instant::now()));
    c.enviado = None;
}

/// Sucesos de los compañeros que aún no se han hecho: (de quién, suceso).
pub fn sucesos_recibidos() -> Vec<(u64, Suceso)> {
    COOP.lock().map(|mut c| std::mem::take(&mut c.recibidos)).unwrap_or_default()
}

/// Daño que no es un golpe de este jugador y va a bajar la vida de `ent`: el de las bendiciones de un
/// invitado que aplica el anfitrión (en los dos juegos).
pub fn eco(ent: u32, n: i32) {
    if let Ok(mut c) = COOP.lock() {
        c.eco.push((ent, n, Instant::now()));
    }
}

/// De una bajada de vida de `ent`, lo que no es daño apuntado con `eco` (o con un `Suceso::Danio` mío).
/// Cinco segundos: la vida que manda el anfitrión puede tardar en llegar (17-10: con dos «Doble llama»
/// ese daño contaba como golpe y el golpe medio, y con él las bolas de fuego, no paraba de crecer).
pub fn sin_eco(ent: u32, baja: i32) -> i32 {
    let Ok(mut c) = COOP.lock() else { return baja };
    c.eco.retain(|e| e.2.elapsed().as_secs_f64() < 5.0);
    let mut queda = baja;
    for e in c.eco.iter_mut().filter(|e| e.0 == ent) {
        let k = e.1.min(queda);
        e.1 -= k;
        queda -= k;
    }
    c.eco.retain(|e| e.1 > 0);
    queda
}

/// Puesto entre los invitados (por SteamID): cada uno tiene sus entidades para las invocaciones.
pub fn indice_invitado(steam: u64) -> Option<usize> {
    let c = COOP.lock().ok()?;
    let mut v: Vec<u64> = c.otros.iter().filter(|j| !j.anfitrion).map(|j| j.steam).collect();
    if !c.anfitrion {
        v.push(c.yo);
    }
    v.sort();
    v.iter().position(|&s| s == steam)
}

/// Estado propio: se envía ya si cambió y luego cada `CADA`.
pub fn publicar(e: Estado) {
    let Ok(mut c) = COOP.lock() else { return };
    if c.mio.as_ref() != Some(&e) {
        c.enviado = None;
    }
    c.mio = Some(e);
}

/// Nodos disponibles del mapa (solo los usa la sombra).
pub fn pista_disponibles(v: Vec<(i32, i32)>) {
    if let Ok(mut c) = COOP.lock() {
        c.disponibles = v;
    }
}

/// Los demás jugadores con su último estado.
pub fn companeros() -> Vec<Companero> {
    let Ok(c) = COOP.lock() else { return vec![] };
    c.otros
        .iter()
        .map(|j| {
            let s = c.suyos.get(&j.steam);
            Companero {
                jugador: j.clone(),
                estado: s.map(|x| x.0.clone()),
                conectado: s.is_some_and(|x| x.1.elapsed().as_secs_f64() < coop::SIN_NOTICIAS),
            }
        })
        .collect()
}

/// Orden `coop sombra <anfitrion|invitado|no>`.
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub fn poner_sombra(modo: &str) -> String {
    let Ok(mut c) = COOP.lock() else { return "ocupado".into() };
    c.sombra = match modo {
        "invitado" => Sombra::Invitado,
        "anfitrion" => Sombra::Anfitrion,
        _ => Sombra::No,
    };
    c.suyos.clear();
    c.sombra_t = None;
    c.otros = match c.sombra {
        Sombra::No => vec![],
        s => vec![Jugador { steam: SOMBRA_ID, nombre: "Sombra".into(), anfitrion: s == Sombra::Anfitrion }],
    };
    c.yo = if c.sombra == Sombra::No { 0 } else { 2 };
    c.anfitrion = c.sombra != Sombra::Anfitrion;
    format!("sombra: {:?} (yo {})", c.sombra, if c.anfitrion { "anfitrión" } else { "invitado" })
}

/// Orden `coop estado`.
#[cfg_attr(not(feature = "banco"), allow(dead_code))]
pub fn resumen() -> String {
    let Ok(c) = COOP.lock() else { return "ocupado".into() };
    serde_json::json!({
        "sombra": format!("{:?}", c.sombra),
        "yo": c.yo,
        "anfitrion": c.anfitrion,
        "otros": c.otros.iter().map(|j| (j.steam, j.nombre.clone(), j.anfitrion)).collect::<Vec<_>>(),
        "steam": steam().is_some(),
        "suyos": c.suyos.iter().map(|(k, (e, t))| (k, e.clone(), t.elapsed().as_secs_f64())).collect::<Vec<_>>(),
        "mio": c.mio,
    })
    .to_string()
}

/// Cada fotograma: ver quién hay en la sesión, enviar el estado y leer el de los demás.
pub fn tick() {
    let Ok(mut c) = COOP.lock() else { return };
    if c.sombra != Sombra::No {
        tick_sombra(&mut c);
        return;
    }
    match sesion() {
        Some((yo, otros)) => {
            let antes: Vec<u64> = c.otros.iter().map(|j| j.steam).collect();
            let ahora: Vec<u64> = otros.iter().map(|j| j.steam).collect();
            if antes != ahora {
                crate::log(&format!(
                    "coop: sesión con {} (yo {})",
                    otros.iter().map(|j| format!("{} {}", j.nombre, j.steam)).collect::<Vec<_>>().join(", "),
                    if yo.anfitrion { "anfitrión" } else { "invitado" }
                ));
                c.suyos.retain(|k, _| ahora.contains(k));
            }
            c.yo = yo.steam;
            c.anfitrion = yo.anfitrion;
            c.otros = otros;
        }
        None => {
            if !c.otros.is_empty() {
                crate::log("coop: fuera de la sesión");
            }
            c.otros.clear();
            c.suyos.clear();
            c.anfitrion = true;
            return;
        }
    }
    if c.otros.is_empty() {
        return;
    }
    if REGISTRO_COMPARTIDO.load(std::sync::atomic::Ordering::Relaxed) {
        tick_registro(&c.otros);
    }
    if c.enviado.is_none_or(|t| t.elapsed().as_secs_f64() >= CADA) {
        c.mis_sucesos.retain(|s| s.2.elapsed().as_secs_f64() < VIDA_SUCESO);
        if let Some(mut e) = c.mio.clone() {
            e.sucesos = c.mis_sucesos.iter().map(|s| (s.0, s.1.clone())).collect();
            let b = coop::a_bytes(&e);
            for j in &c.otros {
                steam_enviar(j.steam, &b);
            }
        }
        c.enviado = Some(Instant::now());
    }
    for (de, b) in steam_recibir() {
        if !c.otros.iter().any(|j| j.steam == de) {
            continue;
        }
        match coop::de_bytes(&b) {
            Some(e) if e.version == coop::VERSION => {
                let visto = c.vistos.get(&de).copied().unwrap_or(0);
                let nuevos: Vec<(u64, Suceso)> = e.sucesos.iter().filter(|s| s.0 > visto).map(|s| (de, s.1.clone())).collect();
                if let Some(m) = e.sucesos.iter().map(|s| s.0).max().filter(|&m| m > visto) {
                    c.vistos.insert(de, m);
                }
                c.recibidos.extend(nuevos);
                c.suyos.insert(de, (e, Instant::now()));
            }
            Some(e) => crate::log(&format!("coop: {de} tiene otra versión del mod (protocolo {})", e.version)),
            None => {}
        }
    }
}

/// Compañero de mentira: sigue tus pasos, con el cursor en otro nodo, y vota 1,5 s después que tú;
/// una vez de cada dos, otro nodo. Como vota el último, decide en «su juego» al momento y se va al
/// combate del paso siguiente con su voto aún publicado (el caso del 16-10). Siendo anfitrión, manda
/// una barra de estados de prueba del rival.
fn tick_sombra(c: &mut Coop) {
    let Some(mio) = c.mio.clone() else { return };
    let mi_voto = mio.voto.filter(|v| v.0 == mio.paso);
    let mi_evento = mio.voto_evento.filter(|v| v.0 == mio.paso);
    let clave = format!("{}|{:?}|{:?}|{:?}", mio.fase, mio.paso, mi_voto, mi_evento);
    let t = match &c.sombra_t {
        Some((k, t)) if *k == clave => *t,
        _ => {
            if mi_voto.is_some() {
                c.sombra_otro = !c.sombra_otro;
            }
            let ahora = Instant::now();
            c.sombra_t = Some((clave, ahora));
            ahora
        }
    };
    let otro = |n: (i32, i32)| c.disponibles.iter().copied().find(|&d| d != n).unwrap_or(n);
    let voto = mi_voto.filter(|_| t.elapsed().as_secs_f64() > 1.5).map(|(p, n)| (p, if c.sombra_otro { otro(n) } else { n }));
    let mut e = Estado {
        version: coop::VERSION,
        nombre: "Sombra".into(),
        fase: if voto.is_some() { "combate".into() } else { mio.fase.clone() },
        paso: if voto.is_some() { coop::Paso { nodo: mio.paso.nodo + 1, ..mio.paso } } else { mio.paso },
        semilla: mio.semilla.clone(),
        voto,
        vida: mio.vida,
        tamano: None,
        // En mi combate (también si entré sin votar, desde el banco), de pie: así se prueban los caídos.
        combate: mio.combate.filter(|_| voto.is_some() || mio.fase == "combate").map(|c| rogue_core::protocolo::Combate { muerto: false, ..c }),
        ultimo: mio.ultimo,
        listo: mio.listo,
        // Si me caigo, la sombra me está levantando (para ver la barra en el banco; no me levanta).
        levantando: mio.combate.filter(|k| k.muerto).map(|_| {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            (c.yo, ((t % 4.0) / 3.0).min(1.0) as f32)
        }),
        invocaciones: vec![],
        sucesos: vec![],
        voto_evento: mi_evento.filter(|_| t.elapsed().as_secs_f64() > 1.5).map(|(p, k)| (p, if c.sombra_otro { k + 1 } else { k })),
        cursor: mio.cursor.filter(|_| voto.is_none()).map(otro),
        rival: None,
        ascension: None,
    };
    if c.sombra == Sombra::Anfitrion && mio.fase == "combate" {
        e.rival = Some(vec![coop::EstadoRival { estado: 0, nombre: "Sombra".into(), color: [200, 40, 40], lleno: 0.6, destello: f32::MAX, texto: String::new() }]);
    }
    c.suyos.insert(SOMBRA_ID, (e, Instant::now()));
}

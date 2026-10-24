//! Silencio mientras el mod tapa la pantalla (arranque y viajes a los combates): detrás se oyen el
//! menú del juego, los avisos y la carga, y con la pantalla en negro suena a algo roto.
//! Se baja el volumen de la sesión de audio del propio juego en Windows y, al destaparse, vuelve poco
//! a poco. Va en su hilo (COM) y nunca toca el audio de otros programas.
//!
//! Windows recuerda el volumen de cada programa: si el juego se cierra en silencio, al volver sonaría
//! mudo. Por eso el volumen que tenía se apunta en `runtime/volumen.txt` mientras está bajado y, al
//! arrancar, si el archivo sigue ahí, se devuelve.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::core::Interface;

/// Lo pide la capa que tapa (overlay): silencio mientras sea true.
pub static SILENCIO: AtomicBool = AtomicBool::new(false);

/// Bajar es casi de golpe; subir, despacio (como el fundido de la imagen).
const BAJADA_S: f32 = 0.15;
const SUBIDA_S: f32 = 1.5;
const PASO: Duration = Duration::from_millis(40);

/// Sesiones de audio de este proceso en el altavoz por defecto (normalmente una).
fn sesiones() -> Vec<ISimpleAudioVolume> {
    let pid = unsafe { GetCurrentProcessId() };
    let r: windows::core::Result<Vec<ISimpleAudioVolume>> = (|| unsafe {
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let dev = en.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let man: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None)?;
        let lista = man.GetSessionEnumerator()?;
        let mut v = vec![];
        for i in 0..lista.GetCount()? {
            let s = lista.GetSession(i)?;
            if s.cast::<IAudioSessionControl2>().and_then(|c| c.GetProcessId()).is_ok_and(|p| p == pid)
                && let Ok(vol) = s.cast::<ISimpleAudioVolume>()
            {
                v.push(vol);
            }
        }
        Ok(v)
    })();
    r.unwrap_or_default()
}

pub fn iniciar(root: PathBuf) {
    std::thread::spawn(move || {
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let nota = root.join("runtime").join("volumen.txt");
        // Volumen de antes de bajarlo; si quedó apuntado de una sesión que se cerró en silencio, se
        // empieza abajo para devolverlo.
        let mut antes: Option<f32> = std::fs::read_to_string(&nota).ok().and_then(|s| s.trim().parse().ok());
        let mut nivel: f32 = if antes.is_some() { 0.0 } else { 1.0 };
        let mut buscadas = std::time::Instant::now() - Duration::from_secs(10);
        let mut vols: Vec<ISimpleAudioVolume> = vec![];
        loop {
            std::thread::sleep(PASO);
            let objetivo = if SILENCIO.load(Ordering::Relaxed) { 0.0 } else { 1.0 };
            if (nivel - objetivo).abs() < f32::EPSILON {
                continue;
            }
            // La sesión del juego aparece cuando empieza a sonar algo: se busca de nuevo cada 2 s.
            if vols.is_empty() && buscadas.elapsed() > Duration::from_secs(2) {
                vols = sesiones();
                buscadas = std::time::Instant::now();
            }
            let Some(primera) = vols.first() else { continue };
            if antes.is_none() {
                let v = unsafe { primera.GetMasterVolume() }.unwrap_or(1.0);
                antes = Some(v);
                let _ = std::fs::write(&nota, format!("{v}"));
            }
            let dt = PASO.as_secs_f32();
            nivel = if objetivo < nivel { (nivel - dt / BAJADA_S).max(0.0) } else { (nivel + dt / SUBIDA_S).min(1.0) };
            let base = antes.unwrap_or(1.0);
            let mut fallo = false;
            for v in &vols {
                fallo |= unsafe { v.SetMasterVolume(base * nivel, std::ptr::null()) }.is_err();
            }
            if fallo {
                vols.clear();
            }
            if nivel >= 1.0 {
                antes = None;
                let _ = std::fs::remove_file(&nota);
            }
        }
    });
}

//! Idioma del mod: el del juego, que Steam guarda en el manifiesto del juego (appmanifest_1245620.acf,
//! «language»: spanish, latam...). runtime/idioma.txt («es» o «en») manda sobre él.

use std::path::Path;
use std::sync::OnceLock;

use rogue_core::idioma::Traduccion;

static TRAD: OnceLock<Option<Traduccion>> = OnceLock::new();

/// Código del idioma del juego si el mod lo tiene traducido («es»); None = inglés.
fn detectar(root: &Path) -> Option<&'static str> {
    if let Ok(t) = std::fs::read_to_string(root.join("runtime").join("idioma.txt")) {
        return match t.trim() {
            "es" => Some("es"),
            _ => None,
        };
    }
    // .../steamapps/common/ELDEN RING/Game/eldenring.exe → .../steamapps/appmanifest_1245620.acf
    let exe = std::env::current_exe().ok()?;
    let steamapps = exe.parent()?.parent()?.parent()?.parent()?;
    let acf = std::fs::read_to_string(steamapps.join("appmanifest_1245620.acf")).ok()?;
    let idioma = acf.lines().find_map(|l| {
        let mut partes = l.split('"').filter(|p| !p.trim().is_empty());
        (partes.next()? == "language").then(|| partes.next().map(str::to_string)).flatten()
    })?;
    matches!(idioma.as_str(), "spanish" | "latam").then_some("es")
}

/// Traducción del idioma del juego (se carga una vez y activa los textos de pantalla).
pub fn traduccion(root: &Path) -> Option<&'static Traduccion> {
    TRAD.get_or_init(|| {
        let codigo = detectar(root)?;
        let ruta = root.join("data").join("i18n").join(format!("{codigo}.json"));
        let t: Traduccion = match std::fs::read_to_string(&ruta).map(|s| serde_json::from_str(&s)) {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => {
                crate::log(&format!("idioma {codigo}: {} mal formado ({e}); se queda en inglés", ruta.display()));
                return None;
            }
            Err(_) => return None,
        };
        rogue_core::idioma::activar(t.ui.clone());
        crate::log(&format!("idioma: {codigo} ({} textos de pantalla, {} bendiciones)", t.ui.len(), t.bendiciones.len()));
        Some(t)
    })
    .as_ref()
}

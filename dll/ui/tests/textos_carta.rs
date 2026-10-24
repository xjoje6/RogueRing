//! El nombre, el texto y el precio de cada bendición caben en su ficha (lo que no cabe no se ve).
//! Mide con la misma letra que la capa (la de los menús del juego o, sin ella, Palatino); sin
//! ninguna, la prueba no hace nada.

use rogue_core::relics::RelicFile;
use rogue_ui::cartas::{LINEAS, lineas};
use rogue_ui::load_fonts;

#[test]
fn las_bendiciones_caben_en_su_carta() {
    let Some((body, title)) = rogue_ui::leer_fuentes(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))) else {
        eprintln!("sin letra: no se mide");
        return;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(root.join("data/reliquias.json")).unwrap()).unwrap();
    // También en español (data/i18n/es.json): suele ocupar más que el inglés.
    let mut es = rel.clone();
    let t: rogue_core::idioma::Traduccion = serde_json::from_str(&std::fs::read_to_string(root.join("data/i18n/es.json")).unwrap()).unwrap();
    t.bendiciones(&mut es);
    let mut ctx = imgui::Context::create();
    ctx.set_ini_filename(None);
    ctx.io_mut().display_size = [1920.0, 1080.0];
    let fonts = load_fonts(&mut ctx, &body, &title, 1.0);
    ctx.fonts().build_rgba32_texture();
    let ui = ctx.new_frame();
    let mut fallos = vec![];
    let mut mas_larga = 0;
    for (idioma, r) in rel.reliquias.iter().map(|r| ("en", r)).chain(es.reliquias.iter().map(|r| ("es", r))) {
        let contra = r.contra.as_deref().unwrap_or("");
        for (i, n) in r.niveles.iter().enumerate() {
            let l = lineas(ui, &fonts, &r.nombre, &n.texto, contra);
            mas_larga = mas_larga.max(l[1]);
            if l.iter().zip(LINEAS).any(|(a, b)| *a > b) {
                fallos.push(format!("{} [{idioma}] (nivel {i}): {l:?} líneas, caben {LINEAS:?}", r.id));
            }
        }
    }
    // Si la medida fallara (todo en una línea), la prueba no serviría de nada. La ficha es ancha: el
    // texto más largo ocupa 2 líneas.
    assert!(mas_larga >= 2, "la medida no funciona: el texto más largo ocupa {mas_larga} líneas");
    assert!(fallos.is_empty(), "no caben:\n{}", fallos.join("\n"));
}

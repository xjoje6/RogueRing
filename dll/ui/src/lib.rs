//! Pantallas de RogueRing en Dear ImGui. Mismo código dentro del juego (hudhook) y en las vistas
//! previas fuera del juego (`soft`, rasterizador CPU).
pub mod adorno;
pub mod aliados;
pub mod bendiciones;
pub mod cartas;
pub mod estilo;
pub mod evento;
pub mod fin;
pub mod grafico;
pub mod jefe;
pub mod mapa;
pub mod pausa;
pub mod personajes;
pub mod soft;
pub mod tira;
pub mod titulo;

use imgui::{FontConfig, FontId, FontSource};

#[derive(Clone, Copy)]
pub struct Fonts {
    pub body: FontId,
    pub title: FontId,
    pub caption: FontId,
    /// Menús nuevos (estilo.rs): título de pantalla, opción de lista, texto de ficha y notas.
    pub grande: FontId,
    pub opcion: FontId,
    pub texto: FontId,
    pub nota: FontId,
    /// Letra enorme, solo mayúsculas: el «HAS MUERTO» del final.
    pub enorme: FontId,
}

/// Letras de la capa, libres (OFL, data/fuentes) y las más parecidas a las del juego: EB Garamond
/// (peso 450) como la Agmena de sus menús, para todo, y Libre Caslon Text (500) como la ITC Caslon de
/// su mapa, para los rótulos sobre el mapa. Sin ellas, el Palatino de Windows (normal y cursiva).
pub fn leer_fuentes(root: &std::path::Path) -> Option<(Vec<u8>, Vec<u8>)> {
    let libres = root.join("data").join("fuentes");
    if let (Ok(a), Ok(b)) = (std::fs::read(libres.join("EBGaramond.ttf")), std::fs::read(libres.join("LibreCaslonText.ttf"))) {
        return Some((a, b));
    }
    match (std::fs::read("C:/Windows/Fonts/pala.ttf"), std::fs::read("C:/Windows/Fonts/palai.ttf")) {
        (Ok(a), Ok(b)) => Some((a, b)),
        _ => None,
    }
}

/// Carga la tipografía (TTF) en sus tamaños. `scale` = alto de pantalla / 1080.
pub fn load_fonts(ctx: &mut imgui::Context, ttf: &[u8], ttf_title: &[u8], scale: f32) -> Fonts {
    let mut add = |data: &[u8], px: f32| {
        ctx.fonts().add_font(&[FontSource::TtfData {
            data,
            size_pixels: (px * scale).round(),
            config: Some(FontConfig { oversample_h: 2, oversample_v: 2, ..Default::default() }),
        }])
    };
    let body = add(ttf, 23.0);
    let title = add(ttf_title, 40.0);
    let caption = add(ttf, 18.0);
    let grande = add(ttf, 49.0);
    let opcion = add(ttf, 35.0);
    let texto = add(ttf, 30.0);
    let nota = add(ttf, 25.0);
    // Solo mayúsculas, cifras y signos (con las mayúsculas acentuadas): el atlas no crece de más.
    static MAYUSCULAS: [u32; 5] = [0x20, 0x5F, 0xC0, 0xDE, 0];
    let enorme = ctx.fonts().add_font(&[FontSource::TtfData {
        data: ttf,
        size_pixels: (120.0 * scale).round(),
        config: Some(FontConfig { glyph_ranges: imgui::FontGlyphRanges::from_slice(&MAYUSCULAS), ..Default::default() }),
    }]);
    Fonts { body, title, caption, grande, opcion, texto, nota, enorme }
}

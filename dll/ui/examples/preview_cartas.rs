//! Vista previa de la pantalla de recompensa sin abrir el juego: un arma, una bendición y una mítica
//! (con su precio en rojo).
//! cargo run --release -p rogue-ui --example preview_cartas -- <salida.png> [mítica] [bendición] [maldición]
use std::collections::HashMap;

use rogue_core::relics::RelicFile;
use rogue_ui::{adorno, cartas, load_fonts, mapa, soft};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or("cartas.png".into());
    let mitica = a.get(2).cloned().unwrap_or("tormenta".into());
    let normal = a.get(3).cloned().unwrap_or("intercambio".into());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (w, h) = (1920u32, 1080u32);
    let mut ctx = imgui::Context::create();
    ctx.set_ini_filename(None);
    ctx.io_mut().display_size = [w as f32, h as f32];
    let (body, title) = rogue_ui::leer_fuentes(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))).unwrap();
    let fonts = load_fonts(&mut ctx, &body, &title, h as f32 / 1080.0);
    let mut textures = HashMap::new();
    {
        let atlas = ctx.fonts();
        let t = atlas.build_rgba32_texture();
        textures.insert(imgui::TextureId::new(1), soft::Texture { w: t.width, h: t.height, rgba: t.data.to_vec() });
        atlas.tex_id = imgui::TextureId::new(1);
    }
    let mut next = 2usize;
    let mut load = |path: std::path::PathBuf| -> Option<(imgui::TextureId, [f32; 2])> {
        let img = image::open(path).ok()?.to_rgba8();
        let size = [img.width() as f32, img.height() as f32];
        let id = imgui::TextureId::new(next);
        next += 1;
        textures.insert(id, soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
        Some((id, size))
    };
    let (bg, bg_size) = load(root.join("extract/ui/mapa_L2_gris.png")).unwrap();
    let mut slots = [None; adorno::MENU_N];
    for (i, n) in adorno::MENU_FILES.iter().enumerate() {
        slots[i] = load(root.join(format!("extract/ui/menu/{n}.png")));
    }
    let pool: Vec<rogue_core::rewards::Item> = serde_json::from_str(&std::fs::read_to_string(root.join("data/objetos/pool.json")).unwrap()).unwrap();
    let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(root.join("data/reliquias.json")).unwrap()).unwrap();
    let arma = pool.iter().find(|i| i.id == 9010000).unwrap();
    let mut card_of = |cat: &str, nombre: &str, tipo: &str, contra: &str, rareza, icono: i32| cartas::Card {
        cat: cat.into(),
        nombre: nombre.into(),
        tipo: tipo.into(),
        contra: contra.into(),
        rareza: Some(rareza),
        icon: load(root.join(format!("extract/ui/iconos/{icono}.png"))).map(|t| t.0),
        pie: String::new(),
        apagada: false,
        nota: None,
        doble: false,
    };
    let mut cards = vec![card_of("WEAPON", &arma.nombre, &arma.tipo, "", arma.rareza, arma.icono)];
    // DOBLE=1: el arma va en las dos manos (icono doble).
    if std::env::var("DOBLE").is_ok() {
        cards[0].doble = true;
        cards[0].cat = "DUAL WEAPON".into();
    }
    for (id, cat) in [(normal.as_str(), "BLESSING"), (mitica.as_str(), "MYTHIC BLESSING")] {
        let r = rel.get(id).unwrap();
        cards.push(card_of(cat, &r.nombre, &r.niveles[0].texto, r.contra.as_deref().unwrap_or(""), r.rareza, r.icono));
    }
    // Quinto argumento: una maldición para la tercera carta (el trato inicial), explicada debajo.
    if let Some(m) = a.get(4) {
        let r = rel.get(m).unwrap();
        cards[2].pie = format!("Curse: {}", r.nombre);
        cards[2].nota = Some((format!("Curse: {}", r.nombre), r.niveles[0].texto.clone()));
    }
    // TIENDA=1: la tienda de Kalé (con su arte de fondo y precios).
    let tienda = std::env::var("TIENDA").is_ok();
    let fondo = if tienda { load(root.join("extract/ui/carga/MENU_Load_00006_PS5.png")) } else { None };
    if tienda {
        for (c, p) in cards.iter_mut().zip([600, 450, 1200]) {
            c.pie = format!("{p} runes");
        }
    }
    let view = cartas::CardScreen {
        cabecera: if tienda { "MERCHANT" } else { "DEFEATED" }.into(),
        titulo: if tienda { "Kalé" } else { "Crucible Knight" }.into(),
        linea: if tienda { "You have 1240 runes" } else { "+ 420 runes" }.into(),
        instruccion: if tienda { "Kalé: \"Have a look. Everything here has a price.\"" } else { "Pick one. It stays with you for the rest of the run." }.into(),
        cards,
        selected: if std::env::var("DOBLE").is_ok() { 0 } else { 2 },
        salir: Some(if tienda { "Leave" } else { "Skip" }.into()),
        ayuda: vec![("A".into(), "Choose".into()), ("B".into(), "Skip".into())],
        destacada: None,
        fondo,
        t: 5.0,
        t_sel: 5.0,
    };
    let ui = ctx.new_frame();
    cartas::draw(
        ui,
        &view,
        &mapa::MapTextures { background: bg, background_size: bg_size, nodes: [None; 8], menu: adorno::MenuTex::from_slots(slots) },
        &fonts,
    );
    let dd = ctx.render();
    soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(&out).unwrap();
}

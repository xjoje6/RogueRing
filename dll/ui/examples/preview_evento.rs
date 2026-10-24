//! Vista previa de un evento de data/eventos.json sin abrir el juego.
//! cargo run --release -p rogue-ui --example preview_evento -- <id> <salida.png> [resultado]
use std::collections::HashMap;

use rogue_core::events::EventFile;
use rogue_ui::{adorno, evento, load_fonts, mapa, soft};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: EventFile = serde_json::from_str(&std::fs::read_to_string(root.join("data/eventos.json")).unwrap()).unwrap();
    let ev = f.eventos.iter().find(|e| e.id == a[1]).expect("evento");
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
    let mut load = |id: usize, path: std::path::PathBuf| {
        let img = image::open(path).unwrap().to_rgba8();
        let size = [img.width() as f32, img.height() as f32];
        textures.insert(imgui::TextureId::new(id), soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
        size
    };
    let bg_size = load(2, root.join("extract/ui/mapa_L2_gris.png"));
    let mut slots = [None; adorno::MENU_N];
    for (i, n) in adorno::MENU_FILES.iter().enumerate() {
        let p = root.join(format!("extract/ui/menu/{n}.png"));
        if p.exists() {
            slots[i] = Some((imgui::TextureId::new(100 + i), load(100 + i, p)));
        }
    }
    let img_size = load(3, root.join(format!("extract/ui/carga/{}.png", ev.imagen)));
    let resultado = a.get(3).is_some();
    let view = evento::EventView {
        cabecera: "ENCUENTRO".into(),
        titulo: ev.titulo.clone(),
        // TEXTO: otro texto (p. ej. la pantalla de la ascensión), con «|» como salto de línea.
        texto: std::env::var("TEXTO").map(|t| t.replace('|', "\n")).unwrap_or_else(|_| if resultado { ev.opciones[0].resultado.clone() } else { ev.texto.clone() }),
        imagen: Some((imgui::TextureId::new(3), img_size)),
        opciones: if resultado {
            vec![evento::EventOptionView { texto: "Seguir".into(), detalle: String::new(), apagada: false, nota: None }]
        } else {
            ev.opciones.iter().map(|o| evento::EventOptionView { texto: o.texto.clone(), detalle: o.detalle.clone(), apagada: false, nota: None }).collect()
        },
        selected: 0,
        ayuda: vec![("A".into(), "Elegir".into())],
        modo: if std::env::var("MENU").is_ok() { evento::Modo::Menu } else { evento::Modo::Encuentro },
        t: 5.0,
        t_sel: 5.0,
    };
    let ui = ctx.new_frame();
    evento::draw(
        ui,
        &view,
        &mapa::MapTextures { background: imgui::TextureId::new(2), background_size: bg_size, nodes: [None; 8], menu: adorno::MenuTex::from_slots(slots) },
        &fonts,
    );
    let dd = ctx.render();
    soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(&a[2]).unwrap();
}

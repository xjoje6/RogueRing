//! Vista previa de la barra del boss sobre una captura del juego.
//! cargo run --release -p rogue-ui --example preview_jefe -- <captura.png> <salida.png>
use std::collections::HashMap;

use rogue_ui::{adorno, jefe, load_fonts, soft};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let shot = image::open(&a[1]).unwrap().to_rgba8();
    let (w, h) = (shot.width(), shot.height());
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
    textures.insert(imgui::TextureId::new(2), soft::Texture { w, h, rgba: shot.into_raw() });
    // Piezas de las barras del juego (extract/ui/menu, tools/recursos_menu.py).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut slots = [None; adorno::MENU_N];
    for (i, n) in adorno::MENU_FILES.iter().enumerate() {
        if let Ok(img) = image::open(root.join(format!("extract/ui/menu/{n}.png"))) {
            let img = img.to_rgba8();
            let id = imgui::TextureId::new(10 + i);
            slots[i] = Some((id, [img.width() as f32, img.height() as f32]));
            textures.insert(id, soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
        }
    }
    let ui = ctx.new_frame();
    ui.get_background_draw_list().add_image(imgui::TextureId::new(2), [0.0, 0.0], [w as f32, h as f32]).build();
    let tex = adorno::MenuTex::from_slots(slots);
    // Estados de ejemplo: sangrado subiendo, congelación recién saltada y una quemadura (sin icono
    // de bendición aquí) con sus pilas.
    let estado = |i: usize, nombre: &str, color: [u8; 3], lleno: f32, destello: f32, texto: &str| jefe::EstadoVista {
        icono: tex.estados[i].map(|t| t.0),
        nombre: nombre.into(),
        color,
        lleno,
        destello,
        texto: texto.into(),
    };
    let estados = vec![
        estado(2, "Bleed", [196, 48, 48], 0.62, f32::MAX, ""),
        estado(4, "Frostbite", [150, 196, 232], 1.0, 0.2, ""),
        jefe::EstadoVista { icono: None, nombre: "Burn".into(), color: [232, 128, 52], lleno: 0.7, destello: f32::MAX, texto: "x3".into() },
        jefe::EstadoVista { icono: None, nombre: "Marked".into(), color: [196, 96, 78], lleno: 1.0, destello: f32::MAX, texto: "840 in 3 s".into() },
    ];
    let v = jefe::BossBar {
        name: "Perfumer Tricia".into(),
        hp: 5200,
        max: 9492,
        trail: 6400.0,
        hit: 1200,
        alpha: 1.0,
        tex,
        muts: "Sturdy  ·  Fury".into(),
        estados,
        marcas: vec![(0.05, true), (0.5, false)],
        nativa: None,
    };
    jefe::draw(ui, &v, &fonts);
    let dd = ctx.render();
    soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(&a[2]).unwrap();
}

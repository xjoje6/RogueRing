//! Vista previa de la tira de bendiciones sobre una captura del juego (área de cliente, sin marco).
//! cargo run --release -p rogue-ui --example preview_tira -- <captura.png> <carpeta de iconos> <salida.png>
use std::collections::HashMap;

use rogue_ui::aliados::{self, Aliado};
use rogue_ui::tira::{self, TiraItem};
use rogue_ui::{jefe, load_fonts, soft};

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
    // Escarcha (contador), Barrera (lista), Intercambio (bufo en marcha), Piel de hierro (pilas),
    // Última voluntad (gastada), Avalancha (pilas largas) y Cuatro guijarros (acaba de saltar).
    let iconos = [18250, 18540, 18890, 18493, 18110, 18070, 18280];
    for (k, i) in iconos.iter().enumerate() {
        let img = image::open(format!("{}/{i}.png", a[2])).unwrap().to_rgba8();
        textures.insert(imgui::TextureId::new(10 + k), soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
    }
    let t = |k: usize| Some(imgui::TextureId::new(10 + k));
    let items = vec![
        TiraItem { tex: t(0), anillo: Some(4.0 / 6.0), destello: 9.0, ..Default::default() },
        TiraItem { tex: t(1), anillo: Some(1.0), lista: true, destello: 9.0, ..Default::default() },
        TiraItem { tex: t(2), anillo: Some(0.55), lista: false, destello: 9.0, ..Default::default() },
        TiraItem { tex: t(3), pilas: Some((6, 10)), destello: 9.0, ..Default::default() },
        TiraItem { tex: t(4), gastada: true, destello: 9.0, ..Default::default() },
        TiraItem { tex: t(5), pilas: Some((17, 30)), destello: 9.0, ..Default::default() },
        TiraItem { tex: t(6), anillo: Some(0.0), destello: 0.12, ..Default::default() },
    ];
    let ui = ctx.new_frame();
    ui.get_background_draw_list().add_image(imgui::TextureId::new(2), [0.0, 0.0], [w as f32, h as f32]).build();
    tira::draw(ui, &items, 1.0);
    // Un compañero y una ceniza de tres lobos, uno caído.
    let al = vec![
        Aliado { nombre: "amigo".into(), vidas: vec![(820, 1216)], companero: true },
        Aliado { nombre: "Cenizas de lobo solitario".into(), vidas: vec![(410, 560), (560, 560), (0, 560)], companero: false },
    ];
    aliados::draw(ui, &al, &fonts, 1.0);
    let v = jefe::BossBar {
        name: "Perfumer Tricia".into(),
        hp: 5200,
        max: 9492,
        trail: 6400.0,
        hit: 1200,
        alpha: 1.0,
        tex: Default::default(),
        muts: "Sturdy".into(),
        estados: vec![],
        marcas: vec![],
        nativa: None,
    };
    jefe::draw(ui, &v, &fonts);
    let dd = ctx.render();
    soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(&a[3]).unwrap();
}

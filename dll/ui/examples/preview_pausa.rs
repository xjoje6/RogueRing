//! Vista previa de la pausa en combate: el detalle de las bendiciones en marcha en el hueco del arte.
//! cargo run --release -p rogue-ui --example preview_pausa -- <salida.png>
use std::collections::HashMap;

use rogue_ui::pausa::PausaItem;
use rogue_ui::{adorno, evento, load_fonts, mapa, pausa, soft};

fn main() {
    let out = std::env::args().nth(1).unwrap_or("pausa.png".into());
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
    let tex = adorno::MenuTex::from_slots(slots);
    let mut icono = |i: i32| load(root.join(format!("extract/ui/iconos/{i}.png"))).map(|t| t.0);
    let items = vec![
        PausaItem {
            tex: icono(18250),
            nombre: "Frost on the Blade".into(),
            texto: "Every sixth hit hurls a frozen spike at the enemy.".into(),
            estado: "4 / 6 hits".into(),
            progreso: Some(4.0 / 6.0),
            ..Default::default()
        },
        PausaItem {
            tex: icono(18540),
            nombre: "Barrier".into(),
            texto: "A shield blocks a whole hit. It returns after 10 s without being hit.".into(),
            estado: "Ready".into(),
            progreso: Some(1.0),
            lista: true,
            ..Default::default()
        },
        PausaItem {
            tex: icono(18493),
            nombre: "Iron Hide".into(),
            texto: "Each hit you take: 3% less damage for 8 s, up to 30%.".into(),
            estado: "Stacks 6 / 10".into(),
            progreso: None,
            ..Default::default()
        },
        PausaItem {
            tex: icono(18521),
            nombre: "Tempest".into(),
            texto: "Every 3 s, lightning strikes the enemy.".into(),
            contra: "You take 25% more damage.".into(),
            estado: "Next in 2 s".into(),
            progreso: Some(0.33),
            ..Default::default()
        },
        PausaItem {
            tex: icono(18110),
            nombre: "Last Will".into(),
            texto: "Once per fight, a killing blow leaves you at 1 HP and invulnerable for 2 s.".into(),
            estado: "Used this fight".into(),
            gastada: true,
            ..Default::default()
        },
    ];
    let resto = vec!["Anvil".to_string(), "Extra Flask".into(), "Keen Eye".into(), "Under the Moon".into()];
    let view = evento::EventView {
        cabecera: "PAUSE".into(),
        titulo: "Paused".into(),
        texto: "Act 1. Seed SEYJGI.".into(),
        imagen: None,
        opciones: ["Resume", "Note a problem", "Main menu", "Abandon run", "Quit game"]
            .iter()
            .map(|o| evento::EventOptionView { texto: o.to_string(), detalle: String::new(), apagada: false, nota: None })
            .collect(),
        selected: 0,
        ayuda: vec![("A".into(), "Choose".into())],
        modo: evento::Modo::Pausa,
        t: 5.0,
        t_sel: 5.0,
    };
    // El combate de fondo: una pantalla de carga hace de escena.
    let escena = load(root.join("extract/ui/carga/MENU_Load_00010_PS5.png")).unwrap();
    let ui = ctx.new_frame();
    ui.get_background_draw_list().add_image(escena.0, [0.0, 0.0], [w as f32, h as f32]).build();
    let mt = mapa::MapTextures { background: bg, background_size: bg_size, nodes: [None; 8], menu: tex };
    evento::draw(ui, &view, &mt, &fonts);
    pausa::draw(ui, evento::hueco_pausa(ui), &items, &resto, "Weapon +12 / 25", &tex, &fonts, 5.0);
    let dd = ctx.render();
    soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(&out).unwrap();
}

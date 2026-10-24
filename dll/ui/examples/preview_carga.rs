//! Vista previa de la pantalla de carga del mod (viaje a un combate) y de la barra del jefe con las
//! piezas de las barras del juego.
//! cargo run --release -p rogue-ui --example preview_carga -- <salida_carga.png> <salida_jefe.png> <captura de combate.png>
use std::collections::HashMap;

use rogue_ui::{adorno, jefe, load_fonts, soft};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (k, out) in [(0, &a[1]), (1, &a[2])] {
        let fondo = if k == 0 { root.join("extract/ui/carga/MENU_Load_00027.png") } else { std::path::PathBuf::from(&a[3]) };
        let shot = image::open(&fondo).unwrap().to_rgba8();
        let (w, h) = if k == 0 { (1920u32, 1080u32) } else { (shot.width(), shot.height()) };
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
        let (sw, sh) = (shot.width() as f32, shot.height() as f32);
        textures.insert(imgui::TextureId::new(2), soft::Texture { w: shot.width(), h: shot.height(), rgba: shot.into_raw() });
        let mut slots = [None; adorno::MENU_N];
        for (i, n) in adorno::MENU_FILES.iter().enumerate() {
            if let Ok(img) = image::open(root.join(format!("extract/ui/menu/{n}.png"))) {
                let img = img.to_rgba8();
                let id = imgui::TextureId::new(10 + i);
                slots[i] = Some((id, [img.width() as f32, img.height() as f32]));
                textures.insert(id, soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
            }
        }
        let tex = adorno::MenuTex::from_slots(slots);
        let ui = ctx.new_frame();
        {
            let (wf, hf) = (w as f32, h as f32);
            let s = hf / 1080.0;
            let dl = ui.get_background_draw_list();
            let kk = (wf / sw).max(hf / sh);
            dl.add_image(imgui::TextureId::new(2), [(wf - sw * kk) / 2.0, (hf - sh * kk) / 2.0], [(wf + sw * kk) / 2.0, (hf + sh * kk) / 2.0]).build();
            if k == 0 {
                dl.add_rect([0.0, 0.0], [wf, hf], imgui::ImColor32::from_rgba(0, 0, 0, 115)).filled(true).build();
                let fg = ui.get_foreground_draw_list();
                let (tipo, nombre) = ("ELITE", "Crucible Knight");
                let cy = hf * 0.46;
                {
                    let _f = ui.push_font(fonts.caption);
                    let tw = ui.calc_text_size(tipo)[0];
                    fg.add_text([(wf - tw) / 2.0, cy - 40.0 * s], imgui::ImColor32::from_rgba(196, 96, 78, 255), tipo);
                }
                let nw = {
                    let _f = ui.push_font(fonts.title);
                    let nw = ui.calc_text_size(nombre)[0];
                    fg.add_text([(wf - nw) / 2.0, cy - 12.0 * s], imgui::ImColor32::from_rgba(226, 218, 198, 255), nombre);
                    nw
                };
                if let Some(u) = tex.subrayado {
                    adorno::subrayado(&fg, u, wf / 2.0, cy + 48.0 * s, nw + 260.0 * s, s, 0.9);
                }
                let bw = wf * 0.3;
                adorno::barra(&fg, &tex, (wf - bw) / 2.0, hf * 0.86, bw, 6.0 * s, 0.62, 0.62, [214, 186, 120], 1.0, s);
                if let Some((t, [tw, th])) = tex.carga_icono {
                    let ih = 110.0 * s;
                    let iw = ih * tw / th;
                    fg.add_image(t, [wf - 60.0 * s - iw, hf - 50.0 * s - ih], [wf - 60.0 * s, hf - 50.0 * s]).build();
                }
            } else {
                let v = jefe::BossBar {
                    name: "Crucible Knight".into(),
                    hp: 5200,
                    max: 9492,
                    trail: 6400.0,
                    hit: 1200,
                    alpha: 1.0,
                    tex,
                    muts: "Sturdy".into(),
                    estados: vec![],
                    marcas: vec![],
                    nativa: None,
                };
                jefe::draw(ui, &v, &fonts);
            }
        }
        let dd = ctx.render();
        soft::render(dd, &textures, w, h, [0, 0, 0, 255]).save(out).unwrap();
    }
}

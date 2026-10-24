//! Lo común de las vistas previas: contexto de ImGui a 1920x1080 con las fuentes del juego, texturas
//! cargadas de extract/ui y el volcado a PNG con el rasterizador de CPU.
#![allow(dead_code)]
use std::collections::HashMap;
use std::path::PathBuf;

use imgui::TextureId;
use rogue_ui::{adorno, load_fonts, mapa, soft, Fonts};

pub struct Vista {
    pub ctx: imgui::Context,
    pub fonts: Fonts,
    pub textures: HashMap<TextureId, soft::Texture>,
    next: usize,
    pub root: PathBuf,
    pub w: u32,
    pub h: u32,
}

impl Vista {
    pub fn new() -> Self {
        let (w, h) = (1920u32, 1080u32);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ctx = imgui::Context::create();
        ctx.set_ini_filename(None);
        ctx.io_mut().display_size = [w as f32, h as f32];
        let (body, title) = rogue_ui::leer_fuentes(&root).unwrap();
        let body: &'static [u8] = Box::leak(body.into_boxed_slice());
        let title: &'static [u8] = Box::leak(title.into_boxed_slice());
        let fonts = load_fonts(&mut ctx, body, title, h as f32 / 1080.0);
        let mut textures = HashMap::new();
        {
            let atlas = ctx.fonts();
            let t = atlas.build_rgba32_texture();
            textures.insert(TextureId::new(1), soft::Texture { w: t.width, h: t.height, rgba: t.data.to_vec() });
            atlas.tex_id = TextureId::new(1);
        }
        Vista { ctx, fonts, textures, next: 2, root, w, h }
    }

    /// Carga un PNG de la raíz del proyecto.
    pub fn load(&mut self, rel: &str) -> Option<(TextureId, [f32; 2])> {
        let img = image::open(self.root.join(rel)).ok()?.to_rgba8();
        let size = [img.width() as f32, img.height() as f32];
        let id = TextureId::new(self.next);
        self.next += 1;
        self.textures.insert(id, soft::Texture { w: img.width(), h: img.height(), rgba: img.into_raw() });
        Some((id, size))
    }

    pub fn icono(&mut self, i: i32) -> Option<TextureId> {
        self.load(&format!("extract/ui/iconos/{i}.png")).map(|t| t.0)
    }

    pub fn menu(&mut self) -> adorno::MenuTex {
        let mut slots = [None; adorno::MENU_N];
        for (i, n) in adorno::MENU_FILES.iter().enumerate() {
            slots[i] = self.load(&format!("extract/ui/menu/{n}.png"));
        }
        adorno::MenuTex::from_slots(slots)
    }

    /// Texturas del mapa con el fondo dado (extract/ui/...).
    pub fn mapa(&mut self, fondo: &str) -> mapa::MapTextures {
        let (background, background_size) = self.load(fondo).unwrap();
        let mut nodes = [None; 8];
        for (i, n) in mapa::NODE_ICONS.iter().enumerate() {
            nodes[i] = self.load(&format!("extract/ui/mapa/{n}.png")).map(|t| t.0);
        }
        let menu = self.menu();
        mapa::MapTextures { background, background_size, nodes, menu }
    }

    /// Dibuja con `f` un fotograma y lo guarda en `out`.
    pub fn render(&mut self, out: &str, f: impl FnOnce(&imgui::Ui, &Fonts)) {
        let fonts = Fonts { ..self.fonts };
        let ui = self.ctx.new_frame();
        f(ui, &fonts);
        let dd = self.ctx.render();
        soft::render(dd, &self.textures, self.w, self.h, [0, 0, 0, 255]).save(out).unwrap();
    }
}

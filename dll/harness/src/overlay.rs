//! Overlay dentro del juego (hudhook, DirectX 12): dibuja las pantallas de `rogue_ui` a partir del
//! estado del director. Qué pantalla se ve lo decide `SCREEN` (el director la pone en cada fotograma).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use hudhook::imgui::{Context, TextureId, Ui};
use hudhook::{ImguiRenderLoop, RenderContext};
use rogue_ui::{Fonts, bendiciones, cartas, evento, fin, jefe, mapa};

use crate::director::{self, UiState};

pub const SCREEN_NONE: u8 = 0;
pub const SCREEN_MAP: u8 = 1;
pub const SCREEN_CARDS: u8 = 2;
pub const SCREEN_OVER: u8 = 3;
pub const SCREEN_EVENT: u8 = 4;
pub const SCREEN_LIST: u8 = 5;
/// Personajes: panel a la izquierda y el mundo visible (el personaje de frente).
pub const SCREEN_PERSONAJES: u8 = 6;
pub static SCREEN: AtomicU8 = AtomicU8::new(SCREEN_NONE);
/// Banco de pruebas: sin pantallas del mod (capturas del personaje en el centro).
pub static OCULTAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Telón durante los viajes (lo pone el director): tapa el mundo entre elegir y cargar.
pub static COVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Arte de las pantallas de carga del mod (extract/ui/carga; las copia tools/empaquetar.py). Solo
/// las de pantalla completa (_PS5): las otras traen bandas negras a los lados. La primera es la del
/// arranque; el telón de cada viaje elige una.
pub const CARGAS: [&str; 7] = [
    "MENU_Load_00001_PS5",
    "MENU_Load_00010_PS5",
    "MENU_Load_00002_PS5",
    "MENU_Load_00005_PS5",
    "MENU_Load_00014_PS5",
    "MENU_Load_00016_PS5",
    "MENU_Load_00021_PS5",
];

pub struct Overlay {
    root: PathBuf,
    fonts: Option<Fonts>,
    bg: Option<(TextureId, [f32; 2])>,
    /// Tramo del mapa del juego de cada acto (extract/ui/fondos, tools/map_background.py).
    fondos: [Option<(TextureId, [f32; 2])>; 3],
    node_icons: [Option<TextureId>; 8],
    /// Piezas de los menús del juego (rogue_ui::adorno::MENU_FILES).
    menu_tex: [Option<(TextureId, [f32; 2])>; rogue_ui::adorno::MENU_N],
    /// Pantalla actual (para las animaciones de entrada y el sonido al abrir) y selección.
    screen_key: String,
    screen_t0: Instant,
    sel_val: usize,
    sel_t0: Instant,
    /// Iconos de objeto ya subidos a la GPU (iconId → textura); None si no hay PNG.
    icons: HashMap<i32, Option<TextureId>>,
    /// Arte de los eventos (pantallas de carga) ya subido: nombre → textura y tamaño.
    event_imgs: HashMap<String, Option<(TextureId, [f32; 2])>>,
    start: Instant,
    /// Opacidad actual del telón (entra de golpe, sale con fundido).
    cover: f32,
    /// Arte de carga subido (CARGAS) y el que lleva el telón ahora.
    cargas: Vec<(TextureId, [f32; 2])>,
    carga: usize,
    /// Desde cuándo está puesto el telón (barra de carga).
    telon_desde: Instant,
    /// Estado animado de la barra del boss.
    bar: BarAnim,
}

/// Tramo de daño reciente y número del golpe, como en la barra nativa.
#[derive(Default)]
struct BarAnim {
    name: String,
    last_hp: i32,
    trail: f32,
    /// Segundos desde el último golpe.
    since_hit: f32,
    hit: i32,
    /// Segundos desde que apareció la barra (fundido de entrada).
    shown: f32,
}

// Solo lo usa hudhook desde el hilo de render; FontId (puntero de imgui) no es Send por defecto.
unsafe impl Send for Overlay {}
unsafe impl Sync for Overlay {}

impl Overlay {
    pub fn new(root: PathBuf) -> Self {
        Overlay {
            root,
            fonts: None,
            bg: None,
            fondos: [None; 3],
            node_icons: [None; 8],
            menu_tex: [None; rogue_ui::adorno::MENU_N],
            screen_key: String::new(),
            screen_t0: Instant::now(),
            sel_val: 0,
            sel_t0: Instant::now(),
            icons: HashMap::new(),
            event_imgs: HashMap::new(),
            start: Instant::now(),
            cover: 0.0,
            cargas: vec![],
            carga: 0,
            telon_desde: Instant::now(),
            bar: BarAnim::default(),
        }
    }

    fn load_png(&self, rc: &mut dyn RenderContext, path: PathBuf) -> Option<(TextureId, [f32; 2])> {
        let img = image::open(&path).ok()?.to_rgba8();
        let id = rc.load_texture(img.as_raw(), img.width(), img.height()).ok()?;
        Some((id, [img.width() as f32, img.height() as f32]))
    }
}

impl Overlay {
    fn draw_boss_bar(&mut self, ui: &Ui) {
        let Some(fonts) = &self.fonts else { return };
        let cur = director::BOSS_BAR.try_lock().ok().and_then(|b| b.clone());
        let Some((name, hp, max)) = cur else {
            self.bar = BarAnim::default();
            return;
        };
        let dt = ui.io().delta_time;
        let b = &mut self.bar;
        if b.name != name {
            *b = BarAnim { name: name.clone(), last_hp: hp, trail: hp as f32, since_hit: 99.0, hit: 0, shown: 0.0 };
        }
        if hp < b.last_hp {
            // Golpes seguidos se suman en el número, como hace el juego.
            b.hit = if b.since_hit < 2.0 { b.hit + (b.last_hp - hp) } else { b.last_hp - hp };
            b.since_hit = 0.0;
        }
        b.last_hp = hp;
        b.since_hit += dt;
        b.shown += dt;
        // El tramo amarillo espera un poco y luego baja rápido hasta la vida real.
        if b.since_hit > 0.8 {
            b.trail = (b.trail - max as f32 * 0.5 * dt).max(hp as f32);
        }
        if hp as f32 > b.trail {
            b.trail = hp as f32;
        }
        let hit = if b.since_hit < 2.5 { b.hit } else { 0 };
        let alpha = rogue_ui::adorno::ease_out(b.shown / 0.7);
        let muts = director::BOSS_MUTS.try_lock().map(|m| m.clone()).unwrap_or_default();
        let tex = rogue_ui::adorno::MenuTex::from_slots(self.menu_tex);
        let (estados, marcas) = match director::RIVAL.try_lock() {
            Ok(r) => (
                r.0.iter()
                    .map(|(icono, e)| jefe::EstadoVista {
                        icono: match e.estado {
                            Some(i) => tex.estados[i].map(|t| t.0),
                            None => self.icons.get(icono).copied().flatten(),
                        },
                        nombre: e.nombre.clone(),
                        color: e.color,
                        lleno: e.lleno,
                        destello: e.destello,
                        texto: e.texto.clone(),
                    })
                    .collect(),
                r.1.clone(),
            ),
            Err(_) => (vec![], vec![]),
        };
        let nativa = match director::BARRA_NATIVA.load(Ordering::Relaxed) {
            0 => None,
            v => Some(((v & 0xff) as usize, (v >> 8) as usize)),
        };
        jefe::draw(ui, &jefe::BossBar { name, hp, max, trail: b.trail, hit, alpha, tex, muts, estados, marcas, nativa }, fonts);
    }
}

impl ImguiRenderLoop for Overlay {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, rc: &'a mut dyn RenderContext) {
        ctx.set_ini_filename(None);
        // Letra: la de los menús y la del mapa del juego (rogue_ui::leer_fuentes).
        if let Some((body, title)) = rogue_ui::leer_fuentes(&self.root) {
            // La letra se rasteriza al tamaño real de la ventana (720 en pruebas, 1440 jugando).
            let h = crate::game_client_height().unwrap_or(1080) as f32;
            let body: &'static [u8] = Box::leak(body.into_boxed_slice());
            let title: &'static [u8] = Box::leak(title.into_boxed_slice());
            self.fonts = Some(rogue_ui::load_fonts(ctx, body, title, h / 1080.0));
        }
        self.bg = self.load_png(rc, self.root.join("extract").join("ui").join("mapa_L2_gris.png"));
        for i in 0..3 {
            self.fondos[i] = self.load_png(rc, self.root.join("extract").join("ui").join("fondos").join(format!("acto{}.png", i + 1)));
        }
        for (i, n) in rogue_ui::adorno::MENU_FILES.iter().enumerate() {
            self.menu_tex[i] = self.load_png(rc, self.root.join("extract").join("ui").join("menu").join(format!("{n}.png")));
        }
        self.cargas = CARGAS.iter().filter_map(|n| self.load_png(rc, self.root.join("extract").join("ui").join("carga").join(format!("{n}.png")))).collect();
        for (i, n) in mapa::NODE_ICONS.iter().enumerate() {
            self.node_icons[i] = self.load_png(rc, self.root.join("extract").join("ui").join("mapa").join(format!("{n}.png"))).map(|t| t.0);
        }
        crate::log(&format!("overlay listo (fuentes: {}, fondo: {})", self.fonts.is_some(), self.bg.is_some()));
    }

    fn before_render<'a>(&'a mut self, _ctx: &mut Context, rc: &'a mut dyn RenderContext) {
        // Iconos de los avisos y de la lista de bendiciones.
        let mut want: Vec<i32> = director::TOASTS.try_lock().map(|t| t.iter().map(|x| x.icono).collect()).unwrap_or_default();
        want.extend(director::TIRA.try_lock().map(|t| t.iter().map(|x| x.0).collect::<Vec<_>>()).unwrap_or_default());
        if let Ok(Some(p)) = director::PAUSA_COMBATE.try_lock().as_deref() {
            want.extend(p.filas.iter().map(|f| f.icono));
        }
        match (SCREEN.load(Ordering::Relaxed), director::ui_state()) {
            (SCREEN_LIST, Some(UiState::Blessings { items, .. })) => want.extend(items.iter().map(|x| x.2)),
            (_, Some(UiState::Titulo { guardada: Some(g), .. })) => {
                want.push(g.arma_icono);
                want.extend(g.bendiciones.iter().map(|b| b.0));
            }
            (_, Some(UiState::Personajes { armas, .. })) => want.extend(armas.iter().map(|a| a.3)),
            (SCREEN_LIST, Some(UiState::Compendio { items, .. })) => want.extend(items.iter().map(|x| x.2)),
            (SCREEN_MAP, Some(UiState::Map { relics, .. })) => want.extend(relics.iter().map(|x| x.2)),
            (SCREEN_OVER, Some(UiState::Over { relics, historia, marcador, .. })) => {
                want.extend(relics.iter().map(|x| x.1));
                want.push(marcador);
                for h in &historia {
                    want.extend(h.bendiciones.iter().map(|b| b.0));
                    want.extend(h.objetos.iter().copied());
                }
            }
            _ => {}
        }
        for icono in want.into_iter().filter(|i| *i > 0) {
            if !self.icons.contains_key(&icono) {
                let tex = self.load_png(rc, self.root.join("extract").join("ui").join("iconos").join(format!("{icono}.png"))).map(|t| t.0);
                self.icons.insert(icono, tex);
            }
        }
        if SCREEN.load(Ordering::Relaxed) == SCREEN_EVENT {
            if let Some(UiState::Event { imagen, .. }) = director::ui_state()
                && !self.event_imgs.contains_key(&imagen)
            {
                let t = self.load_png(rc, self.root.join("extract").join("ui").join("carga").join(format!("{imagen}.png")));
                self.event_imgs.insert(imagen, t);
            }
            return;
        }
        // Subir los iconos de las cartas visibles la primera vez que se ven.
        if SCREEN.load(Ordering::Relaxed) != SCREEN_CARDS {
            return;
        }
        if let Some(UiState::Cards { cards, destacada, fondo, .. }) = director::ui_state() {
            if let Some(f) = fondo
                && !self.event_imgs.contains_key(&f)
            {
                let t = self.load_png(rc, self.root.join("extract").join("ui").join("carga").join(format!("{f}.png")));
                self.event_imgs.insert(f, t);
            }
            for c in cards.iter().chain(destacada.iter()).filter(|c| c.icono > 0) {
                if !self.icons.contains_key(&c.icono) {
                    let tex = self.load_png(rc, self.root.join("extract").join("ui").join("iconos").join(format!("{}.png", c.icono))).map(|t| t.0);
                    self.icons.insert(c.icono, tex);
                }
            }
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        // Arranque directo: tapa el título y la carga hasta la primera pantalla del mod.
        let tapando = crate::arranque::TAPANDO.load(Ordering::Relaxed);
        let telon = COVER.load(Ordering::Relaxed);
        // Lo que pasa detrás de la capa no se oye (sonido.rs).
        crate::sonido::SILENCIO.store(tapando || telon, Ordering::Relaxed);
        if tapando {
            self.draw_arranque(ui);
            return;
        }
        let screen = SCREEN.load(Ordering::Relaxed);
        // Cada viaje, otra imagen (sin repetir la del anterior).
        if telon && self.cover <= 0.0 && self.cargas.len() > 1 {
            let n = self.cargas.len() - 1;
            let salto = 1 + (self.start.elapsed().as_millis() as usize) % (n - 1).max(1);
            self.carga = 1 + (self.carga.saturating_sub(1) + salto) % n;
        }
        if telon && self.cover <= 0.0 {
            self.telon_desde = Instant::now();
        }
        self.cover = if telon { 1.0 } else { (self.cover - ui.io().delta_time / 0.6).max(0.0) };
        if self.cover > 0.0 {
            let [w, h] = ui.io().display_size;
            self.fondo_carga(ui, self.carga, self.cover);
            // Mientras el telón esté puesto la barra avanza sola; al quitarse, se llena.
            let p = if telon { progreso_falso(self.telon_desde.elapsed().as_secs_f32(), 3.0) } else { 1.0 };
            {
                let dl = ui.get_foreground_draw_list();
                self.barra_carga(ui, &dl, p, self.cover);
            }
            // Sobre el telón, a quién te vas a enfrentar.
            let rival = director::COVER_TEXT.try_lock().ok().and_then(|c| c.clone());
            if let (Some((tipo, nombre)), Some(fonts)) = (rival, &self.fonts) {
                let s = h / 1080.0;
                let dl = ui.get_foreground_draw_list();
                let k = self.cover;
                let col = |rgb: [u8; 3], al: f32| hudhook::imgui::ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (al * k * 255.0) as u8);
                // «COMBATE» → «Combate»: sin versalitas.
                let tipo: String = {
                    let low = tipo.to_lowercase();
                    let mut cs = low.chars();
                    cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or_default()
                };
                let (tw, nw) = {
                    let _f = ui.push_font(fonts.nota);
                    let t = ui.calc_text_size(&tipo)[0];
                    drop(_f);
                    let _f = ui.push_font(fonts.grande);
                    (t, ui.calc_text_size(&nombre)[0])
                };
                let cy = h * 0.44;
                {
                    let _f = ui.push_font(fonts.grande);
                    dl.add_text([(w - nw) / 2.0, cy - 24.0 * s], col([227, 215, 185], 1.0), &nombre);
                }
                if let Some(u) = self.menu_tex[3] {
                    rogue_ui::adorno::subrayado(&dl, u, w / 2.0, cy + 46.0 * s, nw + 260.0 * s, s, 0.9 * k);
                }
                {
                    let _f = ui.push_font(fonts.nota);
                    dl.add_text([(w - tw) / 2.0, cy + 66.0 * s], col([192, 87, 74], 1.0), &tipo);
                }
            }
        }
        // Avisos de bendiciones ganadas (encima de todo, salvo el telón).
        if let (Some(fonts), Ok(mut t)) = (&self.fonts, director::TOASTS.try_lock()) {
            t.retain(|x| x.desde.elapsed().as_secs_f32() < 4.6);
            let items: Vec<(bendiciones::BlessingView, f32)> = t
                .iter()
                .map(|a| {
                    (
                        bendiciones::BlessingView {
                            icon: self.icons.get(&a.icono).copied().flatten(),
                            nombre: a.nombre.clone(),
                            texto: a.texto.clone(),
                            maldita: a.maldita,
                        },
                        a.desde.elapsed().as_secs_f32(),
                    )
                })
                .collect();
            if self.cover < 0.5 {
                let en_mapa = SCREEN.load(Ordering::Relaxed) == SCREEN_MAP;
                bendiciones::draw_toasts(ui, &items, &rogue_ui::adorno::MenuTex::from_slots(self.menu_tex), fonts, en_mapa);
            }
        }
        self.draw_sala(ui);
        if screen == SCREEN_NONE {
            self.draw_boss_bar(ui);
            self.draw_tira(ui);
            self.draw_aliados(ui);
            self.draw_caido(ui);
            return;
        }
        let state = director::ui_state();
        // Tiempos para las animaciones: desde que se abrió la pantalla y desde el último cambio de selección.
        let (key, sel) = match &state {
            Some(UiState::Cards { cabecera, titulo, instruccion, selected, .. }) => (format!("c{cabecera}{titulo}{instruccion}"), *selected),
            Some(UiState::Event { titulo, texto, selected, .. }) => (format!("e{titulo}{}", texto.len()), *selected),
            Some(UiState::Blessings { sel, .. }) => ("l".into(), *sel),
            Some(UiState::Titulo { selected, .. }) => ("t".into(), *selected),
            Some(UiState::Compendio { sel, .. }) => ("k".into(), *sel),
            Some(UiState::Over { .. }) => ("o".into(), 0),
            Some(UiState::Map { .. }) => ("m".into(), 0),
            _ => (String::new(), 0),
        };
        if key != self.screen_key {
            if !key.is_empty() && key != "l" && key != "m" && key != "t" {
                director::play_se(director::Se::Open);
            }
            self.screen_key = key;
            self.screen_t0 = Instant::now();
            self.sel_t0 = Instant::now();
        }
        if sel != self.sel_val {
            self.sel_val = sel;
            self.sel_t0 = Instant::now();
        }
        let (t_in, t_sel) = (self.screen_t0.elapsed().as_secs_f32(), self.sel_t0.elapsed().as_secs_f32());
        let (Some(fonts), Some((bg, bg_size))) = (&self.fonts, self.bg) else { return };
        let tex =
            mapa::MapTextures { background: bg, background_size: bg_size, nodes: self.node_icons, menu: rogue_ui::adorno::MenuTex::from_slots(self.menu_tex) };
        let t = self.start.elapsed().as_secs_f32();
        match state {
            Some(UiState::Map { run, act_name, selected, relics, viales, arma, arma_nombre, invocacion, coop }) => {
                let mut view = mapa::MapView::from_run(&run, &act_name, selected);
                view.t_in = t_in;
                view.viales = viales;
                view.arma = arma;
                view.arma_nombre = arma_nombre;
                view.invocacion = invocacion;
                if let Some((votos, mi_voto, espera, cursores, esperar)) = coop {
                    view.coop = true;
                    view.esperar = esperar;
                    view.votos = votos;
                    view.mi_voto = mi_voto;
                    view.espera = espera;
                    view.cursores = cursores;
                }
                let items: Vec<bendiciones::BlessingView> = relics
                    .into_iter()
                    .map(|(nombre, texto, icono, maldita)| bendiciones::BlessingView {
                        icon: self.icons.get(&icono).copied().flatten(),
                        nombre,
                        texto,
                        maldita,
                    })
                    .collect();
                // El tramo del mapa de este acto (sin él, el mapa gris de antes).
                let mut tex = tex;
                if let Some((f, size)) = self.fondos.get(view.act_num.saturating_sub(1)).copied().flatten() {
                    tex.background = f;
                    tex.background_size = size;
                }
                mapa::draw(ui, &view, &tex, fonts, t, &items);
            }
            Some(UiState::Cards { cabecera, titulo, linea, instruccion, cards, selected, salir, ayuda, destacada, fondo }) => {
                let conv = |c: director::UiCard| cartas::Card {
                    icon: self.icons.get(&c.icono).copied().flatten(),
                    cat: c.cat,
                    nombre: c.nombre,
                    tipo: c.tipo,
                    rareza: c.rareza,
                    pie: c.pie,
                    apagada: c.apagada,
                    contra: c.contra,
                    nota: c.nota,
                    doble: c.doble,
                };
                let cards = cards.into_iter().map(conv).collect();
                let destacada = destacada.map(conv);
                let recompensa = cabecera == rogue_core::idioma::t("DEFEATED");
                let fondo = fondo.and_then(|f| self.event_imgs.get(&f).copied().flatten());
                let view = cartas::CardScreen { cabecera, titulo, linea, instruccion, cards, selected, salir, ayuda, destacada, fondo, t: t_in, t_sel };
                cartas::draw(ui, &view, &tex, fonts);
                if recompensa && let Some((seg, danio)) = director::LAST_COMBAT.try_lock().ok().and_then(|c| c.clone()) {
                    rogue_ui::grafico::draw_resumen_combate(ui, seg, &danio, t_in, fonts);
                }
            }
            Some(UiState::Personajes { nombre, lema, pasiva, armas, arma_sel, apariencias, apariencia_sel, invocaciones, invocacion_sel, idx, total }) => {
                let armas = armas.into_iter().map(|(n, ti, k, i)| (n, ti, k, self.icons.get(&i).copied().flatten())).collect();
                let view = rogue_ui::personajes::PersonajesView {
                    nombre,
                    lema,
                    pasiva,
                    armas,
                    arma_sel,
                    apariencias,
                    apariencia_sel,
                    invocaciones,
                    invocacion_sel,
                    idx,
                    total,
                    t: t_in,
                    t_sel,
                };
                rogue_ui::personajes::draw(ui, &view, &tex, fonts);
            }
            Some(UiState::Event { cabecera, titulo, texto, imagen, opciones, selected, modo }) => {
                let view = evento::EventView {
                    cabecera,
                    titulo,
                    texto,
                    imagen: self.event_imgs.get(&imagen).copied().flatten(),
                    opciones: opciones.into_iter().map(|(texto, detalle, apagada, nota)| evento::EventOptionView { texto, detalle, apagada, nota }).collect(),
                    selected,
                    ayuda: vec![("A".into(), "Choose".into())],
                    modo,
                    t: t_in,
                    t_sel,
                };
                evento::draw(ui, &view, &tex, fonts);
                self.draw_pausa_combate(ui, &tex.menu, fonts, t_in);
            }
            Some(UiState::Compendio { items, sel }) => {
                let items: Vec<bendiciones::CompendioItem> = items
                    .into_iter()
                    .map(|(nombre, texto, icono, rareza, etiqueta, vista)| bendiciones::CompendioItem {
                        icon: self.icons.get(&icono).copied().flatten(),
                        nombre,
                        texto,
                        rareza,
                        etiqueta,
                        vista,
                    })
                    .collect();
                bendiciones::draw_compendio(ui, &items, sel, t_in, &tex, fonts);
            }
            Some(UiState::Titulo { opciones, selected, texto, guardada }) => {
                let icon = |i: i32| self.icons.get(&i).copied().flatten();
                let run = guardada.map(|g| {
                    let mut m = mapa::MapView::from_run(&g.run, &g.act_name, None);
                    m.selected = None;
                    rogue_ui::titulo::RunResumen {
                        personaje: g.personaje,
                        arma: icon(g.arma_icono),
                        arma_nombre: g.arma_nombre,
                        semilla: g.run.seed.clone(),
                        mapa: m,
                        fondo: self.fondos.get(g.run.act_index).copied().flatten(),
                        bendiciones: g.bendiciones.iter().map(|(i, m)| (icon(*i), *m)).collect(),
                    }
                });
                let view = rogue_ui::titulo::TituloView { opciones, selected, texto, run, fondo_inicio: self.fondos[0], t: t_in, t_sel };
                rogue_ui::titulo::draw(ui, &view, &tex, fonts);
            }
            Some(UiState::Blessings { items, sel }) => {
                let items: Vec<bendiciones::BlessingView> = items
                    .into_iter()
                    .map(|(nombre, texto, icono, maldita)| bendiciones::BlessingView {
                        icon: self.icons.get(&icono).copied().flatten(),
                        nombre,
                        texto,
                        maldita,
                    })
                    .collect();
                bendiciones::draw_list(ui, &items, sel, t_in, &tex, fonts);
            }
            Some(UiState::Over { won, act, nodes, items, seed, personaje, golpes, elites, miticas, relics, historia, marcador, sel }) => {
                let relics = relics.into_iter().map(|(n, i, m)| (self.icons.get(&i).copied().flatten(), n, m)).collect();
                let icon = |i: i32| self.icons.get(&i).copied().flatten();
                let grafico = (!historia.is_empty()).then(|| rogue_ui::grafico::RunGraph {
                    nodes: historia
                        .iter()
                        .map(|h| rogue_ui::grafico::GraphNode {
                            acto: h.acto,
                            icono: mapa::node_icon_by_name(&tex, &h.tipo),
                            vida_min: h.vida_min,
                            combate: h.detalle.is_some(),
                            bendiciones: h.bendiciones.iter().map(|(i, m)| (icon(*i), *m)).collect(),
                            objetos: h.objetos.iter().map(|i| icon(*i)).collect(),
                            detalle: h.detalle.clone(),
                        })
                        .collect(),
                    marcador: icon(marcador),
                    sel: sel.min(historia.len().saturating_sub(1)),
                    t: t_in - fin::INTRO,
                    ganada: won,
                });
                // Detrás, el tramo del mapa del acto donde acabó.
                let mut tex = tex;
                if let Some((f, size)) = self.fondos.get(act.saturating_sub(1)).copied().flatten() {
                    tex.background = f;
                    tex.background_size = size;
                }
                fin::draw(ui, &fin::EndView { won, act, nodes, items, seed, personaje, golpes, elites, miticas, relics, t: t_in, grafico }, &tex, fonts);
            }
            _ => {}
        }
    }
}

impl Overlay {
    /// Pausa en combate: el detalle de las bendiciones en el hueco del arte (rogue_ui::pausa).
    fn draw_pausa_combate(&self, ui: &Ui, tex: &rogue_ui::adorno::MenuTex, fonts: &rogue_ui::Fonts, t: f32) {
        let Ok(g) = director::PAUSA_COMBATE.try_lock() else { return };
        let Some(p) = g.as_ref() else { return };
        let items: Vec<rogue_ui::pausa::PausaItem> = p
            .filas
            .iter()
            .map(|f| rogue_ui::pausa::PausaItem {
                tex: self.icons.get(&f.icono).copied().flatten(),
                nombre: f.nombre.clone(),
                texto: f.texto.clone(),
                contra: f.contra.clone(),
                estado: f.estado.estado.clone(),
                progreso: f.estado.anillo,
                lista: f.estado.lista,
                gastada: f.estado.gastada,
            })
            .collect();
        rogue_ui::pausa::draw(ui, evento::hueco_pausa(ui), &items, &p.resto, &p.arma, tex, fonts, t);
    }

    /// Vida de los aliados, debajo de la tira (rogue_ui::aliados).
    /// Cooperativo, caídos (director/caido.rs): el aviso y, mientras se levanta a alguien, su barra.
    fn draw_caido(&self, ui: &Ui) {
        use hudhook::imgui::ImColor32;
        let Some(fonts) = &self.fonts else { return };
        let Some((texto, avance)) = director::CAIDO_UI.try_lock().ok().and_then(|c| c.clone()) else { return };
        let [w, h] = ui.io().display_size;
        let _f = ui.push_font(fonts.texto);
        let tam = ui.calc_text_size(&texto);
        let (x, y) = ((w - tam[0]) / 2.0, h * 0.72);
        let dl = ui.get_background_draw_list();
        dl.add_text([x + 1.5, y + 1.5], ImColor32::from_rgba(0, 0, 0, 210), &texto);
        dl.add_text([x, y], ImColor32::from_rgba(236, 229, 212, 255), &texto);
        if avance >= 0.0 {
            let ancho = tam[0].max(240.0);
            let (bx, by) = ((w - ancho) / 2.0, y + tam[1] + 8.0);
            dl.add_rect([bx, by], [bx + ancho, by + 3.0], ImColor32::from_rgba(0, 0, 0, 170)).filled(true).build();
            dl.add_rect([bx, by], [bx + ancho * avance.clamp(0.0, 1.0), by + 3.0], ImColor32::from_rgba(212, 184, 116, 255)).filled(true).build();
        }
    }

    /// La sala del cooperativo en los menús, arriba a la derecha (director::SALA_UI).
    fn draw_sala(&self, ui: &Ui) {
        use hudhook::imgui::ImColor32;
        let Some(fonts) = &self.fonts else { return };
        let lineas = director::SALA_UI.try_lock().map(|s| s.clone()).unwrap_or_default();
        if lineas.is_empty() {
            return;
        }
        let [w, h] = ui.io().display_size;
        let s = h / 1080.0;
        let dl = ui.get_foreground_draw_list();
        let mut y = 40.0 * s;
        for (k, l) in lineas.iter().enumerate() {
            let _f = ui.push_font(if k == 0 { fonts.texto } else { fonts.caption });
            let tam = ui.calc_text_size(l);
            let x = w - 48.0 * s - tam[0];
            dl.add_text([x + 1.5, y + 1.5], ImColor32::from_rgba(0, 0, 0, 200), l);
            dl.add_text([x, y], if k == 0 { ImColor32::from_rgba(214, 186, 120, 255) } else { ImColor32::from_rgba(236, 229, 212, 255) }, l);
            y += tam[1] + 6.0 * s;
        }
    }

    fn draw_aliados(&self, ui: &Ui) {
        let Some(fonts) = &self.fonts else { return };
        let v = director::ALIADOS.try_lock().map(|a| a.clone()).unwrap_or_default();
        rogue_ui::aliados::draw(ui, &v, fonts, 1.0 - self.cover);
    }

    /// Tira de bendiciones del combate, debajo de los bufos del juego (rogue_ui::tira).
    fn draw_tira(&self, ui: &Ui) {
        let items: Vec<rogue_ui::tira::TiraItem> = director::TIRA
            .try_lock()
            .map(|t| {
                t.iter()
                    .map(|(icono, h)| rogue_ui::tira::TiraItem {
                        tex: self.icons.get(icono).copied().flatten(),
                        anillo: h.anillo,
                        lista: h.lista,
                        gastada: h.gastada,
                        pilas: h.pilas,
                        destello: h.destello,
                    })
                    .collect()
            })
            .unwrap_or_default();
        rogue_ui::tira::draw(ui, &items, 1.0 - self.cover);
    }

    /// Fondo de una pantalla de carga: el arte a pantalla completa (recortado, sin bandas) y oscurecido
    /// para que se lea el texto, como las del juego. Sin arte, negro.
    fn fondo_carga(&self, ui: &Ui, i: usize, alpha: f32) {
        let [w, h] = ui.io().display_size;
        let dl = ui.get_background_draw_list();
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        dl.add_rect([0.0, 0.0], [w, h], hudhook::imgui::ImColor32::from_rgba(0, 0, 0, a)).filled(true).build();
        if let Some((tex, [iw, ih])) = self.cargas.get(i).or(self.cargas.first()) {
            let k = (w / iw).max(h / ih);
            let (dw, dh) = (iw * k, ih * k);
            dl.add_image(*tex, [(w - dw) / 2.0, (h - dh) / 2.0], [(w + dw) / 2.0, (h + dh) / 2.0])
                .col(hudhook::imgui::ImColor32::from_rgba(255, 255, 255, a))
                .build();
            dl.add_rect([0.0, 0.0], [w, h], hudhook::imgui::ImColor32::from_rgba(0, 0, 0, (alpha * 0.45 * 255.0) as u8)).filled(true).build();
        }
    }

    /// Pantalla del arranque: el logo del mod en grande y la barra de carga (rogue_ui::titulo::carga).
    fn draw_arranque(&self, ui: &Ui) {
        let tex = rogue_ui::adorno::MenuTex::from_slots(self.menu_tex);
        let t = self.start.elapsed().as_secs_f32();
        rogue_ui::titulo::carga(ui, &tex, t, progreso_falso(t, 10.0));
    }

    /// Barra de carga de las pantallas del mod, con las piezas de las barras del juego, y el icono de
    /// carga del juego en la esquina. No mide nada: avanza sola (`progreso_falso`).
    fn barra_carga(&self, ui: &Ui, dl: &hudhook::imgui::DrawListMut, p: f32, alpha: f32) {
        let [w, h] = ui.io().display_size;
        let s = h / 1080.0;
        let tex = rogue_ui::adorno::MenuTex::from_slots(self.menu_tex);
        let bw = w * 0.3;
        rogue_ui::adorno::barra(dl, &tex, (w - bw) / 2.0, h * 0.86, bw, 6.0 * s, p, p, [196, 164, 104], alpha, s);
        if let Some((t, [tw, th])) = tex.carga_icono {
            let ih = 110.0 * s;
            let iw = ih * tw / th;
            let (x1, y1) = (w - 60.0 * s, h - 50.0 * s);
            dl.add_image(t, [x1 - iw, y1 - ih], [x1, y1]).col(rogue_ui::adorno::white(alpha)).build();
        }
    }
}

/// Progreso de una barra de carga que no mide nada: sube deprisa al principio y se frena antes de
/// llegar (como mucho, al 95 %), con `tau` segundos de escala.
fn progreso_falso(t: f32, tau: f32) -> f32 {
    (0.95 * (1.0 - (-t / tau.max(0.1)).exp())).clamp(0.0, 0.95)
}

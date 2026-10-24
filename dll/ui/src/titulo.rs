//! Menú principal. No imita la pantalla de título del juego: el logo del mod en pequeño arriba a la
//! izquierda, las opciones debajo y, ocupando el resto, la run en curso (su tramo de mapa con el hilo
//! rojo hasta donde la dejaste, el personaje con su arma y lo que lleva). Sin run, el texto de
//! bienvenida.

use imgui::{ImColor32, TextureId, Ui};
use rogue_core::idioma::{t, tf};
use rogue_core::map::NodeType;

use crate::Fonts;
use crate::adorno;
use crate::estilo::{self, APAGADA, LINEA, SANGRE, TENUE, TINTA, c};
use crate::mapa::{MapTextures, MapView, NodeState};

/// La run guardada, para el panel de la derecha.
pub struct RunResumen {
    pub personaje: String,
    pub arma: Option<TextureId>,
    pub arma_nombre: String,
    pub semilla: String,
    /// El mapa del acto (nodos, caminos y lo recorrido).
    pub mapa: MapView,
    /// Tramo del mapa del juego de su acto (fondos/actoN).
    pub fondo: Option<(TextureId, [f32; 2])>,
    /// (icono, maldita) de lo que lleva.
    pub bendiciones: Vec<(Option<TextureId>, bool)>,
}

pub struct TituloView {
    /// (texto, detalle, apagada) de cada opción.
    pub opciones: Vec<(String, String, bool)>,
    pub selected: usize,
    /// Cifras del perfil o el texto de bienvenida.
    pub texto: String,
    pub run: Option<RunResumen>,
    /// Tramo del mapa del primer acto (el panel sin run).
    pub fondo_inicio: Option<(TextureId, [f32; 2])>,
    pub t: f32,
    pub t_sel: f32,
}

const TINTA_MAPA: [u8; 3] = [42, 31, 18];
const HILO: [u8; 3] = [168, 50, 42];

pub fn draw(ui: &Ui, v: &TituloView, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(v.t, 0);
    estilo::fondo(&dl, &lz, 1.0);

    // Logo pequeño.
    if let Some((l, [lw, lh])) = tex.menu.logo {
        let w = lz.d(470.0);
        let h = w * lh / lw;
        dl.add_image(l, lz.p(70.0, 40.0), [lz.x(70.0) + w, lz.y(40.0) + h]).col(adorno::white(a0)).build();
    }
    // Opciones: solo la elegida enseña su detalle.
    let x = lz.x(96.0);
    let ancho = lz.d(560.0);
    let mut y = lz.y(360.0);
    let k_sel = adorno::ease_out(v.t_sel / 0.18);
    for (i, (texto, detalle, apagada)) in v.opciones.iter().enumerate() {
        let a = adorno::appear(v.t, i + 1);
        let sel = v.selected == i;
        let alto = estilo::opcion(ui, &dl, fonts, &tex.menu, &lz, x, y, ancho, texto, if sel { detalle } else { "" }, sel, *apagada, k_sel, a);
        y += alto + lz.d(22.0);
    }

    // Panel de la derecha.
    let px = lz.x(720.0);
    let pw = lz.derecha() - lz.d(75.0) - px;
    let py = lz.y(84.0);
    let ph = lz.d(880.0);
    dl.add_rect([px, py], [px + pw, py + ph], c([19, 17, 13], a0)).filled(true).build();
    dl.add_line([px, py], [px + pw, py], c([58, 50, 38], a0)).thickness(lz.d(1.5)).build();
    match &v.run {
        Some(r) => panel_run(ui, &dl, r, tex, fonts, &lz, [px, py, pw, ph], a0, v.t),
        None => {
            let mh = lz.d(500.0);
            if let Some(f) = v.fondo_inicio {
                tramo(&dl, f, [px, py, px + pw, py + mh], a0);
            }
            estilo::texto(ui, &dl, fonts.title, [px + lz.d(36.0), py + lz.d(26.0)], c(TINTA_MAPA, a0), t("No run in progress"));
            let bienvenida = t("A boss rush across the Lands Between. Pick a character, cross the map, fight what waits.");
            estilo::parrafo(ui, &dl, fonts.texto, [px + lz.d(48.0), py + mh + lz.d(48.0)], pw - lz.d(96.0), lz.d(42.0), c(TINTA, a0), bienvenida, 4);
        }
    }
    // Cifras abajo a la izquierda (sin runs todavía, la bienvenida ya lo dice todo).
    if v.texto != t("A boss rush across the Lands Between. Pick a character, cross the map, fight what waits.") {
        estilo::parrafo(ui, &dl, fonts.nota, [x + lz.d(24.0), lz.y(990.0)], lz.d(560.0), lz.d(30.0), c(TENUE, a0), &v.texto, 2);
    }
    estilo::ayuda(ui, &dl, fonts, &lz, &[("A", "Choose"), ("X", "Compendium")], a0);
}

#[allow(clippy::too_many_arguments)]
fn panel_run(ui: &Ui, dl: &imgui::DrawListMut, r: &RunResumen, tex: &MapTextures, fonts: &Fonts, lz: &estilo::Lienzo, caja: [f32; 4], a: f32, tiempo: f32) {
    let [px, py, pw, _] = caja;
    // Tramo de mapa: el fondo del acto lavado y, encima, unas filas del mapa en horizontal.
    let mh = lz.d(500.0);
    if let Some(f) = r.fondo {
        tramo(dl, f, [px, py, px + pw, py + mh], a);
    }
    let m = &r.mapa;
    estilo::texto(ui, dl, fonts.title, [px + lz.d(36.0), py + lz.d(26.0)], c(TINTA_MAPA, a), &m.act_name);
    estilo::texto(ui, dl, fonts.nota, [px + lz.d(36.0), py + lz.d(80.0)], c(TINTA_MAPA, 0.8 * a), &tf("Act {}. Seed {}.", &[&m.act_num, &r.semilla]));
    // El trayecto en línea: lo recorrido con el hilo rojo (los últimos nodos), dónde lo dejaste con
    // el cursor del mapa, los caminos que salen de ahí y, al fondo, el señor del acto.
    let blanco = |al: f32| ImColor32::from_rgba(255, 255, 255, (al.clamp(0.0, 1.0) * 255.0) as u8);
    let tipo_de = |p: (i32, i32)| m.nodes.iter().find(|n| (n.col, n.row) == p).map(|n| n.kind).unwrap_or(NodeType::Ancient);
    let r_nodo = lz.d(26.0);
    let (x0, x1) = (px + lz.d(90.0), px + pw - lz.d(150.0));
    let ym = py + lz.d(300.0);
    let recorrido: Vec<(i32, i32)> = m.path.iter().rev().take(5).rev().copied().collect();
    let xa = x0 + (x1 - x0) * 0.55;
    let paso = if recorrido.len() > 1 { (xa - x0) / (recorrido.len() - 1) as f32 } else { 0.0 };
    let pos_rec: Vec<[f32; 2]> = (0..recorrido.len()).map(|i| [x0 + paso * i as f32, ym]).collect();
    // Si se cortó el principio, unos puntos lo dicen.
    if m.path.len() > recorrido.len() {
        for k in 1..=3 {
            dl.add_circle([x0 - lz.d(16.0) * k as f32, ym], lz.d(2.5), c(TINTA_MAPA, 0.6 * a)).filled(true).build();
        }
    }
    let siguientes: Vec<(i32, i32)> = m.nodes.iter().filter(|n| n.state == NodeState::Available).map(|n| (n.col, n.row)).collect();
    let actual = pos_rec.last().copied().unwrap_or([x0, ym]);
    let xs = xa + (x1 - xa) * 0.42;
    let sep = lz.d(92.0);
    let pos_sig: Vec<[f32; 2]> = (0..siguientes.len()).map(|i| [xs, ym + (i as f32 - (siguientes.len() as f32 - 1.0) / 2.0) * sep]).collect();
    for q in &pos_sig {
        crate::mapa::discontinua(dl, actual, *q, r_nodo + lz.d(6.0), r_nodo + lz.d(6.0), c([58, 46, 30], 0.75 * a), lz.d(2.4), lz.d(9.0), lz.d(7.0));
    }
    // Hacia el señor del acto: más camino por delante, sin detallar.
    let jefe = [x1 + lz.d(40.0), ym];
    let desde = pos_sig.first().map(|q| [q[0], ym]).unwrap_or(actual);
    crate::mapa::discontinua(dl, desde, jefe, r_nodo + lz.d(40.0), lz.d(70.0), c([58, 46, 30], 0.4 * a), lz.d(2.0), lz.d(4.0), lz.d(12.0));
    for w in pos_rec.windows(2) {
        dl.add_line(w[0], w[1], c(HILO, a)).thickness(lz.d(5.0)).build();
    }
    for (p, q) in recorrido.iter().zip(&pos_rec) {
        if let Some(t) = icono(tex, tipo_de(*p)) {
            dl.add_image(t, [q[0] - r_nodo, q[1] - r_nodo], [q[0] + r_nodo, q[1] + r_nodo]).col(blanco(0.85 * a)).build();
        }
    }
    crate::mapa::cursor_mapa(dl, tex, actual, r_nodo * 1.9, tiempo, a);
    for (p, q) in siguientes.iter().zip(&pos_sig) {
        if let Some(t) = icono(tex, tipo_de(*p)) {
            dl.add_image(t, [q[0] - r_nodo, q[1] - r_nodo], [q[0] + r_nodo, q[1] + r_nodo]).col(blanco(a)).build();
        }
    }
    if let Some(t) = icono(tex, NodeType::Boss) {
        let rb = lz.d(56.0);
        dl.add_image(t, [jefe[0] - rb, jefe[1] - rb], [jefe[0] + rb, jefe[1] + rb]).col(blanco(a)).build();
    }
    let nombre = estilo::ajustar(ui, fonts.title, &m.boss_name, pw - lz.d(80.0));
    estilo::texto_der(ui, dl, fonts.nota, [px + pw - lz.d(36.0), ym + lz.d(76.0)], c(TINTA_MAPA, 0.85 * a), t("Lord of the act"));
    estilo::texto_der(ui, dl, fonts.title, [px + pw - lz.d(36.0), ym + lz.d(104.0)], c(TINTA_MAPA, a), &nombre);

    // Personaje con su arma.
    let mut y = py + mh + lz.d(48.0);
    let x = px + lz.d(48.0);
    let marco = lz.d(110.0);
    estilo::marco_icono(dl, &tex.menu, r.arma, x, y, marco, 1.0, a);
    let tx = x + estilo::ancho_marco(marco) + lz.d(30.0);
    estilo::texto(ui, dl, fonts.grande, [tx, y + lz.d(4.0)], c(TINTA, a), &r.personaje);
    estilo::texto(ui, dl, fonts.texto, [tx, y + lz.d(64.0)], c(APAGADA, a), &r.arma_nombre);
    y += marco + lz.d(40.0);
    dl.add_line([x, y - lz.d(16.0)], [px + pw - lz.d(48.0), y - lz.d(16.0)], c(LINEA, a)).thickness(lz.d(1.5)).build();
    // Lo que lleva.
    let malditas = r.bendiciones.iter().filter(|b| b.1).count();
    let buenas = r.bendiciones.len() - malditas;
    let linea = match (buenas, malditas) {
        (0, 0) => t("No blessings yet.").to_string(),
        (b, 0) => tf("Carries {} blessings", &[&b]),
        (b, 1) => tf("Carries {} blessings and 1 curse", &[&b]),
        (b, m) => tf("Carries {} blessings and {} curses", &[&b, &m]),
    };
    estilo::texto(ui, dl, fonts.texto, [x, y], c(TINTA, a), &linea);
    y += lz.d(54.0);
    let lado = lz.d(76.0);
    let caben = ((pw - lz.d(96.0)) / (lado + lz.d(8.0))) as usize;
    for (i, (icon, maldita)) in r.bendiciones.iter().take(caben).enumerate() {
        let cx = x + i as f32 * (lado + lz.d(8.0));
        estilo::casilla(dl, &tex.menu, *icon, cx, y, lado, adorno::white(a), a);
        if *maldita {
            dl.add_line([cx + lado * 0.18, y + lado * 0.82], [cx + lado * 0.82, y + lado * 0.82], c(SANGRE, a)).thickness(lz.d(2.5)).build();
        }
    }
}

/// El tramo del mapa del juego recortado al rectángulo y lavado (como en el mapa).
fn tramo(dl: &imgui::DrawListMut, f: (TextureId, [f32; 2]), rect: [f32; 4], a: f32) {
    let (f, [fw, fh]) = f;
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    let k = (w / fw).max(h / fh);
    let (u0, v0) = ((1.0 - w / (fw * k)) / 2.0, (1.0 - h / (fh * k)) / 2.0);
    dl.add_image(f, [rect[0], rect[1]], [rect[2], rect[3]]).uv_min([u0, v0]).uv_max([1.0 - u0, 1.0 - v0]).col(adorno::white(a)).build();
    dl.add_rect([rect[0], rect[1]], [rect[2], rect[3]], c([236, 226, 200], 0.58 * a)).filled(true).build();
}

fn icono(tex: &MapTextures, k: NodeType) -> Option<TextureId> {
    let i = match k {
        NodeType::Monster => 0,
        NodeType::Elite => 1,
        NodeType::Rest => 2,
        NodeType::Shop => 3,
        NodeType::Treasure => 4,
        NodeType::Unknown => 5,
        NodeType::Boss => 6,
        NodeType::Ancient => 7,
        _ => return None,
    };
    tex.nodes[i]
}

/// Pantalla de carga del arranque: el logo en grande sobre negro y, debajo, la barra de carga con las
/// piezas de las barras del juego (`progreso` 0..1).
pub fn carga(ui: &Ui, tex: &crate::adorno::MenuTex, tiempo: f32, progreso: f32) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_foreground_draw_list();
    dl.add_rect([0.0, 0.0], [lz.w, lz.h], c([0, 0, 0], 1.0)).filled(true).build();
    let a = adorno::ease_out(tiempo / 1.2);
    if let Some((l, [lw, lh])) = tex.logo {
        let w = lz.d(1230.0);
        let h = w * lh / lw;
        let x = (lz.w - w) / 2.0;
        dl.add_image(l, [x, lz.y(70.0)], [x + w, lz.y(70.0) + h]).col(adorno::white(a)).build();
    }
    let bw = lz.d(620.0);
    adorno::barra(&dl, tex, (lz.w - bw) / 2.0, lz.y(900.0), bw, lz.d(6.0), progreso, progreso, [196, 164, 104], a, lz.s);
}

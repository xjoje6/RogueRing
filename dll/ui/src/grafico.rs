//! Gráfico del final de la run (idea del End Run Graph de Slay the Spire 2, inspirado en el de
//! Jorbs): una línea por los nodos recorridos con lo cerca que estuviste de morir en cada combate,
//! el icono de cada nodo, lo que conseguiste en él y tu arma avanzando mientras se dibuja. Con la
//! cruceta se elige un nodo y abajo sale su detalle: rival, duración y de dónde salió el daño.

use imgui::{ImColor32, TextureId, Ui};
use rogue_core::idioma::{t, tf};

use crate::Fonts;

#[derive(Clone, Debug)]
pub struct Detalle {
    pub enemigo: String,
    /// "Combate", "Élite", "Señor del acto".
    pub tipo: String,
    pub segundos: f64,
    pub recibido: f64,
    pub curado: f64,
    /// (fuente, daño), de mayor a menor.
    pub danio: Vec<(String, f64)>,
}

pub struct GraphNode {
    pub acto: usize,
    pub icono: Option<TextureId>,
    /// Vida mínima (0..1) en el nodo; 1 si no hubo combate.
    pub vida_min: f64,
    pub combate: bool,
    pub bendiciones: Vec<(Option<TextureId>, bool)>,
    pub objetos: Vec<Option<TextureId>>,
    pub detalle: Option<Detalle>,
}

pub struct RunGraph {
    pub nodes: Vec<GraphNode>,
    /// Icono que avanza por la línea (el arma del personaje).
    pub marcador: Option<TextureId>,
    pub sel: usize,
    /// Segundos desde que se abrió la pantalla.
    pub t: f32,
    pub ganada: bool,
}

const INK: [u8; 3] = [226, 218, 198];
const INK_DIM: [u8; 3] = [150, 144, 130];
const GOLD: [u8; 3] = [214, 186, 120];
const BLOOD: [u8; 3] = [196, 96, 78];
const PAPER: [u8; 3] = [18, 16, 15];
/// Colores de las barras de daño (tonos apagados, del dorado al rojo).
const BARRAS: [[u8; 3]; 6] = [[214, 186, 120], [176, 150, 104], [196, 96, 78], [138, 124, 150], [112, 140, 128], [150, 144, 130]];

fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Nodos por segundo al dibujar la línea.
const VELOCIDAD: f32 = 3.5;

/// Dibuja el gráfico en `area` ([x0, y0, x1, y1]) y el detalle del nodo elegido debajo, hasta `bottom`.
pub fn draw(ui: &Ui, dl: &imgui::DrawListMut, g: &RunGraph, area: [f32; 4], bottom: f32, fonts: &Fonts) {
    let [_, h] = ui.io().display_size;
    let s = h / 1080.0;
    let n = g.nodes.len();
    if n == 0 {
        return;
    }
    let [x0, y0, x1, y1] = area;
    // Bandas: bendiciones arriba, gráfico en medio, iconos de nodo y objetos abajo.
    let (gy0, gy1) = (y0 + 70.0 * s, y1 - 80.0 * s);
    let step = if n > 1 { (x1 - x0) / (n - 1) as f32 } else { 0.0 };
    let px = |i: usize| x0 + step * i as f32;
    let py = |v: f64| gy1 - (gy1 - gy0) * v.clamp(0.0, 1.0) as f32;

    // Rejilla: 0, 50 y 100 %.
    for (v, txt) in [(0.0, "0 %"), (0.5, "50 %"), (1.0, "100 %")] {
        let y = py(v);
        dl.add_line([x0, y], [x1, y], c(INK_DIM, 0.18)).thickness(1.0).build();
        let _f = ui.push_font(fonts.nota);
        let w = ui.calc_text_size(txt)[0];
        dl.add_text([x0 - w - 14.0 * s, y - 9.0 * s], c(INK_DIM, 0.7), txt);
    }
    // Separación de actos.
    for i in 1..n {
        if g.nodes[i].acto != g.nodes[i - 1].acto {
            let x = (px(i) + px(i - 1)) / 2.0;
            let mut y = gy0 - 20.0 * s;
            while y < gy1 + 10.0 * s {
                dl.add_line([x, y], [x, y + 6.0 * s], c(INK_DIM, 0.4)).thickness(1.0).build();
                y += 12.0 * s;
            }
            let _f = ui.push_font(fonts.nota);
            dl.add_text([x + 8.0 * s, gy0 - 24.0 * s], c(INK_DIM, 0.8), tf("ACT {}", &[&["", "I", "II", "III"].get(g.nodes[i].acto).unwrap_or(&"?")]));
        }
    }
    // Hasta dónde se ha dibujado (en nodos, con decimales).
    let reveal = (g.t * VELOCIDAD).min((n - 1) as f32);
    let head = reveal.floor() as usize;
    let frac = reveal - head as f32;
    // Línea.
    for i in 0..head.min(n - 1) {
        let (a, b) = ([px(i), py(g.nodes[i].vida_min)], [px(i + 1), py(g.nodes[i + 1].vida_min)]);
        dl.add_line(a, b, c(GOLD, 0.95)).thickness(2.5 * s).build();
    }
    let head_pos = if head + 1 < n {
        let (a, b) = ([px(head), py(g.nodes[head].vida_min)], [px(head + 1), py(g.nodes[head + 1].vida_min)]);
        let p = [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac];
        dl.add_line(a, p, c(GOLD, 0.95)).thickness(2.5 * s).build();
        p
    } else {
        [px(n - 1), py(g.nodes[n - 1].vida_min)]
    };
    // Nodos ya alcanzados: punto, icono del nodo, bendiciones y objetos (aparecen al pasar la cabeza).
    for (i, nd) in g.nodes.iter().enumerate() {
        let k = ((reveal - i as f32) * 3.0 + 1.0).clamp(0.0, 1.0);
        if k <= 0.0 {
            continue;
        }
        let x = px(i);
        let y = py(nd.vida_min);
        let muerte = nd.combate && nd.vida_min <= 0.0;
        let col = if muerte || nd.vida_min < 0.3 { BLOOD } else { GOLD };
        if nd.combate {
            dl.add_circle([x, y], 5.0 * s, c(col, k)).filled(true).num_segments(16).build();
        } else {
            dl.add_circle([x, y], 3.5 * s, c(INK_DIM, k)).filled(true).num_segments(12).build();
        }
        if muerte {
            let r = 9.0 * s;
            dl.add_line([x - r, y - r], [x + r, y + r], c(BLOOD, k)).thickness(2.5 * s).build();
            dl.add_line([x - r, y + r], [x + r, y - r], c(BLOOD, k)).thickness(2.5 * s).build();
        }
        // Icono del nodo bajo el eje.
        let iy = gy1 + 22.0 * s;
        if let Some(t) = nd.icono {
            let r = 13.0 * s;
            dl.add_image(t, [x - r, iy - r], [x + r, iy + r]).col(ImColor32::from_rgba(255, 255, 255, (255.0 * k) as u8)).build();
        }
        // Objetos debajo del icono.
        for (j, o) in nd.objetos.iter().enumerate() {
            if let Some(t) = o {
                let r = 11.0 * s;
                let oy = iy + 30.0 * s + j as f32 * 24.0 * s;
                dl.add_image(*t, [x - r, oy - r], [x + r, oy + r]).col(ImColor32::from_rgba(255, 255, 255, (255.0 * k) as u8)).build();
            }
        }
        // Bendiciones encima del gráfico, apiladas hacia arriba.
        for (j, (t, maldita)) in nd.bendiciones.iter().enumerate().take(3) {
            let r = 11.0 * s;
            let by = gy0 - 36.0 * s - j as f32 * 24.0 * s;
            if let Some(t) = t {
                dl.add_image(*t, [x - r, by - r], [x + r, by + r]).col(ImColor32::from_rgba(255, 255, 255, (255.0 * k) as u8)).build();
            }
            if *maldita {
                dl.add_line([x - r, by + r], [x + r, by + r], c(BLOOD, k)).thickness(1.5 * s).build();
            }
        }
        if nd.bendiciones.len() > 3 {
            let _f = ui.push_font(fonts.nota);
            dl.add_text([x - 8.0 * s, gy0 - 36.0 * s - 3.0 * 24.0 * s - 10.0 * s], c(INK_DIM, k), format!("+{}", nd.bendiciones.len() - 3));
        }
    }
    // Cabeza: el arma avanzando por la línea.
    if let Some(t) = g.marcador {
        let r = 16.0 * s;
        // Al terminar de dibujarse, el arma se aparta un poco para no tapar el último punto.
        let head_pos = if reveal >= (n - 1) as f32 { [head_pos[0], head_pos[1] - 34.0 * s] } else { head_pos };
        dl.add_circle(head_pos, r * 1.1, c(PAPER, 0.9)).filled(true).num_segments(24).build();
        dl.add_image(t, [head_pos[0] - r, head_pos[1] - r], [head_pos[0] + r, head_pos[1] + r]).build();
    }
    // Nodo elegido: línea vertical y detalle.
    let sel = g.sel.min(n - 1);
    if reveal >= (n - 1) as f32 {
        let x = px(sel);
        dl.add_line([x, gy0 - 10.0 * s], [x, gy1 + 8.0 * s], c(GOLD, 0.35)).thickness(1.5 * s).build();
        dl.add_circle([x, py(g.nodes[sel].vida_min)], 9.0 * s, c(GOLD, 1.0)).thickness(2.0 * s).num_segments(20).build();
    }
    draw_detalle(ui, dl, g.nodes.get(sel), [x0, y1 + 10.0 * s, x1, bottom], s, fonts);
}

fn draw_detalle(ui: &Ui, dl: &imgui::DrawListMut, nd: Option<&GraphNode>, area: [f32; 4], s: f32, fonts: &Fonts) {
    let Some(nd) = nd else { return };
    let [x0, y0, x1, _] = area;
    let Some(d) = &nd.detalle else {
        let _f = ui.push_font(fonts.nota);
        dl.add_text([x0, y0], c(INK_DIM, 1.0), t("No fight on this node."));
        return;
    };
    {
        let _f = ui.push_font(fonts.nota);
        dl.add_text([x0, y0], c(if nd.vida_min <= 0.0 { BLOOD } else { GOLD }, 1.0), d.tipo.to_uppercase());
    }
    {
        let _f = ui.push_font(fonts.texto);
        dl.add_text([x0, y0 + 30.0 * s], c(INK, 1.0), &d.enemigo);
    }
    {
        let _f = ui.push_font(fonts.nota);
        let resumen = tf(
            "{} s   ·   taken {}   ·   healed {}   ·   lowest HP {}%",
            &[&d.segundos.round(), &d.recibido.round(), &d.curado.max(0.0).round(), &(nd.vida_min * 100.0).round()],
        );
        dl.add_text([x0, y0 + 74.0 * s], c(INK_DIM, 1.0), &resumen);
    }
    // Barras de daño por fuente (las cinco mayores).
    let total: f64 = d.danio.iter().map(|x| x.1).sum::<f64>().max(1.0);
    let bx = x0 + (x1 - x0) * 0.45;
    let bw = (x1 - bx) * 0.62;
    let mut y = y0;
    let _f = ui.push_font(fonts.nota);
    for (i, (nombre, v)) in d.danio.iter().take(5).enumerate() {
        let f = (*v / total) as f32;
        let col = BARRAS[i % BARRAS.len()];
        dl.add_text([bx, y], c(INK, 1.0), nombre);
        let bar_x = bx + 230.0 * s;
        dl.add_rect([bar_x, y + 9.0 * s], [bar_x + bw, y + 19.0 * s], c(PAPER, 0.9)).filled(true).build();
        dl.add_rect([bar_x, y + 9.0 * s], [bar_x + bw * f, y + 19.0 * s], c(col, 1.0)).filled(true).build();
        dl.add_text([bar_x + bw + 10.0 * s, y], c(INK_DIM, 1.0), format!("{:.0} %", f * 100.0));
        y += 32.0 * s;
    }
}

/// Resumen compacto del combate recién ganado (pantalla de recompensa): duración y una barra con el
/// reparto del daño por fuente, con las cuatro mayores rotuladas. Crece al abrir la pantalla.
pub fn draw_resumen_combate(ui: &Ui, segundos: f64, danio: &[(String, f64)], t: f32, fonts: &Fonts) {
    let [_, h] = ui.io().display_size;
    let s = h / 1080.0;
    let dl = ui.get_foreground_draw_list();
    let total: f64 = danio.iter().map(|x| x.1).sum::<f64>();
    if total <= 0.0 {
        return;
    }
    let (x0, y0) = (56.0 * s, h - 150.0 * s);
    let bw = 520.0 * s;
    let k = crate::adorno::ease_out((t - 0.3) / 0.8);
    {
        let _f = ui.push_font(fonts.caption);
        dl.add_text([x0, y0 - 24.0 * s], c(INK_DIM, k), tf("THIS FIGHT  ·  {} s", &[&segundos.round()]));
    }
    dl.add_rect([x0, y0], [x0 + bw, y0 + 10.0 * s], c(PAPER, 0.9 * k)).filled(true).build();
    let mut x = x0;
    for (i, (_, v)) in danio.iter().enumerate() {
        let w = bw * (*v / total) as f32 * k;
        dl.add_rect([x, y0], [x + w, y0 + 10.0 * s], c(BARRAS[i.min(BARRAS.len() - 1)], k)).filled(true).build();
        x += w;
    }
    let _f = ui.push_font(fonts.caption);
    let mut lx = x0;
    for (i, (nombre, v)) in danio.iter().take(4).enumerate() {
        let txt = format!("{nombre} {:.0} %", *v / total * 100.0);
        dl.add_rect([lx, y0 + 22.0 * s], [lx + 9.0 * s, y0 + 31.0 * s], c(BARRAS[i.min(BARRAS.len() - 1)], k)).filled(true).build();
        dl.add_text([lx + 14.0 * s, y0 + 17.0 * s], c(INK_DIM, k), &txt);
        lx += ui.calc_text_size(&txt)[0] + 34.0 * s;
    }
}

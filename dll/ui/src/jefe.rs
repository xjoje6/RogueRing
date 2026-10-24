//! Barra de vida del boss en el coliseo (allí el juego no pone la suya). Imita la nativa de Elden
//! Ring: abajo y centrada, nombre encima a la izquierda, barra fina con marco y un tramo de daño
//! reciente que se vacía con retraso.

use imgui::{ImColor32, TextureId, Ui};

use crate::Fonts;

pub struct BossBar {
    pub name: String,
    pub hp: i32,
    pub max: i32,
    /// Vida que marca el tramo de daño reciente (>= hp). La anima quien llama.
    pub trail: f32,
    /// Daño del último golpe o combo, como el número que pone el juego a la derecha.
    pub hit: i32,
    /// Opacidad (aparece con fundido al empezar el combate).
    pub alpha: f32,
    /// Piezas de las barras del juego (fondo, relleno, filetes, tope y remates).
    pub tex: crate::adorno::MenuTex,
    /// Mutadores del boss (aumentos de caos del rival), junto al nombre.
    pub muts: String,
    /// Bajo la barra, como en Nightreign: estados y efectos que lleva, con su icono.
    pub estados: Vec<EstadoVista>,
    /// Umbrales de tus bendiciones en la barra (fracción de su vida; `true` = remate, en rojo).
    pub marcas: Vec<(f32, bool)>,
    /// Las barras las pone el juego: (cuántas, cuál es la del rival). No se dibujan ni la barra, ni el
    /// nombre, ni el número de daño; los umbrales van sobre la del rival, los estados bajo la de abajo y
    /// lo que sigue al nombre (« · oleada 1 de 3», el tiempo...) en el centro, encima de la de arriba.
    pub nativa: Option<(usize, usize)>,
}

/// Un estado o efecto del rival bajo su barra.
pub struct EstadoVista {
    pub icono: Option<TextureId>,
    pub nombre: String,
    pub color: [u8; 3],
    /// Barrita de acumulación o de tiempo (0..1).
    pub lleno: f32,
    /// Segundos desde que saltó (destello al saltar).
    pub destello: f32,
    /// Pilas o segundos, junto al nombre.
    pub texto: String,
}

const INK: [u8; 3] = [226, 218, 198];
/// Tinte del relleno (aclarado, ~230 de gris) para dar el rojo de las barras de vida del juego,
/// medido en una captura: (110, 36, 31).
const RED: [u8; 3] = [112, 37, 32];
const MUT: [u8; 3] = [196, 96, 78];
const GOLD: [u8; 3] = [214, 186, 120];

fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn draw(ui: &Ui, v: &BossBar, fonts: &Fonts) {
    let [w, h] = ui.io().display_size;
    let s = h / 1080.0;
    let dl = ui.get_foreground_draw_list();
    // La del juego, medida en una captura a 1280x720: relleno de 994 de ancho y 10,5 de alto, con su
    // borde superior 213 por encima del borde de abajo de la pantalla (en una de 1080 de alto).
    // Con varias, el juego las apila hacia arriba desde esa, 54,5 entre una y otra; la primera, arriba.
    let (bw, bh, x0, y0) = if v.nativa.is_some() {
        (994.0 * s, 10.5 * s, w / 2.0 - 494.0 * s, h - 213.0 * s)
    } else {
        let bw = (w * 0.56).min(1100.0 * s);
        (bw, 9.0 * s, (w - bw) / 2.0, h - 132.0 * s)
    };
    let max = v.max.max(1) as f32;
    let f = (v.hp.max(0) as f32 / max).clamp(0.0, 1.0);
    let ft = (v.trail / max).clamp(f, 1.0);

    let a = v.alpha.clamp(0.0, 1.0);
    // Entra subiendo un poco mientras aparece.
    let y0 = y0 + (1.0 - a) * 12.0 * s;
    let fila = |k: usize| match v.nativa {
        Some((n, _)) => y0 - (n.saturating_sub(1) - k.min(n.saturating_sub(1))) as f32 * 54.5 * s,
        None => y0,
    };
    if v.nativa.is_some() {
        let y0 = fila(0);
        let info = v.name.split_once(" · ").map(|x| x.1).unwrap_or("");
        let _f = ui.push_font(fonts.caption);
        let iw = ui.calc_text_size(info)[0];
        let mw = if v.muts.is_empty() { 0.0 } else { ui.calc_text_size(&v.muts)[0] + if iw > 0.0 { 18.0 * s } else { 0.0 } };
        let x = x0 + (bw - iw - mw) / 2.0;
        dl.add_text([x, y0 - 26.0 * s], c(INK, a), info);
        if !v.muts.is_empty() {
            dl.add_text([x + iw + mw - ui.calc_text_size(&v.muts)[0], y0 - 26.0 * s], c(MUT, a), &v.muts);
        }
    } else {
        let _f = ui.push_font(fonts.body);
        dl.add_text([x0, y0 - 30.0 * s], c(INK, a), &v.name);
        if !v.muts.is_empty() {
            let nw = ui.calc_text_size(&v.name)[0];
            let _f = ui.push_font(fonts.caption);
            dl.add_text([x0 + nw + 18.0 * s, y0 - 26.0 * s], c(MUT, a), &v.muts);
        }
        if v.hit > 0 {
            let t = v.hit.to_string();
            let sz = ui.calc_text_size(&t);
            dl.add_text([x0 + bw - sz[0], y0 - 30.0 * s], c(INK, 0.9 * a), &t);
        }
    }
    // La barra con las piezas del juego (adorno::barra).
    if v.nativa.is_none() {
        crate::adorno::barra(&dl, &v.tex, x0, y0, bw, bh, f, ft, RED, a, s);
    }
    // Umbrales de las bendiciones: una raya fina que cruza la barra (la del rival).
    let y_rival = fila(v.nativa.map(|x| x.1).unwrap_or(0));
    for &(u, remate) in &v.marcas {
        let y0 = y_rival;
        let x = x0 + bw * u.clamp(0.0, 1.0);
        dl.add_line([x, y0 - 4.0 * s], [x, y0 + bh + 4.0 * s], c(if remate { MUT } else { GOLD }, 0.95 * a)).thickness(2.0 * s).build();
    }
    // Estados y efectos, en fila bajo la barra: icono, nombre (y pilas) y una barrita de su color.
    let (icono, gap) = (30.0 * s, 26.0 * s);
    let y = y0 + bh + 14.0 * s;
    let mut x = x0;
    for e in &v.estados {
        let _f = ui.push_font(fonts.caption);
        let etiqueta = if e.texto.is_empty() { e.nombre.clone() } else { format!("{} {}", e.nombre, e.texto) };
        let tw = ui.calc_text_size(&etiqueta)[0].max(70.0 * s);
        let ancho = icono + 8.0 * s + tw;
        if x + ancho > x0 + bw {
            break;
        }
        // Al saltar, destella medio segundo largo.
        let brillo = if e.destello < 0.8 { 1.0 - e.destello / 0.8 } else { 0.0 };
        if brillo > 0.0 {
            dl.add_rect([x - 3.0 * s, y - 3.0 * s], [x + icono + 3.0 * s, y + icono + 3.0 * s], c(e.color, 0.6 * brillo * a)).filled(true).build();
        }
        match e.icono {
            Some(t) => dl.add_image(t, [x, y], [x + icono, y + icono]).col(ImColor32::from_rgba(255, 255, 255, (a * 255.0) as u8)).build(),
            None => dl.add_rect([x + 6.0 * s, y + 6.0 * s], [x + icono - 6.0 * s, y + icono - 6.0 * s], c(e.color, a)).filled(true).build(),
        }
        let tx = x + icono + 8.0 * s;
        dl.add_text([tx, y - 1.0 * s], c(if brillo > 0.0 { e.color } else { INK }, a), &etiqueta);
        let my = y + icono - 6.0 * s;
        dl.add_rect([tx, my], [tx + tw, my + 4.0 * s], c([30, 28, 26], 0.9 * a)).filled(true).build();
        dl.add_rect([tx, my], [tx + tw * e.lleno.clamp(0.0, 1.0), my + 4.0 * s], c(e.color, a)).filled(true).build();
        x += ancho + gap;
    }
}

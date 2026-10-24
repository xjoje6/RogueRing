//! Lenguaje común de los menús (lavado de cara del 19-10, maquetas del canvas «RogueRing: menús
//! nuevos»): fondo casi negro liso, la tienda del juego como guía (lista con el marco de objeto y la
//! barra de selección; ficha con el marco octogonal y el filete) y la ayuda de botones abajo a la
//! derecha. Coordenadas de diseño en 1920x1080, dentro de una caja 16:9 centrada.

use imgui::{FontId, ImColor32, TextureId, Ui};

use crate::adorno::{self, MenuTex};
use crate::cartas::wrap;
use crate::Fonts;

pub const FONDO: [u8; 3] = [12, 11, 9];
pub const TINTA: [u8; 3] = [227, 215, 185];
/// Lo elegido.
pub const CLARA: [u8; 3] = [242, 231, 201];
pub const APAGADA: [u8; 3] = [184, 172, 142];
pub const TENUE: [u8; 3] = [142, 131, 107];
pub const MUY_TENUE: [u8; 3] = [111, 101, 83];
pub const ORO: [u8; 3] = [217, 196, 142];
pub const SANGRE: [u8; 3] = [192, 87, 74];
/// Líneas finas de separación.
pub const LINEA: [u8; 3] = [42, 37, 28];

pub fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Caja de diseño 1920x1080 escalada al alto de la pantalla y centrada a lo ancho.
#[derive(Clone, Copy)]
pub struct Lienzo {
    pub s: f32,
    pub ox: f32,
    pub w: f32,
    pub h: f32,
}

impl Lienzo {
    pub fn new(ui: &Ui) -> Self {
        let [w, h] = ui.io().display_size;
        let s = h / 1080.0;
        Lienzo { s, ox: ((w - 1920.0 * s) / 2.0).max(0.0), w, h }
    }
    pub fn x(&self, v: f32) -> f32 {
        self.ox + v * self.s
    }
    pub fn y(&self, v: f32) -> f32 {
        v * self.s
    }
    pub fn p(&self, x: f32, y: f32) -> [f32; 2] {
        [self.x(x), self.y(y)]
    }
    pub fn d(&self, v: f32) -> f32 {
        v * self.s
    }
    /// Borde derecho de la caja de diseño.
    pub fn derecha(&self) -> f32 {
        self.x(1920.0).min(self.w)
    }
}

/// Fondo liso de los menús.
pub fn fondo(dl: &imgui::DrawListMut, lz: &Lienzo, a: f32) {
    dl.add_rect([0.0, 0.0], [lz.w, lz.h], c(FONDO, a)).filled(true).build();
}

/// Arte a pantalla completa (recortado, sin bandas) con una capa oscura encima.
pub fn arte(dl: &imgui::DrawListMut, lz: &Lienzo, t: (TextureId, [f32; 2]), tapa: f32, a: f32) {
    let (id, [iw, ih]) = t;
    let k = (lz.w / iw).max(lz.h / ih);
    let (dw, dh) = (iw * k, ih * k);
    dl.add_image(id, [(lz.w - dw) / 2.0, (lz.h - dh) / 2.0], [(lz.w + dw) / 2.0, (lz.h + dh) / 2.0]).col(adorno::white(a)).build();
    dl.add_rect([0.0, 0.0], [lz.w, lz.h], c([6, 5, 4], tapa * a)).filled(true).build();
}

pub fn texto(ui: &Ui, dl: &imgui::DrawListMut, font: FontId, p: [f32; 2], col: ImColor32, t: &str) {
    let _f = ui.push_font(font);
    dl.add_text(p, col, t);
}

pub fn ancho(ui: &Ui, font: FontId, t: &str) -> f32 {
    let _f = ui.push_font(font);
    ui.calc_text_size(t)[0]
}

/// Texto alineado a la derecha en `p[0]`.
pub fn texto_der(ui: &Ui, dl: &imgui::DrawListMut, font: FontId, p: [f32; 2], col: ImColor32, t: &str) {
    let w = ancho(ui, font, t);
    texto(ui, dl, font, [p[0] - w, p[1]], col, t);
}

/// Párrafo partido en líneas de `ancho`; devuelve la altura bajo la última línea.
#[allow(clippy::too_many_arguments)]
pub fn parrafo(ui: &Ui, dl: &imgui::DrawListMut, font: FontId, p: [f32; 2], ancho: f32, alto: f32, col: ImColor32, t: &str, max: usize) -> f32 {
    let _f = ui.push_font(font);
    let mut y = p[1];
    for l in wrap(ui, t, ancho).iter().take(max) {
        dl.add_text([p[0], y], col, l);
        y += alto;
    }
    y
}

/// Recorta con puntos suspensivos para que quepa en `max_w`.
pub fn ajustar(ui: &Ui, font: FontId, t: &str, max_w: f32) -> String {
    let _f = ui.push_font(font);
    if ui.calc_text_size(t)[0] <= max_w {
        return t.to_string();
    }
    let mut cs: Vec<char> = t.chars().collect();
    while !cs.is_empty() && ui.calc_text_size(format!("{}...", cs.iter().collect::<String>()))[0] > max_w {
        cs.pop();
    }
    format!("{}...", cs.iter().collect::<String>().trim_end())
}

/// Título de pantalla arriba a la izquierda y, debajo, una línea tenue y otra en oro (opcionales).
#[allow(clippy::too_many_arguments)]
pub fn cabecera(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, lz: &Lienzo, titulo: &str, sub: &str, oro: &str, a: f32) {
    let x = lz.x(120.0);
    texto(ui, dl, fonts.grande, [x, lz.y(72.0)], c(TINTA, a), titulo);
    let mut y = lz.y(142.0);
    if !sub.is_empty() {
        texto(ui, dl, fonts.nota, [x, y], c(TENUE, a), sub);
        y += lz.d(34.0);
    }
    if !oro.is_empty() {
        texto(ui, dl, fonts.nota, [x, y], c(ORO, a), oro);
    }
}

/// Un icono en el cuadrado (x, y, lado). Doble (arma en las dos manos): dos copias cruzadas, la de
/// delante en espejo, para que se vea sin leer la etiqueta.
pub fn icono(dl: &imgui::DrawListMut, i: TextureId, x: f32, y: f32, lado: f32, col: ImColor32, doble: bool) {
    if !doble {
        dl.add_image(i, [x, y], [x + lado, y + lado]).col(col).build();
        return;
    }
    let l = lado * 0.8;
    let d = lado - l;
    dl.add_image(i, [x, y], [x + l, y + l]).col(col).build();
    dl.add_image(i, [x + d, y + d], [x + lado, y + lado]).uv_min([1.0, 0.0]).uv_max([0.0, 1.0]).col(col).build();
}

/// Ancho del marco de objeto para un alto dado (la parte visible del marco).
pub fn ancho_marco(alto: f32) -> f32 {
    alto * 168.0 / 195.0
}

/// Icono dentro del marco de casilla de objeto del juego: `alto` es el de la parte visible del
/// marco (la textura trae margen transparente alrededor). `luz` < 1 lo apaga.
#[allow(clippy::too_many_arguments)]
pub fn marco_icono(dl: &imgui::DrawListMut, tex: &MenuTex, icon: Option<TextureId>, x: f32, y: f32, alto: f32, luz: f32, a: f32) {
    marco_icono_doble(dl, tex, icon, x, y, alto, luz, a, false);
}

/// Como `marco_icono`, con el icono doble si el arma va en las dos manos.
#[allow(clippy::too_many_arguments)]
pub fn marco_icono_doble(dl: &imgui::DrawListMut, tex: &MenuTex, icon: Option<TextureId>, x: f32, y: f32, alto: f32, luz: f32, a: f32, doble: bool) {
    let w = ancho_marco(alto);
    if let Some(i) = icon {
        let lado = w.min(alto) * 0.9;
        let (ix, iy) = (x + (w - lado) / 2.0, y + (alto - lado) / 2.0);
        let g = (255.0 * luz) as u8;
        icono(dl, i, ix, iy, lado, ImColor32::from_rgba(g, g, g, (255.0 * a) as u8), doble);
    }
    match tex.marco_objeto {
        Some((t, _)) => {
            // Visible: x 31..199 de 230, y 26..221 de 242.
            let k = alto / 195.0;
            let (tx, ty) = (x - 31.0 * k, y - 26.0 * k);
            dl.add_image(t, [tx, ty], [tx + 230.0 * k, ty + 242.0 * k]).col(adorno::white(a)).build();
        }
        None => {
            dl.add_rect([x, y], [x + w, y + alto], c(LINEA, a)).thickness(1.0).build();
        }
    }
}

/// Icono dentro de la casilla del inventario (rejilla de «Lo que llevas»).
#[allow(clippy::too_many_arguments)]
pub fn casilla(dl: &imgui::DrawListMut, tex: &MenuTex, icon: Option<TextureId>, x: f32, y: f32, lado: f32, col_icono: ImColor32, a: f32) {
    match tex.casilla {
        Some((t, _)) => {
            dl.add_image(t, [x, y], [x + lado, y + lado * 318.0 / 314.0]).col(adorno::white(a * if icon.is_some() { 1.0 } else { 0.6 })).build();
        }
        None => {
            dl.add_rect([x, y], [x + lado, y + lado], c(LINEA, a)).filled(true).build();
        }
    }
    if let Some(i) = icon {
        let m = lado * 0.15;
        dl.add_image(i, [x + m, y + m], [x + lado - m, y + lado - m]).col(col_icono).build();
    }
}

/// Barra de selección de los menús del juego detrás de una fila (x0..x1, y0..y1 es la fila).
#[allow(clippy::too_many_arguments)]
pub fn barra_sel(dl: &imgui::DrawListMut, tex: &MenuTex, lz: &Lienzo, x0: f32, y0: f32, x1: f32, y1: f32, k: f32) {
    if k <= 0.0 {
        return;
    }
    match tex.cursor {
        Some(t) => adorno::cursor(dl, t, [x0 - lz.d(30.0), y0 - lz.d(26.0), x1, y1 + lz.d(26.0)], lz.s, k),
        None => {
            dl.add_rect([x0 - lz.d(14.0), y0], [x0 - lz.d(10.0), y1], c(ORO, k)).filled(true).build();
        }
    }
}

/// Una fila de lista: marco con el icono, nombre y debajo una línea corta.
pub struct Fila<'a> {
    pub icon: Option<TextureId>,
    pub nombre: &'a str,
    pub sub: &'a str,
    pub sub_col: [u8; 3],
    pub sel: bool,
    pub apagada: bool,
    /// Arma doble: el icono dos veces.
    pub doble: bool,
}

/// Alto de una fila de lista a escala 1.
pub const FILA: f32 = 111.0;

/// Dibuja una fila con su icono enmarcado (alto `alto`); la elegida, más clara y con la barra.
#[allow(clippy::too_many_arguments)]
pub fn fila(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, tex: &MenuTex, lz: &Lienzo, x: f32, y: f32, ancho: f32, alto: f32, f: &Fila, k_sel: f32, a: f32) {
    if f.sel {
        barra_sel(dl, tex, lz, x, y, x + ancho, y + alto, k_sel * a);
    }
    let luz = if f.apagada { 0.45 } else { 1.0 };
    let base = if f.sel { 1.0 } else { 0.75 } * luz * a;
    marco_icono_doble(dl, tex, f.icon, x, y, alto, if f.apagada { 0.5 } else { 1.0 }, base.max(0.35 * a), f.doble);
    let tx = x + ancho_marco(alto) + lz.d(30.0);
    let tw = x + ancho - tx - lz.d(10.0);
    let nombre = ajustar(ui, fonts.opcion, f.nombre, tw);
    let (ny, sy) = if f.sub.is_empty() { (y + alto / 2.0 - lz.d(19.0), 0.0) } else { (y + alto / 2.0 - lz.d(34.0), y + alto / 2.0 + lz.d(8.0)) };
    texto(ui, dl, fonts.opcion, [tx, ny], c(if f.sel { CLARA } else { TINTA }, base), &nombre);
    if !f.sub.is_empty() {
        let sub = ajustar(ui, fonts.nota, f.sub, tw);
        texto(ui, dl, fonts.nota, [tx, sy], c(f.sub_col, base), &sub);
    }
}

/// Opción de menú solo de texto (sin icono), con su detalle opcional debajo.
#[allow(clippy::too_many_arguments)]
pub fn opcion(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, tex: &MenuTex, lz: &Lienzo, x: f32, y: f32, ancho: f32, nombre: &str, detalle: &str, sel: bool, apagada: bool, k_sel: f32, a: f32) -> f32 {
    let luz = if apagada { 0.4 } else { 1.0 };
    let alto = if detalle.is_empty() { lz.d(44.0) } else { lz.d(74.0) };
    if sel {
        barra_sel(dl, tex, lz, x, y - lz.d(6.0), x + ancho, y + alto - lz.d(4.0), k_sel * a);
    }
    texto(ui, dl, fonts.opcion, [x + lz.d(24.0), y], c(if sel { CLARA } else { APAGADA }, a * luz), nombre);
    if !detalle.is_empty() {
        let d = ajustar(ui, fonts.nota, detalle, ancho - lz.d(30.0));
        texto(ui, dl, fonts.nota, [x + lz.d(24.0), y + lz.d(40.0)], c(if sel { APAGADA } else { TENUE }, a * luz), &d);
    }
    alto
}

/// Lo que dice una ficha.
pub struct Ficha<'a> {
    pub icon: Option<TextureId>,
    /// El icono en silueta (compendio: aún sin descubrir).
    pub silueta: bool,
    pub nombre: &'a str,
    /// Línea bajo el filete («Bendición común») y su color.
    pub linea: &'a str,
    pub linea_col: [u8; 3],
    pub texto: &'a str,
    /// En rojo: el precio de las míticas o la maldición que trae.
    pub contra: &'a str,
    /// Nota aparte (título en rojo y texto): la maldición del trato explicada.
    pub nota: Option<(&'a str, &'a str)>,
    /// Pie en oro (precio, nivel...).
    pub pie: &'a str,
    pub apagada: bool,
    /// Arma doble: el icono dos veces.
    pub doble: bool,
}

/// Ficha a la derecha: icono grande en el marco octogonal, nombre con el filete, su línea y el
/// texto debajo. `x` es el borde izquierdo y `ancho` lo que ocupa.
#[allow(clippy::too_many_arguments)]
pub fn ficha(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, tex: &MenuTex, lz: &Lienzo, x: f32, y: f32, ancho: f32, f: &Ficha, a: f32) {
    let lado = lz.d(330.0);
    let (mx, my) = (x, y);
    if let Some((t, _)) = tex.marco_ficha {
        dl.add_image(t, [mx, my], [mx + lado, my + lado * 184.0 / 182.0]).col(adorno::white(a)).build();
    }
    if let Some(i) = f.icon {
        let m = lado * 0.155;
        let col = if f.silueta { ImColor32::from_rgba(0, 0, 0, (230.0 * a) as u8) } else { adorno::white(a * if f.apagada { 0.5 } else { 1.0 }) };
        icono(dl, i, mx + m, my + m, lado - 2.0 * m, col, f.doble);
    }
    // Nombre a la derecha del marco, con el filete y la línea.
    let nx = mx + lado + lz.d(40.0);
    let nw = x + ancho - nx;
    let mut ny = my + lz.d(70.0);
    {
        let _f = ui.push_font(fonts.grande);
        let lineas = wrap(ui, f.nombre, nw);
        if lineas.len() > 1 {
            ny -= lz.d(26.0);
        }
        for l in lineas.iter().take(2) {
            dl.add_text([nx, ny], c(CLARA, a), l);
            ny += lz.d(52.0);
        }
    }
    ny += lz.d(8.0);
    match tex.filete {
        Some((t, _)) => {
            let fw = lz.d(300.0).min(nw);
            dl.add_image(t, [nx, ny], [nx + fw, ny + fw * 18.0 / 182.0]).col(adorno::white(a)).build();
        }
        None => dl.add_line([nx, ny], [nx + lz.d(240.0), ny], c(TENUE, a)).thickness(1.0).build(),
    }
    ny += lz.d(46.0);
    if !f.linea.is_empty() {
        parrafo(ui, dl, fonts.nota, [nx, ny], nw, lz.d(30.0), c(f.linea_col, a), f.linea, 2);
    }
    // Texto bajo el marco, a todo el ancho.
    let mut ty = my + lado + lz.d(36.0);
    ty = parrafo(ui, dl, fonts.texto, [x, ty], ancho, lz.d(40.0), c(TINTA, a), f.texto, 6);
    if !f.contra.is_empty() {
        ty = parrafo(ui, dl, fonts.texto, [x, ty + lz.d(8.0)], ancho, lz.d(40.0), c(SANGRE, a), f.contra, 3);
    }
    if let Some((t, d)) = f.nota {
        ty += lz.d(18.0);
        texto(ui, dl, fonts.texto, [x, ty], c(SANGRE, a), t);
        ty = parrafo(ui, dl, fonts.nota, [x, ty + lz.d(40.0)], ancho, lz.d(32.0), c(APAGADA, a), d, 4);
    }
    if !f.pie.is_empty() {
        texto(ui, dl, fonts.texto, [x, ty + lz.d(18.0)], c(if f.apagada { TENUE } else { ORO }, a), f.pie);
    }
}

/// Raya vertical fina que separa la lista de la ficha.
pub fn separador(dl: &imgui::DrawListMut, lz: &Lienzo, x: f32, a: f32) {
    dl.add_line([x, 0.0], [x, lz.h], c(LINEA, a)).thickness(lz.d(1.5)).build();
}

/// Ayuda de botones abajo a la derecha: el botón en un círculo y su etiqueta (traducida aquí).
pub fn ayuda(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, lz: &Lienzo, items: &[(&str, &str)], a: f32) {
    ayuda_en(ui, dl, fonts, lz, None, items, a);
}

/// Ayuda de botones abajo a la derecha sobre el mundo del juego: con un fondo oscuro detrás.
pub fn ayuda_sobre_mundo(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, lz: &Lienzo, items: &[(&str, &str)], a: f32) {
    ayuda_dibujar(ui, dl, fonts, lz, None, items, a, true);
}

/// Ayuda de botones abajo, empezando en `x` (o alineada a la derecha con `None`).
pub fn ayuda_en(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, lz: &Lienzo, x: Option<f32>, items: &[(&str, &str)], a: f32) {
    ayuda_dibujar(ui, dl, fonts, lz, x, items, a, false);
}

#[allow(clippy::too_many_arguments)]
fn ayuda_dibujar(ui: &Ui, dl: &imgui::DrawListMut, fonts: &Fonts, lz: &Lienzo, x: Option<f32>, items: &[(&str, &str)], a: f32, fondo: bool) {
    let y = lz.h - lz.d(84.0);
    let r = lz.d(14.0);
    let gap = lz.d(36.0);
    let etiquetas: Vec<String> = items.iter().map(|(_, l)| rogue_core::idioma::td(l)).collect();
    let total: f32 = items
        .iter()
        .zip(&etiquetas)
        .map(|((b, _), l)| 2.0 * r.max(ancho(ui, fonts.caption, b) / 2.0 + lz.d(5.0)) + lz.d(10.0) + ancho(ui, fonts.nota, l))
        .sum::<f32>()
        + gap * (items.len().saturating_sub(1)) as f32;
    let mut x = x.unwrap_or(lz.derecha() - lz.d(105.0) - total);
    if fondo {
        dl.add_rect([x - lz.d(28.0), y - lz.d(16.0)], [x + total + lz.d(28.0), y + lz.d(50.0)], c(FONDO, 0.82 * a)).filled(true).build();
    }
    for ((b, _), l) in items.iter().zip(&etiquetas) {
        let bw = ancho(ui, fonts.caption, b);
        let rr = r.max(bw / 2.0 + lz.d(5.0));
        let cp = [x + rr, y + lz.d(15.0)];
        dl.add_circle(cp, rr, c(APAGADA, a)).thickness(lz.d(1.5)).num_segments(24).build();
        {
            let _f = ui.push_font(fonts.caption);
            let sz = ui.calc_text_size(b);
            dl.add_text([cp[0] - sz[0] / 2.0, cp[1] - sz[1] / 2.0 - lz.d(1.0)], c(APAGADA, a), b);
        }
        x += 2.0 * rr + lz.d(10.0);
        texto(ui, dl, fonts.nota, [x, y], c(APAGADA, a), l);
        x += ancho(ui, fonts.nota, l) + gap;
    }
}

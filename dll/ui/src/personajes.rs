//! Pantalla de Personajes: el personaje de verdad en el mundo, de frente a la cámara, y arriba, por
//! encima de su cabeza, una banda con su nombre y su pasiva, las armas iniciales en fila (con su marco
//! de objeto), sus invocaciones y sus apariencias.

use imgui::{TextureId, Ui};
use rogue_core::idioma::t;

use crate::Fonts;
use crate::adorno;
use crate::estilo::{self, APAGADA, CLARA, FONDO, LINEA, ORO, SANGRE, TENUE, TINTA, c};
use crate::mapa::MapTextures;

pub struct PersonajesView {
    pub nombre: String,
    pub lema: String,
    pub pasiva: String,
    /// (nombre, tipo, candado, icono) de cada arma inicial; el candado dice qué falta para elegirla.
    pub armas: Vec<(String, String, Option<String>, Option<TextureId>)>,
    pub arma_sel: usize,
    /// (nombre, candado) de sus apariencias, la de siempre primero, y la elegida.
    pub apariencias: Vec<(String, Option<String>)>,
    pub apariencia_sel: usize,
    /// (nombre, candado) de sus invocaciones (el Invocador) y la elegida.
    pub invocaciones: Vec<(String, Option<String>)>,
    pub invocacion_sel: usize,
    /// Personaje que se ve y cuántos hay.
    pub idx: usize,
    pub total: usize,
    /// Segundos desde que se abrió la pantalla y desde el último cambio.
    pub t: f32,
    pub t_sel: f32,
}

pub fn draw(ui: &Ui, v: &PersonajesView, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(v.t, 0);
    // Banda horizontal arriba, por encima de la cabeza del personaje: el resto es el mundo con él de
    // frente, entero a la vista.
    let alto = lz.y(330.0);
    dl.add_rect([0.0, 0.0], [lz.w, alto], c(FONDO, 0.9 * a0)).filled(true).build();
    dl.add_line([0.0, alto], [lz.w, alto], c(LINEA, a0)).thickness(lz.d(1.5)).build();
    let top = lz.y(44.0);

    // Quién es.
    let x = lz.x(90.0);
    let k = adorno::ease_out(v.t_sel / 0.25);
    estilo::texto(ui, &dl, fonts.grande, [x, top - lz.d(6.0)], c(TINTA, k * a0), &v.nombre);
    let nw = estilo::ancho(ui, fonts.grande, &v.nombre);
    estilo::texto(ui, &dl, fonts.nota, [x + nw + lz.d(22.0), top + lz.d(14.0)], c(TENUE, a0), &format!("{} / {}", v.idx + 1, v.total));
    let y = estilo::parrafo(ui, &dl, fonts.nota, [x, top + lz.d(66.0)], lz.d(540.0), lz.d(32.0), c(ORO, a0), &v.lema, 2);
    estilo::parrafo(ui, &dl, fonts.texto, [x, y + lz.d(8.0)], lz.d(540.0), lz.d(38.0), c(TINTA, a0), &v.pasiva, 4);

    // Armas iniciales: los marcos en fila; la elegida, clara, con su nombre debajo.
    let ax = lz.x(700.0);
    estilo::texto(ui, &dl, fonts.nota, [ax, top], c(TENUE, a0), t("Starting weapon"));
    let marco = lz.d(104.0);
    let paso = estilo::ancho_marco(marco) + lz.d(30.0);
    for (i, (_, _, candado, icon)) in v.armas.iter().enumerate() {
        let a = adorno::appear(v.t, i + 1);
        let sel = i == v.arma_sel;
        let mx = ax + i as f32 * paso;
        let my = top + lz.d(48.0);
        if sel {
            estilo::barra_sel(&dl, &tex.menu, &lz, mx + lz.d(20.0), my + lz.d(10.0), mx + estilo::ancho_marco(marco) + lz.d(10.0), my + marco - lz.d(10.0), a);
        }
        let luz = if candado.is_some() { 0.4 } else if sel { 1.0 } else { 0.7 };
        estilo::marco_icono(&dl, &tex.menu, *icon, mx, my, marco, luz, a);
    }
    if let Some((nombre, tipo, candado, _)) = v.armas.get(v.arma_sel) {
        let ny = top + lz.d(48.0) + marco + lz.d(22.0);
        estilo::texto(ui, &dl, fonts.texto, [ax, ny], c(CLARA, a0), nombre);
        let (linea, col) = match candado {
            Some(k) => (k.as_str(), SANGRE),
            None => (tipo.as_str(), APAGADA),
        };
        estilo::texto(ui, &dl, fonts.nota, [ax, ny + lz.d(40.0)], c(col, a0), linea);
    }

    // Invocaciones y apariencias, en columnas: la elegida en claro con su marca; las bloqueadas,
    // apagadas y con lo que falta debajo.
    let lista = |x: f32, titulo: &str, items: &[(String, Option<String>)], elegido: usize| {
        estilo::texto(ui, &dl, fonts.nota, [x, top], c(TENUE, a0), titulo);
        let mut y = top + lz.d(46.0);
        for (i, (nombre, candado)) in items.iter().enumerate() {
            let sel = i == elegido;
            let luz = if candado.is_some() { 0.45 } else { 1.0 };
            if sel {
                dl.add_rect([x, y + lz.d(8.0)], [x + lz.d(5.0), y + lz.d(30.0)], c(ORO, a0)).filled(true).build();
            }
            let n = estilo::ajustar(ui, fonts.texto, nombre, lz.d(330.0));
            estilo::texto(ui, &dl, fonts.texto, [x + lz.d(20.0), y], c(if sel { CLARA } else { APAGADA }, a0 * luz), &n);
            y += lz.d(38.0);
            if let Some(k) = candado {
                let k = estilo::ajustar(ui, fonts.caption, k, lz.d(330.0));
                estilo::texto(ui, &dl, fonts.caption, [x + lz.d(20.0), y - lz.d(2.0)], c(SANGRE, a0 * 0.8), &k);
                y += lz.d(26.0);
            }
            y += lz.d(6.0);
        }
    };
    let mut cx = lz.x(1190.0);
    if !v.invocaciones.is_empty() {
        lista(cx, t("Summon"), &v.invocaciones, v.invocacion_sel);
        cx += lz.d(370.0);
    }
    if v.apariencias.len() > 1 {
        lista(cx, t("Appearance"), &v.apariencias, v.apariencia_sel);
    }

    let mut ayuda = vec![("<>", "Character"), ("^v", "Weapon")];
    if v.invocaciones.len() > 1 {
        ayuda.push(("X", "Summon"));
    }
    if v.apariencias.len() > 1 {
        ayuda.push(("LB", "Appearance"));
    }
    ayuda.push(("B", "Back"));
    estilo::ayuda_sobre_mundo(ui, &dl, fonts, &lz, &ayuda, a0);
}

//! Pantallas de opciones con texto: el evento («?»), la pausa y los menús sencillos (cooperativo,
//! contraseña, pruebas). El evento sigue a las conversaciones del juego: la escena a pantalla
//! completa y, abajo, una banda oscura con el texto a la izquierda y las respuestas a la derecha. La
//! pausa oscurece el combate sin taparlo. Los menús, opciones a la izquierda y el texto a la derecha.

use imgui::{TextureId, Ui};

use crate::Fonts;
use crate::adorno;
use crate::estilo::{self, APAGADA, CLARA, ORO, SANGRE, TENUE, TINTA, c};
use crate::mapa::MapTextures;

pub struct EventOptionView {
    pub texto: String,
    pub detalle: String,
    pub apagada: bool,
    /// Explicación que se añade al detalle cuando la opción está elegida (la maldición que da).
    pub nota: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modo {
    /// Evento: escena a pantalla completa y banda abajo.
    Encuentro,
    /// Pausa: el combate o la escena oscurecidos; opciones a la izquierda y el hueco de la derecha
    /// para el detalle de las bendiciones (`hueco_pausa`).
    Pausa,
    /// Menú sencillo: opciones a la izquierda y el texto a la derecha.
    Menu,
}

pub struct EventView {
    pub cabecera: String,
    pub titulo: String,
    pub texto: String,
    pub imagen: Option<(TextureId, [f32; 2])>,
    pub opciones: Vec<EventOptionView>,
    pub selected: usize,
    pub ayuda: Vec<(String, String)>,
    pub modo: Modo,
    /// Segundos desde que se abrió la pantalla y desde el último cambio de selección.
    pub t: f32,
    pub t_sel: f32,
}

/// Hueco de la derecha en la pausa ([x0, y0, x1, y1]), donde va lo de este combate (pausa.rs).
pub fn hueco_pausa(ui: &Ui) -> [f32; 4] {
    let lz = estilo::Lienzo::new(ui);
    [lz.x(1035.0), lz.y(150.0), lz.x(1815.0), lz.y(930.0)]
}

pub fn draw(ui: &Ui, v: &EventView, tex: &MapTextures, fonts: &Fonts) {
    match v.modo {
        Modo::Encuentro => encuentro(ui, v, tex, fonts),
        Modo::Pausa | Modo::Menu => menu(ui, v, tex, fonts),
    }
}

fn encuentro(ui: &Ui, v: &EventView, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(v.t, 0);
    estilo::fondo(&dl, &lz, 1.0);
    if let Some(img) = v.imagen {
        estilo::arte(&dl, &lz, img, 0.3, adorno::ease_out(v.t / 0.5));
    }
    // Banda de abajo: crece con lo que haya que leer.
    let (tx, tw) = (lz.x(135.0), lz.d(880.0));
    let (ox, ow) = (lz.x(1155.0), lz.d(660.0));
    let parrafos: Vec<&str> = v.texto.split("\n\n").collect();
    let alto_texto = {
        let _f = ui.push_font(fonts.texto);
        parrafos.iter().map(|p| crate::cartas::wrap(ui, p, tw).len() as f32 * lz.d(40.0) + lz.d(12.0)).sum::<f32>() + lz.d(62.0)
    };
    let alto_op: f32 = v.opciones.iter().map(|o| if o.detalle.is_empty() { lz.d(62.0) } else { lz.d(94.0) }).sum::<f32>() + lz.d(40.0);
    let banda = alto_texto.max(alto_op).max(lz.d(330.0)) + lz.d(110.0);
    let top = lz.h - banda;
    dl.add_rect([0.0, top], [lz.w, lz.h], c([6, 5, 4], 0.76 * a0)).filled(true).build();

    let mut y = top + lz.d(48.0);
    estilo::texto(ui, &dl, fonts.opcion, [tx, y], c(ORO, a0), &v.titulo);
    y += lz.d(58.0);
    for (i, p) in parrafos.iter().enumerate() {
        // Del segundo párrafo en adelante, lo que ha pasado: en oro.
        let col = if i == 0 { TINTA } else { ORO };
        y = estilo::parrafo(ui, &dl, fonts.texto, [tx, y], tw, lz.d(40.0), c(col, a0), p, 8) + lz.d(12.0);
    }

    let k_sel = adorno::ease_out(v.t_sel / 0.18);
    let mut oy = top + lz.d(52.0);
    for (i, o) in v.opciones.iter().enumerate() {
        let a = adorno::appear(v.t, i + 2);
        let sel = v.selected == i;
        let luz = if o.apagada { 0.4 } else { 1.0 };
        let alto = if o.detalle.is_empty() { lz.d(48.0) } else { lz.d(80.0) };
        if sel {
            estilo::barra_sel(&dl, &tex.menu, &lz, ox, oy - lz.d(6.0), ox + ow, oy + alto - lz.d(8.0), k_sel * a);
        }
        estilo::texto(ui, &dl, fonts.opcion, [ox + lz.d(24.0), oy], c(if sel { CLARA } else { APAGADA }, a * luz), &o.texto);
        if !o.detalle.is_empty() {
            let d = estilo::ajustar(ui, fonts.nota, &o.detalle, ow - lz.d(30.0));
            estilo::texto(ui, &dl, fonts.nota, [ox + lz.d(24.0), oy + lz.d(42.0)], c(if sel { ORO } else { TENUE }, a * luz), &d);
        }
        oy += alto + lz.d(14.0);
        if let (true, Some(n)) = (sel, &o.nota) {
            oy = estilo::parrafo(ui, &dl, fonts.nota, [ox + lz.d(24.0), oy], ow - lz.d(30.0), lz.d(30.0), c(SANGRE, a), n, 3) + lz.d(10.0);
        }
    }
    let ayuda: Vec<(&str, &str)> = v.ayuda.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    estilo::ayuda(ui, &dl, fonts, &lz, &ayuda, a0);
}

fn menu(ui: &Ui, v: &EventView, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(v.t, 0);
    match (v.modo, v.imagen) {
        // Pausa en combate: el combate se ve detrás, oscurecido.
        (Modo::Pausa, None) => dl.add_rect([0.0, 0.0], [lz.w, lz.h], c([6, 5, 4], 0.8 * a0)).filled(true).build(),
        (_, Some(img)) => {
            estilo::fondo(&dl, &lz, 1.0);
            estilo::arte(&dl, &lz, img, 0.72, a0);
        }
        _ => estilo::fondo(&dl, &lz, 1.0),
    }
    // En la pausa el texto (acto y semilla) va bajo el título; en los menús, a la derecha.
    let sub = if v.modo == Modo::Pausa { v.texto.as_str() } else { "" };
    estilo::cabecera(ui, &dl, fonts, &lz, &v.titulo, sub, "", a0);
    let x = lz.x(120.0);
    let ancho = lz.d(720.0);
    let mut y = lz.y(260.0);
    let k_sel = adorno::ease_out(v.t_sel / 0.18);
    for (i, o) in v.opciones.iter().enumerate() {
        let a = adorno::appear(v.t, i + 1);
        let alto = estilo::opcion(ui, &dl, fonts, &tex.menu, &lz, x - lz.d(24.0), y, ancho, &o.texto, &o.detalle, v.selected == i, o.apagada, k_sel, a);
        y += alto + lz.d(26.0);
    }
    if v.modo == Modo::Menu && !v.texto.is_empty() {
        estilo::separador(&dl, &lz, lz.x(930.0), a0);
        let mut ty = lz.y(260.0);
        // Listas (líneas sueltas dentro de un párrafo, como la pantalla de la ascensión): una línea por
        // renglón, con letra más pequeña para que quepan.
        let lista = v.texto.split("\n\n").any(|p| p.contains('\n'));
        let (fuente, alto) = if lista { (fonts.nota, lz.d(31.0)) } else { (fonts.texto, lz.d(40.0)) };
        for (i, p) in v.texto.split("\n\n").enumerate() {
            let col = if i == 0 { TINTA } else { ORO };
            for l in p.split('\n') {
                ty = estilo::parrafo(ui, &dl, fuente, [lz.x(1035.0), ty], lz.d(780.0), alto, c(col, a0), l, 12) + if lista { lz.d(2.0) } else { 0.0 };
            }
            ty += lz.d(18.0);
        }
    }
    let ayuda: Vec<(&str, &str)> = v.ayuda.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    estilo::ayuda(ui, &dl, fonts, &lz, &ayuda, a0);
}

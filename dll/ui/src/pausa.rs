//! Pausa en combate: a la derecha de las opciones, las bendiciones que tienen algo en marcha (las de
//! la tira), cada una en su marco con su estado en palabras, una barra de progreso con las piezas
//! del juego y lo que hace (y su precio, si es mítica). Debajo, el resto por su nombre.

use imgui::{TextureId, Ui};
use rogue_core::idioma::tf;

use crate::Fonts;
use crate::adorno::{self, MenuTex};
use crate::cartas::wrap;
use crate::estilo::{self, APAGADA, ORO, SANGRE, TENUE, TINTA, c};

#[derive(Clone, Debug, Default)]
pub struct PausaItem {
    pub tex: Option<TextureId>,
    pub nombre: String,
    pub texto: String,
    /// Precio de las míticas (en rojo).
    pub contra: String,
    /// "4 / 6 hits", "Back in 6 s", "Stacks 6 / 10"...
    pub estado: String,
    pub progreso: Option<f32>,
    pub lista: bool,
    pub gastada: bool,
}

/// Dibuja la lista dentro de `rect` ([x0, y0, x1, y1]). `resto`: nombres de las demás bendiciones;
/// `arma`: el nivel del arma, a la derecha del título.
#[allow(clippy::too_many_arguments)]
pub fn draw(ui: &Ui, rect: [f32; 4], items: &[PausaItem], resto: &[String], arma: &str, tex: &MenuTex, fonts: &Fonts, t: f32) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_foreground_draw_list();
    let a = adorno::ease_out(t / 0.4);
    let (x0, mut y) = (rect[0], rect[1]);
    let ancho = rect[2] - rect[0];
    estilo::texto(ui, &dl, fonts.opcion, [x0, y], c(TINTA, a), rogue_core::idioma::t("In this fight"));
    if !arma.is_empty() {
        estilo::texto_der(ui, &dl, fonts.nota, [rect[2], y + lz.d(8.0)], c(ORO, a), arma);
    }
    y += lz.d(54.0);
    if let Some((f, _)) = tex.filete {
        let fw = lz.d(300.0);
        dl.add_image(f, [x0, y], [x0 + fw, y + fw * 18.0 / 182.0]).col(adorno::white(a)).build();
    }
    y += lz.d(44.0);
    let marco = lz.d(84.0);
    let tx = x0 + estilo::ancho_marco(marco) + lz.d(24.0);
    let tw = rect[2] - tx;
    // Las que no caben pasan a la línea de abajo, con el resto.
    let mut fuera: Vec<String> = vec![];
    for it in items {
        let lineas = {
            let _f = ui.push_font(fonts.nota);
            wrap(ui, &it.texto, tw).len().min(2) + usize::from(!it.contra.is_empty())
        };
        let fila = (lz.d(48.0) + lz.d(30.0) * lineas as f32).max(marco) + lz.d(26.0);
        if !fuera.is_empty() || y + fila > rect[3] - lz.d(70.0) {
            fuera.push(it.nombre.clone());
            continue;
        }
        let luz: f32 = if it.gastada { 0.4 } else { 1.0 };
        estilo::marco_icono(&dl, tex, it.tex, x0, y, marco, luz, a);
        let estado_w = estilo::ancho(ui, fonts.nota, &it.estado);
        let nombre = estilo::ajustar(ui, fonts.texto, &it.nombre, tw - estado_w - lz.d(20.0));
        estilo::texto(ui, &dl, fonts.texto, [tx, y], c(TINTA, a * luz.max(0.6)), &nombre);
        estilo::texto_der(ui, &dl, fonts.nota, [rect[2], y + lz.d(4.0)], c(if it.lista { ORO } else { TENUE }, a), &it.estado);
        let mut yy = y + lz.d(42.0);
        if let Some(p) = it.progreso {
            adorno::barra(&dl, tex, tx, yy - lz.d(2.0), tw * 0.45, lz.d(4.0), p, p, if it.lista { ORO } else { [150, 140, 118] }, a, lz.s * 0.7);
            yy += lz.d(10.0);
        }
        yy = estilo::parrafo(ui, &dl, fonts.nota, [tx, yy], tw, lz.d(30.0), c(APAGADA, a * luz.max(0.6)), &it.texto, 2);
        if !it.contra.is_empty() {
            estilo::texto(ui, &dl, fonts.nota, [tx, yy], c(SANGRE, a), &it.contra);
        }
        y += fila;
    }
    let resto: Vec<String> = fuera.into_iter().chain(resto.iter().cloned()).collect();
    if !resto.is_empty() {
        let _f = ui.push_font(fonts.nota);
        let lineas = wrap(ui, &tf("Also with you: {}.", &[&resto.join(", ")]), ancho);
        let mut yy = (rect[3] - lz.d(32.0) * lineas.len().min(2) as f32).max(y);
        for line in lineas.iter().take(2) {
            dl.add_text([x0, yy], c(TENUE, a), line);
            yy += lz.d(32.0);
        }
    }
}

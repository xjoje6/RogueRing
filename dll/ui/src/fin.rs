//! Pantalla de fin de run (muerte o victoria). Empieza como el «HAS MUERTO» del juego: la banda
//! negra con la letra grande en rojo (en oro si se gana) y, debajo, la run en una línea. Si hay
//! historia de nodos, a los pocos segundos la banda se va y queda el gráfico de la run.

use imgui::{TextureId, Ui};
use rogue_core::idioma::{t, tf};

use crate::Fonts;
use crate::adorno;
use crate::estilo::{self, APAGADA, ORO, c};
use crate::mapa::MapTextures;

pub struct EndView {
    pub won: bool,
    pub act: usize,
    pub nodes: usize,
    pub items: usize,
    pub seed: String,
    pub personaje: String,
    pub golpes: u32,
    pub elites: u32,
    /// Bendiciones míticas de la run.
    pub miticas: Vec<String>,
    /// (icono, nombre, maldita) de todo lo que llevabas.
    pub relics: Vec<(Option<TextureId>, String, bool)>,
    /// Segundos desde que se abrió (entrada animada).
    pub t: f32,
    /// Gráfico de la run (si hay historia de nodos).
    pub grafico: Option<crate::grafico::RunGraph>,
}

/// Segundos que dura la banda antes de dar paso al gráfico (el gráfico empieza a dibujarse aquí).
pub const INTRO: f32 = 3.2;
/// Rojo del «HAS MUERTO» del juego.
const MUERTE: [u8; 3] = [158, 31, 22];

/// Línea de la run: personaje, acto, nodos, élites y golpes.
fn resumen(v: &EndView) -> String {
    let pl = |n: usize, uno: &str, varios: &str| format!("{n} {}", if n == 1 { uno } else { varios });
    let mut partes = vec![];
    if !v.personaje.is_empty() {
        partes.push(v.personaje.clone());
    }
    partes.push(tf("act {}", &[&v.act]));
    partes.push(pl(v.nodes, t("node"), t("nodes")));
    partes.push(pl(v.elites as usize, t("elite"), t("elites")));
    partes.push(pl(v.golpes as usize, t("hit"), t("hits")));
    partes.join(", ")
}

pub fn draw(ui: &Ui, v: &EndView, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    estilo::fondo(&dl, &lz, 1.0);
    let [bw, bh] = tex.background_size;
    estilo::arte(&dl, &lz, (tex.background, [bw, bh]), 0.72, adorno::ease_out(v.t / 0.8));

    // Con gráfico, la banda se va a los INTRO segundos.
    let salida = if v.grafico.is_some() { 1.0 - adorno::ease_out((v.t - INTRO) / 0.5) } else { 1.0 };
    if salida > 0.0 {
        banda(ui, &dl, v, tex, fonts, &lz, salida);
    }
    if let Some(g) = &v.grafico {
        let k = adorno::ease_out((v.t - INTRO + 0.2) / 0.5);
        if k > 0.0 {
            dl.add_rect([0.0, 0.0], [lz.w, lz.h], c([6, 5, 4], 0.6 * k)).filled(true).build();
            let titulo = if v.won { t("Run complete") } else { t("You died") };
            estilo::cabecera(ui, &dl, fonts, &lz, titulo, &resumen(v), &tf("Seed {}", &[&v.seed]), k);
            crate::grafico::draw(ui, &dl, g, [lz.x(190.0), lz.y(300.0), lz.x(1780.0), lz.y(760.0)], lz.y(1000.0), fonts);
            estilo::ayuda(ui, &dl, fonts, &lz, &[("A", "Main menu"), ("+", "Choose node")], k);
        }
    } else {
        estilo::ayuda(ui, &dl, fonts, &lz, &[("A", "Main menu")], adorno::appear(v.t, 6));
    }
}

/// La banda negra con la letra grande, la run en una línea y lo que llevabas.
fn banda(ui: &Ui, dl: &imgui::DrawListMut, v: &EndView, tex: &MapTextures, fonts: &Fonts, lz: &estilo::Lienzo, salida: f32) {
    let entra = adorno::ease_out((v.t - 0.3) / 1.2);
    let a = entra * salida;
    let cy = lz.y(470.0);
    let alto = lz.d(330.0);
    match tex.menu.banda {
        Some((b, _)) => {
            dl.add_image(b, [0.0, cy - alto / 2.0], [lz.w, cy + alto / 2.0]).col(adorno::white(a)).build();
        }
        None => dl.add_rect([0.0, cy - alto / 3.0], [lz.w, cy + alto / 3.0], c([0, 0, 0], 0.8 * a)).filled(true).build(),
    }
    let texto = if v.won { t("RUN COMPLETE") } else { t("YOU DIED") };
    // La letra aparece despacio, como en el juego.
    let al = adorno::ease_out((v.t - 0.6) / 1.4) * salida;
    let tw = estilo::ancho(ui, fonts.enorme, texto);
    estilo::texto(ui, dl, fonts.enorme, [(lz.w - tw) / 2.0, cy - lz.d(70.0)], c(if v.won { ORO } else { MUERTE }, al), texto);

    let a2 = adorno::ease_out((v.t - 1.4) / 0.6) * salida;
    let linea = resumen(v);
    let lw = estilo::ancho(ui, fonts.texto, &linea);
    estilo::texto(ui, dl, fonts.texto, [(lz.w - lw) / 2.0, cy + alto / 2.0 + lz.d(20.0)], c(APAGADA, a2), &linea);
    if !v.miticas.is_empty() {
        let m = tf("Mythic: {}", &[&v.miticas.join(", ")]);
        let mw = estilo::ancho(ui, fonts.nota, &m);
        estilo::texto(ui, dl, fonts.nota, [(lz.w - mw) / 2.0, cy + alto / 2.0 + lz.d(66.0)], c(ORO, a2), &m);
    }
    // Lo que llevabas, en una fila centrada de marcos pequeños.
    if !v.relics.is_empty() {
        let marco = lz.d(62.0);
        let paso = estilo::ancho_marco(marco) + lz.d(10.0);
        let n = v.relics.len().min(((lz.w - lz.d(240.0)) / paso) as usize);
        let x0 = (lz.w - paso * n as f32) / 2.0;
        let y = cy + alto / 2.0 + lz.d(116.0);
        for (i, (icon, _, maldita)) in v.relics.iter().take(n).enumerate() {
            let ai = adorno::ease_out((v.t - 1.8 - 0.03 * i as f32) / 0.3) * salida;
            estilo::marco_icono(dl, &tex.menu, *icon, x0 + i as f32 * paso, y, marco, if *maldita { 0.6 } else { 1.0 }, ai);
        }
    }
}

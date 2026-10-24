//! Vida de los aliados en combate: tus compañeros del cooperativo y tus invocaciones, debajo de la tira
//! de bendiciones (arriba a la izquierda). El nombre y, debajo, una barra fina por cada uno; los
//! espíritus de una misma ceniza, en fila.

use imgui::{ImColor32, Ui};

use crate::Fonts;

#[derive(Clone, Debug, Default)]
pub struct Aliado {
    pub nombre: String,
    /// Vida (actual, máxima) de cada uno: la de un compañero, o la de cada espíritu de una ceniza.
    pub vidas: Vec<(i32, i32)>,
    /// Compañero del cooperativo (nombre en el dorado de los fantasmas aliados) o invocación.
    pub companero: bool,
}

const FANTASMA: [u8; 3] = [214, 186, 120];
const ESPIRITU: [u8; 3] = [168, 190, 204];
const VIDA: [u8; 3] = [150, 38, 32];
const FONDO: [u8; 3] = [16, 14, 12];
const BORDE: [u8; 3] = [90, 80, 64];

fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn draw(ui: &Ui, aliados: &[Aliado], fonts: &Fonts, alpha: f32) {
    if aliados.is_empty() || alpha <= 0.0 {
        return;
    }
    let [_, h] = ui.io().display_size;
    let s = h / 1080.0;
    let dl = ui.get_background_draw_list();
    // Alineado con la tira (empieza a 0,135 de la altura, centrada a 0,165) y por debajo de ella.
    let x0 = h * 0.135;
    let ancho = 230.0 * s;
    let (alto, hueco) = (6.0 * s, 6.0 * s);
    let mut y = h * 0.165 + 34.0 * s;
    for a in aliados {
        let vivo = a.vidas.iter().any(|v| v.0 > 0);
        let color = if a.companero { FANTASMA } else { ESPIRITU };
        {
            let _f = ui.push_font(fonts.caption);
            dl.add_text([x0, y], c(color, if vivo { alpha } else { 0.45 * alpha }), &a.nombre);
        }
        let yb = y + 22.0 * s;
        let n = a.vidas.len().max(1) as f32;
        let w = (ancho - (n - 1.0) * hueco) / n;
        for (k, &(hp, max)) in a.vidas.iter().enumerate() {
            let x = x0 + k as f32 * (w + hueco);
            let p = if max > 0 { (hp.max(0) as f32 / max as f32).clamp(0.0, 1.0) } else { 0.0 };
            dl.add_rect([x, yb], [x + w, yb + alto], c(FONDO, 0.75 * alpha)).filled(true).build();
            if p > 0.0 {
                dl.add_rect([x, yb], [x + w * p, yb + alto], c(VIDA, alpha)).filled(true).build();
            }
            dl.add_rect([x, yb], [x + w, yb + alto], c(BORDE, 0.9 * alpha)).thickness(1.0).build();
        }
        y = yb + alto + 14.0 * s;
    }
}

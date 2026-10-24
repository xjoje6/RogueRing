//! Tira de bendiciones en combate: debajo de los bufos del juego (arriba a la izquierda), un icono
//! redondo por cada bendición que tiene algo que contar, sin texto. El anillo es el contador, la
//! recarga o lo que le queda a un bufo; los puntitos, las pilas; el destello, que acaba de saltar.

use imgui::{ImColor32, TextureId, Ui};

#[derive(Clone, Debug, Default)]
pub struct TiraItem {
    pub tex: Option<TextureId>,
    /// Anillo (0..1): contador, recarga o lo que le queda al bufo. None, sin anillo.
    pub anillo: Option<f32>,
    /// Lista para saltar o bufo en marcha: borde dorado y el icono a plena luz.
    pub lista: bool,
    /// Ya gastada (una vez por combate): apagada.
    pub gastada: bool,
    /// Pilas (actuales, máximo).
    pub pilas: Option<(u32, u32)>,
    /// Segundos desde la última vez que saltó.
    pub destello: f32,
}

const GOLD: [u8; 3] = [214, 186, 120];
const DIM: [u8; 3] = [150, 144, 130];
const TRACK: [u8; 3] = [90, 80, 64];

fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Destello al saltar: dura esto.
const DESTELLO: f32 = 0.4;

pub fn draw(ui: &Ui, items: &[TiraItem], alpha: f32) {
    if items.is_empty() || alpha <= 0.0 {
        return;
    }
    let [_, h] = ui.io().display_size;
    let s = h / 1080.0;
    let dl = ui.get_background_draw_list();
    // Medido en el HUD del juego: la barra de vida empieza a 0,135 de la altura y los bufos acaban
    // hacia 0,135 de alto; la tira va justo debajo.
    let r = 15.0 * s;
    let x0 = h * 0.135 + r;
    let cy = h * 0.165;
    let paso = 2.0 * r + 12.0 * s;
    for (i, it) in items.iter().enumerate() {
        let cx = x0 + i as f32 * paso;
        let d = (it.destello / DESTELLO).clamp(0.0, 1.0);
        let pop = if it.destello < DESTELLO { 1.0 + 0.22 * (1.0 - d) } else { 1.0 };
        let rr = r * pop;
        let luz = if it.gastada {
            0.3
        } else if it.lista || it.destello < DESTELLO {
            1.0
        } else {
            0.62
        };
        dl.add_circle([cx, cy], rr, ImColor32::from_rgba(16, 14, 12, (0.72 * alpha * 255.0) as u8)).filled(true).num_segments(32).build();
        if let Some(t) = it.tex {
            let k = rr * 0.86;
            let g = (255.0 * luz) as u8;
            dl.add_image(t, [cx - k, cy - k], [cx + k, cy + k]).col(ImColor32::from_rgba(g, g, g, (alpha * 255.0) as u8)).build();
        }
        // Aro de fondo y anillo de progreso (desde arriba, en el sentido de las agujas).
        dl.add_circle([cx, cy], rr, c(if it.lista { GOLD } else { TRACK }, 0.9 * alpha)).thickness(1.5 * s).num_segments(32).build();
        if let Some(p) = it.anillo.filter(|p| *p > 0.0 && !it.lista) {
            let n = (48.0 * p).ceil().max(2.0) as usize;
            let pts: Vec<[f32; 2]> = (0..=n)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * p * k as f32 / n as f32;
                    [cx + rr * a.cos(), cy + rr * a.sin()]
                })
                .collect();
            dl.add_polyline(pts, c(DIM, alpha)).thickness(2.2 * s).build();
        }
        // Al saltar, un aro dorado que se abre y se apaga.
        if it.destello < DESTELLO {
            dl.add_circle([cx, cy], rr * (1.0 + 0.6 * d), c(GOLD, (1.0 - d) * alpha)).thickness(2.0 * s).num_segments(32).build();
        }
        // Pilas: puntitos debajo (hasta 10); con más, una barrita.
        if let Some((n, max)) = it.pilas.filter(|p| p.1 > 0) {
            let y = cy + rr + 6.0 * s;
            if max <= 10 {
                let (q, g) = (3.0 * s, 2.0 * s);
                let ancho = max as f32 * q + (max as f32 - 1.0) * g;
                for k in 0..max {
                    let x = cx - ancho / 2.0 + k as f32 * (q + g);
                    dl.add_rect([x, y], [x + q, y + q], c(if k < n { GOLD } else { TRACK }, alpha)).filled(true).build();
                }
            } else {
                let ancho = 2.0 * r;
                dl.add_rect([cx - r, y], [cx + r, y + 3.0 * s], c(TRACK, alpha)).filled(true).build();
                dl.add_rect([cx - r, y], [cx - r + ancho * n as f32 / max as f32, y + 3.0 * s], c(GOLD, alpha)).filled(true).build();
            }
        }
    }
}

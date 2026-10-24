//! Bendiciones a la vista: aviso al ganar una (a la derecha, unos segundos), lo que llevas (desde el
//! mapa con Y), el compendio y la columna del mapa. Lo que llevas y el compendio siguen al
//! inventario del juego: rejilla de casillas a la izquierda y la ficha de la elegida a la derecha.

use imgui::{ImColor32, TextureId, Ui};
use rogue_core::idioma::tf;

use crate::Fonts;
use crate::adorno;
use crate::cartas::wrap;
use crate::estilo::{self, APAGADA, FONDO, ORO, SANGRE, TENUE, TINTA, c};
use crate::mapa::MapTextures;

pub struct BlessingView {
    pub icon: Option<TextureId>,
    pub nombre: String,
    pub texto: String,
    pub maldita: bool,
}

/// Avisos de bendiciones recién ganadas. `age` en segundos: entra en 0,25 s, dura ~4 s y se va.
/// `en_mapa`: en la columna de la izquierda del mapa, de abajo arriba sobre el nodo elegido, sin tapar
/// el mapa (19-10). Si no, a la derecha.
pub fn draw_toasts(ui: &Ui, items: &[(BlessingView, f32)], tex: &adorno::MenuTex, fonts: &Fonts, en_mapa: bool) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_foreground_draw_list();
    let tw = if en_mapa { crate::mapa::ancho_columna(&lz) - lz.x(90.0) - lz.d(30.0) } else { lz.d(560.0) };
    let mut y = lz.h * 0.3;
    let mut base = lz.h - lz.d(230.0);
    for (b, age) in items {
        let a = (age / 0.25).min(1.0).min(((4.5 - age) / 0.5).max(0.0));
        if a <= 0.0 {
            continue;
        }
        // Entra deslizándose desde la derecha (en el mapa, desde la izquierda). La caja crece con el texto.
        let x = if en_mapa { lz.x(90.0) - (1.0 - a) * lz.d(60.0) } else { lz.w - tw - lz.d(48.0) + (1.0 - a) * lz.d(60.0) };
        let pad = lz.d(18.0);
        let marco = lz.d(76.0);
        let tx = x + pad * 2.0 + estilo::ancho_marco(marco);
        let lineas = {
            let _f = ui.push_font(fonts.nota);
            wrap(ui, &b.texto, x + tw - pad - tx).len().min(3)
        };
        let th = (pad + lz.d(42.0) + lineas as f32 * lz.d(30.0) + pad).max(marco + 2.0 * pad);
        if en_mapa {
            base -= th;
            y = base;
            base -= lz.d(14.0);
        }
        dl.add_rect([x, y], [x + tw, y + th], c(FONDO, if en_mapa { 1.0 } else { 0.94 } * a)).filled(true).build();
        dl.add_rect([x, y], [x + lz.d(3.0), y + th], c(if b.maldita { SANGRE } else { ORO }, a)).filled(true).build();
        estilo::marco_icono(&dl, tex, b.icon, x + pad, y + (th - marco) / 2.0, marco, 1.0, a);
        estilo::texto(ui, &dl, fonts.texto, [tx, y + pad - lz.d(4.0)], c(if b.maldita { SANGRE } else { TINTA }, a), &b.nombre);
        estilo::parrafo(ui, &dl, fonts.nota, [tx, y + pad + lz.d(38.0)], x + tw - pad - tx, lz.d(30.0), c(APAGADA, a), &b.texto, 3);
        y += th + lz.d(14.0);
    }
}

/// Columnas y filas a la vista de la rejilla de lo que llevas (con más, baja con la selección).
pub const LLEVAS_COLS: usize = 5;
pub const LLEVAS_FILAS: usize = 5;
/// Columnas de la rejilla del compendio (la navegación arriba/abajo salta de fila en fila).
pub const COMPENDIO_COLS: usize = 10;
const COMPENDIO_FILAS: usize = 7;

/// Una casilla de la rejilla.
struct Casilla {
    icon: Option<TextureId>,
    silueta: bool,
    maldita: bool,
}

/// Rejilla de casillas desde (x, y): la fila de la elegida siempre a la vista. Rellena con casillas
/// vacías hasta `minimo`.
#[allow(clippy::too_many_arguments)]
fn rejilla(dl: &imgui::DrawListMut, tex: &MapTextures, lz: &estilo::Lienzo, x: f32, y: f32, lado: f32, cols: usize, filas: usize, items: &[Casilla], sel: usize, minimo: usize, t: f32) {
    let gap = lz.d(12.0);
    let primera = (sel / cols).saturating_sub(filas - 1);
    let total = items.len().max(minimo).div_ceil(cols) * cols;
    for i in primera * cols..total.min((primera + filas) * cols) {
        let (col, fila) = (i % cols, i / cols - primera);
        let (cx, cy) = (x + col as f32 * (lado + gap), y + fila as f32 * (lado + gap));
        let a = adorno::ease_out((t - 0.01 * i as f32) / 0.3);
        let it = items.get(i);
        let col_icono = match it {
            Some(c) if c.silueta => ImColor32::from_rgba(0, 0, 0, (230.0 * a) as u8),
            _ => adorno::white(a),
        };
        estilo::casilla(dl, &tex.menu, it.and_then(|c| c.icon), cx, cy, lado, col_icono, a);
        if it.is_some_and(|c| c.maldita) {
            let m = lado * 0.18;
            dl.add_line([cx + m, cy + lado - m], [cx + lado - m, cy + lado - m], c(SANGRE, a)).thickness(lz.d(3.0)).build();
        }
        if i == sel && it.is_some() {
            // El marco de objeto encima de la elegida, como la selección del inventario, ajustado a la
            // parte visible de la casilla (la textura trae margen: x 36..278 e y 31..285 de 314).
            let k = lado / 314.0;
            let (vx0, vy0, vx1, vy1) = (cx + 36.0 * k, cy + 31.0 * k, cx + 278.0 * k, cy + 285.0 * k);
            let m = (vx1 - vx0) * 0.07;
            let alto = vy1 - vy0 + 2.0 * m;
            let ancho = estilo::ancho_marco(alto);
            let mx = (vx0 + vx1) / 2.0 - ancho / 2.0;
            estilo::marco_icono(dl, &tex.menu, None, mx, vy0 - m, alto, 1.0, a);
        }
    }
}

/// Lo que llevas: todas las bendiciones y maldiciones de la run; la cruceta elige y a la derecha sale
/// la ficha de la elegida.
pub fn draw_list(ui: &Ui, items: &[BlessingView], sel: usize, t_in: f32, tex: &MapTextures, fonts: &Fonts) {
    let t = rogue_core::idioma::t;
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(t_in, 0);
    estilo::fondo(&dl, &lz, 1.0);
    let malditas = items.iter().filter(|b| b.maldita).count();
    let buenas = items.len() - malditas;
    let sub = match malditas {
        0 => tf("{} blessings", &[&buenas]),
        1 => tf("{} blessings, 1 curse", &[&buenas]),
        n => tf("{} blessings, {} curses", &[&buenas, &n]),
    };
    estilo::cabecera(ui, &dl, fonts, &lz, t("What you carry"), &sub, "", a0);
    if items.is_empty() {
        estilo::texto(ui, &dl, fonts.texto, lz.p(120.0, 250.0), c(TENUE, a0), t("None yet."));
    }
    let sel = sel.min(items.len().saturating_sub(1));
    let casillas: Vec<Casilla> = items.iter().map(|b| Casilla { icon: b.icon, silueta: false, maldita: b.maldita }).collect();
    if !items.is_empty() {
        rejilla(&dl, tex, &lz, lz.x(105.0), lz.y(225.0), lz.d(129.0), LLEVAS_COLS, LLEVAS_FILAS, &casillas, sel, 10, t_in);
    }
    estilo::separador(&dl, &lz, lz.x(930.0), a0);
    if let Some(b) = items.get(sel) {
        let f = estilo::Ficha {
            icon: b.icon,
            silueta: false,
            nombre: &b.nombre,
            linea: if b.maldita { t("Curse") } else { t("Blessing") },
            linea_col: if b.maldita { SANGRE } else { APAGADA },
            texto: &b.texto,
            contra: "",
            nota: None,
            pie: "",
            apagada: false,
            doble: false,
        };
        estilo::ficha(ui, &dl, fonts, &tex.menu, &lz, lz.x(1035.0), lz.y(150.0), lz.d(780.0), &f, a0);
    }
    estilo::ayuda(ui, &dl, fonts, &lz, &[("B", "Back")], a0);
}

/// Una entrada del compendio.
pub struct CompendioItem {
    pub icon: Option<TextureId>,
    pub nombre: String,
    pub texto: String,
    /// "Común", "Poco común", "Rara" o "Maldición".
    pub rareza: String,
    /// "Caos", un personaje o vacío.
    pub etiqueta: String,
    pub vista: bool,
}

/// Compendio: todas las bendiciones en rejilla (las no descubiertas, en silueta) y la ficha de la
/// elegida a la derecha.
pub fn draw_compendio(ui: &Ui, items: &[CompendioItem], sel: usize, t: f32, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(t, 0);
    estilo::fondo(&dl, &lz, 1.0);
    let vistas = items.iter().filter(|i| i.vista).count();
    estilo::cabecera(ui, &dl, fonts, &lz, rogue_core::idioma::t("Compendium"), &tf("{} of {} found", &[&vistas, &items.len()]), "", a0);
    let casillas: Vec<Casilla> = items.iter().map(|i| Casilla { icon: i.icon, silueta: !i.vista, maldita: false }).collect();
    rejilla(&dl, tex, &lz, lz.x(105.0), lz.y(225.0), lz.d(66.0), COMPENDIO_COLS, COMPENDIO_FILAS, &casillas, sel, 0, t);
    estilo::separador(&dl, &lz, lz.x(930.0), a0);
    if let Some(it) = items.get(sel) {
        let linea = if it.etiqueta.is_empty() { it.rareza.clone() } else { format!("{}  ·  {}", it.rareza, it.etiqueta) };
        let f = estilo::Ficha {
            icon: it.icon,
            silueta: !it.vista,
            nombre: if it.vista { &it.nombre } else { "???" },
            linea: &linea,
            linea_col: ORO,
            texto: if it.vista { &it.texto } else { rogue_core::idioma::t("You haven't found this one yet.") },
            contra: "",
            nota: None,
            pie: "",
            apagada: false,
            doble: false,
        };
        estilo::ficha(ui, &dl, fonts, &tex.menu, &lz, lz.x(1035.0), lz.y(150.0), lz.d(780.0), &f, a0);
    }
    estilo::ayuda(ui, &dl, fonts, &lz, &[("B", "Back")], a0);
}

/// Un tamaño de la columna del mapa: alto del marco, letra del nombre y de la descripción, alto de
/// línea de la descripción y cuántas líneas de descripción caben.
struct Talla {
    marco: f32,
    nombre: imgui::FontId,
    alto_nombre: f32,
    desc: imgui::FontId,
    alto_desc: f32,
    lineas: usize,
}

/// Columna de lo que llevas en el mapa, dentro de `area` ([x0, y0, x1, y1]): cada bendición con su
/// icono enmarcado, su nombre y su descripción debajo. Si no caben, todo se va encogiendo (marco,
/// letra y líneas de descripción); si ni en la talla más pequeña caben, las que quepan y cuántas
/// faltan, que se ven todas con Y.
pub fn draw_panel(ui: &Ui, dl: &imgui::DrawListMut, items: &[BlessingView], area: [f32; 4], tex: &MapTextures, fonts: &Fonts, a: f32) {
    if items.is_empty() {
        return;
    }
    let lz = estilo::Lienzo::new(ui);
    let [x0, y0, x1, y1] = area;
    let buenas = items.iter().filter(|b| !b.maldita).count();
    estilo::texto(ui, dl, fonts.nota, [x0, y0], c(TENUE, a), &tf("Blessings ({})", &[&buenas]));
    let top = y0 + lz.d(46.0);
    let tallas = [
        Talla { marco: lz.d(60.0), nombre: fonts.texto, alto_nombre: lz.d(34.0), desc: fonts.nota, alto_desc: lz.d(27.0), lineas: 3 },
        Talla { marco: lz.d(52.0), nombre: fonts.texto, alto_nombre: lz.d(33.0), desc: fonts.body, alto_desc: lz.d(25.0), lineas: 2 },
        Talla { marco: lz.d(44.0), nombre: fonts.body, alto_nombre: lz.d(27.0), desc: fonts.caption, alto_desc: lz.d(21.0), lineas: 2 },
        Talla { marco: lz.d(36.0), nombre: fonts.body, alto_nombre: lz.d(26.0), desc: fonts.caption, alto_desc: lz.d(21.0), lineas: 1 },
        Talla { marco: lz.d(30.0), nombre: fonts.caption, alto_nombre: lz.d(22.0), desc: fonts.caption, alto_desc: lz.d(20.0), lineas: 1 },
    ];
    // Líneas de descripción de cada una con una talla (la última recortada con puntos si sobra).
    let medir = |t: &Talla| -> Vec<Vec<String>> {
        let tx = x0 + estilo::ancho_marco(t.marco) + lz.d(18.0);
        let todas: Vec<Vec<String>> = {
            let _f = ui.push_font(t.desc);
            items.iter().map(|b| wrap(ui, &b.texto, x1 - tx)).collect()
        };
        todas
            .into_iter()
            .map(|mut l| {
                if l.len() > t.lineas {
                    let resto = l[t.lineas - 1..].join(" ");
                    l.truncate(t.lineas - 1);
                    l.push(estilo::ajustar(ui, t.desc, &resto, x1 - tx));
                }
                l
            })
            .collect()
    };
    let alto_fila = |t: &Talla, n: usize| (t.alto_nombre + t.alto_desc * n as f32).max(t.marco) + lz.d(12.0);
    let disponible = y1 - top;
    let mut elegida = None;
    for t in &tallas {
        let lineas = medir(t);
        let total: f32 = lineas.iter().map(|l| alto_fila(t, l.len())).sum();
        if total <= disponible {
            elegida = Some((t, lineas));
            break;
        }
    }
    let aviso = lz.d(36.0);
    let (t, lineas, mostrar) = match elegida {
        Some((t, l)) => (t, l, items.len()),
        None => {
            // Ni en la más pequeña: las que quepan dejando sitio al aviso.
            let t = &tallas[tallas.len() - 1];
            let l = medir(t);
            let mut acc = 0.0;
            let mut n = 0;
            for f in &l {
                let h = alto_fila(t, f.len());
                if acc + h > disponible - aviso {
                    break;
                }
                acc += h;
                n += 1;
            }
            (t, l, n)
        }
    };
    let tx = x0 + estilo::ancho_marco(t.marco) + lz.d(18.0);
    let mut y = top;
    for (b, l) in items.iter().zip(&lineas).take(mostrar) {
        estilo::marco_icono(dl, &tex.menu, b.icon, x0, y, t.marco, 1.0, a);
        let n = estilo::ajustar(ui, t.nombre, &b.nombre, x1 - tx);
        estilo::texto(ui, dl, t.nombre, [tx, y - lz.d(2.0)], c(if b.maldita { SANGRE } else { TINTA }, a), &n);
        let mut yy = y + t.alto_nombre;
        for linea in l {
            estilo::texto(ui, dl, t.desc, [tx, yy], c(APAGADA, a), linea);
            yy += t.alto_desc;
        }
        y += alto_fila(t, l.len());
    }
    if mostrar < items.len() {
        estilo::texto(ui, dl, fonts.nota, [x0, y + lz.d(4.0)], c(ORO, a), &tf("and {} more. Y to see them all.", &[&(items.len() - mostrar)]));
    }
}

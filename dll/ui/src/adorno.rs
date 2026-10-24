//! Piezas de los menús de ER (extract/ui/menu/, tools/recursos_menu.py) y ayudas para dibujarlas
//! estiradas sin deformar sus adornos, más las curvas de las animaciones de entrada.

use imgui::{ImColor32, TextureId};

pub type Tex = (TextureId, [f32; 2]);

#[derive(Clone, Copy, Default)]
pub struct MenuTex {
    /// Panel oscuro con esquinas doradas (fondo de carta).
    pub panel: Option<Tex>,
    /// Resalte dorado con chispas de la opción elegida.
    pub cursor: Option<Tex>,
    /// Brillo naranja alrededor de lo elegido.
    pub brillo: Option<Tex>,
    /// Brillo de la pantalla de título (subrayado de títulos).
    pub subrayado: Option<Tex>,
    /// Remate dorado de la barra de jefe del juego.
    pub barra_jefe: Option<Tex>,
    /// Piezas de las barras de vida del juego: fondo, relleno (gris, se tiñe), filete dorado y tope.
    pub barra_base: Option<Tex>,
    pub barra_relleno: Option<Tex>,
    pub barra_filete: Option<Tex>,
    pub barra_borde: Option<Tex>,
    /// Icono de la pantalla de carga del juego.
    pub carga_icono: Option<Tex>,
    /// Iconos de los estados del juego, en el orden del módulo de resistencias (veneno, podredumbre,
    /// sangrado, plaga, congelación, sueño, locura).
    pub estados: [Option<Tex>; 7],
    /// Marco de casilla de objeto (listas de la tienda y del inventario).
    pub marco_objeto: Option<Tex>,
    /// Marco octogonal del icono grande de la ficha.
    pub marco_ficha: Option<Tex>,
    /// Filete ornamental bajo el nombre en la ficha.
    pub filete: Option<Tex>,
    /// Casilla vacía del inventario.
    pub casilla: Option<Tex>,
    /// Cursor del mapa del mundo, el marcador de jugador y el emblema azul.
    pub mapa_cursor: Option<Tex>,
    pub mapa_coop: Option<Tex>,
    pub mapa_amigo: Option<Tex>,
    /// Banda negra del «HAS MUERTO».
    pub banda: Option<Tex>,
    /// Logo del mod.
    pub logo: Option<Tex>,
}

pub const MENU_N: usize = 26;

/// Archivos de extract/ui/menu/ en el orden de los campos de `MenuTex`.
pub const MENU_FILES: [&str; MENU_N] = [
    "panel",
    "cursor",
    "brillo",
    "subrayado",
    "barra_jefe",
    "barra_base",
    "barra_relleno",
    "barra_filete",
    "barra_borde",
    "carga_icono",
    "estado_veneno",
    "estado_podredumbre",
    "estado_sangrado",
    "estado_plaga",
    "estado_congelacion",
    "estado_sueno",
    "estado_locura",
    "marco_objeto",
    "marco_ficha",
    "filete",
    "casilla",
    "mapa_cursor",
    "mapa_coop",
    "mapa_amigo",
    "banda",
    "logo",
];

impl MenuTex {
    pub fn from_slots(t: [Option<Tex>; MENU_N]) -> Self {
        MenuTex {
            panel: t[0],
            cursor: t[1],
            brillo: t[2],
            subrayado: t[3],
            barra_jefe: t[4],
            barra_base: t[5],
            barra_relleno: t[6],
            barra_filete: t[7],
            barra_borde: t[8],
            carga_icono: t[9],
            estados: [t[10], t[11], t[12], t[13], t[14], t[15], t[16]],
            marco_objeto: t[17],
            marco_ficha: t[18],
            filete: t[19],
            casilla: t[20],
            mapa_cursor: t[21],
            mapa_coop: t[22],
            mapa_amigo: t[23],
            banda: t[24],
            logo: t[25],
        }
    }
}

/// Barra al estilo de las del juego, con sus piezas: fondo oscuro, `rastro` (tramo recién perdido,
/// hasta esa fracción) en oro, relleno teñido de `color` hasta `f`, tope al final del relleno y
/// filetes dorados arriba y abajo con su redoble en los dos extremos. Sin los remates de jefe en los
/// extremos: se confunden con la propia barra. Sin las piezas, rectángulos lisos.
#[allow(clippy::too_many_arguments)]
pub fn barra(dl: &imgui::DrawListMut, t: &MenuTex, x0: f32, y0: f32, bw: f32, bh: f32, f: f32, rastro: f32, color: [u8; 3], a: f32, s: f32) {
    let col = |rgb: [u8; 3], al: f32| ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (al.clamp(0.0, 1.0) * 255.0) as u8);
    let f = f.clamp(0.0, 1.0);
    let rastro = rastro.clamp(f, 1.0);
    // Fondo: la banda opaca de la pieza (filas 8-35 de 44, columnas 17-2907).
    match t.barra_base {
        Some((tb, [tw, th])) => {
            let m = 3.0 * s;
            dl.add_image(tb, [x0 - m, y0 - m], [x0 + bw + m, y0 + bh + m]).uv_min([17.0 / tw, 8.0 / th]).uv_max([2907.0 / tw, 35.0 / th]).col(white(a)).build();
        }
        None => dl.add_rect([x0, y0], [x0 + bw, y0 + bh], ImColor32::from_rgba(8, 7, 6, (200.0 * a) as u8)).filled(true).build(),
    }
    let tramo = |desde: f32, hasta: f32, rgb: [u8; 3], al: f32| {
        if hasta <= desde {
            return;
        }
        match t.barra_relleno {
            // La textura del relleno, recortada al tramo (no estirada entera en un trozo).
            Some((tr, _)) => {
                dl.add_image(tr, [x0 + bw * desde, y0], [x0 + bw * hasta, y0 + bh])
                    .uv_min([desde, 1.0 / 24.0])
                    .uv_max([hasta, 22.0 / 24.0])
                    .col(col(rgb, al))
                    .build();
            }
            None => dl.add_rect([x0 + bw * desde, y0], [x0 + bw * hasta, y0 + bh], col(rgb, al)).filled(true).build(),
        }
    };
    tramo(f, rastro, [214, 186, 120], 0.85 * a);
    tramo(0.0, f, color, a);
    if let (Some((tb, [tw, th])), true) = (t.barra_borde, f > 0.002) {
        let eh = bh * 1.5;
        let ew = eh * tw / th;
        let x = x0 + bw * f;
        dl.add_image(tb, [x - ew / 2.0, y0 + bh / 2.0 - eh / 2.0], [x + ew / 2.0, y0 + bh / 2.0 + eh / 2.0]).col(white(0.8 * a)).build();
    }
    // Filetes dorados, abajo y arriba en espejo, con el redoble del borde (el gancho del final de la
    // pieza) en los dos extremos: la mitad derecha de la pieza a la derecha y, volteada, a la izquierda.
    if let Some((tf, [tw, th])) = t.barra_filete {
        let fh = 6.0 * s;
        let (v0, v1) = (11.0 / th, 24.0 / th);
        let (um, u1) = (1465.0 / tw, 2930.0 / tw);
        let (xa, xm, xb) = (x0 - 4.0 * s, x0 + bw / 2.0, x0 + bw + 4.0 * s);
        for (ya, yb, va, vb) in [(y0 + bh + 1.0 * s, y0 + bh + 1.0 * s + fh, v0, v1), (y0 - 1.0 * s - fh, y0 - 1.0 * s, v1, v0)] {
            dl.add_image(tf, [xm, ya], [xb, yb]).uv_min([um, va]).uv_max([u1, vb]).col(white(a)).build();
            dl.add_image(tf, [xa, ya], [xm, yb]).uv_min([u1, va]).uv_max([um, vb]).col(white(a)).build();
        }
    }
}

/// Tramo de la textura (en píxeles) y si se estira (true) o mantiene su tamaño escalado (false).
pub type Seg = (f32, f32, bool);

fn layout(segs: &[Seg], start: f32, len: f32, sc: f32) -> Vec<(f32, f32, f32, f32)> {
    let fixed: f32 = segs.iter().filter(|s| !s.2).map(|s| (s.1 - s.0) * sc).sum();
    let stretch_tex: f32 = segs.iter().filter(|s| s.2).map(|s| s.1 - s.0).sum();
    // Si no cabe lo fijo, se reduce todo en proporción.
    let k = if fixed > len { len / fixed } else { 1.0 };
    let free = (len - fixed * k).max(0.0);
    let mut p = start;
    segs.iter()
        .map(|&(a, b, st)| {
            let size = if st { if stretch_tex > 0.0 { free * (b - a) / stretch_tex } else { 0.0 } } else { (b - a) * sc * k };
            let r = (p, p + size, a, b);
            p += size;
            r
        })
        .collect()
}

/// Dibuja `t` en `rect` troceada en rejilla: los tramos fijos conservan su tamaño (× `sc`) y los
/// demás se estiran. Así las esquinas y los adornos centrales no se deforman.
pub fn grid(dl: &imgui::DrawListMut, t: Tex, rect: [f32; 4], xs: &[Seg], ys: &[Seg], sc: f32, col: ImColor32) {
    let (id, [tw, th]) = t;
    let cols = layout(xs, rect[0], rect[2] - rect[0], sc);
    let rows = layout(ys, rect[1], rect[3] - rect[1], sc);
    for &(y0, y1, v0, v1) in &rows {
        for &(x0, x1, u0, u1) in &cols {
            if x1 - x0 < 0.5 || y1 - y0 < 0.5 {
                continue;
            }
            dl.add_image(id, [x0, y0], [x1, y1]).uv_min([u0 / tw, v0 / th]).uv_max([u1 / tw, v1 / th]).col(col).build();
        }
    }
}

/// Resalte de opción elegida: raya dorada arriba y abajo con la flecha centrada.
pub fn cursor(dl: &imgui::DrawListMut, t: Tex, rect: [f32; 4], s: f32, alpha: f32) {
    let xs = [(0.0, 24.0, false), (24.0, 130.0, true), (130.0, 174.0, false), (174.0, 280.0, true), (280.0, 304.0, false)];
    let ys = [(0.0, 30.0, true), (30.0, 50.0, false), (50.0, 180.0, true), (180.0, 214.0, false), (214.0, 238.0, true)];
    grid(dl, t, rect, &xs, &ys, 0.7 * s, white(alpha));
}

/// Subrayado luminoso centrado en `cx`, a la altura `y`.
pub fn subrayado(dl: &imgui::DrawListMut, t: Tex, cx: f32, y: f32, width: f32, s: f32, alpha: f32) {
    let h = 58.0 * s;
    dl.add_image(t.0, [cx - width / 2.0, y - h / 2.0], [cx + width / 2.0, y + h / 2.0]).col(white(alpha)).build();
}

pub fn white(a: f32) -> ImColor32 {
    ImColor32::from_rgba(255, 255, 255, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// 0..1 → 0..1 con frenada al final.
pub fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

/// Aparición escalonada: elemento `i`, `t` segundos desde que se abrió la pantalla.
pub fn appear(t: f32, i: usize) -> f32 {
    ease_out((t - 0.06 * i as f32) / 0.32)
}

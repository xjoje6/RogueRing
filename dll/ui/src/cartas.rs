//! Pantalla genérica de elegir: recompensa, tesoro, mercader, gracia, personaje... comparten la misma
//! forma, la de la tienda del juego: lista a la izquierda (marco de objeto, barra de selección) y la
//! ficha de lo elegido a la derecha (marco octogonal, filete y texto).

use imgui::{TextureId, Ui};
use rogue_core::idioma::t;
use rogue_core::rewards::Rarity;

use crate::Fonts;
use crate::adorno;
use crate::estilo::{self, APAGADA, ORO, SANGRE, TENUE, c};
use crate::mapa::MapTextures;

pub struct Card {
    /// Categoría en versalitas arriba de la carta ("ARMA", "RELIQUIA"...).
    pub cat: String,
    pub nombre: String,
    /// Subtipo o texto corto bajo el nombre.
    pub tipo: String,
    pub rareza: Option<Rarity>,
    pub icon: Option<TextureId>,
    /// Pie de la carta: requisitos o precio.
    pub pie: String,
    /// Apagada: ya comprada o no alcanza el dinero.
    pub apagada: bool,
    /// Lo malo que trae (bendiciones míticas): en rojo, debajo del texto.
    pub contra: String,
    /// Nota que se explica debajo de las cartas cuando esta es la elegida: (título, texto). La
    /// maldición del trato inicial, que en la carta solo cabe por su nombre.
    pub nota: Option<(String, String)>,
    /// Arma doble: el icono sale dos veces.
    pub doble: bool,
}

pub struct CardScreen {
    pub cabecera: String,
    pub titulo: String,
    pub linea: String,
    pub instruccion: String,
    pub cards: Vec<Card>,
    /// 0..cards.len() = carta; cards.len() = opción de salir (si hay).
    pub selected: usize,
    pub salir: Option<String>,
    pub ayuda: Vec<(String, String)>,
    /// Carta aparte a la izquierda (el objeto nuevo en "Sin hueco"); no se selecciona.
    pub destacada: Option<Card>,
    /// Arte de fondo en lugar del mapa (textura y tamaño): la tienda de Kalé.
    pub fondo: Option<(TextureId, [f32; 2])>,
    /// Segundos desde que se abrió la pantalla (entrada escalonada) y desde el último cambio de selección.
    pub t: f32,
    pub t_sel: f32,
}

/// Míticas: un lila pálido, por encima del rojo de las raras.
const MYTH: [u8; 3] = [184, 158, 232];

pub fn rarity(r: Rarity) -> (&'static str, [u8; 3]) {
    match r {
        Rarity::Comun => (t("Common"), APAGADA),
        Rarity::PocoComun => (t("Uncommon"), ORO),
        Rarity::Rara => (t("Rare"), SANGRE),
        Rarity::Mitica => (t("Mythic"), MYTH),
    }
}

/// Ancho del texto de la ficha y del nombre (a la derecha del marco) a 1080p.
pub const ANCHO: f32 = 780.0;
const ANCHO_NOMBRE: f32 = ANCHO - 330.0 - 40.0;
/// Líneas que se dibujan del nombre, del texto y del precio (lo que no cabe, no se ve).
pub const LINEAS: [usize; 3] = [2, 6, 3];

/// Líneas que ocupan el nombre, el texto y el precio en la ficha a 1080p (fuentes de `load_fonts`
/// con escala 1). Para comprobar que los textos caben en `LINEAS`.
pub fn lineas(ui: &Ui, fonts: &Fonts, nombre: &str, texto: &str, contra: &str) -> [usize; 3] {
    let medir = |f, t: &str, w: f32| {
        let _f = ui.push_font(f);
        wrap(ui, t, w).len()
    };
    [medir(fonts.grande, nombre, ANCHO_NOMBRE), medir(fonts.texto, texto, ANCHO), medir(fonts.texto, contra, ANCHO)]
}

/// Parte un texto en líneas que caben en `max_w` con la fuente actual.
pub(crate) fn wrap(ui: &Ui, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = vec![];
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let probe = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if ui.calc_text_size(&probe)[0] > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = probe;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// «BENDICIÓN» → «Bendición».
pub(crate) fn frase(s: &str) -> String {
    let low = s.to_lowercase();
    let mut cs = low.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().chain(cs).collect(),
        None => String::new(),
    }
}

/// Línea corta de la fila: el precio o el pie si lo hay; si no, la rareza.
fn sub_de(card: &Card) -> (String, [u8; 3]) {
    if !card.pie.is_empty() {
        let col = if card.nota.is_some() {
            SANGRE
        } else if card.apagada {
            TENUE
        } else {
            ORO
        };
        return (card.pie.clone(), col);
    }
    match card.rareza {
        Some(r) => {
            let (l, col) = rarity(r);
            (l.to_string(), col)
        }
        None => (String::new(), TENUE),
    }
}

/// Línea de la ficha bajo el filete: «Bendición · Común», «Arma · Rara», o el lema del personaje.
fn linea_de(card: &Card) -> (String, [u8; 3]) {
    let cat = frase(&card.cat);
    match card.rareza {
        Some(r) => {
            let (l, col) = rarity(r);
            // «Bendición mítica» ya lo dice: sin repetir la rareza.
            let ya = cat.to_lowercase().contains(&l.to_lowercase());
            (if cat.is_empty() { l.to_string() } else if ya { cat } else { format!("{cat}  ·  {l}") }, col)
        }
        None => (cat, APAGADA),
    }
}

pub fn draw(ui: &Ui, v: &CardScreen, tex: &MapTextures, fonts: &Fonts) {
    let lz = estilo::Lienzo::new(ui);
    let dl = ui.get_background_draw_list();
    let a0 = adorno::appear(v.t, 0);
    estilo::fondo(&dl, &lz, 1.0);
    // El arte propio de la pantalla (la tienda de Kalé), muy tapado: se adivina la escena.
    if let Some(f) = v.fondo {
        estilo::arte(&dl, &lz, f, 0.8, a0);
    }
    estilo::cabecera(ui, &dl, fonts, &lz, &v.titulo, &v.instruccion, &v.linea, a0);

    // Lo nuevo que no cabe («Sin hueco»): arriba de la lista, fuera de la selección.
    let x = lz.x(120.0);
    let ancho = lz.d(720.0);
    let mut top = lz.y(250.0);
    if let Some(d) = &v.destacada {
        estilo::texto(ui, &dl, fonts.nota, [x, top], c(ORO, a0), t("New"));
        let f = estilo::Fila { icon: d.icon, nombre: &d.nombre, sub: &d.tipo, sub_col: TENUE, sel: false, apagada: false, doble: d.doble };
        estilo::fila(ui, &dl, fonts, &tex.menu, &lz, x, top + lz.d(36.0), ancho, lz.d(90.0), &f, 0.0, a0);
        top += lz.d(170.0);
    }
    // Filas: con muchas (mercader) se aprietan.
    let n = v.cards.len() + usize::from(v.salir.is_some());
    let disponible = lz.y(930.0) - top;
    let paso = (disponible / n.max(1) as f32).min(lz.d(186.0));
    let alto = (paso - lz.d(30.0)).min(lz.d(estilo::FILA));
    let k_sel = adorno::ease_out(v.t_sel / 0.18);
    for (i, card) in v.cards.iter().enumerate() {
        let a = adorno::appear(v.t, i + 1);
        let (sub, sub_col) = sub_de(card);
        let f = estilo::Fila { icon: card.icon, nombre: &card.nombre, sub: &sub, sub_col, sel: v.selected == i, apagada: card.apagada, doble: card.doble };
        let y = top + paso * i as f32 + (paso - alto) / 2.0;
        estilo::fila(ui, &dl, fonts, &tex.menu, &lz, x + (1.0 - a) * lz.d(30.0), y, ancho, alto, &f, k_sel, a);
    }
    if let Some(sal) = &v.salir {
        let i = v.cards.len();
        let a = adorno::appear(v.t, i + 1);
        let y = top + paso * i as f32 + (paso - lz.d(44.0)) / 2.0;
        estilo::opcion(ui, &dl, fonts, &tex.menu, &lz, x, y, ancho, sal, "", v.selected == i, false, k_sel, a);
    }

    // Ficha de lo elegido, a la derecha de la raya.
    estilo::separador(&dl, &lz, lz.x(930.0), a0);
    if let Some(card) = v.cards.get(v.selected) {
        let k = adorno::ease_out(v.t_sel / 0.22) * a0;
        let (linea, linea_col) = linea_de(card);
        let nota = card.nota.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        let f = estilo::Ficha {
            icon: card.icon,
            silueta: false,
            nombre: &card.nombre,
            linea: &linea,
            linea_col,
            texto: &card.tipo,
            contra: &card.contra,
            nota,
            pie: if card.nota.is_some() { "" } else { &card.pie },
            apagada: card.apagada,
            doble: card.doble,
        };
        estilo::ficha(ui, &dl, fonts, &tex.menu, &lz, lz.x(1035.0) + (1.0 - k) * lz.d(16.0), lz.y(150.0), lz.d(ANCHO), &f, k);
    }
    let ayuda: Vec<(&str, &str)> = v.ayuda.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    estilo::ayuda(ui, &dl, fonts, &lz, &ayuda, a0);
}

//! Pantalla del mapa de la run, estilo Slay the Spire sobre el tramo del mapa de Elden Ring que toca
//! a cada acto (tools/map_background.py), lavado para que se lea la ruta. Columna oscura a la
//! izquierda con el acto y lo que llevas. Dibuja con la draw list de fondo de ImGui.

use imgui::{ImColor32, TextureId, Ui};
use rogue_core::idioma::{t, tf};
use rogue_core::map::NodeType;
use rogue_core::run::RunState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Visited,
    Current,
    Available,
    Locked,
}

pub struct NodeView {
    pub col: i32,
    pub row: i32,
    pub kind: NodeType,
    pub state: NodeState,
}

pub struct MapView {
    pub act_num: usize,
    pub act_name: String,
    pub seed: String,
    /// Ascensión de la run (0 = normal).
    pub ascension: u32,
    pub boss_name: String,
    pub columns: i32,
    pub rooms: i32,
    pub nodes: Vec<NodeView>,
    pub edges: Vec<((i32, i32), (i32, i32))>,
    pub path: Vec<(i32, i32)>,
    pub start: (i32, i32),
    pub boss: (i32, i32),
    pub selected: Option<(i32, i32)>,
    /// Segundos desde que se abrió el mapa (entrada animada).
    pub t_in: f32,
    /// Viales que quedan y máximo.
    pub viales: Option<(u32, u32)>,
    /// Nivel de mejora del arma y máximo.
    pub arma: Option<(u32, u32)>,
    /// Nombre del arma que llevas.
    pub arma_nombre: Option<String>,
    /// Invocador: la invocación que lleva.
    pub invocacion: Option<String>,
    /// Cooperativo: se vota el nodo. Votos de los demás (nodo, nombre), el mío y a quién se espera.
    pub coop: bool,
    pub votos: Vec<((i32, i32), String)>,
    pub mi_voto: Option<(i32, i32)>,
    pub espera: Vec<String>,
    /// Aún no se puede votar: alguno sigue con lo del nodo anterior (`espera` dice quién y qué hace).
    pub esperar: bool,
    /// Cursor de los compañeros que miran el mapa (nodo, nombre): se ve moverse.
    pub cursores: Vec<((i32, i32), String)>,
}

impl MapView {
    pub fn from_run(run: &RunState, act_name: &str, selected: Option<(i32, i32)>) -> Self {
        let m = &run.act.map;
        let avail = run.available();
        let mut nodes: Vec<NodeView> = m
            .nodes
            .iter()
            .map(|n| {
                let p = (n.col, n.row);
                let state = if p == run.position {
                    NodeState::Current
                } else if run.path.contains(&p) {
                    NodeState::Visited
                } else if avail.contains(&p) {
                    NodeState::Available
                } else {
                    NodeState::Locked
                };
                NodeView { col: n.col, row: n.row, kind: n.kind, state }
            })
            .collect();
        let boss_state = if run.position == m.boss {
            NodeState::Current
        } else if avail.contains(&m.boss) {
            NodeState::Available
        } else {
            NodeState::Locked
        };
        nodes.push(NodeView { col: m.boss.0, row: m.boss.1, kind: NodeType::Boss, state: boss_state });
        let mut edges = vec![];
        for n in &m.nodes {
            for ch in &n.children {
                edges.push(((n.col, n.row), *ch));
            }
            if n.row == 1 {
                edges.push((m.start, (n.col, n.row)));
            }
            if n.row == m.shape.rooms {
                edges.push(((n.col, n.row), m.boss));
            }
        }
        MapView {
            act_num: run.act_index + 1,
            act_name: act_name.to_string(),
            seed: run.seed.clone(),
            ascension: run.ascension,
            viales: None,
            arma: None,
            arma_nombre: None,
            invocacion: None,
            boss_name: run.act.boss.name.clone(),
            columns: m.shape.columns,
            rooms: m.shape.rooms,
            nodes,
            edges,
            path: run.path.clone(),
            start: m.start,
            boss: m.boss,
            selected: selected.or_else(|| avail.first().copied()),
            t_in: 10.0,
            coop: false,
            votos: vec![],
            mi_voto: None,
            espera: vec![],
            esperar: false,
            cursores: vec![],
        }
    }
}

pub struct MapTextures {
    pub background: TextureId,
    pub background_size: [f32; 2],
    /// Iconos del mapa de ER por nodo, en el orden de `NODE_ICONS` (si faltan, se dibuja la letra).
    pub nodes: [Option<TextureId>; 8],
    /// Piezas de los menús del juego (paneles, resaltes).
    pub menu: crate::adorno::MenuTex,
}

/// Archivos de extract/ui/mapa/ (tools/iconos_mapa.py), en el orden de `icon_slot`.
pub const NODE_ICONS: [&str; 8] = ["combate", "elite", "gracia", "mercader", "tesoro", "evento", "jefe", "inicio"];

fn icon_slot(k: NodeType) -> Option<usize> {
    Some(match k {
        NodeType::Monster => 0,
        NodeType::Elite => 1,
        NodeType::Rest => 2,
        NodeType::Shop => 3,
        NodeType::Treasure => 4,
        NodeType::Unknown => 5,
        NodeType::Boss => 6,
        NodeType::Ancient => 7,
        _ => return None,
    })
}

/// Icono del mapa para un tipo de nodo escrito ("Monster", "Elite"...; "Inicio" = rosa de los vientos).
pub fn node_icon_by_name(tex: &MapTextures, tipo: &str) -> Option<TextureId> {
    let i = match tipo {
        "Monster" => 0,
        "Elite" => 1,
        "Rest" => 2,
        "Shop" => 3,
        "Treasure" => 4,
        "Unknown" => 5,
        "Boss" => 6,
        _ => 7,
    };
    tex.nodes[i]
}

fn node_icon(tex: &MapTextures, k: NodeType) -> Option<TextureId> {
    icon_slot(k).and_then(|i| tex.nodes[i])
}

fn image_centered(dl: &imgui::DrawListMut, t: TextureId, p: [f32; 2], half: f32, col: ImColor32) {
    dl.add_image(t, [p[0] - half, p[1] - half], [p[0] + half, p[1] + half]).col(col).build();
}

// Tinta del mapa (sobre el pergamino lavado) y de la columna oscura.
const TINTA_MAPA: [u8; 3] = [42, 31, 18];
const TRAZO: [u8; 3] = [58, 46, 30];
const HILO: [u8; 3] = [168, 50, 42];
/// Lavado claro sobre el mapa del acto: se ve el mapa y se lee la ruta.
const LAVADO: [u8; 3] = [236, 226, 200];

fn c(rgb: [u8; 3], a: f32) -> ImColor32 {
    ImColor32::from_rgba(rgb[0], rgb[1], rgb[2], (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn roman(n: usize) -> &'static str {
    ["", "I", "II", "III", "IV", "V"].get(n).copied().unwrap_or("?")
}

pub fn kind_label(k: NodeType) -> &'static str {
    match k {
        NodeType::Monster => t("Fight"),
        NodeType::Elite => t("Elite"),
        NodeType::Rest => t("Grace"),
        NodeType::Shop => t("Merchant"),
        NodeType::Treasure => t("Treasure"),
        NodeType::Unknown => t("Unknown"),
        NodeType::Boss => t("Lord of the act"),
        NodeType::Ancient => t("Start"),
        NodeType::Unassigned => "",
    }
}

/// Posición en pantalla de un nodo. La fila 0 abajo.
fn node_pos(v: &MapView, p: (i32, i32), area: [f32; 4]) -> [f32; 2] {
    let [x0, y0, x1, y1] = area;
    let x = x0 + (x1 - x0) * p.0 as f32 / (v.columns - 1).max(1) as f32;
    let y = y1 - (y1 - y0) * p.1 as f32 / (v.rooms + 1) as f32;
    // Desplazamiento pequeño y estable por nodo (como StS) para que no parezca una rejilla.
    if p.1 > 0 && p.1 <= v.rooms {
        let h = (p.0 as u32).wrapping_mul(2654435761) ^ (p.1 as u32).wrapping_mul(40503) ^ (v.seed.len() as u32 * 97);
        let jx = ((h % 1000) as f32 / 1000.0 - 0.5) * 26.0;
        let jy = (((h / 1000) % 1000) as f32 / 1000.0 - 0.5) * 18.0;
        return [x + jx, y + jy];
    }
    [x, y]
}

/// Trazo discontinuo de `a` a `b`, sin pisar los iconos de los extremos.
#[allow(clippy::too_many_arguments)]
pub fn discontinua(dl: &imgui::DrawListMut, a: [f32; 2], b: [f32; 2], r0: f32, r1: f32, col: ImColor32, grosor: f32, raya: f32, hueco: f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= r0 + r1 {
        return;
    }
    let mut t = r0;
    while t < len - r1 {
        let t2 = (t + raya).min(len - r1);
        dl.add_line([a[0] + dx * t / len, a[1] + dy * t / len], [a[0] + dx * t2 / len, a[1] + dy * t2 / len], col).thickness(grosor).build();
        t += raya + hueco;
    }
}

/// Ancho de la columna oscura de la izquierda (datos del acto y lo que llevas).
pub fn ancho_columna(lz: &crate::estilo::Lienzo) -> f32 {
    lz.x(600.0)
}

/// El mapa del acto. `relics`: lo que llevas, en la columna de la izquierda.
pub fn draw(ui: &Ui, v: &MapView, tex: &MapTextures, fonts: &super::Fonts, time: f32, relics: &[crate::bendiciones::BlessingView]) {
    use crate::estilo::{self, CLARA, FONDO, ORO, SANGRE, TENUE, TINTA};
    let lz = estilo::Lienzo::new(ui);
    let (w, h) = (lz.w, lz.h);
    let dl = ui.get_background_draw_list();
    let a0 = crate::adorno::appear(v.t_in, 0);

    // Fondo: el tramo del mapa del juego para este acto, con el lavado claro.
    let [bw, bh] = tex.background_size;
    let scale = (w / bw).max(h / bh);
    let (sw, sh) = (bw * scale, bh * scale);
    dl.add_image(tex.background, [(w - sw) / 2.0, (h - sh) / 2.0], [(w + sw) / 2.0, (h + sh) / 2.0]).build();
    dl.add_rect([0.0, 0.0], [w, h], c(LAVADO, 0.6)).filled(true).build();

    // Columna oscura a la izquierda.
    let col_w = ancho_columna(&lz);
    dl.add_rect([0.0, 0.0], [col_w, h], c(FONDO, 0.93)).filled(true).build();

    // Zona del mapa: lo que queda a la derecha de la columna, dejando sitio a la derecha para el
    // nombre del señor del acto y la leyenda.
    let mx0 = col_w + lz.d(140.0);
    let mx1 = w - lz.d(60.0);
    let mw = (lz.d(820.0)).min(mx1 - mx0 - lz.d(380.0));
    let cx = mx0 + (mx1 - mx0 - lz.d(300.0)) / 2.0;
    let area = [cx - mw / 2.0, lz.y(200.0), cx + mw / 2.0, h - lz.d(70.0)];
    let pos = |p: (i32, i32)| node_pos(v, p, area);

    // Entrada: los caminos aparecen con un fundido y los nodos fila a fila, de abajo arriba.
    let fade = crate::adorno::ease_out(v.t_in / 0.6);
    let row_in = |row: i32| crate::adorno::ease_out((v.t_in - 0.04 * row.max(0) as f32) / 0.3);
    let on_path = |a: (i32, i32), b: (i32, i32)| v.path.windows(2).any(|w| w[0] == a && w[1] == b);
    let r = lz.d(26.0);
    // Caminos por recorrer, en tinta discontinua; el recorrido, el hilo rojo continuo encima.
    for &(a, b) in &v.edges {
        if on_path(a, b) {
            continue;
        }
        let r_end = if b == v.boss { lz.d(70.0) } else { r + lz.d(6.0) };
        let k = fade * row_in(b.1.min(a.1));
        discontinua(&dl, pos(a), pos(b), r + lz.d(6.0), r_end, c(TRAZO, 0.75 * k), lz.d(2.6), lz.d(10.0), lz.d(8.0));
    }
    for wv in v.path.windows(2) {
        let (pa, pb) = (pos(wv[0]), pos(wv[1]));
        dl.add_line(pa, pb, c(HILO, fade)).thickness(lz.d(5.0)).build();
    }
    // Cooperativo: tu voto, el hilo hasta el nodo elegido, más tenue (19-10: votando el primero no
    // se veía la marca, bajo tu propio cursor).
    if let (Some(voto), Some(&aqui)) = (v.mi_voto, v.path.last()) {
        let latido = 0.55 + 0.25 * (0.5 + 0.5 * (time * 2.0).sin());
        dl.add_line(pos(aqui), pos(voto), c(HILO, latido * fade)).thickness(lz.d(5.0)).build();
    }

    // Nodos.
    let pulse = 0.5 + 0.5 * (time * 3.0).sin();
    let blanco = |al: f32| ImColor32::from_rgba(255, 255, 255, (al.clamp(0.0, 1.0) * 255.0) as u8);
    for n in &v.nodes {
        let p = pos((n.col, n.row));
        let k = row_in(n.row);
        if n.kind == NodeType::Boss {
            let rb = lz.d(62.0);
            if let Some(t) = node_icon(tex, n.kind) {
                let beat = 1.0 + 0.04 * (time * 1.6).sin();
                image_centered(&dl, t, p, rb * beat * (0.7 + 0.3 * k), blanco(k));
            }
            // Nombre encima del sello, centrado y en una línea: a la derecha, los largos bajaban en tres
            // líneas por encima de los nodos (23-10). Si no cabe en el título, con la letra de las
            // opciones; y si tampoco, en dos líneas hacia arriba.
            let max = (w - lz.d(360.0)).min(lz.d(1100.0));
            let fuente = if crate::estilo::ancho(ui, fonts.title, &v.boss_name) <= max { fonts.title } else { fonts.opcion };
            let lineas = {
                let _f = ui.push_font(fuente);
                crate::cartas::wrap(ui, &v.boss_name, max)
            };
            let alto = { let _f = ui.push_font(fuente); ui.text_line_height() };
            let mut y = p[1] - rb - lz.d(12.0) - alto * lineas.len().min(2) as f32;
            let cartel = t("Lord of the act");
            let an = crate::estilo::ancho(ui, fonts.nota, cartel);
            crate::estilo::texto(ui, &dl, fonts.nota, [p[0] - an / 2.0, y - lz.d(30.0)], c(TINTA_MAPA, 0.85 * k), cartel);
            for l in lineas.iter().take(2) {
                let an = crate::estilo::ancho(ui, fuente, l);
                crate::estilo::texto(ui, &dl, fuente, [p[0] - an / 2.0, y], c(TINTA_MAPA, k), l);
                y += alto;
            }
            if v.selected == Some((n.col, n.row)) {
                cursor_mapa(&dl, tex, p, rb * 1.25, time, k);
            }
            continue;
        }
        let (alpha, half) = match n.state {
            NodeState::Locked => (0.55, r),
            NodeState::Visited | NodeState::Current => (0.8, r),
            NodeState::Available => (1.0, r * (1.12 + 0.06 * pulse)),
        };
        if let Some(t) = node_icon(tex, n.kind) {
            image_centered(&dl, t, p, half * (0.6 + 0.4 * k), blanco(alpha * k));
        }
        if n.state == NodeState::Available {
            dl.add_circle(p, r + lz.d(8.0) + lz.d(3.0) * pulse, c(TINTA_MAPA, (0.35 + 0.35 * pulse) * k)).thickness(lz.d(1.6)).num_segments(32).build();
        }
        if v.selected == Some((n.col, n.row)) {
            cursor_mapa(&dl, tex, p, r * 1.9, time, k);
        }
        // Cooperativo: el alfiler de cada compañero a la izquierda del nodo, con su nombre encima.
        // Parpadea mientras elige; fijo, con el emblema azul en el nodo, cuando ya ha votado.
        let mut quien: Vec<String> = v.votos.iter().filter(|x| x.0 == (n.col, n.row)).map(|x| x.1.clone()).collect();
        if v.mi_voto == Some((n.col, n.row)) {
            quien.insert(0, t("You").to_string());
        }
        let mirando: Vec<&str> = v
            .cursores
            .iter()
            .filter(|x| x.0 == (n.col, n.row) && !v.votos.iter().any(|vt| vt.1 == x.1))
            .map(|x| x.1.as_str())
            .collect();
        if !quien.is_empty()
            && let Some((tid, _)) = tex.menu.mapa_amigo
        {
            let e = lz.d(22.0);
            let q = [p[0] + r * 0.9, p[1] - r * 0.9];
            dl.add_image(tid, [q[0] - e, q[1] - e], [q[0] + e, q[1] + e]).col(blanco(k)).build();
        }
        let parpadeo = 0.3 + 0.7 * (0.5 + 0.5 * (time * 5.5).sin());
        let alfileres = quien.iter().map(|q| (q.as_str(), 1.0)).chain(mirando.iter().map(|m| (*m, parpadeo)));
        // El anillo del marcador de jugador del juego rodea el lugar donde está: centrado en el nodo
        // (con varios, un poco corridos). Los nombres, a la derecha.
        let alfileres: Vec<(&str, f32)> = alfileres.collect();
        for (i, (_, al)) in alfileres.iter().enumerate() {
            let dx = (i as f32 - (alfileres.len() as f32 - 1.0) / 2.0) * lz.d(10.0);
            marcador_en(&dl, tex, [p[0] + dx, p[1]], r * 1.22, al * k);
        }
        if !alfileres.is_empty() {
            let al = alfileres.iter().map(|x| x.1).fold(0.0, f32::max);
            let txt = alfileres.iter().map(|x| x.0).collect::<Vec<_>>().join(", ");
            crate::estilo::texto(ui, &dl, fonts.nota, [p[0] + r * 1.5 + lz.d(12.0), p[1] - lz.d(16.0)], c(TINTA_MAPA, al * k), &txt);
        }
    }

    // Leyenda arriba a la derecha, en la tinta del mapa.
    let lx = w - lz.d(290.0);
    let mut ly = lz.y(56.0);
    for k in [NodeType::Monster, NodeType::Elite, NodeType::Rest, NodeType::Shop, NodeType::Treasure, NodeType::Unknown] {
        if let Some(t) = node_icon(tex, k) {
            image_centered(&dl, t, [lx + lz.d(20.0), ly + lz.d(20.0)], lz.d(19.0), blanco(fade));
        }
        crate::estilo::texto(ui, &dl, fonts.texto, [lx + lz.d(52.0), ly + lz.d(3.0)], c(TINTA_MAPA, fade), kind_label(k));
        ly += lz.d(46.0);
    }

    // Inicio del acto.
    if let Some(t) = node_icon(tex, NodeType::Ancient) {
        image_centered(&dl, t, pos(v.start), lz.d(32.0), blanco(fade));
    }

    // Columna: el acto, la semilla, lo que hay que saber y lo que llevas.
    let x = lz.x(90.0);
    let cw = col_w - x - lz.d(40.0);
    let mut y = lz.y(72.0);
    {
        let _f = ui.push_font(fonts.grande);
        for l in crate::cartas::wrap(ui, &v.act_name, cw).iter().take(2) {
            dl.add_text([x, y], c(TINTA, a0), l);
            y += lz.d(54.0);
        }
    }
    let linea = if v.ascension > 0 {
        tf("Act {}. Seed {}. Ascension {}.", &[&roman(v.act_num), &v.seed, &v.ascension])
    } else {
        tf("Act {}. Seed {}.", &[&roman(v.act_num), &v.seed])
    };
    crate::estilo::texto(ui, &dl, fonts.nota, [x, y + lz.d(6.0)], c(TENUE, a0), &linea);
    y += lz.d(66.0);
    let mut datos: Vec<(String, [u8; 3])> = vec![];
    if let Some((quedan, max)) = v.viales {
        datos.push((tf("Flasks {} / {}", &[&quedan, &max]), if quedan == 0 { SANGRE } else { TINTA }));
    }
    match (&v.arma_nombre, v.arma) {
        (Some(n), Some((nivel, max))) => datos.push((tf("{} +{} / {}", &[n, &nivel, &max]), TINTA)),
        (Some(n), None) => datos.push((n.clone(), TINTA)),
        (None, Some((nivel, max))) => datos.push((tf("Weapon +{} / {}", &[&nivel, &max]), TINTA)),
        (None, None) => {}
    }
    if let Some(inv) = &v.invocacion {
        datos.push((tf("Summon: {}", &[inv]), TINTA));
    }
    for (d, col) in &datos {
        crate::estilo::texto(ui, &dl, fonts.texto, [x, y], c(*col, a0), d);
        y += lz.d(42.0);
    }
    // Cooperativo: a quién se espera.
    if v.coop {
        let linea = match (v.mi_voto, v.espera.is_empty()) {
            (None, false) if v.esperar => tf("Waiting for {}", &[&v.espera.join(", ")]),
            (Some(_), false) => tf("Waiting for {}", &[&v.espera.join(", ")]),
            (Some(_), true) => t("Everyone has voted").to_string(),
            (None, _) => t("Vote a node: you go when everyone has voted").to_string(),
        };
        y += lz.d(10.0);
        y = crate::estilo::parrafo(ui, &dl, fonts.nota, [x, y], cw, lz.d(30.0), c(ORO, a0), &linea, 3);
    }
    y += lz.d(30.0);
    crate::bendiciones::draw_panel(ui, &dl, relics, [x, y, x + cw, h - lz.d(230.0)], tex, fonts, a0);
    // Lo elegido, sobre la ayuda: su icono del mapa y lo que es.
    if let Some(sel) = v.selected {
        let kind = if sel == v.boss { NodeType::Boss } else { v.nodes.iter().find(|n| (n.col, n.row) == sel).map(|n| n.kind).unwrap_or(NodeType::Unassigned) };
        let yy = h - lz.d(190.0);
        dl.add_line([x, yy - lz.d(18.0)], [x + cw, yy - lz.d(18.0)], c(crate::estilo::LINEA, a0)).thickness(lz.d(1.5)).build();
        if let Some(t) = node_icon(tex, kind) {
            image_centered(&dl, t, [x + lz.d(26.0), yy + lz.d(30.0)], lz.d(26.0), blanco(a0));
        }
        let label = if kind == NodeType::Boss { v.boss_name.as_str() } else { kind_label(kind) };
        crate::estilo::texto(ui, &dl, fonts.opcion, [x + lz.d(70.0), yy + lz.d(10.0)], c(CLARA, a0), label);
    }
    let viajar = if v.coop { "Vote" } else { "Travel" };
    crate::estilo::ayuda_en(ui, &dl, fonts, &lz, Some(x), &[("A", viajar), ("Y", "What you carry")], a0);
}

/// Cursor del mapa del juego alrededor de un nodo, latiendo despacio.
pub fn cursor_mapa(dl: &imgui::DrawListMut, tex: &MapTextures, p: [f32; 2], half: f32, time: f32, k: f32) {
    let beat = 1.0 + 0.05 * (time * 2.4).sin();
    match tex.menu.mapa_cursor {
        Some((t, _)) => {
            // La imagen lleva su sombra abajo a la derecha: el anillo blanco tiene el centro en
            // (76,3; 75,4) de 160, no en el centro. Se corre para que caiga justo en el nodo.
            let h = half * beat;
            let q = [p[0] + (80.0 - 76.3) / 80.0 * h, p[1] + (80.0 - 75.4) / 80.0 * h];
            image_centered(dl, t, q, h, ImColor32::from_rgba(255, 255, 255, (255.0 * k) as u8));
        }
        None => {
            dl.add_circle(p, half * 0.7, c(TINTA_MAPA, k)).thickness(2.0).num_segments(32).build();
        }
    }
}

/// Marcador de jugador del mapa del juego con el centro de su anillo en `p` y ese anillo de radio
/// `radio` (en la imagen, 72x150, el anillo tiene el centro en (35,5; 87) y radio exterior 31,5).
fn marcador_en(dl: &imgui::DrawListMut, tex: &MapTextures, p: [f32; 2], radio: f32, a: f32) {
    if let Some((t, _)) = tex.menu.mapa_coop {
        // Entero, con la flecha y la cola: recortado solo al anillo se veían los muñones cortados de las
        // dos (23-10).
        let k = radio / 31.5;
        let (v0, v1) = (0.0, 150.0);
        let (x0, y0) = (p[0] - 35.5 * k, p[1] - (87.0 - v0) * k);
        dl.add_image(t, [x0, y0], [x0 + 72.0 * k, y0 + (v1 - v0) * k])
            .uv_min([0.0, v0 / 150.0])
            .uv_max([1.0, v1 / 150.0])
            .col(ImColor32::from_rgba(255, 255, 255, (a.clamp(0.0, 1.0) * 255.0) as u8))
            .build();
    }
}

/// Marcador de jugador del mapa del juego (alfiler), centrado en `p`, de alto `alto`.
#[allow(dead_code)]
fn marcador(dl: &imgui::DrawListMut, tex: &MapTextures, p: [f32; 2], alto: f32, a: f32) {
    if let Some((t, [tw, th])) = tex.menu.mapa_coop {
        let w = alto * tw / th;
        dl.add_image(t, [p[0] - w / 2.0, p[1] - alto / 2.0], [p[0] + w / 2.0, p[1] + alto / 2.0]).col(ImColor32::from_rgba(255, 255, 255, (a.clamp(0.0, 1.0) * 255.0) as u8)).build();
    }
}

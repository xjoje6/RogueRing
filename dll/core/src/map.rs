//! Generación del mapa de un acto. Port de `StandardActMap` de StS2:
//! N caminos que suben fila a fila (columna -1/0/+1 sin cruzarse), luego se reparten los tipos
//! de nodo con las mismas reglas (sin élites ni descansos abajo, sin repetir tipos especiales
//! entre padre e hijo, sin hermanos del mismo tipo).
//! Pendiente respecto al original: `MapPathPruning` y el post-procesado visual (centrar, separar,
//! enderezar), que solo afectan a la estética.

use crate::rng::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    Unassigned,
    /// Combate normal: miniboss o enemigo fuerte.
    Monster,
    /// Boss importante, garantiza reliquia.
    Elite,
    /// Gracia: curar o forjar.
    Rest,
    Shop,
    Treasure,
    /// Nodo "?": evento, a veces combate o tienda.
    Unknown,
    /// Inicio del acto (Neow o el Antiguo de turno).
    Ancient,
    /// Boss de Recuerdo del acto.
    Boss,
}

impl NodeType {
    fn no_repeat_with_neighbors(self) -> bool {
        matches!(self, NodeType::Elite | NodeType::Rest | NodeType::Treasure | NodeType::Shop)
    }
    fn no_repeat_with_siblings(self) -> bool {
        matches!(self, NodeType::Rest | NodeType::Monster | NodeType::Unknown | NodeType::Elite | NodeType::Shop)
    }
}

/// Parámetros de forma del acto. Los valores de StS2 (acto 1) son: 7 columnas, 15 salas,
/// tesoro en la fila 9, élites y descansos a partir de la fila 6, 5 élites, 3 tiendas,
/// ~7 descansos y ~12 nodos "?".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActShape {
    pub columns: i32,
    /// Filas jugables (sin contar inicio ni boss).
    pub rooms: i32,
    pub paths: i32,
    pub treasure_row: i32,
    /// Primera fila en la que puede haber élite o descanso.
    pub min_row_elite_rest: i32,
    /// Sin descanso en las últimas N filas antes de la fila fija de descanso.
    pub no_rest_last_rows: i32,
    pub elites: i32,
    pub shops: i32,
    pub rests: (f64, f64, i32, i32),
    pub unknowns: (f64, f64, i32, i32),
}

impl ActShape {
    pub fn sts2_act1() -> Self {
        ActShape {
            columns: 7,
            rooms: 15,
            paths: 7,
            treasure_row: 9,
            min_row_elite_rest: 6,
            no_rest_last_rows: 3,
            elites: 5,
            shops: 3,
            rests: (7.0, 1.0, 6, 7),
            unknowns: (12.0, 1.0, 10, 14),
        }
    }

    /// Propuesta inicial para Elden Ring: actos más cortos porque cada combate dura minutos.
    pub fn rogue_ring_default() -> Self {
        ActShape {
            columns: 7,
            rooms: 10,
            paths: 6,
            treasure_row: 5,
            min_row_elite_rest: 3,
            no_rest_last_rows: 2,
            elites: 3,
            shops: 2,
            rests: (4.0, 1.0, 3, 5),
            unknowns: (7.0, 1.0, 5, 9),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub col: i32,
    pub row: i32,
    pub kind: NodeType,
    #[serde(skip)]
    pub fixed: bool,
    pub children: Vec<(i32, i32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActMap {
    pub shape: ActShape,
    /// Nodos jugables, filas 1..=rooms.
    pub nodes: Vec<Node>,
    /// Inicio en la fila 0, boss en la fila rooms+1, ambos en la columna central.
    pub start: (i32, i32),
    pub boss: (i32, i32),
}

impl ActMap {
    pub fn node(&self, c: i32, r: i32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.col == c && n.row == r)
    }
    pub fn parents_of(&self, c: i32, r: i32) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.children.contains(&(c, r)))
    }
    /// Nodos accesibles desde el inicio (fila 1).
    pub fn first_row(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.row == 1)
    }
}

struct Gen<'a> {
    s: &'a ActShape,
    rng: &'a mut Rng,
    /// grid[col][row], filas 0..=rooms (la 0 no se usa para nodos).
    grid: Vec<Vec<Option<Node>>>,
}

impl<'a> Gen<'a> {
    fn get(&self, c: i32, r: i32) -> Option<&Node> {
        self.grid.get(c as usize)?.get(r as usize)?.as_ref()
    }
    fn get_or_create(&mut self, c: i32, r: i32) -> (i32, i32) {
        let cell = &mut self.grid[c as usize][r as usize];
        if cell.is_none() {
            *cell = Some(Node { col: c, row: r, kind: NodeType::Unassigned, fixed: false, children: vec![] });
        }
        (c, r)
    }
    fn add_child(&mut self, from: (i32, i32), to: (i32, i32)) {
        let n = self.grid[from.0 as usize][from.1 as usize].as_mut().unwrap();
        if !n.children.contains(&to) {
            n.children.push(to);
        }
    }

    /// Un paso de un camino ¿cruzaría una arista ya existente? (`HasInvalidCrossover`).
    fn invalid_crossover(&self, cur: (i32, i32), target_col: i32) -> bool {
        let dx = target_col - cur.0;
        if dx == 0 {
            return false;
        }
        match self.get(target_col, cur.1) {
            Some(nb) => nb.children.iter().any(|ch| ch.0 - nb.col == -dx),
            None => false,
        }
    }

    fn path(&mut self, start: (i32, i32)) {
        let mut cur = start;
        while cur.1 < self.s.rooms {
            let mut dirs = [-1, 0, 1];
            self.rng.shuffle(&mut dirs);
            let next_col =
                dirs.iter().map(|d| (cur.0 + d).clamp(0, self.s.columns - 1)).find(|&c| !self.invalid_crossover(cur, c)).expect("siempre existe el paso recto");
            let nxt = self.get_or_create(next_col, cur.1 + 1);
            self.add_child(cur, nxt);
            cur = nxt;
        }
    }

    fn all(&self) -> Vec<(i32, i32)> {
        let mut v = vec![];
        for c in 0..self.s.columns {
            for r in 1..=self.s.rooms {
                if self.get(c, r).is_some() {
                    v.push((c, r));
                }
            }
        }
        v
    }

    fn kind(&self, p: (i32, i32)) -> NodeType {
        self.get(p.0, p.1).map(|n| n.kind).unwrap_or(NodeType::Unassigned)
    }

    fn parents(&self, p: (i32, i32)) -> Vec<(i32, i32)> {
        self.all().into_iter().filter(|q| self.get(q.0, q.1).unwrap().children.contains(&p)).collect()
    }

    fn valid(&self, t: NodeType, p: (i32, i32)) -> bool {
        let (_, row) = p;
        if row < self.s.min_row_elite_rest && matches!(t, NodeType::Rest | NodeType::Elite) {
            return false;
        }
        if row >= self.s.rooms - self.s.no_rest_last_rows && t == NodeType::Rest {
            return false;
        }
        let node = self.get(p.0, p.1).unwrap();
        let parents = self.parents(p);
        if t.no_repeat_with_neighbors() && parents.iter().chain(node.children.iter()).any(|q| self.kind(*q) == t) {
            return false;
        }
        if t.no_repeat_with_siblings() {
            let sib = parents.iter().flat_map(|q| self.get(q.0, q.1).unwrap().children.clone()).filter(|q| *q != p);
            if sib.map(|q| self.kind(q)).any(|k| k == t) {
                return false;
            }
        }
        true
    }

    fn set(&mut self, p: (i32, i32), t: NodeType, fixed: bool) {
        let n = self.grid[p.0 as usize][p.1 as usize].as_mut().unwrap();
        n.kind = t;
        n.fixed = fixed;
    }

    fn assign_types(&mut self) {
        let s = self.s.clone();
        for p in self.all() {
            if p.1 == s.rooms {
                self.set(p, NodeType::Rest, true);
            } else if p.1 == s.treasure_row {
                self.set(p, NodeType::Treasure, true);
            } else if p.1 == 1 {
                self.set(p, NodeType::Monster, true);
            }
        }
        let rests = self.rng.gaussian_int(s.rests.0, s.rests.1, s.rests.2, s.rests.3);
        let unknowns = self.rng.gaussian_int(s.unknowns.0, s.unknowns.1, s.unknowns.2, s.unknowns.3);
        let mut queue: std::collections::VecDeque<NodeType> = std::iter::empty()
            .chain(std::iter::repeat_n(NodeType::Rest, rests as usize))
            .chain(std::iter::repeat_n(NodeType::Shop, s.shops as usize))
            .chain(std::iter::repeat_n(NodeType::Elite, s.elites as usize))
            .chain(std::iter::repeat_n(NodeType::Unknown, unknowns as usize))
            .collect();
        // Tres pasadas como el original: lo que no encaje por reglas se queda sin poner.
        for _ in 0..3 {
            if queue.is_empty() {
                break;
            }
            let mut free: Vec<_> = self.all().into_iter().filter(|p| self.kind(*p) == NodeType::Unassigned).collect();
            self.rng.shuffle(&mut free);
            for p in free {
                if queue.is_empty() {
                    break;
                }
                for _ in 0..queue.len() {
                    let t = queue.pop_front().unwrap();
                    if self.valid(t, p) {
                        self.set(p, t, false);
                        break;
                    }
                    queue.push_back(t);
                }
            }
        }
        for p in self.all() {
            if self.kind(p) == NodeType::Unassigned {
                self.set(p, NodeType::Monster, false);
            }
        }
    }
}

pub fn generate(shape: &ActShape, rng: &mut Rng) -> ActMap {
    let mut g = Gen { s: shape, rng, grid: vec![vec![None; (shape.rooms + 1) as usize]; shape.columns as usize] };
    let mut starts: Vec<(i32, i32)> = vec![];
    for i in 0..shape.paths {
        let mut c = g.rng.range(0, shape.columns);
        // Como en StS2: el segundo camino nunca empieza donde el primero, para garantizar elección.
        if i == 1 {
            while starts.iter().any(|s| s.0 == c) {
                c = g.rng.range(0, shape.columns);
            }
        }
        let p = g.get_or_create(c, 1);
        starts.push(p);
        g.path(p);
    }
    g.assign_types();
    let nodes = g.grid.into_iter().flatten().flatten().collect();
    ActMap { shape: shape.clone(), nodes, start: (shape.columns / 2, 0), boss: (shape.columns / 2, shape.rooms + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{hash_str, Rng};

    fn maps(n: u64) -> Vec<ActMap> {
        (0..n).map(|i| generate(&ActShape::rogue_ring_default(), &mut Rng::named(hash_str(&i.to_string()), "act_1_map"))).collect()
    }

    #[test]
    fn determinista() {
        let a = generate(&ActShape::sts2_act1(), &mut Rng::named(42, "act_1_map"));
        let b = generate(&ActShape::sts2_act1(), &mut Rng::named(42, "act_1_map"));
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }

    #[test]
    fn reglas_se_cumplen() {
        for m in maps(500) {
            let s = &m.shape;
            assert!(m.first_row().count() >= 2, "siempre al menos dos opciones de inicio");
            for n in &m.nodes {
                // Todo nodo salvo la última fila tiene hijos; todos menos la primera fila, padres.
                assert!(n.row == s.rooms || !n.children.is_empty());
                assert!(n.row == 1 || m.parents_of(n.col, n.row).count() > 0);
                if n.row < s.min_row_elite_rest {
                    assert!(!matches!(n.kind, NodeType::Elite | NodeType::Rest));
                }
                for ch in &n.children {
                    assert_eq!(ch.1, n.row + 1);
                    assert!((ch.0 - n.col).abs() <= 1);
                    let k = m.node(ch.0, ch.1).unwrap().kind;
                    if n.kind.no_repeat_with_neighbors() && !n.fixed {
                        assert_ne!(k, n.kind, "tipo especial repetido entre padre e hijo");
                    }
                }
            }
            // Sin aristas cruzadas.
            for a in &m.nodes {
                for b in &m.nodes {
                    if a.row != b.row || b.col != a.col + 1 {
                        continue;
                    }
                    assert!(!(a.children.contains(&(b.col, a.row + 1)) && b.children.contains(&(a.col, a.row + 1))), "cruce");
                }
            }
        }
    }
}

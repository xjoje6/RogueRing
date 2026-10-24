//! Recompensas tras un combate. Equivalente a la recompensa de cartas de StS: elegir 1 de 3 (o
//! saltar), con rareza según el tipo de combate. Aquí las "cartas" son armas, cenizas de guerra,
//! hechizos y talismanes del catálogo (data/objetos).

use crate::rng::Rng;
use crate::run::EncounterKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Arma,
    Ceniza,
    Hechizo,
    Talisman,
    /// Ceniza de espíritu: cambia la invocación del Invocador (solo se le ofrece a él).
    Invocacion,
}

impl Category {
    /// Categoría del ItemLot del juego para entregarlo (orden `dar`).
    pub fn itemlot_category(self) -> i32 {
        match self {
            Category::Arma => 2,
            Category::Talisman => 4,
            Category::Ceniza => 5,
            Category::Hechizo => 1, // los hechizos son objetos (Goods)
            Category::Invocacion => 1, // la ceniza de espíritu también (no se da: la invoca la DLL)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Comun,
    PocoComun,
    Rara,
    /// Solo bendiciones: las más locas, con su precio (`RelicDef::contra`). Salen poco.
    Mitica,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: i32,
    pub nombre: String,
    pub categoria: Category,
    pub rareza: Rarity,
    /// Subtipo legible: "Espadón", "Encantamiento"...
    pub tipo: String,
    /// Requisitos [fue, des, int, fe, arc].
    pub req: [u8; 5],
    /// iconId del juego (extract/ui/iconos/<icono>.png).
    #[serde(default)]
    pub icono: i32,
    /// Arma única (de boss): no admite cenizas.
    #[serde(default)]
    pub unica: bool,
    /// Ceniza: tipos de arma en los que se puede montar.
    #[serde(default)]
    pub encaja: Vec<String>,
    /// Arma doble: se equipa en las dos manos (la decide la oferta, no el pool).
    #[serde(default)]
    pub doble: bool,
    /// Ceniza de espíritu: a quién invoca.
    #[serde(default)]
    pub invocacion: Option<crate::characters::Invocacion>,
    /// Peso al sortearlo dentro de su categoría y rareza (0 = 1): las armas del estilo del personaje
    /// pesan más (director, offer_pool). No se guarda.
    #[serde(default, skip_serializing)]
    pub peso: f64,
}

/// Tipos de arma ligera que pueden salir dobles (una en cada mano).
pub const TIPOS_DOBLES: [&str; 8] = ["Dagger", "Katana", "Curved Sword", "Straight Sword", "Thrusting Sword", "Axe", "Hammer", "Whip"];

/// Armas de pareja del juego: una sola, a dos manos, ya ataca con las dos (17-10: «no des dos
/// armas, da una y que venga a dos manos»). Nunca salen dobles.
pub const TIPOS_PAREJA: [&str; 3] = ["Fist", "Claw", "Hand-to-Hand Art"];

impl Item {
    pub fn can_be_double(&self) -> bool {
        self.categoria == Category::Arma && TIPOS_DOBLES.contains(&self.tipo.as_str())
    }

    /// Arma de pareja: va a dos manos.
    pub fn en_pareja(&self) -> bool {
        self.categoria == Category::Arma && TIPOS_PAREJA.contains(&self.tipo.as_str())
    }
}

/// Marca como dobles algunas armas ligeras de una oferta (35 %). Con `permitidas` a false (personaje
/// `sin_dobles`) ninguna, pero la tirada se hace igual: la semilla da las mismas ofertas después.
pub fn roll_doubles(items: &mut [Item], rng: &mut Rng, permitidas: bool) {
    for it in items.iter_mut().filter(|it| it.can_be_double()) {
        it.doble = rng.range(0, 100) < 35 && permitidas;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RewardOffer {
    pub runes: u32,
    pub choices: Vec<Item>,
}

/// Probabilidad de cada rareza (común, poco común, rara) según el combate, como StS.
fn rarity_odds(kind: EncounterKind) -> [f64; 3] {
    match kind {
        EncounterKind::Weak | EncounterKind::Normal => [0.60, 0.37, 0.03],
        EncounterKind::Elite => [0.50, 0.40, 0.10],
        EncounterKind::Boss => [0.0, 0.0, 1.0],
    }
}

/// Peso de cada categoría (las que no tengan nada que ofrecer no cuentan). Se elige primero la
/// categoría y luego el objeto: si no, la que tiene más objetos (las armas) sale casi siempre.
const CATEGORY_WEIGHTS: [(Category, f64); 5] =
    [(Category::Arma, 0.5), (Category::Ceniza, 0.5), (Category::Hechizo, 0.25), (Category::Talisman, 0.25), (Category::Invocacion, 0.5)];
/// Lo que pesa una categoría por cada carta suya que ya está en la oferta (que no salgan tres iguales).
const CATEGORY_REPEAT: f64 = 0.35;

fn pick_weighted<T: Copy>(rng: &mut Rng, items: &[(T, f64)]) -> T {
    let total: f64 = items.iter().map(|x| x.1).sum();
    let mut r = rng.next_f64() * total;
    for (v, w) in items {
        if r < *w {
            return *v;
        }
        r -= w;
    }
    items[items.len() - 1].0
}

/// Genera la oferta. `owned` = ids ya conseguidos en la run (no se repiten).
pub fn generate(pool: &[Item], kind: EncounterKind, act: usize, rng: &mut Rng, owned: &[(Category, i32)]) -> RewardOffer {
    generate_n(pool, kind, act, rng, owned, 3)
}

/// Como `generate`, con `n` opciones (Ojo avizor da una más).
pub fn generate_n(pool: &[Item], kind: EncounterKind, act: usize, rng: &mut Rng, owned: &[(Category, i32)], n: usize) -> RewardOffer {
    let odds = rarity_odds(kind);
    let mut choices: Vec<Item> = vec![];
    for _ in 0..n {
        // Rareza por las probabilidades del encuentro; si no queda nada de esa rareza, la siguiente
        // más cercana. Dentro de la rareza, un objeto real del pool con peso por categoría.
        let rar = pick_weighted(rng, &[(Rarity::Comun, odds[0].max(0.0001)), (Rarity::PocoComun, odds[1].max(0.0001)), (Rarity::Rara, odds[2].max(0.0001))]);
        let order: [Rarity; 3] = match rar {
            Rarity::Comun => [Rarity::Comun, Rarity::PocoComun, Rarity::Rara],
            Rarity::PocoComun => [Rarity::PocoComun, Rarity::Comun, Rarity::Rara],
            Rarity::Rara | Rarity::Mitica => [Rarity::Rara, Rarity::PocoComun, Rarity::Comun],
        };
        let free = |i: &&Item| !owned.contains(&(i.categoria, i.id)) && !choices.iter().any(|c| c.categoria == i.categoria && c.id == i.id);
        let cats: Vec<(Category, f64)> = CATEGORY_WEIGHTS
            .iter()
            .filter(|(cat, _)| pool.iter().filter(free).any(|i| i.categoria == *cat))
            .map(|&(cat, w)| (cat, w * CATEGORY_REPEAT.powi(choices.iter().filter(|c| c.categoria == cat).count() as i32)))
            .collect();
        if cats.is_empty() {
            break;
        }
        let cat = pick_weighted(rng, &cats);
        let chosen = order.iter().find_map(|r| {
            let cands: Vec<&Item> = pool.iter().filter(|i| i.categoria == cat && i.rareza == *r).filter(free).collect();
            let pesos: Vec<(usize, f64)> = cands.iter().enumerate().map(|(k, i)| (k, if i.peso > 0.0 { i.peso } else { 1.0 })).collect();
            (!cands.is_empty()).then(|| cands[pick_weighted(rng, &pesos)].clone())
        });
        if let Some(c) = chosen {
            choices.push(c);
        }
    }
    let base = match kind {
        EncounterKind::Weak => 400,
        EncounterKind::Normal => 700,
        EncounterKind::Elite => 1500,
        EncounterKind::Boss => 4000,
    };
    // Runas = oro de StS: suben por acto y varían un ±10 %.
    let runes = (base as f64 * (1.0 + 0.6 * act as f64) * (0.9 + 0.2 * rng.next_f64())) as u32;
    RewardOffer { runes, choices }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Vec<Item> {
        let mut v = vec![];
        for (ci, cat) in [Category::Arma, Category::Ceniza, Category::Hechizo, Category::Talisman].iter().enumerate() {
            for (ri, rar) in [Rarity::Comun, Rarity::PocoComun, Rarity::Rara].iter().enumerate() {
                for k in 0..5 {
                    v.push(Item {
                        id: (ci * 100 + ri * 10 + k) as i32,
                        nombre: format!("{cat:?}{rar:?}{k}"),
                        categoria: *cat,
                        rareza: *rar,
                        tipo: String::new(),
                        req: [0; 5],
                        icono: 0,
                        unica: false,
                        encaja: vec![],
                        doble: false,
                        invocacion: None,
                        peso: 0.0,
                    });
                }
            }
        }
        v
    }

    #[test]
    fn tres_distintas_y_sin_repetir_lo_que_tienes() {
        let p = pool();
        let owned: Vec<_> = p.iter().take(30).map(|i| (i.categoria, i.id)).collect();
        for s in 0..300 {
            let o = generate(&p, EncounterKind::Normal, 0, &mut Rng::named(s, "rewards"), &owned);
            assert_eq!(o.choices.len(), 3);
            for c in &o.choices {
                assert!(!owned.contains(&(c.categoria, c.id)));
            }
            assert!(o.choices[0] != o.choices[1] && o.choices[1] != o.choices[2] && o.choices[0] != o.choices[2]);
        }
    }

    #[test]
    fn jefe_solo_raras_y_proporciones_normales() {
        let p = pool();
        let o = generate(&p, EncounterKind::Boss, 0, &mut Rng::named(1, "rewards"), &[]);
        assert!(o.choices.iter().all(|c| c.rareza == Rarity::Rara));
        let mut raras = 0;
        for s in 0..2000 {
            let o = generate(&p, EncounterKind::Normal, 0, &mut Rng::named(s, "rewards"), &[]);
            raras += o.choices.iter().filter(|c| c.rareza == Rarity::Rara).count();
        }
        let prop = raras as f64 / 6000.0;
        assert!((0.015..0.05).contains(&prop), "proporción de raras {prop}");
    }

    #[test]
    fn siempre_hay_tres_opciones_con_el_pool_real() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/objetos/pool.json")).unwrap();
        let pool: Vec<Item> = serde_json::from_str(&t).unwrap();
        // Un estilo estrecho: pocas armas de rareza alta entre las que elegir.
        let estilo = ["Straight Sword", "Curved Sword", "Katana", "Thrusting Sword", "Twinblade", "Axe", "Spear"];
        let pool: Vec<Item> = pool.into_iter().filter(|i| i.categoria != Category::Arma || estilo.contains(&i.tipo.as_str())).collect();
        let mut rng = Rng::named(7, "t");
        for kind in [EncounterKind::Weak, EncounterKind::Normal, EncounterKind::Elite, EncounterKind::Boss] {
            for _ in 0..50 {
                let o = generate(&pool, kind, 0, &mut rng, &[]);
                assert_eq!(o.choices.len(), 3, "{kind:?}");
            }
        }
    }
}

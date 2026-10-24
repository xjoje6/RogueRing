//! Personajes (data/personajes.json): atributos fijos, armas iniciales a elegir, ropa propia y estilo
//! de armas que se le ofrecen. Las pasivas viven en la DLL.

use serde::{Deserialize, Serialize};

/// Victorias con el personaje que pide cada arma de su lista: la primera es la de salida.
pub const VICTORIAS_ARMA: [u32; 3] = [0, 3, 10];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterDef {
    pub id: String,
    pub nombre: String,
    pub lema: String,
    /// vig men agu fue des int fe arc.
    pub atributos: [u32; 8],
    /// Armas iniciales (ids de EquipParamWeapon): la primera de salida y las demás se desbloquean
    /// ganando runs con el personaje (VICTORIAS_ARMA). `doble`: van en las dos manos.
    pub armas: Vec<i32>,
    #[serde(default)]
    pub doble: bool,
    /// No se le ofrecen armas dobles (el Samurái: desvía con una sola). Las de pareja, que son un
    /// arma que ya va en las dos manos, sí.
    #[serde(default)]
    pub sin_dobles: bool,
    /// Ropa propia: cabeza, torso, brazos y piernas (ids de EquipParamProtector).
    #[serde(default)]
    pub ropa: Vec<i32>,
    /// Ceniza de guerra montada en el arma inicial (EquipParamGem).
    #[serde(default)]
    pub ceniza: Option<i32>,
    #[serde(default)]
    pub frascos: u32,
    /// Tipos de arma que se le ofrecen (los del pool: "Katana", "Daga"...).
    pub estilo: Vec<String>,
    #[serde(default)]
    pub pasiva: String,
    /// Apariencias desbloqueables (otra ropa, como el cuerpo de un héroe de Nightreign).
    #[serde(default)]
    pub apariencias: Vec<Apariencia>,
    /// Invocaciones a elegir (el Invocador): la primera de salida y las demás, como las armas, ganando
    /// runs con él (VICTORIAS_ARMA). La elegida sale a su lado al empezar cada combate.
    #[serde(default)]
    pub invocaciones: Vec<Invocacion>,
    /// Ajuste de balance: daño que hace y que recibe (multiplicadores, 1 = sin cambio).
    #[serde(default = "uno")]
    pub danio: f64,
    #[serde(default = "uno")]
    pub recibido: f64,
}

fn uno() -> f64 {
    1.0
}

/// Apariencia desbloqueable de un personaje: su ropa (cabeza, torso, brazos, piernas; -1 = la ranura
/// vacía) y las victorias con él que pide. Las de Nightreign son solo el torso (el héroe entero,
/// data/nightreign.json «apariencias») con las defensas de su ropa de siempre.
/// Invocación de una ceniza de espíritu del juego (BuddyParam): modelo, NpcParam e IA de cada uno, y
/// dónde aparece cada uno respecto al jugador (los lobos son tres).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Invocacion {
    pub id: String,
    pub nombre: String,
    pub chr: i32,
    pub npc: i32,
    pub think: i32,
    /// (a un lado, delante) en metros, uno por espíritu.
    pub posiciones: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Apariencia {
    pub id: String,
    pub nombre: String,
    pub ropa: Vec<i32>,
    #[serde(default)]
    pub victorias: u32,
}

impl CharacterDef {
    /// Victorias que pide el arma `k` de su lista.
    pub fn victorias_para(k: usize) -> u32 {
        VICTORIAS_ARMA.get(k).copied().unwrap_or(VICTORIAS_ARMA[VICTORIAS_ARMA.len() - 1])
    }

    /// ¿Está desbloqueada el arma `k` con estas victorias?
    pub fn arma_libre(&self, k: usize, victorias: u32) -> bool {
        k < self.armas.len() && victorias >= Self::victorias_para(k)
    }

    /// Arma inicial: la preferida si es una de las suyas y ya está desbloqueada; si no, la primera.
    pub fn arma(&self, preferida: Option<i32>, victorias: u32) -> i32 {
        preferida
            .filter(|a| self.armas.iter().position(|x| x == a).is_some_and(|k| self.arma_libre(k, victorias)))
            .or_else(|| self.armas.first().copied())
            .unwrap_or(0)
    }

    /// Atributos para empezar con un arma de estos requisitos (fue des int fe arc): los suyos, con
    /// lo que falte subido justo hasta el requisito.
    pub fn atributos_para(&self, req: [u8; 5]) -> [u32; 8] {
        let mut a = self.atributos;
        for (i, r) in req.iter().enumerate() {
            a[3 + i] = a[3 + i].max(*r as u32);
        }
        a
    }

    /// Invocación: la preferida si es suya y ya está desbloqueada; si no, la primera.
    pub fn invocacion(&self, preferida: Option<&str>, victorias: u32) -> Option<&Invocacion> {
        preferida
            .and_then(|id| self.invocaciones.iter().position(|i| i.id == id))
            .filter(|&k| victorias >= Self::victorias_para(k))
            .and_then(|k| self.invocaciones.get(k))
            .or_else(|| self.invocaciones.first())
    }

    /// Ropa con la apariencia preferida si existe y ya está desbloqueada; si no, la de siempre.
    pub fn ropa_con(&self, apariencia: Option<&str>, victorias: u32) -> Vec<i32> {
        apariencia
            .and_then(|id| self.apariencias.iter().find(|a| a.id == id && victorias >= a.victorias))
            .map(|a| a.ropa.clone())
            .unwrap_or_else(|| self.ropa.clone())
    }

    pub fn level(&self) -> u32 {
        self.atributos.iter().sum::<u32>().saturating_sub(79)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CharacterFile {
    pub personajes: Vec<CharacterDef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carga_el_archivo_real() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/personajes.json")).unwrap();
        let f: CharacterFile = serde_json::from_str(&t).unwrap();
        assert_eq!(f.personajes.len(), 4);
        // El escalado de los enemigos va por SpEffect, no por nivel. Vigor bajo a propósito (se empieza
        // con poca vida y se gana con bendiciones); el resto de atributos, de nivel 150.
        assert!(f.personajes.iter().all(|c| c.level() <= 150));
        // Las armas iniciales están en el pool y son de su propio estilo; la ropa es de cuatro piezas.
        let pool: Vec<crate::rewards::Item> =
            serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/objetos/pool.json")).unwrap()).unwrap();
        for c in &f.personajes {
            assert_eq!(c.armas.len(), VICTORIAS_ARMA.len(), "{}: una de salida y dos por desbloquear", c.id);
            for a in &c.armas {
                assert!(pool.iter().any(|i| i.id == *a), "{}: {a} no está en el pool", c.id);
            }
            // La de salida se empuña con sus atributos de siempre (nivel 150 justo).
            let w = pool.iter().find(|i| i.id == c.armas[0]).unwrap();
            assert!(c.estilo.contains(&w.tipo), "{}: {} no es de su estilo", c.id, w.tipo);
            assert_eq!(c.atributos_para(w.req), c.atributos, "{}: le faltan atributos para su arma de salida", c.id);
            assert_eq!(c.ropa.len(), 4, "{}: la ropa son cuatro piezas", c.id);
            for a in &c.apariencias {
                assert_eq!(a.ropa.len(), 4, "{}: la apariencia {} son cuatro piezas", c.id, a.id);
                // Bloqueada hasta sus victorias: hasta entonces, la ropa de siempre.
                assert_eq!(c.ropa_con(Some(&a.id), a.victorias.saturating_sub(1)), if a.victorias > 0 { c.ropa.clone() } else { a.ropa.clone() });
                assert_eq!(c.ropa_con(Some(&a.id), a.victorias), a.ropa);
            }
            // Bloqueadas hasta las victorias que piden.
            assert_eq!(c.arma(Some(c.armas[1]), 2), c.armas[0]);
            assert_eq!(c.arma(Some(c.armas[1]), 3), c.armas[1]);
            assert_eq!(c.arma(Some(c.armas[2]), 9), c.armas[0]);
            assert_eq!(c.arma(Some(c.armas[2]), 10), c.armas[2]);
            assert_eq!(c.arma(Some(1), 99), c.armas[0]);
            // Invocaciones: cada una con alguien que sacar; bloqueadas como las armas.
            for i in &c.invocaciones {
                assert!(i.npc > 0 && i.think > 0 && !i.posiciones.is_empty(), "{}: invocación {} sin datos", c.id, i.id);
            }
            if c.invocaciones.len() > 1 {
                assert_eq!(c.invocacion(Some(&c.invocaciones[1].id), 2).map(|i| &i.id), Some(&c.invocaciones[0].id));
                assert_eq!(c.invocacion(Some(&c.invocaciones[1].id), 3).map(|i| &i.id), Some(&c.invocaciones[1].id));
            }
        }
    }
}

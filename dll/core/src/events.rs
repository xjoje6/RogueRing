//! Eventos ("?" del mapa), como los de Slay the Spire: una escena con arte de pantalla de carga,
//! un texto corto y opciones con efectos. Definición en data/eventos.json.

use serde::{Deserialize, Serialize};

use crate::rewards::Rarity;
use crate::rng::Rng;
use crate::run::EncounterKind;

/// Lo que hace una opción. En JSON: `{"runas": -150}`, `{"reliquia": "PocoComun"}`,
/// `{"maldicion": "fragil"}`, `{"objeto": "Rara"}`, `"templar"`, `{"combate": "Elite"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Runas ganadas (negativo = coste; la opción se apaga si no llegas).
    Runas(i32),
    /// Una reliquia (no maldita) de esa rareza, o de otra si no quedan.
    Reliquia(Rarity),
    /// Una maldición concreta (reliquia con `maldita: true`).
    Maldicion(String),
    /// Una reliquia concreta por id (p. ej. la del cofre trampa, que se cobra al ganar).
    #[serde(rename = "reliquia_fija")]
    ReliquiaFija(String),
    /// Un objeto del pool de esa rareza; se equipa como cualquier recompensa.
    Objeto(Rarity),
    /// Sube de nivel una reliquia al azar (la opción se apaga si no hay ninguna que templar).
    Templar,
    /// Pelea con el siguiente encuentro de esa cola; al ganar, recompensa normal.
    Combate(EncounterKind),
    /// Los enemigos reciben un mutador de ese nivel ("plata", "oro", "prismatico") el resto de la run.
    Mutador(String),
}

/// Resultado posible de una opción con azar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub peso: u32,
    #[serde(default)]
    pub efectos: Vec<Effect>,
    pub resultado: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventOption {
    /// Lo que haces ("Sentarte junto al fuego").
    pub texto: String,
    /// Lo que pasa, en corto ("Ganas 200 runas."). Si hay azar, lo que se sabe.
    #[serde(default)]
    pub detalle: String,
    #[serde(default)]
    pub efectos: Vec<Effect>,
    /// Si hay ramas, se elige una por peso y sus efectos se suman a `efectos`.
    #[serde(default)]
    pub azar: Vec<Branch>,
    /// Texto tras elegir (sin azar).
    #[serde(default)]
    pub resultado: String,
}

impl EventOption {
    /// Runas que cuesta (suma de los efectos de runas negativos fijos).
    pub fn cost(&self) -> u32 {
        self.efectos.iter().map(|e| if let Effect::Runas(n) = e { (-n).max(0) as u32 } else { 0 }).sum()
    }

    pub fn needs_temper(&self) -> bool {
        self.efectos.contains(&Effect::Templar)
    }

    /// Efectos finales y texto de resultado (tira el azar si lo hay).
    pub fn resolve(&self, rng: &mut Rng) -> (Vec<Effect>, String) {
        let total: u32 = self.azar.iter().map(|b| b.peso).sum();
        if total == 0 {
            return (self.efectos.clone(), self.resultado.clone());
        }
        let mut roll = rng.range(0, total as i32) as u32;
        for b in &self.azar {
            if roll < b.peso {
                let mut ef = self.efectos.clone();
                ef.extend(b.efectos.iter().cloned());
                return (ef, b.resultado.clone());
            }
            roll -= b.peso;
        }
        (self.efectos.clone(), self.resultado.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDef {
    pub id: String,
    pub titulo: String,
    /// Nombre de la imagen en extract/ui/carga (sin .png).
    pub imagen: String,
    pub texto: String,
    /// Actos en que puede salir (1..). Vacío = todos.
    #[serde(default)]
    pub actos: Vec<usize>,
    pub opciones: Vec<EventOption>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EventFile {
    pub eventos: Vec<EventDef>,
}

impl EventFile {
    /// Un evento del acto (índice 0..) que no hayas visto en esta run. Si ya se vieron todos, cualquiera.
    pub fn pick(&self, act_index: usize, seen: &[String], rng: &mut Rng) -> Option<EventDef> {
        let of_act: Vec<&EventDef> = self.eventos.iter().filter(|e| e.actos.is_empty() || e.actos.contains(&(act_index + 1))).collect();
        let fresh: Vec<&&EventDef> = of_act.iter().filter(|e| !seen.contains(&e.id)).collect();
        if !fresh.is_empty() {
            return Some((*fresh[rng.range(0, fresh.len() as i32) as usize]).clone());
        }
        if of_act.is_empty() {
            return None;
        }
        Some(of_act[rng.range(0, of_act.len() as i32) as usize].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carga_el_archivo_real() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/eventos.json")).unwrap();
        let f: EventFile = serde_json::from_str(&t).unwrap();
        assert!(f.eventos.len() >= 8);
        for e in &f.eventos {
            assert!(!e.opciones.is_empty(), "{} sin opciones", e.id);
            // Textos de carta: sin guion largo.
            let all = std::iter::once(&e.texto).chain(e.opciones.iter().flat_map(|o| [&o.texto, &o.detalle, &o.resultado]));
            for s in all.chain(e.opciones.iter().flat_map(|o| o.azar.iter().map(|b| &b.resultado))) {
                assert!(!s.contains('\u{2014}'), "{}: guion largo en {s:?}", e.id);
            }
            // Toda opción tiene resultado (propio o en cada rama).
            for o in &e.opciones {
                assert!(!o.resultado.is_empty() || (!o.azar.is_empty() && o.azar.iter().all(|b| !b.resultado.is_empty())), "{}: opción sin resultado", e.id);
            }
        }
        let mut rng = Rng::named(3, "ev");
        let a = f.pick(0, &[], &mut rng).unwrap();
        let b = f.pick(0, std::slice::from_ref(&a.id), &mut rng).unwrap();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn azar_y_coste() {
        let o: EventOption = serde_json::from_str(
            r#"{"texto":"x","efectos":[{"runas":-150}],"azar":[{"peso":1,"efectos":["templar"],"resultado":"a"},{"peso":1,"resultado":"b"}]}"#,
        )
        .unwrap();
        assert_eq!(o.cost(), 150);
        let mut rng = Rng::named(1, "t");
        let (ef, r) = o.resolve(&mut rng);
        assert!(r == "a" || r == "b");
        assert_eq!(ef[0], Effect::Runas(-150));
    }
}

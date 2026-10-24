//! Perfil persistente (runtime/perfil.json): victorias y ascensión desbloqueada por personaje,
//! historial de runs y bendiciones descubiertas. Como el perfil de Slay the Spire.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// Ascensiones (acumulativas): la N incluye todas las anteriores. Cada salto se tiene que notar (el jugador,
/// 23-10: en la 10 apenas se notaba; luego, el triple de exigente): en la 10, enemigos con x3,7 de vida
/// y x2,1 de daño, y todos los combates con mutador. Los mutadores, uno por run y tipo de combate
/// (director, mutador_ascension), se ven en la pantalla de la ascensión (pausa).
pub const ASCENSIONES: [&str; 10] = [
    "Enemies have 45% more HP.",
    "Enemies deal 45% more damage.",
    "You start with one less flask.",
    "Elites carry a silver mutator.",
    "Enemies have another 60% more HP.",
    "You start with a random curse.",
    "Enemies deal another 45% more damage, and the merchant charges 50% more.",
    "Elites and act bosses carry a gold mutator.",
    "Enemies have another 60% more HP, and you gain 40% fewer runes.",
    "The other fights carry a silver mutator, and the final boss a prismatic one.",
];

/// Vida y daño de los enemigos con la ascensión `asc` (multiplicadores).
pub fn factores_ascension(asc: u32) -> (f64, f64) {
    let vida = if asc >= 1 { 1.45 } else { 1.0 } * if asc >= 5 { 1.6 } else { 1.0 } * if asc >= 9 { 1.6 } else { 1.0 };
    let danio = if asc >= 2 { 1.45 } else { 1.0 } * if asc >= 7 { 1.45 } else { 1.0 };
    (vida, danio)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resumen {
    pub personaje: String,
    pub ganada: bool,
    pub acto: usize,
    pub ascension: u32,
    pub semilla: String,
    pub golpes: u32,
    pub bendiciones: Vec<String>,
    /// Rivales que salieron (rogue_core::run::rival): la run siguiente prefiere otros.
    #[serde(default)]
    pub rivales: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Perfil {
    #[serde(default)]
    pub runs: u32,
    #[serde(default)]
    pub victorias: HashMap<String, u32>,
    /// Arma inicial elegida para cada personaje en la pantalla de Personajes.
    #[serde(default)]
    pub armas: HashMap<String, i32>,
    /// Apariencia elegida para cada personaje (id de su lista; sin entrada, la de siempre).
    #[serde(default)]
    pub apariencias: HashMap<String, String>,
    /// Invocación elegida para cada personaje que tiene (id de su lista; sin entrada, la primera).
    #[serde(default)]
    pub invocaciones: HashMap<String, String>,
    /// Ascensión más alta que se puede elegir con cada personaje (0 = solo la normal).
    #[serde(default)]
    pub ascension: HashMap<String, u32>,
    #[serde(default)]
    pub historial: Vec<Resumen>,
    /// Bendiciones conseguidas alguna vez (compendio).
    #[serde(default)]
    pub vistas: BTreeSet<String>,
}

impl Perfil {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, t);
        }
    }

    /// Rivales de la última run, para que la siguiente prefiera otros. Solo la última: con los débiles
    /// del acto 1 (6 rivales, 3 por run), recordar más los daría todos por vistos.
    pub fn rivales_recientes(&self) -> Vec<String> {
        self.historial.last().map(|r| r.rivales.clone()).unwrap_or_default()
    }

    pub fn max_ascension(&self, personaje: &str) -> u32 {
        self.ascension.get(personaje).copied().unwrap_or(0)
    }

    /// Apunta una run terminada: al ganar se desbloquea la ascensión siguiente de ese personaje.
    pub fn registrar(&mut self, r: Resumen) {
        self.runs += 1;
        if r.ganada {
            *self.victorias.entry(r.personaje.clone()).or_insert(0) += 1;
            let next = (r.ascension + 1).min(ASCENSIONES.len() as u32);
            let e = self.ascension.entry(r.personaje.clone()).or_insert(0);
            *e = (*e).max(next);
        }
        self.historial.push(r);
        if self.historial.len() > 50 {
            self.historial.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganar_desbloquea_la_siguiente_ascension() {
        let mut p = Perfil::default();
        let r = Resumen { personaje: "samurai".into(), ganada: true, ascension: 0, ..Default::default() };
        p.registrar(r.clone());
        assert_eq!(p.max_ascension("samurai"), 1);
        p.registrar(Resumen { ganada: false, ascension: 1, ..r.clone() });
        assert_eq!(p.max_ascension("samurai"), 1);
        p.registrar(Resumen { ascension: 1, ..r });
        assert_eq!(p.max_ascension("samurai"), 2);
        assert_eq!(p.max_ascension("monje"), 0);
        assert_eq!(p.runs, 3);
        assert_eq!(p.victorias["samurai"], 2);
    }
}

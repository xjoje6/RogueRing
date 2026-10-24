//! Reliquias: efectos pasivos de la run con niveles ("templar" en la gracia sube un nivel).
//! Definición en data/reliquias.json; cada nivel es un SpEffect generado por tools/ModBuilder.

use serde::{Deserialize, Serialize};

use crate::rewards::Rarity;
use crate::rng::Rng;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelicLevel {
    pub texto: String,
    /// Efectos que aplica la DLL por su cuenta (p. ej. {"runas": 1.3}).
    #[serde(default)]
    pub dll: std::collections::HashMap<String, f64>,
    /// Efectos con disparador (ver `efectos`).
    #[serde(default)]
    pub efectos: Vec<crate::efectos::Efecto>,
    #[serde(default)]
    pub pasivo: Option<crate::efectos::Pasivo>,
    #[serde(default)]
    pub al_coger: Option<crate::efectos::AlCoger>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelicDef {
    pub id: String,
    pub nombre: String,
    pub rareza: Rarity,
    #[serde(default)]
    pub icono: i32,
    /// Maldición: solo llega por eventos, nunca en tesoros ni tiendas.
    #[serde(default)]
    pub maldita: bool,
    /// Solo para este personaje ("monje", "samurai", "tanque"); None = común a todos.
    #[serde(default)]
    pub personaje: Option<String>,
    /// Aún no programada: no sale en las tiradas.
    #[serde(default)]
    pub pendiente: bool,
    /// Lo malo que trae (las míticas): se enseña en la carta antes de cogerla.
    #[serde(default)]
    pub contra: Option<String>,
    pub niveles: Vec<RelicLevel>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnedRelic {
    pub id: String,
    /// 0 = primer nivel.
    pub nivel: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RelicFile {
    pub reliquias: Vec<RelicDef>,
    /// Run de cooperativo: no salen las que dependen de no morir de un golpe (`GolpeMortal`). Con
    /// Seamless el juego no deja que «sin muerte» funcione (probado el 17-10) y no harían nada.
    #[serde(skip)]
    pub coop: bool,
}

impl RelicDef {
    /// ¿Necesita que el golpe mortal deje a 1 de vida (Última voluntad, Cebo, Inmortal)?
    pub fn de_golpe_mortal(&self) -> bool {
        self.niveles.iter().any(|l| l.efectos.iter().any(|e| e.cuando == crate::efectos::Cuando::GolpeMortal))
    }

    /// ¿Cambia tu tamaño (Goliat, Pequeñajo, Científico loco)? Solo se puede llevar una así.
    pub fn cambia_tamano(&self) -> bool {
        self.niveles.iter().any(|l| {
            l.pasivo.as_ref().is_some_and(|p| (p.tamano - 1.0).abs() > 0.001) || l.efectos.iter().any(|e| e.hacer.iter().any(|h| h.tamano > 0.0 && (h.tamano - 1.0).abs() > 0.001))
        })
    }
}

impl RelicFile {
    /// ¿Llevas ya una que cambia tu tamaño? Entonces no sale otra (19-10).
    pub fn lleva_tamano(&self, owned: &[OwnedRelic]) -> bool {
        owned.iter().any(|o| self.get(&o.id).is_some_and(|r| r.cambia_tamano()))
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.reliquias.iter().position(|r| r.id == id)
    }

    pub fn get(&self, id: &str) -> Option<&RelicDef> {
        self.reliquias.iter().find(|r| r.id == id)
    }

    /// SpEffect del nivel (misma fórmula que tools/ModBuilder).
    pub fn speffect(&self, id: &str, nivel: usize) -> Option<i32> {
        Some(79_610_000 + self.index_of(id)? as i32 * 10 + nivel as i32)
    }

    /// Multiplicador de un efecto de la DLL acumulado entre todas las reliquias que tienes.
    pub fn dll_factor(&self, owned: &[OwnedRelic], key: &str) -> f64 {
        owned.iter().filter_map(|o| self.get(&o.id)?.niveles.get(o.nivel)?.dll.get(key).copied()).product()
    }

    /// Suma de un efecto de la DLL entre todas las bendiciones que tienes (p. ej. opciones extra).
    pub fn dll_sum(&self, owned: &[OwnedRelic], key: &str) -> f64 {
        owned.iter().filter_map(|o| self.get(&o.id)?.niveles.get(o.nivel)?.dll.get(key).copied()).sum()
    }

    /// ¿Tienes alguna bendición con este efecto de la DLL?
    pub fn has(&self, owned: &[OwnedRelic], key: &str) -> bool {
        owned.iter().any(|o| self.get(&o.id).and_then(|r| r.niveles.get(o.nivel)).is_some_and(|l| l.dll.contains_key(key)))
    }

    /// Una bendición que no tengas, con la rareza pedida si la hay (si no, cualquiera menos las
    /// míticas). Solo comunes o del personaje; nunca maldiciones ni pendientes. Una mítica solo sale
    /// si se pide (y si no queda ninguna, None).
    pub fn roll(&self, owned: &[OwnedRelic], rareza: Rarity, rng: &mut Rng, personaje: Option<&str>) -> Option<RelicDef> {
        let free: Vec<&RelicDef> = self
            .reliquias
            .iter()
            .filter(|r| !r.maldita && !r.pendiente && !owned.iter().any(|o| o.id == r.id))
            .filter(|r| r.personaje.is_none() || r.personaje.as_deref() == personaje)
            .filter(|r| !(self.coop && r.de_golpe_mortal()))
            .filter(|r| !(r.cambia_tamano() && self.lleva_tamano(owned)))
            .collect();
        let same: Vec<&&RelicDef> = free.iter().filter(|r| r.rareza == rareza).collect();
        if !same.is_empty() {
            return Some((*same[rng.range(0, same.len() as i32) as usize]).clone());
        }
        let free: Vec<&&RelicDef> = free.iter().filter(|r| r.rareza != Rarity::Mitica).collect();
        if free.is_empty() || rareza == Rarity::Mitica {
            return None;
        }
        Some((*free[rng.range(0, free.len() as i32) as usize]).clone())
    }

    /// Una bendición cualquiera que no tengas (sin míticas, salvo que se pidan); con `rareza`, solo de esa.
    pub fn roll_any(&self, owned: &[OwnedRelic], rareza: Option<Rarity>, rng: &mut Rng, personaje: Option<&str>) -> Option<RelicDef> {
        let free: Vec<&RelicDef> = self
            .reliquias
            .iter()
            .filter(|r| !r.maldita && !r.pendiente && !owned.iter().any(|o| o.id == r.id))
            .filter(|r| r.personaje.is_none() || r.personaje.as_deref() == personaje)
            .filter(|r| !(self.coop && r.de_golpe_mortal()))
            .filter(|r| !(r.cambia_tamano() && self.lleva_tamano(owned)))
            .filter(|r| rareza.map_or(r.rareza != Rarity::Mitica, |x| r.rareza == x))
            .collect();
        if free.is_empty() {
            return None;
        }
        Some(free[rng.range(0, free.len() as i32) as usize].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carga_el_archivo_real_y_calcula_ids() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reliquias.json")).unwrap();
        let f: RelicFile = serde_json::from_str(&t).unwrap();
        assert!(f.reliquias.len() >= 30);
        assert_eq!(f.speffect(&f.reliquias[1].id, 0), Some(79_610_010));
        let owned = vec![OwnedRelic { id: "conciencia".into(), nivel: 0 }, OwnedRelic { id: "ojo_avizor".into(), nivel: 0 }];
        assert!((f.dll_factor(&owned, "runas") - 0.8).abs() < 1e-9);
        assert_eq!(f.dll_factor(&[], "runas"), 1.0);
        assert_eq!(f.dll_sum(&owned, "opciones"), 1.0);
        assert!(f.has(&owned, "opciones"));
        // Las míticas solo salen si se piden, y todas dicen lo malo que traen.
        let mut rng = Rng::named(2, "c");
        let m = f.roll(&[], Rarity::Mitica, &mut rng, None).unwrap();
        assert_eq!(m.rareza, Rarity::Mitica);
        for r in f.reliquias.iter().filter(|r| r.rareza == Rarity::Mitica) {
            assert!(r.contra.as_deref().is_some_and(|c| !c.is_empty()), "{}: mítica sin precio", r.id);
        }
        let todas: Vec<OwnedRelic> = f.reliquias.iter().filter(|r| r.rareza == Rarity::Mitica).map(|r| OwnedRelic { id: r.id.clone(), nivel: 0 }).collect();
        assert!(f.roll(&todas, Rarity::Mitica, &mut rng, None).is_none());
        // Con una que cambia el tamaño, no sale otra.
        let tam: Vec<&str> = f.reliquias.iter().filter(|r| r.cambia_tamano()).map(|r| r.id.as_str()).collect();
        assert!(tam.len() >= 3, "{tam:?}");
        let una = vec![OwnedRelic { id: "pequenajo".into(), nivel: 0 }];
        for _ in 0..300 {
            for rar in [Rarity::Comun, Rarity::PocoComun, Rarity::Rara, Rarity::Mitica] {
                assert!(f.roll(&una, rar, &mut rng, None).is_none_or(|r| !r.cambia_tamano()));
            }
            assert!(f.roll_any(&una, None, &mut rng, None).is_none_or(|r| !r.cambia_tamano()));
        }
        // Las tiradas nunca dan maldiciones, pendientes, míticas sin pedirlas ni bendiciones de otro personaje.
        let mut rng = Rng::named(1, "t");
        for _ in 0..200 {
            let r = f.roll(&[], Rarity::PocoComun, &mut rng, None).unwrap();
            assert!(!r.maldita && !r.pendiente && r.rareza != Rarity::Mitica && r.personaje.is_none(), "{}", r.id);
            let a = f.roll_any(&[], None, &mut rng, None).unwrap();
            assert!(a.rareza != Rarity::Mitica, "{}", a.id);
        }
    }
}

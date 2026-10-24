//! Idioma de los textos del mod: el del juego. El código y los datos están en inglés; las traducciones
//! viven en data/i18n/<idioma>.json: «ui» (textos de pantalla, por su texto en inglés) y lo de los
//! datos por id (bendiciones, eventos, personajes...). Lo que no tenga traducción se queda en inglés.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::characters::CharacterDef;
use crate::events::EventFile;
use crate::relics::RelicFile;
use crate::rewards::Item;
use crate::run::ActDef;

static UI: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Activa un idioma con sus textos de pantalla (una vez, al arrancar).
pub fn activar(ui: HashMap<String, String>) {
    let _ = UI.set(ui);
}

/// ¿Hay un idioma distinto del inglés activo?
pub fn traducido() -> bool {
    UI.get().is_some()
}

/// Texto de pantalla en el idioma activo.
pub fn t(en: &'static str) -> &'static str {
    match UI.get().and_then(|m| m.get(en)) {
        Some(s) => s.as_str(),
        None => en,
    }
}

/// Como `t`, para textos que no son literales (tipos de arma, nombres de acto...).
pub fn td(en: &str) -> String {
    UI.get().and_then(|m| m.get(en)).cloned().unwrap_or_else(|| en.to_string())
}

/// Plantilla con huecos «{}» que se rellenan en orden: `tf("Flasks {} / {}", &[&n, &max])`.
pub fn tf(en: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut resto = t(en);
    let mut args = args.iter();
    while let Some(i) = resto.find("{}") {
        out.push_str(&resto[..i]);
        if let Some(a) = args.next() {
            out.push_str(&a.to_string());
        }
        resto = &resto[i + 2..];
    }
    out.push_str(resto);
    out
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TradBendicion {
    pub nombre: String,
    /// Texto de cada nivel, en orden.
    #[serde(default)]
    pub niveles: Vec<String>,
    #[serde(default)]
    pub contra: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TradOpcion {
    pub texto: String,
    #[serde(default)]
    pub detalle: String,
    #[serde(default)]
    pub resultado: String,
    /// Resultado de cada rama al azar, en orden.
    #[serde(default)]
    pub azar: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TradEvento {
    pub titulo: String,
    pub texto: String,
    #[serde(default)]
    pub opciones: Vec<TradOpcion>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TradPersonaje {
    pub nombre: String,
    pub lema: String,
    pub pasiva: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TradMutador {
    pub nombre: String,
    #[serde(default)]
    pub texto: String,
}

/// Traducción de un idioma (data/i18n/<idioma>.json).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Traduccion {
    /// Textos de pantalla por su texto en inglés (también tipos de arma, nombres de acto y
    /// Ascensiones, que se traducen al enseñarlos).
    #[serde(default)]
    pub ui: HashMap<String, String>,
    #[serde(default)]
    pub bendiciones: HashMap<String, TradBendicion>,
    #[serde(default)]
    pub eventos: HashMap<String, TradEvento>,
    #[serde(default)]
    pub personajes: HashMap<String, TradPersonaje>,
    #[serde(default)]
    pub mutadores: HashMap<String, TradMutador>,
    /// Nombre de cada objeto del pool, por id (sale de los textos del juego: tools/i18n.py).
    #[serde(default)]
    pub objetos: HashMap<String, String>,
    /// Nombre de cada enemigo por su nombre en inglés (textos del juego: tools/i18n.py).
    #[serde(default)]
    pub enemigos: HashMap<String, String>,
}

impl Traduccion {
    pub fn bendiciones(&self, f: &mut RelicFile) {
        for r in &mut f.reliquias {
            let Some(t) = self.bendiciones.get(&r.id) else { continue };
            r.nombre = t.nombre.clone();
            for (n, texto) in r.niveles.iter_mut().zip(&t.niveles) {
                n.texto = texto.clone();
            }
            if let (Some(c), Some(tc)) = (r.contra.as_mut(), t.contra.as_ref()) {
                *c = tc.clone();
            }
        }
    }

    pub fn eventos(&self, f: &mut EventFile) {
        for e in &mut f.eventos {
            let Some(t) = self.eventos.get(&e.id) else { continue };
            e.titulo = t.titulo.clone();
            e.texto = t.texto.clone();
            for (o, to) in e.opciones.iter_mut().zip(&t.opciones) {
                o.texto = to.texto.clone();
                o.detalle = to.detalle.clone();
                o.resultado = to.resultado.clone();
                for (b, tb) in o.azar.iter_mut().zip(&to.azar) {
                    b.resultado = tb.clone();
                }
            }
        }
    }

    pub fn personajes(&self, l: &mut [CharacterDef]) {
        for c in l {
            let Some(t) = self.personajes.get(&c.id) else { continue };
            c.nombre = t.nombre.clone();
            c.lema = t.lema.clone();
            c.pasiva = t.pasiva.clone();
            for a in &mut c.apariencias {
                if let Some(n) = self.ui.get(&a.nombre) {
                    a.nombre = n.clone();
                }
            }
            for i in &mut c.invocaciones {
                if let Some(n) = self.ui.get(&i.nombre) {
                    i.nombre = n.clone();
                }
            }
        }
    }

    pub fn objetos(&self, l: &mut [Item]) {
        for it in l {
            if let Some(n) = self.objetos.get(&it.id.to_string()) {
                it.nombre = n.clone();
            }
        }
    }

    pub fn actos(&self, l: &mut [ActDef]) {
        for a in l {
            a.name = self.ui.get(&a.name).cloned().unwrap_or_else(|| a.name.clone());
            for e in &mut a.encounters {
                if let Some(n) = self.enemigos.get(&e.name) {
                    e.name = n.clone();
                }
            }
        }
    }
}

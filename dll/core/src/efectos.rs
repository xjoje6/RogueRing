//! Lenguaje de efectos de las bendiciones (data/reliquias.json, campo `efectos` y `pasivo` de cada
//! nivel). La DLL lo interpreta en cada fotograma del combate: así una bendición nueva es casi
//! siempre solo datos.
//!
//! Un efecto dice CUÁNDO actúa (un disparador y sus condiciones) y QUÉ hace (una lista de acciones).
//! El pasivo son modificadores fijos (daño, defensa, vida...) y las pilas que pueden acumular sus
//! acciones.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cuándo actúa un efecto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cuando {
    /// Cada golpe que baja la vida del boss.
    Golpe,
    /// Cada golpe que recibes.
    Recibir,
    /// Cada `valor` segundos de combate.
    Segundos,
    /// Al desviar un golpe.
    Desvio,
    /// Al romper la postura del boss.
    Postura,
    /// Cuando estalla cualquier estado en el boss (sangrado, podredumbre, congelación...).
    Estado,
    /// Al beber un frasco (subida de vida grande que no pone la DLL).
    Frasco,
    Inicio,
    /// Al bajar tu vida de `valor` (fracción).
    VidaBaja,
    /// Al bajar la vida del boss de `valor` (fracción).
    BossVidaBaja,
    Rodar,
    /// Un golpe que te iba a matar (se queda a 1 de vida y dispara).
    GolpeMortal,
    Victoria,
    /// Al usar una ceniza de guerra o un hechizo (gasto de PF).
    Ceniza,
    /// Ataque cargado (RT mantenido y soltado).
    Cargado,
    /// Le quitas al boss `valor` (fracción de su vida) en 2,5 s.
    Rafaga,
}

fn uno() -> u32 {
    1
}
fn uno_f() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Efecto {
    pub cuando: Cuando,
    /// Segundos (Segundos) o umbral de vida (VidaBaja, BossVidaBaja).
    #[serde(default)]
    pub valor: f64,
    /// Solo una de cada `cada` veces.
    #[serde(default = "uno")]
    pub cada: u32,
    /// Probabilidad (0..1). Se tira una vez cada vez que se dispara (al empezar el combate, una vez
    /// por combate).
    #[serde(default = "uno_f")]
    pub prob: f64,
    /// Alternativa del efecto anterior de la misma bendición: actúa justo cuando a ese le falla la
    /// probabilidad (para «una cosa u otra», sin que salgan las dos ni ninguna).
    #[serde(default)]
    pub alternativa: bool,
    /// Una vez por combate.
    #[serde(default)]
    pub una_vez: bool,
    /// Segundos mínimos entre disparos.
    #[serde(default)]
    pub enfriamiento: f64,
    /// Condiciones (0 = sin condición).
    #[serde(default)]
    pub si_vida_menor: f64,
    #[serde(default)]
    pub si_boss_vida_menor: f64,
    /// Boss a esta distancia o menos (metros).
    #[serde(default)]
    pub si_cerca: f64,
    /// Sin recibir daño en los últimos segundos.
    #[serde(default)]
    pub si_sin_danio: f64,
    /// Sin haber bebido en todo el combate.
    #[serde(default)]
    pub si_sin_frasco: bool,
    /// Con Estado: solo este estado (0 veneno, 1 podredumbre, 2 sangrado, 3 plaga, 4 congelación,
    /// 5 sueño, 6 locura).
    #[serde(default)]
    pub estado: Option<usize>,
    pub hacer: Vec<Accion>,
}

/// Una acción. Todos los campos son opcionales; se hacen todos los que tengan valor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accion {
    /// Proyectil (fila de Bullet) lanzado contra el boss; solo se ve (el daño lo pone la DLL).
    #[serde(default)]
    pub bala: Option<i32>,
    /// Cuántos (se lanzan seguidos).
    #[serde(default = "uno")]
    pub balas: u32,
    /// Lanzado desdel jugador sin objetivo (efectos alrededor de ti).
    #[serde(default)]
    pub en_jugador: bool,
    /// Daño al boss: fracción de su vida máxima.
    #[serde(default)]
    pub danio_boss: f64,
    /// Daño al boss: fracción del golpe que lo disparó.
    #[serde(default)]
    pub danio_golpe: f64,
    /// Daño al boss: tantos golpes tuyos (la media de tus golpes en este combate). Así crece con el
    /// arma y el personaje, también contra los jefes con mucha vida.
    #[serde(default)]
    pub danio_golpes: f64,
    /// Daño al boss: fracción de TU vida máxima.
    #[serde(default)]
    pub danio_vida: f64,
    /// Daño al boss: fracción del daño que acabas de recibir.
    #[serde(default)]
    pub danio_recibido: f64,
    /// Daño al boss: fracción de lo que te acabas de curar.
    #[serde(default)]
    pub danio_curado: f64,
    /// Cura: fracción de tu vida máxima.
    #[serde(default)]
    pub curar: f64,
    /// Cura: fracción del golpe que disparó el efecto.
    #[serde(default)]
    pub curar_golpe: f64,
    /// SpEffect al jugador / al boss (ids del juego o de tools/ModBuilder).
    #[serde(default)]
    pub efecto_jugador: Option<i32>,
    #[serde(default)]
    pub efecto_boss: Option<i32>,
    /// Suma una pila (definida en el pasivo de la misma bendición).
    #[serde(default)]
    pub pila: Option<String>,
    /// Quita todas las pilas de este nombre.
    #[serde(default)]
    pub quitar_pila: Option<String>,
    /// Suma 1 a un contador de la run (persiste entre combates; lo leen los pasivos `run_*`).
    #[serde(default)]
    pub run: Option<String>,
    /// Invulnerable estos segundos.
    #[serde(default)]
    pub invulnerable: f64,
    /// Recibes este daño (multiplicador, p. ej. 0.5) durante `segundos`.
    #[serde(default)]
    pub defensa: f64,
    /// Haces este daño (multiplicador) durante `segundos`.
    #[serde(default)]
    pub danio: f64,
    /// Velocidad de ataque (multiplicador) durante `segundos`.
    #[serde(default)]
    pub velocidad: f64,
    /// El boss recibe más daño (multiplicador, p. ej. 1.5) durante `segundos`.
    #[serde(default)]
    pub boss_debil: f64,
    /// El boss hace este daño (multiplicador, p. ej. 0.85) durante `segundos`.
    #[serde(default)]
    pub boss_flojo: f64,
    /// Duración de defensa/danio/velocidad/boss_debil/boss_flojo.
    #[serde(default)]
    pub segundos: f64,
    #[serde(default)]
    pub runas: u32,
    /// Rompe la postura del boss.
    #[serde(default)]
    pub postura: bool,
    /// Guarda esta fracción de todo el daño que hagas durante `segundos` y se la da al boss al final.
    #[serde(default)]
    pub marca: f64,
    /// Da este daño aplazado: el que recibes llega `segundos` después (fracción aplazada).
    #[serde(default)]
    pub aplazar: f64,
    /// Borra el daño aplazado pendiente.
    #[serde(default)]
    pub perdonar: bool,
    /// Aguante y PF al máximo.
    #[serde(default)]
    pub recargar: bool,
    /// Revive con esta fracción de vida (tras un golpe mortal).
    #[serde(default)]
    pub revivir: f64,
    /// Quita esta fracción de la postura máxima del boss.
    #[serde(default)]
    pub postura_danio: f64,
    /// Quemadura: fracción de la vida máxima del boss por segundo durante `segundos` (se acumula).
    #[serde(default)]
    pub quemar: f64,
    /// Anula el golpe que acabas de recibir (te devuelve esa vida).
    #[serde(default)]
    pub anular: bool,
    /// Robo de vida (fracción del daño que haces) durante `segundos`.
    #[serde(default)]
    pub robo: f64,
    /// Te devuelve esta fracción de los PF que acabas de gastar.
    #[serde(default)]
    pub devolver_pf: f64,
    /// Gastas además esta fracción de los PF que acabas de gastar (si no los tienes, se paga en vida).
    #[serde(default)]
    pub gastar_pf: f64,
    /// Pierdes esta fracción de tu vida máxima.
    #[serde(default)]
    pub coste_vida: f64,
    /// Guarda esta fracción del daño recibido en las púas (se sueltan con `soltar_puas`).
    #[serde(default)]
    pub puas: f64,
    #[serde(default)]
    pub soltar_puas: bool,
    /// Guarda esta fracción de lo curado en la bomba; al llegar a tu vida máxima cae sobre el boss.
    #[serde(default)]
    pub bomba: f64,
    /// Invoca un aliado (ceniza espiritual del juego: [modelo, NpcParam, NpcThinkParam]); si hay
    /// varios, uno al azar. Dura `segundos`.
    #[serde(default)]
    pub invocar: Vec<[i32; 3]>,
    /// Cuántos aliados (del mismo tipo).
    #[serde(default = "uno")]
    pub invocados: u32,
    /// Tu tamaño pasa a ser este (1 = normal) el resto del combate.
    #[serde(default)]
    pub tamano: f64,
    /// El boss se mueve a esta velocidad (p. ej. 0.05: casi quieto) durante `segundos`.
    #[serde(default)]
    pub frenar_boss: f64,
}

/// Pila acumulable (Ritmo, Titán, Sinfonía...). Cada pila suma sus valores; caducan a los
/// `segundos` del último refresco (0 = no caducan en el combate).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pila {
    pub id: String,
    pub max: u32,
    #[serde(default)]
    pub segundos: f64,
    #[serde(default)]
    pub danio: f64,
    /// Menos daño recibido por pila (0.01 = un 1 % menos).
    #[serde(default)]
    pub defensa: f64,
    #[serde(default)]
    pub velocidad: f64,
    /// El boss recibe más daño por pila.
    #[serde(default)]
    pub boss_debil: f64,
}

/// Modificadores fijos mientras tengas la bendición.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pasivo {
    #[serde(default = "uno_f")]
    pub danio: f64,
    /// Daño recibido (multiplicador).
    #[serde(default = "uno_f")]
    pub defensa: f64,
    /// Vida máxima (multiplicador).
    #[serde(default = "uno_f")]
    pub vida: f64,
    /// Velocidad de ataque (multiplicador).
    #[serde(default = "uno_f")]
    pub velocidad: f64,
    /// Tamaño del personaje (multiplicador).
    #[serde(default = "uno_f")]
    pub tamano: f64,
    /// Daño con tu vida por debajo de esta fracción: [umbral, multiplicador].
    #[serde(default)]
    pub danio_vida_menor: Option<[f64; 2]>,
    /// Daño contra el boss por debajo de esta fracción de vida.
    #[serde(default)]
    pub danio_boss_vida_menor: Option<[f64; 2]>,
    /// Daño por cada unidad de un contador de la run (p. ej. {"crecer": 0.03}).
    #[serde(default)]
    pub run_danio: HashMap<String, f64>,
    /// Vida máxima por cada unidad de un contador de la run.
    #[serde(default)]
    pub run_vida: HashMap<String, f64>,
    /// Los frascos no curan.
    #[serde(default)]
    pub frascos_no_curan: bool,
    /// Tope de vida (fracción de la máxima).
    #[serde(default)]
    pub tope_vida: f64,
    /// Remate: el boss por debajo de esta fracción de vida muere al siguiente golpe.
    #[serde(default)]
    pub remate: f64,
    /// Probabilidad de recibir la mitad de daño.
    #[serde(default)]
    pub parada: f64,
    /// Robo de vida: fracción del daño que haces.
    #[serde(default)]
    pub robo: f64,
    #[serde(default)]
    pub pilas: Vec<Pila>,
}

impl Default for Pasivo {
    fn default() -> Self {
        Pasivo {
            danio: 1.0,
            defensa: 1.0,
            vida: 1.0,
            velocidad: 1.0,
            tamano: 1.0,
            danio_vida_menor: None,
            danio_boss_vida_menor: None,
            run_danio: HashMap::new(),
            run_vida: HashMap::new(),
            frascos_no_curan: false,
            tope_vida: 0.0,
            remate: 0.0,
            parada: 0.0,
            robo: 0.0,
            pilas: vec![],
        }
    }
}

/// Lo que pasa al coger la bendición (una vez).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlCoger {
    #[serde(default)]
    pub runas: u32,
    /// Bendiciones al azar (cualquier rareza menos mítica).
    #[serde(default)]
    pub bendiciones: u32,
    /// Bendiciones al azar de esta rareza ("Comun", "PocoComun", "Rara").
    #[serde(default)]
    pub bendiciones_rareza: Option<String>,
    /// Cambia todas tus bendiciones por otras al azar de rareza rara.
    #[serde(default)]
    pub pandora: bool,
    #[serde(default)]
    pub forja: u32,
    #[serde(default)]
    pub frascos: i32,
    /// Maldiciones al azar (el precio de algunas míticas).
    #[serde(default)]
    pub maldiciones: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_un_efecto_completo() {
        let e: Efecto = serde_json::from_str(r#"{"cuando":"golpe","cada":4,"hacer":[{"bala":10683000,"danio_boss":0.012}]}"#).unwrap();
        assert_eq!(e.cuando, Cuando::Golpe);
        assert_eq!(e.cada, 4);
        assert_eq!(e.prob, 1.0);
        assert_eq!(e.hacer[0].balas, 1);
        let p: Pasivo = serde_json::from_str(r#"{"danio":1.2,"pilas":[{"id":"ritmo","max":10,"segundos":4,"velocidad":0.03}]}"#).unwrap();
        assert_eq!(p.defensa, 1.0);
        assert_eq!(p.pilas[0].max, 10);
        // Un campo mal escrito es un error, no se ignora.
        assert!(serde_json::from_str::<Accion>(r#"{"danio_bos":0.1}"#).is_err());
    }
}

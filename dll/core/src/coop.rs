//! Cooperativo: lo que se cuentan los juegos de una sesión y cómo se decide el siguiente nodo.
//!
//! Todos generan la misma run a partir de la misma semilla (la del anfitrión), así que solo viajan
//! los votos. Como en Slay the Spire 2: cada uno vota un nodo y nadie entra hasta que han votado
//! todos; si los votos no coinciden, se sortea entre ellos. El sorteo sale de la semilla y del paso,
//! así que cada juego llega al mismo resultado por su cuenta, sin que nadie mande. Un voto no se
//! cambia: con votos que cambian, dos juegos podrían decidir con votos distintos.
//!
//! Cada voto lleva el paso en que se hizo y se sigue mandando después de decidir. Quien vota el
//! último decide en ese mismo fotograma y sigue adelante: si su voto se borrara al decidir, los demás
//! no llegarían a verlo y se quedarían esperándolo (16-10: el invitado se quedó en la carga y el
//! anfitrión en el mapa «esperando a» él).
//!
//! Los eventos se votan igual: la opción que sale es la misma para todos, y con ella el resultado
//! (que sale del sorteo común) y el combate si lo hay. Si cada uno eligiera la suya, uno podía irse a
//! pelear y el otro no.
//!
//! El anfitrión manda además cómo va el rival (la barra de estados bajo su vida): los demás no la
//! pueden leer bien (sus resistencias no se sincronizan).

use serde::{Deserialize, Serialize};

/// Versión del protocolo: si no coincide, ese compañero no cuenta (se avisa en el registro).
pub const VERSION: u32 = 8;
/// Segundos sin noticias de un compañero para darlo por desconectado (y dejar de esperar su voto).
pub const SIN_NOTICIAS: f64 = 10.0;

/// Dónde está cada jugador dentro de la run: acto y nodos recorridos. Los votos valen solo para el
/// paso en que se hicieron.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Paso {
    pub acto: usize,
    pub nodo: usize,
}

/// Lo que cada juego cuenta de sí mismo varias veces por segundo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Estado {
    pub version: u32,
    pub nombre: String,
    /// Pantalla en la que está: «mapa», «combate», «recompensa»...
    pub fase: String,
    pub paso: Paso,
    /// Semilla de la run: solo cuentan los votos de la misma run.
    pub semilla: Option<String>,
    /// Último nodo votado (columna, fila) y el paso en que se votó.
    pub voto: Option<(Paso, (i32, i32))>,
    /// Nodo bajo el cursor en el mapa: los demás lo ven moverse.
    #[serde(default)]
    pub cursor: Option<(i32, i32)>,
    /// Última opción votada en un evento y su paso (como `voto`).
    #[serde(default)]
    pub voto_evento: Option<(Paso, usize)>,
    /// Vida (actual, máxima), redondeada al 2 % para no mandar un mensaje por cada punto.
    #[serde(default)]
    pub vida: Option<(i32, i32)>,
    /// Tamaño del personaje si no es el normal (Diminuto, Goliat...): cada juego solo escala el suyo,
    /// así que los demás lo escalan con esto.
    #[serde(default)]
    pub tamano: Option<f32>,
    /// Solo el anfitrión: vida de las invocaciones del Invocador (entidad, vida), redondeada al 5 %. Su
    /// copia es la que pelea de verdad (los rivales son suyos); los demás ajustan la suya a esta.
    #[serde(default)]
    pub invocaciones: Vec<(u32, i32)>,
    /// Listo para el combate que sale de un evento, en este paso: se va cuando lo están todos.
    #[serde(default)]
    pub listo: Option<Paso>,
    /// Caídos (director/caido.rs): a quién estoy levantando (SteamID) y cuánto llevo, de 0 a 1. El
    /// caído ve la misma barra.
    #[serde(default)]
    pub levantando: Option<(u64, f32)>,
    /// El combate en que está (protocolo.rs): paso, si ha llegado, si ha muerto y su oleada.
    #[serde(default)]
    pub combate: Option<crate::protocolo::Combate>,
    /// Último combate cerrado y cómo acabó: el del anfitrión manda (protocolo.rs).
    #[serde(default)]
    pub ultimo: Option<(Paso, crate::protocolo::Resultado)>,
    /// Sucesos de los últimos segundos con su número (creciente: cada juego hace cada uno una vez,
    /// aunque el estado le llegue varias veces).
    #[serde(default)]
    pub sucesos: Vec<(u64, Suceso)>,
    /// Solo el anfitrión: la barra de estados del rival (lo que dibuja la DLL bajo su vida).
    #[serde(default)]
    pub rival: Option<Vec<EstadoRival>>,
    /// Solo el anfitrión: la ascensión de su run. La elige él y los demás juegan con la misma (el jugador,
    /// 23-10: el invitado entraba siempre en la 0).
    #[serde(default)]
    pub ascension: Option<u32>,
}

/// Algo que pasa una vez y tienen que saber los demás (16-10: no se veían las bolas de fuego
/// del otro ni sus invocaciones).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Suceso {
    /// Daño de tus bendiciones a un rival. Lo aplica el anfitrión: los rivales son suyos y lo que un
    /// invitado escribe en su copia se pierde.
    Danio { ent: u32, n: i32 },
    /// Proyectil de una bendición (copia sin ataque, data/proyectiles.json), para que los demás lo vean.
    Bala { id: i32, objetivo: Option<u32>, desde: [f32; 3] },
    /// Invocación del Invocador. Lo que crea la DLL no viaja por la red (ni lo del anfitrión): cada juego
    /// crea las de todos, junto a quien invoca, en sus entidades y con su mejora `sp`. Las del anfitrión
    /// son las que pegan de verdad (los rivales son suyos); las demás, para verlas. `nombre`: la ceniza.
    Invocar { chr: i32, npc: i32, think: i32, posiciones: Vec<[f32; 2]>, tandas: usize, sp: i32, centro: [f32; 3], nombre: String },
    /// Aliado de una bendición (un espíritu al azar unos segundos): igual que `Invocar`, cada juego lo
    /// crea junto a quien lo invocó y lo quita cuando se le acaba el tiempo.
    Aliado { chr: i32, npc: i32, think: i32, n: u32, segundos: f64, centro: [f32; 3] },
    /// Versión de pruebas: el anfitrión ha elegido este rival (id del encuentro) en su menú de pruebas;
    /// cada juego entra en ese combate desde el mapa.
    Rival { id: String },
    /// Cooperativo, caídos (director/caido.rs): he levantado a este jugador (SteamID).
    Levantar { a: u64 },
}

/// Un estado del juego en el rival (sangrado, congelación...) tal como lo dibuja la barra: su índice
/// en la lista de estados de la DLL, nombre y color, cuánto lleva (0 a 1), segundos desde que saltó
/// (f32::MAX si no) y el texto de al lado. Los efectos de las bendiciones no viajan: son de cada uno.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EstadoRival {
    pub estado: usize,
    pub nombre: String,
    pub color: [u8; 3],
    pub lleno: f32,
    pub destello: f32,
    pub texto: String,
}

pub fn a_bytes(e: &Estado) -> Vec<u8> {
    serde_json::to_vec(e).unwrap_or_default()
}

pub fn de_bytes(b: &[u8]) -> Option<Estado> {
    serde_json::from_slice(b).ok()
}

/// Votos de un paso: (SteamID, nodo) de todos los que cuentan, el propio incluido. Con todos los
/// votos, el nodo al que se va: si coinciden, ese; si no, uno de ellos al azar con la semilla y el
/// paso (el mismo en todos los juegos, porque los votos se ordenan por SteamID).
pub fn decidir(votos: &[(u64, (i32, i32))], semilla: &str, paso: Paso) -> Option<(i32, i32)> {
    sortear(votos, semilla, "voto", paso)
}

/// Como `decidir`, para cualquier votación (`que` la distingue en el sorteo: «voto», «evento»).
pub fn sortear<T: Copy + PartialEq>(votos: &[(u64, T)], semilla: &str, que: &str, paso: Paso) -> Option<T> {
    let mut v = votos.to_vec();
    v.sort_by_key(|x| x.0);
    let primero = v.first()?.1;
    if v.iter().all(|x| x.1 == primero) {
        return Some(primero);
    }
    let h = crate::rng::hash_str(&format!("{semilla}|{que}|{}|{}", paso.acto, paso.nodo));
    Some(v[(h % v.len() as u64) as usize].1)
}

/// ¿Está todo votado? `companeros`: (SteamID, estado, conectado). Cuentan los conectados de la misma
/// run con su voto de este paso, aunque ya hayan decidido y sigan adelante; los que aún no han
/// votado en este paso (siguen en la recompensa o mirando el mapa) se esperan.
pub fn votos_completos(yo: (u64, &Estado), companeros: &[(u64, Estado, bool)]) -> Option<Vec<(u64, (i32, i32))>> {
    completos(yo, companeros, |e| e.voto)
}

/// Como `votos_completos`, para cualquier votación: `de` saca el voto (y su paso) de un estado.
pub fn completos<T: Copy>(yo: (u64, &Estado), companeros: &[(u64, Estado, bool)], de: impl Fn(&Estado) -> Option<(Paso, T)>) -> Option<Vec<(u64, T)>> {
    let paso = yo.1.paso;
    let mio = de(yo.1).filter(|v| v.0 == paso)?.1;
    let mut v = vec![(yo.0, mio)];
    for (id, e, conectado) in companeros {
        if !conectado || e.version != VERSION {
            continue;
        }
        // En la sala pero aún no en esta run (en un menú): se le espera (23-10: el anfitrión se fue
        // solo al primer combate y el invitado se quedó fuera).
        if e.semilla != yo.1.semilla {
            return None;
        }
        v.push((*id, de(e).filter(|v| v.0 == paso)?.1));
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(paso: Paso, voto: Option<(i32, i32)>) -> Estado {
        Estado { version: VERSION, semilla: Some("CO1".into()), paso, voto: voto.map(|n| (paso, n)), ..Default::default() }
    }

    #[test]
    fn coinciden_va_al_votado() {
        let p = Paso { acto: 0, nodo: 2 };
        assert_eq!(decidir(&[(5, (1, 3)), (9, (1, 3))], "CO1", p), Some((1, 3)));
    }

    #[test]
    fn el_sorteo_es_el_mismo_en_todos_los_juegos() {
        let p = Paso { acto: 1, nodo: 4 };
        let a = decidir(&[(5, (1, 3)), (9, (2, 3))], "CO1", p);
        let b = decidir(&[(9, (2, 3)), (5, (1, 3))], "CO1", p);
        assert_eq!(a, b);
        assert!(a == Some((1, 3)) || a == Some((2, 3)));
        // Con muchas semillas salen los dos votos alguna vez.
        let distintos: std::collections::HashSet<_> =
            (0..40).map(|k| decidir(&[(5, (1, 3)), (9, (2, 3))], &format!("S{k}"), p)).collect();
        assert_eq!(distintos.len(), 2);
    }

    #[test]
    fn se_espera_a_todos() {
        let p = Paso { acto: 0, nodo: 1 };
        let yo = est(p, Some((1, 2)));
        // Compañero sin votar: no se decide.
        assert!(votos_completos((1, &yo), &[(2, est(p, None), true)]).is_none());
        // Compañero aún en el paso anterior: tampoco.
        assert!(votos_completos((1, &yo), &[(2, est(Paso { acto: 0, nodo: 0 }, None), true)]).is_none());
        // Desconectado: no se le espera.
        assert_eq!(votos_completos((1, &yo), &[(2, est(p, None), false)]).map(|v| v.len()), Some(1));
        // Votado: ya está.
        assert_eq!(votos_completos((1, &yo), &[(2, est(p, Some((3, 2))), true)]).map(|v| v.len()), Some(2));
        // Sin mi voto, nada.
        assert!(votos_completos((1, &est(p, None)), &[(2, est(p, Some((3, 2))), true)]).is_none());
    }

    #[test]
    fn eventos_por_votos() {
        let p = Paso { acto: 1, nodo: 3 };
        let mut yo = est(p, None);
        yo.voto_evento = Some((p, 0));
        let mut el = est(p, None);
        // Sin su voto, nada; con él, se sortea entre las dos opciones igual en los dos juegos.
        assert!(completos((1, &yo), &[(2, el.clone(), true)], |e| e.voto_evento).is_none());
        el.voto_evento = Some((p, 2));
        let v = completos((1, &yo), &[(2, el.clone(), true)], |e| e.voto_evento).unwrap();
        let mut v2 = completos((2, &el), &[(1, yo.clone(), true)], |e| e.voto_evento).unwrap();
        v2.reverse();
        assert_eq!(sortear(&v, "CO1", "evento", p), sortear(&v2, "CO1", "evento", p));
        // El voto del mapa no cuenta como voto del evento.
        let mut solo_mapa = est(p, Some((1, 1)));
        solo_mapa.voto_evento = None;
        assert!(completos((1, &yo), &[(2, solo_mapa, true)], |e| e.voto_evento).is_none());
    }

    #[test]
    fn cuenta_el_voto_de_quien_ya_ha_seguido() {
        // El compañero votó el último, decidió en su juego y ya va por el combate del paso siguiente:
        // su voto de este paso sigue contando.
        let p = Paso { acto: 0, nodo: 1 };
        let yo = est(p, Some((1, 2)));
        let mut el = est(Paso { acto: 0, nodo: 2 }, None);
        el.voto = Some((p, (1, 2)));
        assert_eq!(votos_completos((1, &yo), &[(2, el, true)]), Some(vec![(1, (1, 2)), (2, (1, 2))]));
        // Un voto de un paso anterior no vale para este.
        let mut viejo = est(p, None);
        viejo.voto = Some((Paso { acto: 0, nodo: 0 }, (0, 1)));
        assert!(votos_completos((1, &yo), &[(2, viejo)].map(|(i, e)| (i, e, true))).is_none());
        // Mi voto de un paso anterior tampoco.
        let mut yo_viejo = est(p, None);
        yo_viejo.voto = Some((Paso { acto: 0, nodo: 0 }, (0, 1)));
        assert!(votos_completos((1, &yo_viejo), &[]).is_none());
    }
}

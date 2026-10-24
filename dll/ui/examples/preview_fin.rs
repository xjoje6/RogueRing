//! Vista previa de la pantalla de fin con el gráfico, a partir de una run guardada en runtime/runs/.
//! cargo run --release -p rogue-ui --example preview_fin -- <semilla> <salida.png> [segundos] [nodo]
//! SIN_GRAFICO=1: sin historia (solo la banda). GANADA=1: la victoria.
#[path = "comun/mod.rs"]
mod comun;

use rogue_core::relics::RelicFile;
use rogue_core::run::RunState;
use rogue_ui::{fin, grafico};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut v = comun::Vista::new();
    let run = RunState::from_json(&std::fs::read_to_string(v.root.join(format!("runtime/runs/{}.json", a[1]))).unwrap()).unwrap();
    let t: f32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(10.0);
    let sel: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(usize::MAX);
    let tex = v.mapa(&format!("extract/ui/fondos/acto{}.png", run.act_index + 1));
    let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(v.root.join("data/reliquias.json")).unwrap()).unwrap();
    let relics = run
        .relics
        .iter()
        .filter_map(|o| rel.get(&o.id))
        .map(|r| (v.icono(r.icono), r.nombre.clone(), r.maldita))
        .collect::<Vec<_>>();
    let n = run.historia.len();
    let mut nodos = vec![];
    for h in &run.historia {
        nodos.push(grafico::GraphNode {
            acto: h.acto,
            icono: None,
            vida_min: h.vida_min,
            combate: h.combate.is_some(),
            bendiciones: h.bendiciones.iter().map(|_| (None, false)).collect(),
            objetos: h.objetos.iter().map(|_| None).collect(),
            detalle: h.combate.and_then(|i| run.combates.get(i)).map(|c| grafico::Detalle {
                enemigo: c.enemigo.clone(),
                tipo: c.tipo.clone(),
                segundos: c.segundos,
                recibido: c.recibido,
                curado: c.curacion.iter().map(|x| x.1).sum(),
                danio: c.danio.clone(),
            }),
        });
    }
    let g = grafico::RunGraph { nodes: nodos, marcador: None, sel: sel.min(n.saturating_sub(1)), t: t - fin::INTRO, ganada: false };
    let view = fin::EndView {
        won: std::env::var("GANADA").is_ok(),
        act: run.act_index + 1,
        nodes: run.path.len().saturating_sub(1),
        items: run.items.len(),
        seed: run.seed.clone(),
        personaje: run.personaje.clone().unwrap_or_default(),
        golpes: run.golpes,
        elites: run.elites_won,
        miticas: vec![],
        relics,
        t,
        grafico: if std::env::var("SIN_GRAFICO").is_ok() { None } else { Some(g) },
    };
    v.render(&a[2], |ui, fonts| fin::draw(ui, &view, &tex, fonts));
}

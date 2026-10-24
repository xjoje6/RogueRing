//! Vista previa del menú principal con una run guardada de runtime/runs/ (o sin run).
//! cargo run --release -p rogue-ui --example preview_titulo -- <salida.png> [semilla]
#[path = "comun/mod.rs"]
mod comun;

use rogue_core::relics::RelicFile;
use rogue_core::run::{ActDef, RunState};
use rogue_ui::{mapa, titulo};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut v = comun::Vista::new();
    let tex = v.mapa("extract/ui/fondos/acto1.png");
    let run = a.get(2).map(|s| {
        let run = RunState::from_json(&std::fs::read_to_string(v.root.join(format!("runtime/runs/{s}.json"))).unwrap()).unwrap();
        let act: ActDef = serde_json::from_str(&std::fs::read_to_string(v.root.join(format!("data/acts/{}.json", run.act_index + 1))).unwrap()).unwrap();
        let rel: RelicFile = serde_json::from_str(&std::fs::read_to_string(v.root.join("data/reliquias.json")).unwrap()).unwrap();
        let bendiciones = run.relics.iter().filter_map(|o| rel.get(&o.id)).map(|r| (v.icono(r.icono), r.maldita)).collect();
        let mut m = mapa::MapView::from_run(&run, &act.name, None);
        m.act_num = run.act_index + 1;
        titulo::RunResumen {
            personaje: "Monk".into(),
            arma: v.icono(4280),
            arma_nombre: "Dryleaf Arts +4".into(),
            semilla: run.seed.clone(),
            mapa: m,
            fondo: v.load(&format!("extract/ui/fondos/acto{}.png", run.act_index + 1)),
            bendiciones,
        }
    });
    let opciones = vec![
        ("Continue".to_string(), "Monk, act 1. Seed UFJXQG.".to_string(), false),
        ("New run".into(), "Your current run will be lost.".into(), false),
        ("Characters".into(), "Outfits and starting weapons.".into(), false),
        ("Co-op".into(), "Create a session or join a friend's.".into(), false),
        ("Compendium".into(), "Every blessing you have found.".into(), false),
        ("Quit game".into(), String::new(), false),
    ];
    let texto = if run.is_some() { "Runs: 7. Wins: 0." } else { "A boss rush across the Lands Between. Pick a character, cross the map, fight what waits." };
    let fondo_inicio = v.load("extract/ui/fondos/acto1.png");
    let view = titulo::TituloView { opciones, selected: 0, texto: texto.into(), run, fondo_inicio, t: 5.0, t_sel: 5.0 };
    v.render(&a[1], |ui, fonts| titulo::draw(ui, &view, &tex, fonts));
}
